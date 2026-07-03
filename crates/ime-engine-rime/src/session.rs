use std::ffi::{CStr, CString};
use std::mem;
use std::path::Path;
use std::ptr;
use std::slice;

use radishlex_ime_core::{
    Candidate, Commit, CommitSource, Composition, CoreError, CoreResult, Engine, KeyEvent,
    KeyOutcome, SchemaId,
};

use crate::config::RimeEngineConfig;
use crate::convert::{candidate_from_view, composition_from_parts, RimeCandidateView};
use crate::error::{RimeEngineError, RimeEngineResult};
use crate::ffi::{self, Bool, RimeApi, RimeCommit, RimeContext, RimeSessionId, RimeTraits, TRUE};
use crate::keymap::{classify_key_event, rime_keycode};

#[derive(Debug)]
pub struct RimeEngine {
    config: RimeEngineConfig,
    api: *mut RimeApi,
    session_id: RimeSessionId,
    _native_strings: NativeRimeStrings,
}

impl RimeEngine {
    pub fn new(config: RimeEngineConfig) -> RimeEngineResult<Self> {
        let native_strings = NativeRimeStrings::new(&config)?;
        // SAFETY: rime_get_api is the official librime API entry point. The pointer
        // is checked for null before field access, and every required function
        // pointer is checked before it is called.
        let api = unsafe { ffi::rime_get_api() };
        if api.is_null() {
            return Err(RimeEngineError::NullApi);
        }

        let mut traits = native_strings.traits();
        // SAFETY: api is non-null and each function pointer is checked before use.
        unsafe {
            require_startup_api_functions(&*api, config.deploy_on_start())?;
            require_runtime_api_functions(&*api)?;
            require_api_function((*api).setup, "setup")?(&mut traits);

            if config.deploy_on_start() {
                require_api_function((*api).deployer_initialize, "deployer_initialize")?(
                    &mut traits,
                );
                let deployed = require_api_function((*api).deploy, "deploy")?();
                ensure_true("deploy", deployed)?;
            }

            require_api_function((*api).initialize, "initialize")?(&mut traits);
            let session_id = require_api_function((*api).create_session, "create_session")?();
            if session_id == 0 {
                return Err(RimeEngineError::FfiFailure {
                    stage: "create_session",
                    message: "librime returned an empty session id".to_owned(),
                });
            }

            let selected = require_api_function((*api).select_schema, "select_schema")?(
                session_id,
                native_strings.schema.as_ptr(),
            );
            if selected != TRUE {
                let destroy_session =
                    require_api_function((*api).destroy_session, "destroy_session")?;
                destroy_session(session_id);
                return Err(RimeEngineError::FfiFailure {
                    stage: "select_schema",
                    message: format!("failed to select schema {}", config.schema().as_str()),
                });
            }

            Ok(Self {
                config,
                api,
                session_id,
                _native_strings: native_strings,
            })
        }
    }

    pub fn config(&self) -> &RimeEngineConfig {
        &self.config
    }

    pub fn session_id(&self) -> RimeSessionId {
        self.session_id
    }

    fn read_commit(&self) -> RimeEngineResult<Option<Commit>> {
        // SAFETY: api and session_id are owned by this RimeEngine. RimeCommit is
        // initialized with the self-versioned data_size field expected by librime.
        unsafe {
            let get_commit = require_api_function((*self.api).get_commit, "get_commit")?;
            let free_commit = require_api_function((*self.api).free_commit, "free_commit")?;
            let mut commit = rime_commit();
            if get_commit(self.session_id, &mut commit) != TRUE {
                return Ok(None);
            }

            let text = c_string_field("commit.text", commit.text);
            let freed = free_commit(&mut commit);
            ensure_true("free_commit", freed)?;

            Ok(Some(Commit::new(text?, CommitSource::Engine)))
        }
    }

    fn with_context<T>(
        &self,
        stage: &'static str,
        read: impl FnOnce(&RimeContext) -> RimeEngineResult<T>,
    ) -> RimeEngineResult<T> {
        // SAFETY: api and session_id are owned by this RimeEngine. RimeContext is
        // initialized with the self-versioned data_size field expected by librime.
        unsafe {
            let get_context = require_api_function((*self.api).get_context, "get_context")?;
            let free_context = require_api_function((*self.api).free_context, "free_context")?;
            let mut context = rime_context();
            let got_context = get_context(self.session_id, &mut context);
            ensure_true(stage, got_context)?;

            let result = read(&context);
            let freed = free_context(&mut context);
            ensure_true("free_context", freed)?;
            result
        }
    }
}

impl Engine for RimeEngine {
    fn reset(&mut self) -> CoreResult<()> {
        // SAFETY: api and session_id are owned by this RimeEngine, and the function
        // pointer is checked before it is called.
        unsafe {
            let clear_composition =
                require_api_function((*self.api).clear_composition, "clear_composition")
                    .map_err(rime_to_core)?;
            clear_composition(self.session_id);
        }
        Ok(())
    }

    fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome> {
        let input = classify_key_event(key);
        let Some(keycode) = rime_keycode(input) else {
            return Ok(KeyOutcome::ignored());
        };

        // SAFETY: api and session_id are owned by this RimeEngine, and the function
        // pointer is checked before it is called.
        let consumed = unsafe {
            let process_key = require_api_function((*self.api).process_key, "process_key")
                .map_err(rime_to_core)?;
            process_key(self.session_id, keycode, 0)
        };
        if consumed != TRUE {
            return Ok(KeyOutcome::ignored());
        }

        let commit = self.read_commit().map_err(rime_to_core)?;
        Ok(KeyOutcome::new(true, commit))
    }

    fn composition(&self) -> CoreResult<Composition> {
        self.with_context("get_context", |context| {
            let preedit = if context.composition.preedit.is_null() {
                String::new()
            } else {
                // SAFETY: librime owns this null-terminated string until free_context.
                unsafe { c_string_field("composition.preedit", context.composition.preedit)? }
            };
            composition_from_parts(&preedit, context.composition.cursor_pos)
        })
        .map_err(rime_to_core)
    }

    fn candidates(&self) -> CoreResult<Vec<Candidate>> {
        self.with_context("get_context", |context| {
            if context.menu.num_candidates <= 0 || context.menu.candidates.is_null() {
                return Ok(Vec::new());
            }

            // SAFETY: librime owns the candidate array until free_context. The
            // length comes from the same RimeMenu structure.
            let raw_candidates = unsafe {
                slice::from_raw_parts(
                    context.menu.candidates,
                    context.menu.num_candidates as usize,
                )
            };

            raw_candidates
                .iter()
                .map(|candidate| {
                    let text = unsafe { c_string_field("candidate.text", candidate.text)? };
                    let annotation = unsafe { optional_c_string(candidate.comment)? };
                    candidate_from_view(RimeCandidateView {
                        text: &text,
                        reading: None,
                        annotation: annotation.as_deref(),
                    })
                })
                .collect()
        })
        .map_err(rime_to_core)
    }

    fn commit_candidate(&mut self, index: usize) -> CoreResult<Commit> {
        let select_key = self
            .with_context("get_context", |context| {
                select_key_for_candidate(context, index)
            })
            .map_err(rime_to_core)?;

        // SAFETY: api and session_id are owned by this RimeEngine, and the function
        // pointer is checked before it is called.
        let consumed = unsafe {
            let process_key = require_api_function((*self.api).process_key, "process_key")
                .map_err(rime_to_core)?;
            process_key(self.session_id, select_key as i32, 0)
        };
        if consumed != TRUE {
            return Err(rime_to_core(RimeEngineError::FfiFailure {
                stage: "commit_candidate",
                message: format!("candidate index {index} was not accepted by librime"),
            }));
        }

        self.read_commit().map_err(rime_to_core)?.ok_or_else(|| {
            rime_to_core(RimeEngineError::FfiFailure {
                stage: "get_commit",
                message: format!("candidate index {index} did not produce commit text"),
            })
        })
    }

    fn set_schema(&mut self, schema: SchemaId) -> CoreResult<()> {
        let schema_cstring = CString::new(schema.as_str()).map_err(|error| {
            rime_to_core(RimeEngineError::EncodingFailure {
                field: "schema",
                message: error.to_string(),
            })
        })?;

        // SAFETY: api and session_id are owned by this RimeEngine, and the function
        // pointer is checked before it is called.
        let selected = unsafe {
            let select_schema = require_api_function((*self.api).select_schema, "select_schema")
                .map_err(rime_to_core)?;
            select_schema(self.session_id, schema_cstring.as_ptr())
        };
        if selected != TRUE {
            return Err(rime_to_core(RimeEngineError::FfiFailure {
                stage: "select_schema",
                message: format!("failed to select schema {}", schema.as_str()),
            }));
        }

        self.config.replace_schema(schema);
        Ok(())
    }

    fn schema(&self) -> CoreResult<SchemaId> {
        const SCHEMA_BUFFER_SIZE: usize = 256;
        let mut buffer = [0_i8; SCHEMA_BUFFER_SIZE];

        // SAFETY: buffer is valid for writes of SCHEMA_BUFFER_SIZE bytes. The
        // function pointer is checked before it is called.
        let got_schema = unsafe {
            let get_current_schema =
                require_api_function((*self.api).get_current_schema, "get_current_schema")
                    .map_err(rime_to_core)?;
            get_current_schema(self.session_id, buffer.as_mut_ptr(), buffer.len())
        };
        if got_schema != TRUE {
            return Err(rime_to_core(RimeEngineError::FfiFailure {
                stage: "get_current_schema",
                message: "librime did not return current schema".to_owned(),
            }));
        }

        // SAFETY: get_current_schema writes a null-terminated C string on success.
        let schema = unsafe { c_string_field("schema", buffer.as_ptr()) }.map_err(rime_to_core)?;
        SchemaId::new(schema)
    }
}

impl Drop for RimeEngine {
    fn drop(&mut self) {
        if self.session_id == 0 || self.api.is_null() {
            return;
        }

        // SAFETY: api and session_id were created by RimeEngine::new. Drop cannot
        // report errors, so destroy failure is intentionally ignored here.
        unsafe {
            if let Some(destroy_session) = (*self.api).destroy_session {
                destroy_session(self.session_id);
            }
        }
        self.session_id = 0;
    }
}

#[derive(Debug)]
struct NativeRimeStrings {
    shared_data_dir: CString,
    user_data_dir: CString,
    app_name: CString,
    log_dir: Option<CString>,
    schema: CString,
}

impl NativeRimeStrings {
    fn new(config: &RimeEngineConfig) -> RimeEngineResult<Self> {
        Ok(Self {
            shared_data_dir: path_to_cstring("shared_data_dir", config.shared_data_dir())?,
            user_data_dir: path_to_cstring("user_data_dir", config.user_data_dir())?,
            app_name: CString::new("rime.radishlex").map_err(|error| {
                RimeEngineError::EncodingFailure {
                    field: "app_name",
                    message: error.to_string(),
                }
            })?,
            log_dir: config
                .log_dir()
                .map(|path| path_to_cstring("log_dir", path))
                .transpose()?,
            schema: CString::new(config.schema().as_str()).map_err(|error| {
                RimeEngineError::EncodingFailure {
                    field: "schema",
                    message: error.to_string(),
                }
            })?,
        })
    }

    fn traits(&self) -> RimeTraits {
        RimeTraits {
            data_size: (mem::size_of::<RimeTraits>() - mem::size_of::<i32>()) as i32,
            shared_data_dir: self.shared_data_dir.as_ptr(),
            user_data_dir: self.user_data_dir.as_ptr(),
            distribution_name: ptr::null(),
            distribution_code_name: ptr::null(),
            distribution_version: ptr::null(),
            app_name: self.app_name.as_ptr(),
            modules: ptr::null(),
            min_log_level: 1,
            log_dir: self
                .log_dir
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
            prebuilt_data_dir: ptr::null(),
            staging_dir: ptr::null(),
        }
    }
}

fn path_to_cstring(field: &'static str, path: &Path) -> RimeEngineResult<CString> {
    let value = path
        .to_str()
        .ok_or_else(|| RimeEngineError::EncodingFailure {
            field,
            message: "path is not valid UTF-8".to_owned(),
        })?;

    CString::new(value).map_err(|error| RimeEngineError::EncodingFailure {
        field,
        message: error.to_string(),
    })
}

fn require_api_function<T>(function: Option<T>, name: &'static str) -> RimeEngineResult<T> {
    function.ok_or(RimeEngineError::MissingApiFunction { name })
}

fn require_startup_api_functions(api: &RimeApi, deploy_on_start: bool) -> RimeEngineResult<()> {
    require_api_function(api.setup, "setup")?;
    if deploy_on_start {
        require_api_function(api.deployer_initialize, "deployer_initialize")?;
        require_api_function(api.deploy, "deploy")?;
    }
    require_api_function(api.initialize, "initialize")?;
    require_api_function(api.create_session, "create_session")?;
    require_api_function(api.select_schema, "select_schema")?;
    require_api_function(api.destroy_session, "destroy_session")?;
    Ok(())
}

fn require_runtime_api_functions(api: &RimeApi) -> RimeEngineResult<()> {
    require_api_function(api.clear_composition, "clear_composition")?;
    require_api_function(api.process_key, "process_key")?;
    require_api_function(api.get_commit, "get_commit")?;
    require_api_function(api.free_commit, "free_commit")?;
    require_api_function(api.get_context, "get_context")?;
    require_api_function(api.free_context, "free_context")?;
    require_api_function(api.get_current_schema, "get_current_schema")?;
    Ok(())
}

fn ensure_true(stage: &'static str, value: Bool) -> RimeEngineResult<()> {
    if value == TRUE {
        Ok(())
    } else {
        Err(RimeEngineError::FfiFailure {
            stage,
            message: "librime returned false".to_owned(),
        })
    }
}

fn rime_commit() -> RimeCommit {
    RimeCommit {
        data_size: (mem::size_of::<RimeCommit>() - mem::size_of::<i32>()) as i32,
        text: ptr::null_mut(),
    }
}

fn rime_context() -> RimeContext {
    RimeContext {
        data_size: (mem::size_of::<RimeContext>() - mem::size_of::<i32>()) as i32,
        composition: ffi::RimeComposition {
            length: 0,
            cursor_pos: 0,
            sel_start: 0,
            sel_end: 0,
            preedit: ptr::null_mut(),
        },
        menu: ffi::RimeMenu {
            page_size: 0,
            page_no: 0,
            is_last_page: TRUE,
            highlighted_candidate_index: 0,
            num_candidates: 0,
            candidates: ptr::null_mut(),
            select_keys: ptr::null_mut(),
        },
        commit_text_preview: ptr::null_mut(),
        select_labels: ptr::null_mut(),
    }
}

fn select_key_for_candidate(context: &RimeContext, index: usize) -> RimeEngineResult<char> {
    if context.menu.num_candidates <= 0 || index >= context.menu.num_candidates as usize {
        return Err(RimeEngineError::Core(CoreError::InvalidCandidateIndex {
            index,
            len: context.menu.num_candidates.max(0) as usize,
        }));
    }

    if !context.menu.select_keys.is_null() {
        // SAFETY: librime owns this null-terminated string until free_context.
        let keys = unsafe { c_string_field("menu.select_keys", context.menu.select_keys)? };
        if let Some(ch) = keys.chars().nth(index) {
            return Ok(ch);
        }
    }

    "1234567890"
        .chars()
        .nth(index)
        .ok_or_else(|| RimeEngineError::FfiFailure {
            stage: "commit_candidate",
            message: format!("candidate index {index} has no select key"),
        })
}

unsafe fn c_string_field(field: &'static str, value: *const i8) -> RimeEngineResult<String> {
    if value.is_null() {
        return Err(RimeEngineError::EncodingFailure {
            field,
            message: "null C string".to_owned(),
        });
    }

    CStr::from_ptr(value)
        .to_str()
        .map(|value| value.to_owned())
        .map_err(|error| RimeEngineError::EncodingFailure {
            field,
            message: error.to_string(),
        })
}

unsafe fn optional_c_string(value: *const i8) -> RimeEngineResult<Option<String>> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(c_string_field("candidate.comment", value)?))
    }
}

fn rime_to_core(error: RimeEngineError) -> CoreError {
    CoreError::engine(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::{RimeCommit, RimeContext};
    use std::os::raw::{c_char, c_int};

    #[test]
    fn startup_api_validation_reports_missing_required_functions() {
        let cases: [(&str, fn(&mut RimeApi)); 5] = [
            ("setup", |api| api.setup = None),
            ("initialize", |api| api.initialize = None),
            ("create_session", |api| api.create_session = None),
            ("select_schema", |api| api.select_schema = None),
            ("destroy_session", |api| api.destroy_session = None),
        ];

        for (name, remove) in cases {
            let mut api = stub_api();
            remove(&mut api);
            let error = require_startup_api_functions(&api, false)
                .expect_err("missing startup function must fail");
            assert_eq!(error, RimeEngineError::MissingApiFunction { name });
        }
    }

    #[test]
    fn deploy_startup_validation_reports_deploy_functions() {
        let cases: [(&str, fn(&mut RimeApi)); 2] = [
            ("deployer_initialize", |api| api.deployer_initialize = None),
            ("deploy", |api| api.deploy = None),
        ];

        for (name, remove) in cases {
            let mut api = stub_api();
            remove(&mut api);
            let error = require_startup_api_functions(&api, true)
                .expect_err("missing deploy function must fail");
            assert_eq!(error, RimeEngineError::MissingApiFunction { name });
        }
    }

    #[test]
    fn runtime_api_validation_reports_missing_required_functions() {
        let cases: [(&str, fn(&mut RimeApi)); 7] = [
            ("clear_composition", |api| api.clear_composition = None),
            ("process_key", |api| api.process_key = None),
            ("get_commit", |api| api.get_commit = None),
            ("free_commit", |api| api.free_commit = None),
            ("get_context", |api| api.get_context = None),
            ("free_context", |api| api.free_context = None),
            ("get_current_schema", |api| api.get_current_schema = None),
        ];

        for (name, remove) in cases {
            let mut api = stub_api();
            remove(&mut api);
            let error = require_runtime_api_functions(&api)
                .expect_err("missing runtime function must fail");
            assert_eq!(error, RimeEngineError::MissingApiFunction { name });
        }
    }

    fn stub_api() -> RimeApi {
        RimeApi {
            data_size: 0,
            setup: Some(stub_traits),
            set_notification_handler: None,
            initialize: Some(stub_traits),
            finalize: None,
            start_maintenance: None,
            is_maintenance_mode: None,
            join_maintenance_thread: None,
            deployer_initialize: Some(stub_traits),
            prebuild: None,
            deploy: Some(stub_bool),
            deploy_schema: None,
            deploy_config_file: None,
            sync_user_data: None,
            create_session: Some(stub_create_session),
            find_session: None,
            destroy_session: Some(stub_destroy_session),
            cleanup_stale_sessions: None,
            cleanup_all_sessions: None,
            process_key: Some(stub_process_key),
            commit_composition: None,
            clear_composition: Some(stub_clear_composition),
            get_commit: Some(stub_get_commit),
            free_commit: Some(stub_free_commit),
            get_context: Some(stub_get_context),
            free_context: Some(stub_free_context),
            get_status: None,
            free_status: None,
            set_option: None,
            get_option: None,
            set_property: None,
            get_property: None,
            get_schema_list: None,
            free_schema_list: None,
            get_current_schema: Some(stub_get_current_schema),
            select_schema: Some(stub_select_schema),
        }
    }

    unsafe extern "C" fn stub_traits(_traits: *mut RimeTraits) {}

    unsafe extern "C" fn stub_bool() -> Bool {
        TRUE
    }

    unsafe extern "C" fn stub_create_session() -> RimeSessionId {
        1
    }

    unsafe extern "C" fn stub_destroy_session(_session_id: RimeSessionId) -> Bool {
        TRUE
    }

    unsafe extern "C" fn stub_process_key(
        _session_id: RimeSessionId,
        _keycode: c_int,
        _mask: c_int,
    ) -> Bool {
        TRUE
    }

    unsafe extern "C" fn stub_clear_composition(_session_id: RimeSessionId) {}

    unsafe extern "C" fn stub_get_commit(
        _session_id: RimeSessionId,
        _commit: *mut RimeCommit,
    ) -> Bool {
        TRUE
    }

    unsafe extern "C" fn stub_free_commit(_commit: *mut RimeCommit) -> Bool {
        TRUE
    }

    unsafe extern "C" fn stub_get_context(
        _session_id: RimeSessionId,
        _context: *mut RimeContext,
    ) -> Bool {
        TRUE
    }

    unsafe extern "C" fn stub_free_context(_context: *mut RimeContext) -> Bool {
        TRUE
    }

    unsafe extern "C" fn stub_get_current_schema(
        _session_id: RimeSessionId,
        _buffer: *mut c_char,
        _buffer_size: usize,
    ) -> Bool {
        TRUE
    }

    unsafe extern "C" fn stub_select_schema(
        _session_id: RimeSessionId,
        _schema: *const c_char,
    ) -> Bool {
        TRUE
    }
}
