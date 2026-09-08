use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_int;
use std::sync::{Mutex, OnceLock};

use radishlex_ime_core::{Engine, SchemaId};

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
    selected_schema_override: Option<String>,
    selected_schemas: Vec<String>,
    user_dict: Option<bool>,
    translator: String,
    config_open_count: usize,
    config_close_count: usize,
    tasks: Vec<String>,
    current_schemas: HashMap<RimeSessionId, String>,
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
            selected_schema_override: None,
            selected_schemas: Vec::new(),
            user_dict: Some(false),
            translator: "script_translator".to_owned(),
            config_open_count: 0,
            config_close_count: 0,
            tasks: Vec::new(),
            current_schemas: HashMap::new(),
        }
    }
}

#[test]
fn startup_api_validation_reports_missing_required_functions() {
    let cases: [ApiMutation; 12] = [
        ("setup", |api| api.setup = None),
        ("initialize", |api| api.initialize = None),
        ("finalize", |api| api.finalize = None),
        ("create_session", |api| api.create_session = None),
        ("select_schema", |api| api.select_schema = None),
        ("destroy_session", |api| api.destroy_session = None),
        ("config_open", |api| api.config_open = None),
        ("schema_open", |api| api.schema_open = None),
        ("config_close", |api| api.config_close = None),
        ("config_get_bool", |api| api.config_get_bool = None),
        ("config_get_string", |api| api.config_get_string = None),
        ("config_list_size", |api| api.config_list_size = None),
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
        ("run_task", |api| api.run_task = None),
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
    let cases: [ApiMutation; 10] = [
        ("clear_composition", |api| api.clear_composition = None),
        ("process_key", |api| api.process_key = None),
        ("get_commit", |api| api.get_commit = None),
        ("free_commit", |api| api.free_commit = None),
        ("get_context", |api| api.get_context = None),
        ("free_context", |api| api.free_context = None),
        ("get_schema_list", |api| api.get_schema_list = None),
        ("free_schema_list", |api| api.free_schema_list = None),
        ("get_current_schema", |api| api.get_current_schema = None),
        ("select_candidate_on_current_page", |api| {
            api.select_candidate_on_current_page = None
        }),
    ];

    for (name, remove) in cases {
        let mut api = stub_api();
        remove(&mut api);
        let error =
            require_runtime_api_functions(&api).expect_err("missing runtime function must fail");
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
fn unavailable_schema_rolls_back_before_native_selection() {
    let _serial = TEST_SERIAL.lock().expect("test lock");
    reset_test_state();
    let mut api = Box::new(stub_api());
    let runtime = RimeRuntime::for_test(api.as_mut());

    let error = RimeEngine::new_with_runtime(test_config("schema.absent"), runtime)
        .expect_err("an undeployed schema must fail");
    assert!(matches!(
        error,
        RimeEngineError::FfiFailure {
            stage: "learning_guard",
            ..
        }
    ));
    assert!(error.to_string().contains("validated schema_list"));
    with_test_state(|state| {
        assert_eq!(state.select_count, 0);
        assert_eq!(state.create_count, 0);
        assert_eq!(state.destroy_count, 0);
        assert_eq!(state.finalize_count, 1);
    });
}

#[test]
fn mismatched_schema_selection_rolls_back_initialized_runtime() {
    let _serial = TEST_SERIAL.lock().expect("test lock");
    reset_test_state();
    with_test_state_mut(|state| {
        state.selected_schema_override = Some("schema.fallback".to_owned());
    });
    let mut api = Box::new(stub_api());
    let runtime = RimeRuntime::for_test(api.as_mut());

    let error = RimeEngine::new_with_runtime(test_config("schema.missing"), runtime)
        .expect_err("a different active schema must reject session creation");
    assert!(matches!(
        error,
        RimeEngineError::FfiFailure {
            stage: "select_schema",
            ..
        }
    ));
    assert!(error.to_string().contains("schema.fallback"));
    assert!(error.to_string().contains("schema.missing"));
    with_test_state(|state| {
        assert_eq!(state.create_count, 1);
        assert_eq!(state.destroy_count, 1);
        assert_eq!(state.finalize_count, 1);
    });
}

#[test]
fn mismatched_runtime_schema_change_preserves_engine_config() {
    let _serial = TEST_SERIAL.lock().expect("test lock");
    reset_test_state();
    let mut api = Box::new(stub_api());
    let runtime = RimeRuntime::for_test(api.as_mut());
    let mut engine = RimeEngine::new_with_runtime(test_config("schema.one"), Arc::clone(&runtime))
        .expect("session");

    with_test_state_mut(|state| {
        state.selected_schema_override = Some("schema.one".to_owned());
    });
    let error = engine
        .set_schema(SchemaId::new("schema.missing").expect("schema"))
        .expect_err("schema mismatch must fail");
    assert!(error.to_string().contains("schema.missing"));
    assert_eq!(engine.config().schema().as_str(), "schema.one");

    drop(engine);
    runtime.shutdown().expect("idle runtime shuts down");
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
            stage: "deploy_workspace",
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
    RimeEngineConfig::new("shared", "user", SchemaId::new(schema).expect("schema")).expect("config")
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
        get_schema_list: Some(stub_get_schema_list),
        free_schema_list: Some(stub_free_schema_list),
        get_current_schema: Some(stub_get_current_schema),
        select_schema: Some(stub_select_schema),
        schema_open: Some(stub_schema_open),
        config_open: Some(stub_config_open),
        config_close: Some(stub_config_close),
        config_get_bool: Some(stub_config_get_bool),
        _config_get_int: None,
        _config_get_double: None,
        config_get_string: Some(stub_config_get_string),
        _config_get_cstring: None,
        _config_update_signature: None,
        _config_begin_map: None,
        _config_next: None,
        _config_end: None,
        _simulate_key_sequence: None,
        _register_module: None,
        _find_module: None,
        run_task: Some(stub_run_task),
        _get_shared_data_dir: None,
        _get_user_data_dir: None,
        _get_sync_dir: None,
        _get_user_id: None,
        _get_user_data_sync_dir: None,
        _config_init: None,
        _config_load_string: None,
        _config_set_bool: None,
        _config_set_int: None,
        _config_set_double: None,
        _config_set_string: None,
        _config_get_item: None,
        _config_set_item: None,
        _config_clear: None,
        _config_create_list: None,
        _config_create_map: None,
        config_list_size: Some(stub_config_list_size),
        _config_begin_list: None,
        get_input: None,
        _get_caret_pos: None,
        select_candidate: None,
        _get_version: None,
        _set_caret_pos: None,
        select_candidate_on_current_page: Some(stub_select_candidate_on_current_page),
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

unsafe extern "C" fn stub_destroy_session(session_id: RimeSessionId) -> Bool {
    with_test_state_mut(|state| {
        state.destroy_count += 1;
        state.current_schemas.remove(&session_id);
    });
    TRUE
}

unsafe extern "C" fn stub_process_key(
    _session_id: RimeSessionId,
    _keycode: c_int,
    _mask: c_int,
) -> Bool {
    TRUE
}

unsafe extern "C" fn stub_select_candidate_on_current_page(
    _session_id: RimeSessionId,
    _index: usize,
) -> Bool {
    TRUE
}

unsafe extern "C" fn stub_clear_composition(_session_id: RimeSessionId) {}

unsafe extern "C" fn stub_get_commit(_session_id: RimeSessionId, _commit: *mut RimeCommit) -> Bool {
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

unsafe extern "C" fn stub_get_schema_list(schema_list: *mut RimeSchemaList) -> Bool {
    if schema_list.is_null() {
        return 0;
    }
    const SCHEMAS: [&[u8]; 7] = [
        b"schema.one\0",
        b"schema.two\0",
        b"schema.three\0",
        b"schema.ok\0",
        b"schema.missing\0",
        b"schema.fallback\0",
        b"missing\0",
    ];
    let items = SCHEMAS
        .iter()
        .map(|schema| ffi::RimeSchemaListItem {
            schema_id: schema.as_ptr().cast::<c_char>().cast_mut(),
            name: ptr::null_mut(),
            reserved: ptr::null_mut(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    (*schema_list).size = items.len();
    (*schema_list).list = Box::into_raw(items).cast::<ffi::RimeSchemaListItem>();
    TRUE
}

unsafe extern "C" fn stub_free_schema_list(schema_list: *mut RimeSchemaList) {
    if schema_list.is_null() || (*schema_list).list.is_null() {
        return;
    }
    let items = ptr::slice_from_raw_parts_mut((*schema_list).list, (*schema_list).size);
    drop(Box::from_raw(items));
    (*schema_list).size = 0;
    (*schema_list).list = ptr::null_mut();
}

unsafe extern "C" fn stub_get_current_schema(
    session_id: RimeSessionId,
    buffer: *mut c_char,
    buffer_size: usize,
) -> Bool {
    with_test_state(|state| {
        let Some(schema) = state.current_schemas.get(&session_id) else {
            return 0;
        };
        let bytes = schema.as_bytes();
        if buffer.is_null() || bytes.len() + 1 > buffer_size {
            return 0;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), buffer, bytes.len());
        *buffer.add(bytes.len()) = 0;
        TRUE
    })
}

unsafe extern "C" fn stub_select_schema(session_id: RimeSessionId, schema: *const c_char) -> Bool {
    with_test_state_mut(|state| {
        state.select_count += 1;
        if !schema.is_null() {
            let requested = CStr::from_ptr(schema).to_string_lossy().into_owned();
            state.selected_schemas.push(requested.clone());
            let selected = state.selected_schema_override.clone().unwrap_or(requested);
            state.current_schemas.insert(session_id, selected);
        }
        if state.select_succeeds {
            TRUE
        } else {
            0
        }
    })
}

const TEST_SCHEMAS: [&str; 7] = [
    "schema.one",
    "schema.two",
    "schema.three",
    "schema.ok",
    "schema.missing",
    "schema.fallback",
    "missing",
];

unsafe extern "C" fn stub_config_open(_: *const c_char, config: *mut ffi::RimeConfig) -> Bool {
    (*config).ptr = Box::into_raw(Box::new(0u8)).cast();
    with_test_state_mut(|state| state.config_open_count += 1);
    TRUE
}

unsafe extern "C" fn stub_schema_open(name: *const c_char, config: *mut ffi::RimeConfig) -> Bool {
    stub_config_open(name, config)
}

unsafe extern "C" fn stub_config_close(config: *mut ffi::RimeConfig) -> Bool {
    drop(Box::from_raw((*config).ptr.cast::<u8>()));
    (*config).ptr = ptr::null_mut();
    with_test_state_mut(|state| state.config_close_count += 1);
    TRUE
}

unsafe extern "C" fn stub_config_get_bool(
    _: *mut ffi::RimeConfig,
    _: *const c_char,
    value: *mut Bool,
) -> Bool {
    with_test_state(|state| match state.user_dict {
        Some(enabled) => {
            *value = i32::from(enabled);
            TRUE
        }
        None => 0,
    })
}

unsafe extern "C" fn stub_config_list_size(_: *mut ffi::RimeConfig, key: *const c_char) -> usize {
    match CStr::from_ptr(key).to_str().unwrap() {
        "schema_list" => TEST_SCHEMAS.len(),
        "engine/processors" | "engine/segmentors" | "engine/translators" => 1,
        _ => 0,
    }
}

unsafe extern "C" fn stub_config_get_string(
    _: *mut ffi::RimeConfig,
    key: *const c_char,
    buffer: *mut c_char,
    size: usize,
) -> Bool {
    let key = CStr::from_ptr(key).to_str().unwrap();
    let value = if let Some(index) = key.strip_prefix("schema_list/@") {
        TEST_SCHEMAS[index
            .strip_suffix("/schema")
            .unwrap()
            .parse::<usize>()
            .unwrap()]
        .to_owned()
    } else {
        match key {
            "engine/processors/@0" => "express_editor".to_owned(),
            "engine/segmentors/@0" => "abc_segmentor".to_owned(),
            "engine/translators/@0" => with_test_state(|state| state.translator.clone()),
            _ => return 0,
        }
    };
    let bytes = value.as_bytes();
    if bytes.len() + 1 > size {
        return 0;
    }
    ptr::copy_nonoverlapping(bytes.as_ptr().cast(), buffer, bytes.len());
    *buffer.add(bytes.len()) = 0;
    TRUE
}

unsafe extern "C" fn stub_run_task(name: *const c_char) -> Bool {
    with_test_state_mut(|state| {
        state
            .tasks
            .push(CStr::from_ptr(name).to_str().unwrap().to_owned())
    });
    stub_bool()
}

#[test]
fn learning_guard_rejects_enabled_missing_and_custom_learning_before_session_creation() {
    let _serial = TEST_SERIAL.lock().expect("test lock");
    for (enabled, translator) in [
        (Some(true), "script_translator"),
        (None, "script_translator"),
        (Some(false), "script_translator@other"),
        (Some(false), "lua_translator@custom"),
        (Some(false), "table_translator"),
    ] {
        reset_test_state();
        with_test_state_mut(|state| {
            state.user_dict = enabled;
            state.translator = translator.to_owned();
        });
        let mut api = Box::new(stub_api());
        let runtime = RimeRuntime::for_test(api.as_mut());
        let error = RimeEngine::new_with_runtime(test_config("schema.one"), runtime)
            .expect_err("unreviewed learning path must fail before native creation");
        assert!(matches!(
            error,
            RimeEngineError::FfiFailure {
                stage: "learning_guard",
                ..
            }
        ));
        with_test_state(|state| {
            assert_eq!(state.create_count, 0);
            assert_eq!(state.finalize_count, 1);
            assert_eq!(state.config_open_count, state.config_close_count);
        });
    }
}

#[test]
fn learning_guard_holds_configs_across_idle_gap_and_deploy_avoids_userdict_tasks() {
    let _serial = TEST_SERIAL.lock().expect("test lock");
    reset_test_state();
    let mut api = Box::new(stub_api());
    let runtime = RimeRuntime::for_test(api.as_mut());
    for schema in ["schema.one", "schema.two"] {
        let session = RimeEngine::new_with_runtime(
            test_config(schema).with_deploy_on_start(true),
            Arc::clone(&runtime),
        )
        .unwrap();
        drop(session);
        with_test_state(|state| {
            assert_eq!(state.config_open_count, 8);
            assert_eq!(state.config_close_count, 0);
        });
    }
    runtime.shutdown().unwrap();
    with_test_state(|state| {
        assert_eq!(state.config_close_count, 8);
        assert_eq!(state.tasks, ["installation_update", "workspace_update"]);
    });
}
