use std::ffi::CString;
use std::mem;
use std::path::Path;
use std::ptr;

use crate::config::RimeEngineConfig;
use crate::error::{RimeEngineError, RimeEngineResult};
use crate::ffi::{self, Bool, RimeApi, RimeSessionId, RimeTraits, TRUE};

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
