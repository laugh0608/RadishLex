//! Validate and pin effective native configs before Rime creates any engine.
//!
//! A schema handle shares librime's cached ConfigData with engines. Keeping it
//! open prevents a later session (including after a zero-session gap) from
//! reloading a replaced file. No config setters or private librime APIs are used.

use std::ffi::{c_char, CString};
use std::ptr;

use crate::error::{RimeEngineError, RimeEngineResult};
use crate::ffi::{RimeApi, RimeConfig, TRUE};
use crate::runtime::{ensure_true, require_api_function};

#[derive(Default)]
pub(crate) struct LearningGuard {
    configs: Vec<RimeConfig>,
    schemas: Vec<String>,
}

// SAFETY: handles are accessed and closed only under the runtime mutex on its
// initializing thread. They never escape to callers or outlive native finalize.
unsafe impl Send for LearningGuard {}

impl LearningGuard {
    pub(crate) fn require_api(api: &RimeApi) -> RimeEngineResult<()> {
        require_api_function(api.config_open, "config_open")?;
        require_api_function(api.schema_open, "schema_open")?;
        require_api_function(api.config_close, "config_close")?;
        require_api_function(api.config_get_bool, "config_get_bool")?;
        require_api_function(api.config_get_string, "config_get_string")?;
        require_api_function(api.config_list_size, "config_list_size")?;
        Ok(())
    }

    pub(crate) fn initialize(&mut self, api: &RimeApi) -> RimeEngineResult<()> {
        self.open(api, "default", false)?;
        let default = &mut self.configs[0];
        let count = list_size(api, default, "schema_list")?;
        if count == 0 || count > 64 {
            return Err(rejected("default schema_list must contain 1..64 schemas"));
        }
        // Check every entry, even conditionally hidden ones: native startup may
        // restore a previous schema before the adapter selects the requested one.
        for index in 0..count {
            let schema = string(api, default, &format!("schema_list/@{index}/schema"))?;
            if schema.is_empty()
                || schema.starts_with('.')
                || !schema
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
            {
                return Err(rejected("unsupported schema id in default schema_list"));
            }
            if !self.schemas.contains(&schema) {
                self.schemas.push(schema);
            }
        }
        for schema in self.schemas.clone() {
            self.open(api, &schema, true)?;
            validate_schema(api, self.configs.last_mut().expect("opened config"))?;
        }
        Ok(())
    }

    pub(crate) fn ensure_schema(&self, schema: &str) -> RimeEngineResult<()> {
        if self.schemas.iter().any(|allowed| allowed == schema) {
            Ok(())
        } else {
            Err(rejected(&format!(
                "requested schema {schema} is outside the validated schema_list"
            )))
        }
    }

    fn open(&mut self, api: &RimeApi, name: &str, schema: bool) -> RimeEngineResult<()> {
        let name = key(name)?;
        let open = if schema {
            require_api_function(api.schema_open, "schema_open")?
        } else {
            require_api_function(api.config_open, "config_open")?
        };
        let mut config = RimeConfig {
            ptr: ptr::null_mut(),
        };
        // SAFETY: native runtime is initialized and name/config are live.
        let opened = unsafe { open(name.as_ptr(), &mut config) };
        let has_handle = !config.ptr.is_null();
        if has_handle {
            self.configs.push(config);
        }
        ensure_true("learning_guard_open", opened)?;
        if !has_handle {
            return Err(rejected("native config handle is empty"));
        }
        Ok(())
    }

    pub(crate) fn close(&mut self, api: &RimeApi) -> RimeEngineResult<()> {
        let close = require_api_function(api.config_close, "config_close")?;
        let mut result = Ok(());
        for mut config in self.configs.drain(..).rev() {
            // SAFETY: each successful open is closed once before finalize, on
            // the same serialized owner thread; no engine remains active.
            let closed = unsafe { close(&mut config) };
            if closed != TRUE {
                result = ensure_true("learning_guard_close", closed);
            }
        }
        self.schemas.clear();
        result
    }
}

fn validate_schema(api: &RimeApi, config: &mut RimeConfig) -> RimeEngineResult<()> {
    // Only built-in components reviewed for the product's base-candidate path.
    // Namespaced/custom translators, filters and Lua must be reviewed separately.
    for (field, allowed) in [
        (
            "processors",
            &[
                "ascii_composer",
                "key_binder",
                "speller",
                "punctuator",
                "selector",
                "navigator",
                "express_editor",
            ][..],
        ),
        (
            "segmentors",
            &[
                "ascii_segmentor",
                "abc_segmentor",
                "punct_segmentor",
                "fallback_segmentor",
            ][..],
        ),
        (
            "translators",
            &["punct_translator", "script_translator"][..],
        ),
        ("filters", &[][..]),
    ] {
        let path = format!("engine/{field}");
        let count = list_size(api, config, &path)?;
        if count > 64 || (field != "filters" && count == 0) {
            return Err(rejected(&format!("unsupported {path} length")));
        }
        for index in 0..count {
            let component = string(api, config, &format!("{path}/@{index}"))?;
            if !allowed.contains(&component.as_str()) {
                return Err(rejected(&format!("unsupported component in {path}")));
            }
        }
    }
    let path = key("translator/enable_user_dict")?;
    let mut enabled = -1;
    // SAFETY: live config, nul-terminated path and valid output pointer.
    let found = unsafe {
        require_api_function(api.config_get_bool, "config_get_bool")?(
            config,
            path.as_ptr(),
            &mut enabled,
        )
    };
    if found != TRUE || enabled != 0 {
        return Err(rejected(
            "translator/enable_user_dict must be explicitly false",
        ));
    }
    Ok(())
}

fn list_size(api: &RimeApi, config: &mut RimeConfig, path: &str) -> RimeEngineResult<usize> {
    let path = key(path)?;
    // SAFETY: live config and nul-terminated path, under the runtime mutex.
    Ok(unsafe {
        require_api_function(api.config_list_size, "config_list_size")?(config, path.as_ptr())
    })
}

fn string(api: &RimeApi, config: &mut RimeConfig, path: &str) -> RimeEngineResult<String> {
    let path = key(path)?;
    let mut buffer = [1 as c_char; 256];
    // SAFETY: live config/path and writable bounded buffer. Reject missing,
    // oversized or malformed values rather than accepting a truncated prefix.
    let found = unsafe {
        require_api_function(api.config_get_string, "config_get_string")?(
            config,
            path.as_ptr(),
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    };
    if found != TRUE {
        return Err(rejected("required config string is missing"));
    }
    let len = buffer
        .iter()
        .position(|c| *c == 0)
        .filter(|len| *len < buffer.len() - 1)
        .ok_or_else(|| rejected("config string is oversized or unterminated"))?;
    String::from_utf8(buffer[..len].iter().map(|c| *c as u8).collect())
        .map_err(|_| rejected("config string is not UTF-8"))
}

fn key(value: &str) -> RimeEngineResult<CString> {
    CString::new(value).map_err(|_| rejected("config key contains nul"))
}

fn rejected(message: &str) -> RimeEngineError {
    RimeEngineError::FfiFailure {
        stage: "learning_guard",
        message: message.to_owned(),
    }
}
