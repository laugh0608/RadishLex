use std::ffi::CString;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread::{self, ThreadId};

use crate::config::RimeEngineConfig;
use crate::error::{RimeEngineError, RimeEngineResult};
use crate::ffi::{self, Bool, RimeApi, RimeSessionId, TRUE};
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

        // SAFETY: the API pointer is process-owned and non-null. All native
        // calls are serialized by state, and required functions were validated
        // before initialization.
        let session_id = unsafe {
            let api = state.api.as_ref();
            require_api_function(api.create_session, "create_session")?()
        };
        if session_id == 0 {
            if initialized_here {
                finalize_runtime(&mut state);
            }
            return Err(RimeEngineError::FfiFailure {
                stage: "create_session",
                message: "librime returned an empty session id".to_owned(),
            });
        }

        // SAFETY: session_id was returned by the initialized process runtime;
        // schema is a valid, live C string for the duration of this call.
        let selected = unsafe {
            let api = state.api.as_ref();
            require_api_function(api.select_schema, "select_schema")?(session_id, schema.as_ptr())
        };
        if selected != TRUE {
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
                finalize_runtime(&mut state);
            }
            return Err(RimeEngineError::FfiFailure {
                stage: "select_schema",
                message: format!("failed to select schema {}", config.schema().as_str()),
            });
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

        finalize_runtime(&mut state);
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
                require_api_function(api.deploy, "deploy")?,
            ))
        } else {
            None
        };

        setup(&mut traits);

        if let Some((deployer_initialize, deploy)) = deployment {
            deployer_initialize(&mut traits);
            if let Err(error) = ensure_true("deploy", deploy()) {
                finalize();
                return Err(error);
            }
        }

        initialize(&mut traits);
    }

    state.config = Some(runtime_config);
    state.owner_thread = Some(thread::current().id());
    state._native_strings = Some(native_strings);
    Ok(())
}

fn finalize_runtime(state: &mut RimeRuntimeState) {
    if state.config.is_none() {
        return;
    }

    // SAFETY: finalize was validated before initialization and is called while
    // the runtime lock is held after the active-session count reached zero.
    unsafe {
        let api = state.api.as_ref();
        if let Some(finalize) = api.finalize {
            finalize();
        }
    }
    state.config = None;
    state.owner_thread = None;
    state._native_strings = None;
}

pub(crate) fn require_api_function<T>(
    function: Option<T>,
    name: &'static str,
) -> RimeEngineResult<T> {
    function.ok_or(RimeEngineError::MissingApiFunction { name })
}

fn require_startup_api_functions(api: &RimeApi, deploy_on_start: bool) -> RimeEngineResult<()> {
    require_api_function(api.setup, "setup")?;
    if deploy_on_start {
        require_api_function(api.deployer_initialize, "deployer_initialize")?;
        require_api_function(api.deploy, "deploy")?;
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
    require_api_function(api.get_current_schema, "get_current_schema")?;
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

#[cfg(test)]
mod tests {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int};
    use std::sync::{Mutex, OnceLock};

    use radishlex_ime_core::SchemaId;

    use super::*;
    use crate::ffi::{RimeCommit, RimeContext, RimeTraits};
    use crate::session::RimeEngine;

    type ApiMutation = (&'static str, fn(&mut RimeApi));

    static TEST_SERIAL: Mutex<()> = Mutex::new(());
    static TEST_STATE: OnceLock<Mutex<TestState>> = OnceLock::new();

    #[derive(Debug)]
    struct TestState {
        setup_count: usize,
        initialize_count: usize,
        finalize_count: usize,
        create_count: usize,
        destroy_count: usize,
        select_count: usize,
        next_session_id: RimeSessionId,
        create_returns_empty: bool,
        deploy_succeeds: bool,
        select_succeeds: bool,
        selected_schemas: Vec<String>,
    }

    impl Default for TestState {
        fn default() -> Self {
            Self {
                setup_count: 0,
                initialize_count: 0,
                finalize_count: 0,
                create_count: 0,
                destroy_count: 0,
                select_count: 0,
                next_session_id: 1,
                create_returns_empty: false,
                deploy_succeeds: true,
                select_succeeds: true,
                selected_schemas: Vec::new(),
            }
        }
    }

    #[test]
    fn startup_api_validation_reports_missing_required_functions() {
        let cases: [ApiMutation; 6] = [
            ("setup", |api| api.setup = None),
            ("initialize", |api| api.initialize = None),
            ("finalize", |api| api.finalize = None),
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
        let cases: [ApiMutation; 2] = [
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
        let cases: [ApiMutation; 7] = [
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

    #[test]
    fn process_runtime_initializes_once_and_finalizes_only_on_shutdown() {
        let _serial = TEST_SERIAL.lock().expect("test lock");
        reset_test_state();
        let mut api = Box::new(stub_api());
        let runtime = RimeRuntime::for_test(api.as_mut());

        let first = RimeEngine::new_with_runtime(test_config("schema.one"), Arc::clone(&runtime))
            .expect("first session");
        let second = RimeEngine::new_with_runtime(test_config("schema.two"), Arc::clone(&runtime))
            .expect("second session");

        with_test_state(|state| {
            assert_eq!(state.setup_count, 1);
            assert_eq!(state.initialize_count, 1);
            assert_eq!(state.create_count, 2);
            assert_eq!(state.select_count, 2);
            assert_eq!(state.finalize_count, 0);
            assert_eq!(state.selected_schemas, ["schema.one", "schema.two"]);
        });

        drop(first);
        with_test_state(|state| {
            assert_eq!(state.destroy_count, 1);
            assert_eq!(state.finalize_count, 0);
        });

        drop(second);
        with_test_state(|state| {
            assert_eq!(state.destroy_count, 2);
            assert_eq!(state.finalize_count, 0);
        });

        let third = RimeEngine::new_with_runtime(test_config("schema.three"), Arc::clone(&runtime))
            .expect("runtime stays initialized across a zero-session gap");
        with_test_state(|state| {
            assert_eq!(state.setup_count, 1);
            assert_eq!(state.initialize_count, 1);
            assert_eq!(state.create_count, 3);
        });

        let error = runtime
            .shutdown()
            .expect_err("shutdown must reject an active session");
        assert_eq!(
            error,
            RimeEngineError::RuntimeHasActiveSessions { count: 1 }
        );
        drop(third);
        runtime.shutdown().expect("idle runtime shuts down");
        runtime.shutdown().expect("shutdown is idempotent");
        with_test_state(|state| {
            assert_eq!(state.destroy_count, 3);
            assert_eq!(state.finalize_count, 1);
        });
    }

    #[test]
    fn initialized_runtime_requires_explicit_shutdown_before_config_change() {
        let _serial = TEST_SERIAL.lock().expect("test lock");
        reset_test_state();
        let mut api = Box::new(stub_api());
        let runtime = RimeRuntime::for_test(api.as_mut());
        let first = RimeEngine::new_with_runtime(test_config("schema.one"), Arc::clone(&runtime))
            .expect("first session");

        let incompatible = RimeEngineConfig::new(
            "other-shared",
            "user",
            SchemaId::new("schema.two").expect("schema"),
        )
        .expect("config");
        let error = RimeEngine::new_with_runtime(incompatible, Arc::clone(&runtime))
            .expect_err("initialized runtime must reject a different shared directory");
        assert_eq!(
            error,
            RimeEngineError::IncompatibleRuntimeConfig {
                field: "shared_data_dir"
            }
        );

        with_test_state(|state| {
            assert_eq!(state.setup_count, 1);
            assert_eq!(state.create_count, 1);
            assert_eq!(state.finalize_count, 0);
        });
        drop(first);
        with_test_state(|state| assert_eq!(state.finalize_count, 0));

        let incompatible = RimeEngineConfig::new(
            "other-shared",
            "user",
            SchemaId::new("schema.two").expect("schema"),
        )
        .expect("config");
        let error = RimeEngine::new_with_runtime(incompatible, Arc::clone(&runtime))
            .expect_err("zero active sessions must not implicitly replace process config");
        assert_eq!(
            error,
            RimeEngineError::IncompatibleRuntimeConfig {
                field: "shared_data_dir"
            }
        );

        runtime.shutdown().expect("idle runtime shuts down");
        with_test_state(|state| assert_eq!(state.finalize_count, 1));

        let replacement = RimeEngineConfig::new(
            "other-shared",
            "user",
            SchemaId::new("schema.two").expect("schema"),
        )
        .expect("config");
        let session = RimeEngine::new_with_runtime(replacement, Arc::clone(&runtime))
            .expect("explicit shutdown permits a new process config");
        drop(session);
        runtime.shutdown().expect("replacement runtime shuts down");
        with_test_state(|state| {
            assert_eq!(state.setup_count, 2);
            assert_eq!(state.initialize_count, 2);
            assert_eq!(state.finalize_count, 2);
        });
    }

    #[test]
    fn process_runtime_rejects_sessions_from_another_thread() {
        let _serial = TEST_SERIAL.lock().expect("test lock");
        reset_test_state();
        let mut api = Box::new(stub_api());
        let runtime = RimeRuntime::for_test(api.as_mut());
        let first = RimeEngine::new_with_runtime(test_config("schema.one"), Arc::clone(&runtime))
            .expect("first session");
        let peer_runtime = Arc::clone(&runtime);

        let error = std::thread::spawn(move || {
            RimeEngine::new_with_runtime(test_config("schema.two"), peer_runtime)
                .expect_err("process runtime must preserve its owner thread")
        })
        .join()
        .expect("peer thread joins");
        assert_eq!(error, RimeEngineError::RuntimeThreadMismatch);
        with_test_state(|state| {
            assert_eq!(state.setup_count, 1);
            assert_eq!(state.create_count, 1);
        });

        drop(first);
        runtime.shutdown().expect("owner thread shuts down runtime");
        with_test_state(|state| assert_eq!(state.finalize_count, 1));
    }

    #[test]
    fn failed_schema_selection_rolls_back_and_allows_reinitialize() {
        let _serial = TEST_SERIAL.lock().expect("test lock");
        reset_test_state();
        with_test_state_mut(|state| state.select_succeeds = false);
        let mut api = Box::new(stub_api());
        let runtime = RimeRuntime::for_test(api.as_mut());

        let error = RimeEngine::new_with_runtime(test_config("missing"), Arc::clone(&runtime))
            .expect_err("schema selection must fail");
        assert!(matches!(
            error,
            RimeEngineError::FfiFailure {
                stage: "select_schema",
                ..
            }
        ));
        with_test_state(|state| {
            assert_eq!(state.setup_count, 1);
            assert_eq!(state.initialize_count, 1);
            assert_eq!(state.create_count, 1);
            assert_eq!(state.destroy_count, 1);
            assert_eq!(state.finalize_count, 1);
        });

        with_test_state_mut(|state| state.select_succeeds = true);
        let session = RimeEngine::new_with_runtime(test_config("schema.ok"), Arc::clone(&runtime))
            .expect("runtime can initialize after rollback");
        drop(session);
        with_test_state(|state| {
            assert_eq!(state.setup_count, 2);
            assert_eq!(state.initialize_count, 2);
            assert_eq!(state.finalize_count, 1);
        });
        runtime.shutdown().expect("idle runtime shuts down");
        with_test_state(|state| assert_eq!(state.finalize_count, 2));
    }

    #[test]
    fn empty_session_creation_finalizes_initialized_runtime() {
        let _serial = TEST_SERIAL.lock().expect("test lock");
        reset_test_state();
        with_test_state_mut(|state| state.create_returns_empty = true);
        let mut api = Box::new(stub_api());
        let runtime = RimeRuntime::for_test(api.as_mut());

        let error = RimeEngine::new_with_runtime(test_config("schema.one"), runtime)
            .expect_err("empty session id must fail");
        assert!(matches!(
            error,
            RimeEngineError::FfiFailure {
                stage: "create_session",
                ..
            }
        ));
        with_test_state(|state| {
            assert_eq!(state.initialize_count, 1);
            assert_eq!(state.create_count, 1);
            assert_eq!(state.destroy_count, 0);
            assert_eq!(state.finalize_count, 1);
        });
    }

    #[test]
    fn failed_deploy_finalizes_setup_before_returning_error() {
        let _serial = TEST_SERIAL.lock().expect("test lock");
        reset_test_state();
        with_test_state_mut(|state| state.deploy_succeeds = false);
        let mut api = Box::new(stub_api());
        let runtime = RimeRuntime::for_test(api.as_mut());
        let config = test_config("schema.one").with_deploy_on_start(true);

        let error = RimeEngine::new_with_runtime(config, runtime)
            .expect_err("failed deployment must abort initialization");
        assert!(matches!(
            error,
            RimeEngineError::FfiFailure {
                stage: "deploy",
                ..
            }
        ));
        with_test_state(|state| {
            assert_eq!(state.setup_count, 1);
            assert_eq!(state.initialize_count, 0);
            assert_eq!(state.create_count, 0);
            assert_eq!(state.finalize_count, 1);
        });
    }

    #[test]
    fn missing_finalize_fails_before_setup() {
        let _serial = TEST_SERIAL.lock().expect("test lock");
        reset_test_state();
        let mut api = Box::new(stub_api());
        api.finalize = None;
        let runtime = RimeRuntime::for_test(api.as_mut());

        let error = RimeEngine::new_with_runtime(test_config("schema.one"), runtime)
            .expect_err("finalize is required for balanced lifecycle");
        assert_eq!(
            error,
            RimeEngineError::MissingApiFunction { name: "finalize" }
        );
        with_test_state(|state| {
            assert_eq!(state.setup_count, 0);
            assert_eq!(state.initialize_count, 0);
            assert_eq!(state.finalize_count, 0);
        });
    }

    fn test_config(schema: &str) -> RimeEngineConfig {
        RimeEngineConfig::new("shared", "user", SchemaId::new(schema).expect("schema"))
            .expect("config")
    }

    fn test_state() -> &'static Mutex<TestState> {
        TEST_STATE.get_or_init(|| Mutex::new(TestState::default()))
    }

    fn reset_test_state() {
        *test_state().lock().expect("state lock") = TestState::default();
    }

    fn with_test_state<T>(read: impl FnOnce(&TestState) -> T) -> T {
        read(&test_state().lock().expect("state lock"))
    }

    fn with_test_state_mut<T>(write: impl FnOnce(&mut TestState) -> T) -> T {
        write(&mut test_state().lock().expect("state lock"))
    }

    fn stub_api() -> RimeApi {
        RimeApi {
            data_size: 0,
            setup: Some(stub_setup),
            set_notification_handler: None,
            initialize: Some(stub_initialize),
            finalize: Some(stub_finalize),
            start_maintenance: None,
            is_maintenance_mode: None,
            join_maintenance_thread: None,
            deployer_initialize: Some(stub_deployer_initialize),
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

    unsafe extern "C" fn stub_setup(_traits: *mut RimeTraits) {
        with_test_state_mut(|state| state.setup_count += 1);
    }

    unsafe extern "C" fn stub_initialize(_traits: *mut RimeTraits) {
        with_test_state_mut(|state| state.initialize_count += 1);
    }

    unsafe extern "C" fn stub_finalize() {
        with_test_state_mut(|state| state.finalize_count += 1);
    }

    unsafe extern "C" fn stub_deployer_initialize(_traits: *mut RimeTraits) {}

    unsafe extern "C" fn stub_bool() -> Bool {
        with_test_state(|state| if state.deploy_succeeds { TRUE } else { 0 })
    }

    unsafe extern "C" fn stub_create_session() -> RimeSessionId {
        with_test_state_mut(|state| {
            state.create_count += 1;
            if state.create_returns_empty {
                0
            } else {
                let session_id = state.next_session_id;
                state.next_session_id += 1;
                session_id
            }
        })
    }

    unsafe extern "C" fn stub_destroy_session(_session_id: RimeSessionId) -> Bool {
        with_test_state_mut(|state| state.destroy_count += 1);
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
        0
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
        0
    }

    unsafe extern "C" fn stub_select_schema(
        _session_id: RimeSessionId,
        schema: *const c_char,
    ) -> Bool {
        with_test_state_mut(|state| {
            state.select_count += 1;
            if !schema.is_null() {
                state
                    .selected_schemas
                    .push(CStr::from_ptr(schema).to_string_lossy().into_owned());
            }
            if state.select_succeeds {
                TRUE
            } else {
                0
            }
        })
    }
}
