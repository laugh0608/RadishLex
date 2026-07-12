use std::ffi::CString;
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr;

use crate::config::RimeEngineConfig;
use crate::error::{RimeEngineError, RimeEngineResult};
use crate::ffi::RimeTraits;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RimeRuntimeConfig {
    shared_data_dir: PathBuf,
    user_data_dir: PathBuf,
    log_dir: Option<PathBuf>,
    deploy_on_start: bool,
}

impl RimeRuntimeConfig {
    pub(crate) fn incompatible_field(&self, other: &Self) -> Option<&'static str> {
        if self.shared_data_dir != other.shared_data_dir {
            Some("shared_data_dir")
        } else if self.user_data_dir != other.user_data_dir {
            Some("user_data_dir")
        } else if self.log_dir != other.log_dir {
            Some("log_dir")
        } else if self.deploy_on_start != other.deploy_on_start {
            Some("deploy_on_start")
        } else {
            None
        }
    }
}

impl From<&RimeEngineConfig> for RimeRuntimeConfig {
    fn from(config: &RimeEngineConfig) -> Self {
        Self {
            shared_data_dir: config.shared_data_dir().to_owned(),
            user_data_dir: config.user_data_dir().to_owned(),
            log_dir: config.log_dir().map(Path::to_owned),
            deploy_on_start: config.deploy_on_start(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct NativeRimeStrings {
    shared_data_dir: CString,
    user_data_dir: CString,
    app_name: CString,
    log_dir: Option<CString>,
}

impl NativeRimeStrings {
    pub(crate) fn new(config: &RimeEngineConfig) -> RimeEngineResult<Self> {
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
        })
    }

    pub(crate) fn traits(&self) -> RimeTraits {
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
