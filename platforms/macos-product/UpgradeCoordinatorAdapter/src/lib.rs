//! Manifest-bound macOS host adapter for product data upgrades.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::Path;

use radishlex_ime_product_upgrade::{
    ProductRelease, UpgradeCandidateValidationReport, UpgradeCoordinatorCheckpoint,
    UpgradeCoordinatorPort, UpgradeInputMethodValidationEvidence, UpgradeManagerValidationEvidence,
    UpgradePostSwitchValidationReport, UpgradeRollbackValidationEvidence,
    UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION, UPGRADE_VALIDATION_EVIDENCE_VERSION,
};
use serde::Deserialize;

mod manifest;
mod runner;

use manifest::{ProductRole, VerifiedExecutable, VerifiedProductAssembly};
use runner::{HostMode, HostOutput, ProcessProductHostRunner, ProductHostRunner};

const PREFLIGHT_FORMAT: &str = "radishlex-upgrade-preflight-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacOsUpgradeAdapterError {
    UnsafeProduct,
    InvalidManifest,
    ProductChanged,
    PreflightRejected,
}

impl fmt::Display for MacOsUpgradeAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsafeProduct => "macOS upgrade product root is unsafe",
            Self::InvalidManifest => "macOS upgrade product manifest is invalid",
            Self::ProductChanged => "macOS upgrade product content changed",
            Self::PreflightRejected => "macOS upgrade preflight was not proven",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for MacOsUpgradeAdapterError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOsUpgradePreflight {
    available_bytes: u64,
}

impl MacOsUpgradePreflight {
    pub const fn available_bytes(self) -> u64 {
        self.available_bytes
    }
}

pub struct MacOsUpgradeCoordinatorAdapter {
    source: VerifiedProductAssembly,
    target: VerifiedProductAssembly,
    runner: Box<dyn ProductHostRunner>,
}

impl fmt::Debug for MacOsUpgradeCoordinatorAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MacOsUpgradeCoordinatorAdapter")
            .field("source_schema_version", &self.source.schema_version())
            .field("target_schema_version", &self.target.schema_version())
            .finish_non_exhaustive()
    }
}

impl MacOsUpgradeCoordinatorAdapter {
    pub fn load(
        source_product_root: &Path,
        target_product_root: &Path,
    ) -> Result<Self, MacOsUpgradeAdapterError> {
        let source = VerifiedProductAssembly::load(source_product_root, ProductRole::Source)?;
        let target = VerifiedProductAssembly::load(target_product_root, ProductRole::Target)?;
        Ok(Self {
            source,
            target,
            runner: Box::new(ProcessProductHostRunner),
        })
    }

    pub fn inspect_preflight(&mut self) -> Result<MacOsUpgradePreflight, MacOsUpgradeAdapterError> {
        let executable = self
            .target
            .preflight()
            .ok_or(MacOsUpgradeAdapterError::InvalidManifest)?;
        executable.revalidate()?;
        parse_preflight(self.runner.run(executable, HostMode::Preflight))
    }

    #[cfg(test)]
    fn with_runner(
        source_product_root: &Path,
        target_product_root: &Path,
        runner: Box<dyn ProductHostRunner>,
    ) -> Result<Self, MacOsUpgradeAdapterError> {
        let mut adapter = Self::load(source_product_root, target_product_root)?;
        adapter.runner = runner;
        Ok(adapter)
    }

    fn validation_host_succeeded(
        &mut self,
        executable: &VerifiedExecutable,
        mode: HostMode,
    ) -> bool {
        executable.revalidate().is_ok() && {
            let output = self.runner.run(executable, mode);
            output.success && output.stdout.is_empty() && output.stderr.is_empty()
        }
    }
}

impl UpgradeCoordinatorPort for MacOsUpgradeCoordinatorAdapter {
    fn confirm_quiescence(&mut self, _checkpoint: UpgradeCoordinatorCheckpoint) -> bool {
        self.inspect_preflight().is_ok()
    }

    fn validate_candidate(
        &mut self,
        target_release: &ProductRelease,
        target_schema_version: i64,
    ) -> UpgradeCandidateValidationReport {
        if !self.target.matches(target_release, target_schema_version) {
            return UpgradeCandidateValidationReport::manager_failed();
        }
        let manager = self.target.manager_validation().clone();
        let input_method = self.target.input_method_validation().clone();
        if !self.validation_host_succeeded(&manager, HostMode::Candidate) {
            return UpgradeCandidateValidationReport::manager_failed();
        }
        let manager_evidence = manager_evidence(target_schema_version);
        if !self.validation_host_succeeded(&input_method, HostMode::Candidate) {
            return UpgradeCandidateValidationReport::input_method_failed(manager_evidence);
        }
        UpgradeCandidateValidationReport::passed(
            manager_evidence,
            input_method_evidence(target_schema_version),
        )
    }

    fn validate_post_switch(
        &mut self,
        target_release: &ProductRelease,
        target_schema_version: i64,
    ) -> UpgradePostSwitchValidationReport {
        if !self.target.matches(target_release, target_schema_version) {
            return UpgradePostSwitchValidationReport::manager_failed();
        }
        let manager = self.target.manager_validation().clone();
        let input_method = self.target.input_method_validation().clone();
        if !self.validation_host_succeeded(&manager, HostMode::PostSwitch) {
            return UpgradePostSwitchValidationReport::manager_failed();
        }
        let manager_evidence = manager_evidence(target_schema_version);
        if !self.validation_host_succeeded(&input_method, HostMode::PostSwitch) {
            return UpgradePostSwitchValidationReport::input_method_failed(manager_evidence);
        }
        UpgradePostSwitchValidationReport::passed(
            manager_evidence,
            input_method_evidence(target_schema_version),
        )
    }

    fn validate_restored_source(
        &mut self,
        source_release: &ProductRelease,
        source_schema_version: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        if !self.source.matches(source_release, source_schema_version) {
            return None;
        }
        let manager = self.source.manager_validation().clone();
        let input_method = self.source.input_method_validation().clone();
        if !self.validation_host_succeeded(&manager, HostMode::PostSwitch)
            || !self.validation_host_succeeded(&input_method, HostMode::PostSwitch)
        {
            return None;
        }
        Some(UpgradeRollbackValidationEvidence::new(
            UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
            source_schema_version,
            1,
            1,
        ))
    }
}

fn manager_evidence(schema_version: i64) -> UpgradeManagerValidationEvidence {
    UpgradeManagerValidationEvidence::new(UPGRADE_VALIDATION_EVIDENCE_VERSION, schema_version, 1, 1)
}

fn input_method_evidence(schema_version: i64) -> UpgradeInputMethodValidationEvidence {
    UpgradeInputMethodValidationEvidence::new(
        UPGRADE_VALIDATION_EVIDENCE_VERSION,
        schema_version,
        1,
        1,
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreflightOutput {
    format: String,
    result: String,
    available_bytes: u64,
    quiescent: bool,
}

fn parse_preflight(output: HostOutput) -> Result<MacOsUpgradePreflight, MacOsUpgradeAdapterError> {
    if !output.success || !output.stderr.is_empty() || output.stdout.len() > 16 * 1024 {
        return Err(MacOsUpgradeAdapterError::PreflightRejected);
    }
    let parsed: PreflightOutput = serde_json::from_slice(&output.stdout)
        .map_err(|_| MacOsUpgradeAdapterError::PreflightRejected)?;
    if parsed.format != PREFLIGHT_FORMAT
        || parsed.result != "ready"
        || !parsed.quiescent
        || parsed.available_bytes == 0
    {
        return Err(MacOsUpgradeAdapterError::PreflightRejected);
    }
    Ok(MacOsUpgradePreflight {
        available_bytes: parsed.available_bytes,
    })
}

#[cfg(test)]
mod tests;
