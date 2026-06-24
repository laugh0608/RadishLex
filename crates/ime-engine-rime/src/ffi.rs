use std::ffi::{c_char, c_int, c_void};

pub type RimeSessionId = usize;
pub type Bool = c_int;

pub const TRUE: Bool = 1;

#[repr(C)]
pub struct RimeTraits {
    pub data_size: c_int,
    pub shared_data_dir: *const c_char,
    pub user_data_dir: *const c_char,
    pub distribution_name: *const c_char,
    pub distribution_code_name: *const c_char,
    pub distribution_version: *const c_char,
    pub app_name: *const c_char,
    pub modules: *const *const c_char,
    pub min_log_level: c_int,
    pub log_dir: *const c_char,
    pub prebuilt_data_dir: *const c_char,
    pub staging_dir: *const c_char,
}

#[repr(C)]
pub struct RimeApi {
    pub data_size: c_int,
    pub setup: Option<unsafe extern "C" fn(*mut RimeTraits)>,
    pub set_notification_handler:
        Option<unsafe extern "C" fn(RimeNotificationHandler, *mut c_void)>,
    pub initialize: Option<unsafe extern "C" fn(*mut RimeTraits)>,
    pub finalize: Option<unsafe extern "C" fn()>,
    pub start_maintenance: Option<unsafe extern "C" fn(Bool) -> Bool>,
    pub is_maintenance_mode: Option<unsafe extern "C" fn() -> Bool>,
    pub join_maintenance_thread: Option<unsafe extern "C" fn()>,
    pub deployer_initialize: Option<unsafe extern "C" fn(*mut RimeTraits)>,
    pub prebuild: Option<unsafe extern "C" fn() -> Bool>,
    pub deploy: Option<unsafe extern "C" fn() -> Bool>,
    pub deploy_schema: Option<unsafe extern "C" fn(*const c_char) -> Bool>,
    pub deploy_config_file: Option<unsafe extern "C" fn(*const c_char, *const c_char) -> Bool>,
    pub sync_user_data: Option<unsafe extern "C" fn() -> Bool>,
    pub create_session: Option<unsafe extern "C" fn() -> RimeSessionId>,
    pub find_session: Option<unsafe extern "C" fn(RimeSessionId) -> Bool>,
    pub destroy_session: Option<unsafe extern "C" fn(RimeSessionId) -> Bool>,
    pub cleanup_stale_sessions: Option<unsafe extern "C" fn()>,
    pub cleanup_all_sessions: Option<unsafe extern "C" fn()>,
    pub process_key: Option<unsafe extern "C" fn(RimeSessionId, c_int, c_int) -> Bool>,
    pub commit_composition: Option<unsafe extern "C" fn(RimeSessionId) -> Bool>,
    pub clear_composition: Option<unsafe extern "C" fn(RimeSessionId)>,
    pub get_commit: Option<unsafe extern "C" fn(RimeSessionId, *mut RimeCommit) -> Bool>,
    pub free_commit: Option<unsafe extern "C" fn(*mut RimeCommit) -> Bool>,
    pub get_context: Option<unsafe extern "C" fn(RimeSessionId, *mut RimeContext) -> Bool>,
    pub free_context: Option<unsafe extern "C" fn(*mut RimeContext) -> Bool>,
    pub get_status: Option<unsafe extern "C" fn(RimeSessionId, *mut RimeStatus) -> Bool>,
    pub free_status: Option<unsafe extern "C" fn(*mut RimeStatus) -> Bool>,
    pub set_option: Option<unsafe extern "C" fn(RimeSessionId, *const c_char, Bool)>,
    pub get_option: Option<unsafe extern "C" fn(RimeSessionId, *const c_char) -> Bool>,
    pub set_property: Option<unsafe extern "C" fn(RimeSessionId, *const c_char, *const c_char)>,
    pub get_property:
        Option<unsafe extern "C" fn(RimeSessionId, *const c_char, *mut c_char, usize) -> Bool>,
    pub get_schema_list: Option<unsafe extern "C" fn(*mut RimeSchemaList) -> Bool>,
    pub free_schema_list: Option<unsafe extern "C" fn(*mut RimeSchemaList)>,
    pub get_current_schema: Option<unsafe extern "C" fn(RimeSessionId, *mut c_char, usize) -> Bool>,
    pub select_schema: Option<unsafe extern "C" fn(RimeSessionId, *const c_char) -> Bool>,
}

#[repr(C)]
pub struct RimeContext {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RimeCommit {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RimeStatus {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RimeSchemaList {
    _private: [u8; 0],
}

pub type RimeNotificationHandler =
    Option<unsafe extern "C" fn(*mut c_void, RimeSessionId, *const c_char, *const c_char)>;
extern "C" {
    pub fn rime_get_api() -> *mut RimeApi;
}
