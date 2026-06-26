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
