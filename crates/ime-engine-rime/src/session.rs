use crate::config::RimeEngineConfig;
use crate::error::{RimeEngineError, RimeEngineResult};

#[derive(Debug)]
pub struct RimeEngine {
    config: RimeEngineConfig,
}

impl RimeEngine {
    pub fn new(config: RimeEngineConfig) -> RimeEngineResult<Self> {
        let _ = config;
        Err(RimeEngineError::FfiFailure {
            stage: "initialize",
            message: "native Rime session is not implemented in this skeleton".to_owned(),
        })
    }

    pub fn config(&self) -> &RimeEngineConfig {
        &self.config
    }
}
