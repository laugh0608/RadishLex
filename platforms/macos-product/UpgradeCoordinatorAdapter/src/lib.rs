//! Manifest-bound macOS host adapter for product data upgrades.

#![forbid(unsafe_code)]

use std::fmt;
#[cfg(feature = "qualification-harness")]
use std::fs;
#[cfg(feature = "qualification-harness")]
use std::os::unix::fs::MetadataExt;
use std::path::Path;
#[cfg(feature = "qualification-harness")]
use std::path::{Component, PathBuf};

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
#[cfg(feature = "qualification-harness")]
const QUALIFICATION_MARKER_FILE: &str = "radishlex-upgrade-qualification.marker";
#[cfg(feature = "qualification-harness")]
const QUALIFICATION_MARKER_BYTES: &[u8] = b"radishlex-upgrade-qualification-v1\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacOsUpgradeAdapterError {
    UnsafeProduct,
    InvalidManifest,
    ProductChanged,
    PreflightRejected,
    #[cfg(feature = "qualification-harness")]
    UnsafeQualification,
}

impl fmt::Display for MacOsUpgradeAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsafeProduct => "macOS upgrade product root is unsafe",
            Self::InvalidManifest => "macOS upgrade product manifest is invalid",
            Self::ProductChanged => "macOS upgrade product content changed",
            Self::PreflightRejected => "macOS upgrade preflight was not proven",
            #[cfg(feature = "qualification-harness")]
            Self::UnsafeQualification => "macOS upgrade qualification root is unsafe",
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
            runner: Box::new(ProcessProductHostRunner::production()),
        })
    }

    #[cfg(feature = "qualification-harness")]
    pub fn load_for_qualification(
        source_product_root: &Path,
        target_product_root: &Path,
        qualification_root: &Path,
        synthetic_user_home: &Path,
    ) -> Result<Self, MacOsUpgradeAdapterError> {
        let qualification_root_input = qualification_root.to_path_buf();
        let qualification_root = verify_qualification_root(qualification_root)?;
        let synthetic_user_home = verify_qualification_path(
            &qualification_root_input,
            &qualification_root,
            synthetic_user_home,
            0o700,
        )?;
        let source_product_root = verify_qualification_path(
            &qualification_root_input,
            &qualification_root,
            source_product_root,
            0o700,
        )?;
        let target_product_root = verify_qualification_path(
            &qualification_root_input,
            &qualification_root,
            target_product_root,
            0o700,
        )?;
        let source = VerifiedProductAssembly::load(&source_product_root, ProductRole::Source)?;
        let target = VerifiedProductAssembly::load(&target_product_root, ProductRole::Target)?;
        Ok(Self {
            source,
            target,
            runner: Box::new(ProcessProductHostRunner::qualification(synthetic_user_home)),
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

#[cfg(feature = "qualification-harness")]
fn verify_qualification_root(root: &Path) -> Result<PathBuf, MacOsUpgradeAdapterError> {
    let root_metadata =
        fs::symlink_metadata(root).map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(MacOsUpgradeAdapterError::UnsafeQualification);
    }
    let temp_root = fs::canonicalize(std::env::temp_dir())
        .map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    let root = fs::canonicalize(root).map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    if root == temp_root || !root.starts_with(&temp_root) {
        return Err(MacOsUpgradeAdapterError::UnsafeQualification);
    }
    let owner_id = verify_private_directory(&root)?;
    let marker = root.join(QUALIFICATION_MARKER_FILE);
    let metadata =
        fs::symlink_metadata(&marker).map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != owner_id
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != 0o600
        || fs::read(&marker).map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?
            != QUALIFICATION_MARKER_BYTES
    {
        return Err(MacOsUpgradeAdapterError::UnsafeQualification);
    }
    Ok(root)
}

#[cfg(feature = "qualification-harness")]
fn verify_qualification_path(
    qualification_root_input: &Path,
    qualification_root: &Path,
    path: &Path,
    expected_mode: u32,
) -> Result<PathBuf, MacOsUpgradeAdapterError> {
    let relative = path
        .strip_prefix(qualification_root_input)
        .or_else(|_| path.strip_prefix(qualification_root))
        .map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    if relative.as_os_str().is_empty() {
        return Err(MacOsUpgradeAdapterError::UnsafeQualification);
    }
    let mut current = if path.starts_with(qualification_root_input) {
        qualification_root_input.to_path_buf()
    } else {
        qualification_root.to_path_buf()
    };
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(MacOsUpgradeAdapterError::UnsafeQualification);
        }
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
        if metadata.file_type().is_symlink() {
            return Err(MacOsUpgradeAdapterError::UnsafeQualification);
        }
    }
    let canonical =
        fs::canonicalize(path).map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    if canonical == qualification_root || !canonical.starts_with(qualification_root) {
        return Err(MacOsUpgradeAdapterError::UnsafeQualification);
    }
    let owner_id = fs::symlink_metadata(qualification_root)
        .map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?
        .uid();
    let mut current = qualification_root.to_path_buf();
    let relative = canonical
        .strip_prefix(qualification_root)
        .map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        if !matches!(component, Component::Normal(_)) {
            return Err(MacOsUpgradeAdapterError::UnsafeQualification);
        }
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != owner_id
            || (index + 1 < components.len() && !metadata.is_dir())
        {
            return Err(MacOsUpgradeAdapterError::UnsafeQualification);
        }
    }
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    if !metadata.is_dir() || metadata.mode() & 0o7777 != expected_mode {
        return Err(MacOsUpgradeAdapterError::UnsafeQualification);
    }
    Ok(canonical)
}

#[cfg(feature = "qualification-harness")]
fn verify_private_directory(path: &Path) -> Result<u32, MacOsUpgradeAdapterError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| MacOsUpgradeAdapterError::UnsafeQualification)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.mode() & 0o7777 != 0o700
    {
        return Err(MacOsUpgradeAdapterError::UnsafeQualification);
    }
    Ok(metadata.uid())
}

#[cfg(test)]
mod tests;
