use std::ffi::{c_char, CStr, CString};
use std::fmt;
use std::ptr;
use std::slice;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread::{self, ThreadId};

use crate::config::RimeEngineConfig;
use crate::error::{RimeEngineError, RimeEngineResult};
use crate::ffi::{self, Bool, RimeApi, RimeSchemaList, RimeSessionId, TRUE};
use crate::learning_guard::LearningGuard;
use crate::runtime_config::{NativeRimeStrings, RimeRuntimeConfig};

static PROCESS_RUNTIME: OnceLock<Arc<RimeRuntime>> = OnceLock::new();

pub(crate) struct RimeRuntime {
    state: Mutex<RimeRuntimeState>,
}

impl RimeRuntime {
    pub(crate) fn process() -> RimeEngineResult<Arc<Self>> {
        if let Some(runtime) = PROCESS_RUNTIME.get() {
            return Ok(Arc::clone(runtime));
        }

        // SAFETY: rime_get_api is the official librime API entry point. The
        // pointer is checked for null before it is stored or dereferenced.
        let api = unsafe { ffi::rime_get_api() };
        if api.is_null() {
            return Err(RimeEngineError::NullApi);
        }

        Ok(Arc::clone(
            PROCESS_RUNTIME.get_or_init(|| Arc::new(Self::from_api(api))),
        ))
    }

    fn from_api(api: *mut RimeApi) -> Self {
        debug_assert!(!api.is_null());
        Self {
            state: Mutex::new(RimeRuntimeState {
                api: RimeApiPointer(api),
                active_sessions: 0,
                config: None,
                owner_thread: None,
                _native_strings: None,
                learning_guard: LearningGuard::default(),
            }),
        }
    }

    pub(crate) fn create_session(
        &self,
        config: &RimeEngineConfig,
    ) -> RimeEngineResult<RimeSessionId> {
        let schema = CString::new(config.schema().as_str()).map_err(|error| {
            RimeEngineError::EncodingFailure {
                field: "schema",
                message: error.to_string(),
            }
        })?;
        let requested_config = RimeRuntimeConfig::from(config);
        let mut state = self.lock_state()?;

        let initialized_here = if state.config.is_none() {
            initialize_runtime(&mut state, requested_config, config)?;
            true
        } else {
            state.ensure_owner_thread()?;
            state.ensure_compatible(&requested_config)?;
            false
        };

        if let Err(error) = state.learning_guard.ensure_schema(config.schema().as_str()) {
            if initialized_here {
                finalize_runtime(&mut state)?;
            }
            return Err(error);
        }

        // SAFETY: the API pointer is process-owned and non-null. All native
        // calls are serialized by state, and required functions were validated
        // before initialization.
        let session_id = unsafe {
            let api = state.api.as_ref();
            require_api_function(api.create_session, "create_session")?()
        };
        if session_id == 0 {
            if initialized_here {
                finalize_runtime(&mut state)?;
            }
            return Err(RimeEngineError::FfiFailure {
                stage: "create_session",
                message: "librime returned an empty session id".to_owned(),
            });
        }

        // SAFETY: session_id was returned by the initialized process runtime;
        // schema is a valid, live C string for the duration of this call.
        let selection = unsafe {
            let api = state.api.as_ref();
            select_schema_exact(api, session_id, schema.as_c_str())
        };
        if let Err(error) = selection {
            // SAFETY: session_id was created above and the function was checked
            // before initialization. Cleanup failure cannot replace the primary
            // select_schema error.
            unsafe {
                let api = state.api.as_ref();
                if let Some(destroy_session) = api.destroy_session {
                    destroy_session(session_id);
                }
            }
            if initialized_here {
                finalize_runtime(&mut state)?;
            }
            return Err(error);
        }

        state.active_sessions += 1;
        Ok(session_id)
    }

    pub(crate) fn with_api<T>(
        &self,
        call: impl FnOnce(&RimeApi) -> RimeEngineResult<T>,
    ) -> RimeEngineResult<T> {
        let state = self.lock_state()?;
        state.ensure_owner_thread()?;
        if state.config.is_none() {
            return Err(RimeEngineError::FfiFailure {
                stage: "runtime",
                message: "librime runtime is not initialized".to_owned(),
            });
        }
        if state.active_sessions == 0 {
            return Err(RimeEngineError::FfiFailure {
                stage: "runtime",
                message: "librime call requires an active session".to_owned(),
            });
        }

        // SAFETY: the process API pointer is non-null and remains valid for the
        // lifetime of librime. Holding state prevents session release/finalize
        // and serializes the native call with every other session.
        unsafe { call(state.api.as_ref()) }
    }

    pub(crate) fn release_session(&self, session_id: RimeSessionId) {
        if session_id == 0 {
            return;
        }

        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        if state.active_sessions == 0 || state.config.is_none() {
            return;
        }
        if state.ensure_owner_thread().is_err() {
            return;
        }

        // SAFETY: the session belongs to this initialized runtime. Drop cannot
        // report an error, so destroy failure is intentionally ignored.
        unsafe {
            let api = state.api.as_ref();
            if let Some(destroy_session) = api.destroy_session {
                destroy_session(session_id);
            }
        }
        state.active_sessions -= 1;
    }

    pub(crate) fn shutdown(&self) -> RimeEngineResult<()> {
        let mut state = self.lock_state()?;
        if state.config.is_some() {
            state.ensure_owner_thread()?;
        }
        if state.active_sessions > 0 {
            return Err(RimeEngineError::RuntimeHasActiveSessions {
                count: state.active_sessions,
            });
        }

        finalize_runtime(&mut state)?;
        Ok(())
    }

    fn lock_state(&self) -> RimeEngineResult<MutexGuard<'_, RimeRuntimeState>> {
        self.state.lock().map_err(|_| RimeEngineError::FfiFailure {
            stage: "runtime_lock",
            message: "librime runtime lock is poisoned".to_owned(),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(api: *mut RimeApi) -> Arc<Self> {
        assert!(!api.is_null(), "test Rime API must not be null");
        Arc::new(Self::from_api(api))
    }
}

pub(crate) fn shutdown_process_runtime() -> RimeEngineResult<()> {
    match PROCESS_RUNTIME.get() {
        Some(runtime) => runtime.shutdown(),
        None => Ok(()),
    }
}

impl fmt::Debug for RimeRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RimeRuntime")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
struct RimeApiPointer(*mut RimeApi);

// SAFETY: RimeApiPointer is only stored inside RimeRuntimeState. Every pointer
// dereference and librime call occurs while the enclosing Mutex is held, so the
// process-global API is never accessed concurrently through this adapter.
unsafe impl Send for RimeApiPointer {}

impl RimeApiPointer {
    unsafe fn as_ref(&self) -> &RimeApi {
        &*self.0
    }
}

struct RimeRuntimeState {
    api: RimeApiPointer,
    active_sessions: usize,
    config: Option<RimeRuntimeConfig>,
    owner_thread: Option<ThreadId>,
    _native_strings: Option<NativeRimeStrings>,
    learning_guard: LearningGuard,
}

impl RimeRuntimeState {
    fn ensure_owner_thread(&self) -> RimeEngineResult<()> {
        match self.owner_thread {
            Some(owner_thread) if owner_thread != thread::current().id() => {
                Err(RimeEngineError::RuntimeThreadMismatch)
            }
            _ => Ok(()),
        }
    }

    fn ensure_compatible(&self, requested: &RimeRuntimeConfig) -> RimeEngineResult<()> {
        let Some(active) = self.config.as_ref() else {
            return Err(RimeEngineError::FfiFailure {
                stage: "runtime",
                message: "initialized librime runtime has no process configuration".to_owned(),
            });
        };

        if let Some(field) = active.incompatible_field(requested) {
            return Err(RimeEngineError::IncompatibleRuntimeConfig { field });
        }
        Ok(())
    }
}

fn initialize_runtime(
    state: &mut RimeRuntimeState,
    runtime_config: RimeRuntimeConfig,
    config: &RimeEngineConfig,
) -> RimeEngineResult<()> {
    debug_assert_eq!(state.active_sessions, 0);
    debug_assert!(state.config.is_none());

    let native_strings = NativeRimeStrings::new(config)?;
    let mut traits = native_strings.traits();

    // SAFETY: the process API pointer is non-null. Every required function is
    // validated before setup, and native_strings keeps every trait pointer alive
    // until finalization.
    unsafe {
        let api = state.api.as_ref();
        require_startup_api_functions(api, config.deploy_on_start())?;
        require_runtime_api_functions(api)?;
        let setup = require_api_function(api.setup, "setup")?;
        let initialize = require_api_function(api.initialize, "initialize")?;
        let finalize = require_api_function(api.finalize, "finalize")?;
        let deployment = if config.deploy_on_start() {
            Some((
                require_api_function(api.deployer_initialize, "deployer_initialize")?,
                require_api_function(api.run_task, "run_task")?,
            ))
        } else {
            None
        };

        setup(&mut traits);

        if let Some((deployer_initialize, run_task)) = deployment {
            deployer_initialize(&mut traits);
            // The full deploy task also upgrades user dictionaries and cleans
            // trash. Only prepare installation metadata and schema workspace;
            // existing learned data must remain untouched.
            for name in [c"installation_update", c"workspace_update"] {
                if let Err(error) = ensure_true("deploy_workspace", run_task(name.as_ptr())) {
                    finalize();
                    return Err(error);
                }
            }
        }

        initialize(&mut traits);
    }

    state.config = Some(runtime_config);
    state.owner_thread = Some(thread::current().id());
    state._native_strings = Some(native_strings);
    // SAFETY: initialized runtime and owner-thread mutex held; pin before the
    // first create_session, which can initialize an engine for a saved schema.
    let guarded = unsafe { state.learning_guard.initialize(state.api.as_ref()) };
    if let Err(error) = guarded {
        finalize_runtime(state)?;
        return Err(error);
    }
    Ok(())
}

fn finalize_runtime(state: &mut RimeRuntimeState) -> RimeEngineResult<()> {
    if state.config.is_none() {
        return Ok(());
    }

    // SAFETY: finalize was validated before initialization and is called while
    // the runtime lock is held after the active-session count reached zero.
    let closed = unsafe {
        let api = state.api.as_ref();
        let closed = state.learning_guard.close(api);
        if let Some(finalize) = api.finalize {
            finalize();
        }
        closed
    };
    state.config = None;
    state.owner_thread = None;
    state._native_strings = None;
    closed
}

pub(crate) fn require_api_function<T>(
    function: Option<T>,
    name: &'static str,
) -> RimeEngineResult<T> {
    function.ok_or(RimeEngineError::MissingApiFunction { name })
}

fn require_startup_api_functions(api: &RimeApi, deploy_on_start: bool) -> RimeEngineResult<()> {
    LearningGuard::require_api(api)?;
    require_api_function(api.setup, "setup")?;
    if deploy_on_start {
        require_api_function(api.deployer_initialize, "deployer_initialize")?;
        require_api_function(api.run_task, "run_task")?;
    }
    require_api_function(api.initialize, "initialize")?;
    require_api_function(api.finalize, "finalize")?;
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
    require_api_function(api.get_schema_list, "get_schema_list")?;
    require_api_function(api.free_schema_list, "free_schema_list")?;
    require_api_function(api.get_current_schema, "get_current_schema")?;
    require_api_function(
        api.select_candidate_on_current_page,
        "select_candidate_on_current_page",
    )?;
    Ok(())
}

pub(crate) fn ensure_true(stage: &'static str, value: Bool) -> RimeEngineResult<()> {
    if value == TRUE {
        Ok(())
    } else {
        Err(RimeEngineError::FfiFailure {
            stage,
            message: "librime returned false".to_owned(),
        })
    }
}

pub(crate) fn current_schema(api: &RimeApi, session_id: RimeSessionId) -> RimeEngineResult<String> {
    const SCHEMA_BUFFER_SIZE: usize = 256;
    let mut buffer = [0 as c_char; SCHEMA_BUFFER_SIZE];

    // SAFETY: buffer is valid for SCHEMA_BUFFER_SIZE writes and session_id
    // belongs to the runtime that supplied api.
    let got_schema = unsafe {
        let get_current_schema =
            require_api_function(api.get_current_schema, "get_current_schema")?;
        get_current_schema(session_id, buffer.as_mut_ptr(), buffer.len())
    };
    ensure_true("get_current_schema", got_schema)?;

    let nul = buffer.iter().position(|byte| *byte == 0).ok_or_else(|| {
        RimeEngineError::EncodingFailure {
            field: "schema",
            message: "librime returned a schema id without a null terminator".to_owned(),
        }
    })?;
    let bytes = buffer[..nul]
        .iter()
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    String::from_utf8(bytes).map_err(|error| RimeEngineError::EncodingFailure {
        field: "schema",
        message: error.to_string(),
    })
}

pub(crate) fn select_schema_exact(
    api: &RimeApi,
    session_id: RimeSessionId,
    schema: &CStr,
) -> RimeEngineResult<()> {
    let requested = schema
        .to_str()
        .map_err(|error| RimeEngineError::EncodingFailure {
            field: "schema",
            message: error.to_string(),
        })?;

    ensure_schema_available(api, requested)?;

    // SAFETY: session_id belongs to this runtime and schema remains alive for
    // the duration of the native call.
    let selected = unsafe {
        let select_schema = require_api_function(api.select_schema, "select_schema")?;
        select_schema(session_id, schema.as_ptr())
    };
    if selected != TRUE {
        return Err(RimeEngineError::FfiFailure {
            stage: "select_schema",
            message: format!("failed to select schema {requested}"),
        });
    }

    let actual = current_schema(api, session_id)?;
    if actual != requested {
        return Err(RimeEngineError::FfiFailure {
            stage: "select_schema",
            message: format!(
                "librime kept schema {actual} after selecting requested schema {requested}"
            ),
        });
    }
    Ok(())
}

fn ensure_schema_available(api: &RimeApi, requested: &str) -> RimeEngineResult<()> {
    let mut schemas = RimeSchemaList {
        size: 0,
        list: ptr::null_mut(),
    };

    // SAFETY: schemas is initialized for librime to populate. Both functions
    // are validated at runtime startup, and free_schema_list is called exactly
    // once after a successful get_schema_list call.
    unsafe {
        let get_schema_list = require_api_function(api.get_schema_list, "get_schema_list")?;
        let free_schema_list = require_api_function(api.free_schema_list, "free_schema_list")?;
        ensure_true("get_schema_list", get_schema_list(&mut schemas))?;

        let result = if schemas.size > 0 && schemas.list.is_null() {
            Err(RimeEngineError::FfiFailure {
                stage: "get_schema_list",
                message: "librime returned a non-empty schema list without items".to_owned(),
            })
        } else {
            let items = if schemas.size == 0 {
                &[][..]
            } else {
                slice::from_raw_parts(schemas.list, schemas.size)
            };
            let found = items.iter().any(|item| {
                !item.schema_id.is_null()
                    && CStr::from_ptr(item.schema_id).to_bytes() == requested.as_bytes()
            });
            if found {
                Ok(())
            } else {
                Err(RimeEngineError::FfiFailure {
                    stage: "select_schema",
                    message: format!(
                        "schema {requested} is not present in librime's deployed schema list"
                    ),
                })
            }
        };

        free_schema_list(&mut schemas);
        result
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
