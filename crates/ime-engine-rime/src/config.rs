use std::path::{Path, PathBuf};

use radishlex_ime_core::SchemaId;

use crate::error::{RimeEngineError, RimeEngineResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RimeEngineConfig {
    shared_data_dir: PathBuf,
    user_data_dir: PathBuf,
    log_dir: Option<PathBuf>,
    schema: SchemaId,
    deploy_on_start: bool,
}

impl RimeEngineConfig {
    pub fn new(
        shared_data_dir: impl Into<PathBuf>,
        user_data_dir: impl Into<PathBuf>,
        schema: SchemaId,
    ) -> RimeEngineResult<Self> {
        let shared_data_dir = shared_data_dir.into();
        let user_data_dir = user_data_dir.into();
        validate_config_path("shared_data_dir", &shared_data_dir)?;
        validate_config_path("user_data_dir", &user_data_dir)?;

        Ok(Self {
            shared_data_dir,
            user_data_dir,
            log_dir: None,
            schema,
            deploy_on_start: false,
        })
    }

    pub fn with_log_dir(mut self, log_dir: impl Into<PathBuf>) -> RimeEngineResult<Self> {
        let log_dir = log_dir.into();
        validate_config_path("log_dir", &log_dir)?;
        self.log_dir = Some(log_dir);
        Ok(self)
    }

    pub fn with_deploy_on_start(mut self, deploy_on_start: bool) -> Self {
        self.deploy_on_start = deploy_on_start;
        self
    }

    pub fn shared_data_dir(&self) -> &Path {
        &self.shared_data_dir
    }

    pub fn user_data_dir(&self) -> &Path {
        &self.user_data_dir
    }

    pub fn log_dir(&self) -> Option<&Path> {
        self.log_dir.as_deref()
    }

    pub fn schema(&self) -> &SchemaId {
        &self.schema
    }

    pub fn deploy_on_start(&self) -> bool {
        self.deploy_on_start
    }
}

fn validate_config_path(field: &'static str, path: &Path) -> RimeEngineResult<()> {
    if path.as_os_str().is_empty() {
        return Err(RimeEngineError::MissingConfigPath { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use radishlex_ime_core::SchemaId;

    use super::RimeEngineConfig;
    use crate::RimeEngineError;

    #[test]
    fn config_requires_non_empty_data_dirs() {
        let err = RimeEngineConfig::new("", "user", SchemaId::new("demo").expect("valid schema"))
            .expect_err("empty shared dir must fail");

        assert_eq!(
            err,
            RimeEngineError::MissingConfigPath {
                field: "shared_data_dir"
            }
        );
    }

    #[test]
    fn config_stores_schema_and_paths() {
        let config = RimeEngineConfig::new(
            "shared",
            "user",
            SchemaId::new("luna_pinyin").expect("valid schema"),
        )
        .expect("config is valid")
        .with_log_dir("logs")
        .expect("log dir is valid")
        .with_deploy_on_start(true);

        assert_eq!(config.shared_data_dir().to_string_lossy(), "shared");
        assert_eq!(config.user_data_dir().to_string_lossy(), "user");
        assert_eq!(config.log_dir().expect("log dir").to_string_lossy(), "logs");
        assert_eq!(config.schema().as_str(), "luna_pinyin");
        assert!(config.deploy_on_start());
    }
}
