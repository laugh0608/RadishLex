use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const LINUX_INSTALL_RECEIPT_FORMAT: &str = "radishlex-linux-install-receipt-v1";
pub const LINUX_INSTALL_PRODUCT_ID: &str = "radishlex-linux";
pub const LINUX_DISTRIBUTION_IDENTITY: &str = "debian-local-deb-v1";
pub const MAX_LINUX_INSTALL_RECEIPT_BYTES: usize = 64 * 1024;
const MAX_OPERATION_CHAIN_LENGTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinuxOperationKind {
    Install,
    Upgrade,
    Repair,
    Remove,
    Rollback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactVersionRelation {
    TargetNewer,
    SameRelease,
    TargetOlder,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinuxInstallState {
    Prepared,
    ArtifactsStaged,
    Quiesced,
    PackageMutating,
    PackageVerified,
    Completed,
    AbortedPreserved,
    RollbackRequired,
    SourceRestoring,
    SourceVerified,
    RolledBack,
}

impl LinuxInstallState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::AbortedPreserved | Self::RolledBack
        )
    }

    const fn is_before_package_mutation(self) -> bool {
        matches!(
            self,
            Self::Prepared | Self::ArtifactsStaged | Self::Quiesced
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinuxFailureCode {
    ReceiptInconsistent,
    ArtifactInvalid,
    ArtifactStagingFailed,
    EnvironmentUnsupported,
    DependencyUnavailable,
    VersionRelationInvalid,
    ProgramsRunning,
    ProcessInspectionUnavailable,
    PackageStateUnavailable,
    PackageStateUnknown,
    PackageStateUnexpected,
    CommandInvalid,
    PackageMutationFailed,
    TargetValidationFailed,
    SourceRestoreFailed,
    SourceValidationFailed,
    PermissionDenied,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DpkgPackageState {
    NotInstalled,
    ConfigFiles,
    Installed,
    Unpacked,
    HalfConfigured,
    HalfInstalled,
    TriggersAwaited,
    TriggersPending,
    Unknown,
}

impl DpkgPackageState {
    pub const fn is_absent(self) -> bool {
        matches!(self, Self::NotInstalled | Self::ConfigFiles)
    }

    pub const fn is_recoverable(self) -> bool {
        matches!(
            self,
            Self::Installed
                | Self::Unpacked
                | Self::HalfConfigured
                | Self::HalfInstalled
                | Self::TriggersAwaited
                | Self::TriggersPending
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataContractIdentity {
    ffi_abi_version: u32,
    userdb_schema_version: u32,
    runtime_layout: String,
    data_layout: String,
    settings_format_version: u32,
    privacy_format_version: u32,
    rime_schema_id: String,
    rime_data_lock_sha256: String,
}

impl DataContractIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ffi_abi_version: u32,
        userdb_schema_version: u32,
        runtime_layout: impl Into<String>,
        data_layout: impl Into<String>,
        settings_format_version: u32,
        privacy_format_version: u32,
        rime_schema_id: impl Into<String>,
        rime_data_lock_sha256: impl Into<String>,
    ) -> Result<Self, LinuxInstallReceiptError> {
        let identity = Self {
            ffi_abi_version,
            userdb_schema_version,
            runtime_layout: runtime_layout.into(),
            data_layout: data_layout.into(),
            settings_format_version,
            privacy_format_version,
            rime_schema_id: rime_schema_id.into(),
            rime_data_lock_sha256: rime_data_lock_sha256.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub const fn ffi_abi_version(&self) -> u32 {
        self.ffi_abi_version
    }

    pub const fn userdb_schema_version(&self) -> u32 {
        self.userdb_schema_version
    }

    pub fn runtime_layout(&self) -> &str {
        &self.runtime_layout
    }

    pub fn data_layout(&self) -> &str {
        &self.data_layout
    }

    pub const fn settings_format_version(&self) -> u32 {
        self.settings_format_version
    }

    pub const fn privacy_format_version(&self) -> u32 {
        self.privacy_format_version
    }

    pub fn rime_schema_id(&self) -> &str {
        &self.rime_schema_id
    }

    pub fn rime_data_lock_sha256(&self) -> &str {
        &self.rime_data_lock_sha256
    }

    fn validate(&self) -> Result<(), LinuxInstallReceiptError> {
        if self.ffi_abi_version == 0
            || self.userdb_schema_version == 0
            || self.settings_format_version == 0
            || self.privacy_format_version == 0
            || !valid_identifier(&self.runtime_layout)
            || !valid_identifier(&self.data_layout)
            || !valid_identifier(&self.rime_schema_id)
        {
            return Err(LinuxInstallReceiptError::invalid(
                "data_contract",
                "data contract fields are invalid",
            ));
        }
        validate_sha256(&self.rime_data_lock_sha256, "rime_data_lock_sha256")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinuxArtifactIdentity {
    product_id: String,
    distribution_identity: String,
    package_name: String,
    package_version: String,
    architecture: String,
    package_filename: String,
    evidence_filename: String,
    package_size: u64,
    evidence_size: u64,
    package_sha256: String,
    evidence_sha256: String,
    product_manifest_sha256: String,
    data_contract: DataContractIdentity,
}

impl LinuxArtifactIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        package_version: impl Into<String>,
        package_size: u64,
        evidence_size: u64,
        package_sha256: impl Into<String>,
        evidence_sha256: impl Into<String>,
        product_manifest_sha256: impl Into<String>,
        data_contract: DataContractIdentity,
    ) -> Result<Self, LinuxInstallReceiptError> {
        let package_version = package_version.into();
        let package_filename = format!("radishlex_{package_version}_arm64.deb");
        let identity = Self {
            product_id: LINUX_INSTALL_PRODUCT_ID.to_owned(),
            distribution_identity: LINUX_DISTRIBUTION_IDENTITY.to_owned(),
            package_name: "radishlex".to_owned(),
            package_version,
            architecture: "arm64".to_owned(),
            evidence_filename: format!("{package_filename}.evidence.json"),
            package_filename,
            package_size,
            evidence_size,
            package_sha256: package_sha256.into(),
            evidence_sha256: evidence_sha256.into(),
            product_manifest_sha256: product_manifest_sha256.into(),
            data_contract,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn package_version(&self) -> &str {
        &self.package_version
    }

    pub fn package_filename(&self) -> &str {
        &self.package_filename
    }

    pub fn evidence_filename(&self) -> &str {
        &self.evidence_filename
    }

    pub const fn package_size(&self) -> u64 {
        self.package_size
    }

    pub const fn evidence_size(&self) -> u64 {
        self.evidence_size
    }

    pub fn package_sha256(&self) -> &str {
        &self.package_sha256
    }

    pub fn evidence_sha256(&self) -> &str {
        &self.evidence_sha256
    }

    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    pub fn product_manifest_sha256(&self) -> &str {
        &self.product_manifest_sha256
    }

    pub fn data_contract(&self) -> &DataContractIdentity {
        &self.data_contract
    }

    fn compatible_with(&self, other: &Self) -> bool {
        self.product_id == other.product_id
            && self.distribution_identity == other.distribution_identity
            && self.package_name == other.package_name
            && self.architecture == other.architecture
            && self.data_contract == other.data_contract
    }

    fn validate(&self) -> Result<(), LinuxInstallReceiptError> {
        if self.product_id != LINUX_INSTALL_PRODUCT_ID
            || self.distribution_identity != LINUX_DISTRIBUTION_IDENTITY
            || self.package_name != "radishlex"
            || self.architecture != "arm64"
            || !valid_debian_version(&self.package_version)
            || self.package_filename != format!("radishlex_{}_arm64.deb", self.package_version)
            || self.evidence_filename != format!("{}.evidence.json", self.package_filename)
            || self.package_size == 0
            || self.package_size > 512 * 1024 * 1024
            || self.evidence_size == 0
            || self.evidence_size > 1024 * 1024
        {
            return Err(LinuxInstallReceiptError::invalid(
                "artifact_identity",
                "Linux artifact identity differs from debian-local-deb-v1",
            ));
        }
        validate_sha256(&self.package_sha256, "package_sha256")?;
        validate_sha256(&self.evidence_sha256, "evidence_sha256")?;
        validate_sha256(&self.product_manifest_sha256, "product_manifest_sha256")?;
        self.data_contract.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSnapshot {
    state: DpkgPackageState,
    artifact: Option<LinuxArtifactIdentity>,
}

impl PackageSnapshot {
    pub fn new(
        state: DpkgPackageState,
        artifact: Option<LinuxArtifactIdentity>,
    ) -> Result<Self, LinuxInstallReceiptError> {
        let snapshot = Self { state, artifact };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub const fn state(&self) -> DpkgPackageState {
        self.state
    }

    pub fn artifact(&self) -> Option<&LinuxArtifactIdentity> {
        self.artifact.as_ref()
    }

    pub fn exact_installed(artifact: LinuxArtifactIdentity) -> Self {
        Self {
            state: DpkgPackageState::Installed,
            artifact: Some(artifact),
        }
    }

    pub fn absent() -> Self {
        Self {
            state: DpkgPackageState::NotInstalled,
            artifact: None,
        }
    }

    pub(crate) fn matches_installed(&self, expected: &LinuxArtifactIdentity) -> bool {
        self.state == DpkgPackageState::Installed && self.artifact.as_ref() == Some(expected)
    }

    pub(crate) fn matches_absent(&self) -> bool {
        self.state.is_absent() && self.artifact.is_none()
    }

    fn validate(&self) -> Result<(), LinuxInstallReceiptError> {
        if let Some(artifact) = &self.artifact {
            artifact.validate()?;
        }
        if self.state.is_absent() && self.artifact.is_some() {
            return Err(LinuxInstallReceiptError::invalid(
                "package_snapshot",
                "absent package state cannot carry an artifact",
            ));
        }
        if self.state.is_recoverable() && self.artifact.is_none() {
            return Err(LinuxInstallReceiptError::invalid(
                "package_snapshot",
                "recoverable package state requires an exact artifact identity",
            ));
        }
        if self.state == DpkgPackageState::Unknown && self.artifact.is_some() {
            return Err(LinuxInstallReceiptError::invalid(
                "package_snapshot",
                "unknown package state cannot claim an artifact identity",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxOperationRequest {
    operation_id: String,
    operation_kind: LinuxOperationKind,
    relation: ArtifactVersionRelation,
    source: Option<LinuxArtifactIdentity>,
    target: Option<LinuxArtifactIdentity>,
}

impl LinuxOperationRequest {
    pub fn new(
        operation_id: impl Into<String>,
        operation_kind: LinuxOperationKind,
        relation: ArtifactVersionRelation,
        source: Option<LinuxArtifactIdentity>,
        target: Option<LinuxArtifactIdentity>,
    ) -> Result<Self, LinuxInstallReceiptError> {
        let request = Self {
            operation_id: operation_id.into(),
            operation_kind,
            relation,
            source,
            target,
        };
        request.validate_products()?;
        validate_operation_id(&request.operation_id, "operation_id")?;
        Ok(request)
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub const fn operation_kind(&self) -> LinuxOperationKind {
        self.operation_kind
    }

    pub const fn version_relation(&self) -> ArtifactVersionRelation {
        self.relation
    }

    pub fn source_artifact(&self) -> Option<&LinuxArtifactIdentity> {
        self.source.as_ref()
    }

    pub fn target_artifact(&self) -> Option<&LinuxArtifactIdentity> {
        self.target.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinuxInstallRootIdentity {
    device_id: u64,
    inode: u64,
    owner_id: u32,
    group_id: u32,
    mode: u32,
}

impl LinuxInstallRootIdentity {
    pub fn new(
        device_id: u64,
        inode: u64,
        owner_id: u32,
        group_id: u32,
        mode: u32,
    ) -> Result<Self, LinuxInstallReceiptError> {
        let identity = Self {
            device_id,
            inode,
            owner_id,
            group_id,
            mode,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub const fn owner_id(&self) -> u32 {
        self.owner_id
    }

    fn validate(&self) -> Result<(), LinuxInstallReceiptError> {
        if self.inode == 0 || self.mode != 0o755 {
            return Err(LinuxInstallReceiptError::invalid(
                "root_identity",
                "Linux install root identity requires a positive inode and mode 0755",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSlot {
    Source,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactFileIdentity {
    device_id: u64,
    inode: u64,
    owner_id: u32,
    group_id: u32,
    mode: u32,
    hardlink_count: u64,
    size: u64,
    sha256: String,
}

impl ArtifactFileIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device_id: u64,
        inode: u64,
        owner_id: u32,
        group_id: u32,
        mode: u32,
        hardlink_count: u64,
        size: u64,
        sha256: impl Into<String>,
    ) -> Result<Self, LinuxInstallReceiptError> {
        let identity = Self {
            device_id,
            inode,
            owner_id,
            group_id,
            mode,
            hardlink_count,
            size,
            sha256: sha256.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) const fn device_id(&self) -> u64 {
        self.device_id
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) const fn inode(&self) -> u64 {
        self.inode
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) const fn owner_id(&self) -> u32 {
        self.owner_id
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) const fn group_id(&self) -> u32 {
        self.group_id
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) const fn mode(&self) -> u32 {
        self.mode
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) const fn hardlink_count(&self) -> u64 {
        self.hardlink_count
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) const fn size(&self) -> u64 {
        self.size
    }

    fn validate(&self) -> Result<(), LinuxInstallReceiptError> {
        if self.inode == 0 || self.mode != 0o600 || self.hardlink_count != 1 || self.size == 0 {
            return Err(LinuxInstallReceiptError::invalid(
                "artifact_file_identity",
                "staged artifact file identity is invalid",
            ));
        }
        validate_sha256(&self.sha256, "artifact_file_sha256")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedArtifactEvidence {
    slot: ArtifactSlot,
    artifact: LinuxArtifactIdentity,
    package_file: ArtifactFileIdentity,
    evidence_file: ArtifactFileIdentity,
}

impl StagedArtifactEvidence {
    pub fn new(
        slot: ArtifactSlot,
        artifact: LinuxArtifactIdentity,
        package_file: ArtifactFileIdentity,
        evidence_file: ArtifactFileIdentity,
    ) -> Result<Self, LinuxInstallReceiptError> {
        if package_file.size != artifact.package_size
            || evidence_file.size != artifact.evidence_size
            || package_file.sha256 != artifact.package_sha256
            || evidence_file.sha256 != artifact.evidence_sha256
        {
            return Err(LinuxInstallReceiptError::invalid(
                "staged_artifact",
                "staged file identity differs from artifact identity",
            ));
        }
        artifact.validate()?;
        package_file.validate()?;
        evidence_file.validate()?;
        Ok(Self {
            slot,
            artifact,
            package_file,
            evidence_file,
        })
    }

    pub const fn slot(&self) -> ArtifactSlot {
        self.slot
    }

    pub fn artifact(&self) -> &LinuxArtifactIdentity {
        &self.artifact
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) fn package_file(&self) -> &ArtifactFileIdentity {
        &self.package_file
    }

    #[cfg(any(
        target_os = "linux",
        all(feature = "l6-acceptance-checkpoints", unix),
        all(test, unix)
    ))]
    pub(crate) fn evidence_file(&self) -> &ArtifactFileIdentity {
        &self.evidence_file
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinuxInstallReceipt {
    receipt_format: String,
    product_id: String,
    distribution_identity: String,
    operation_chain: Vec<String>,
    operation_id: String,
    operation_kind: LinuxOperationKind,
    version_relation: ArtifactVersionRelation,
    root_identity: LinuxInstallRootIdentity,
    source_artifact: Option<LinuxArtifactIdentity>,
    target_artifact: Option<LinuxArtifactIdentity>,
    initial_package: PackageSnapshot,
    state: LinuxInstallState,
    failure_code: Option<LinuxFailureCode>,
    failure_after_state: Option<LinuxInstallState>,
    manual_recovery_required: bool,
    staged_artifacts: Vec<StagedArtifactEvidence>,
    target_proof: Option<PackageSnapshot>,
    source_proof: Option<PackageSnapshot>,
}

impl LinuxInstallReceipt {
    pub(crate) fn from_request(
        request: LinuxOperationRequest,
        operation_chain: Vec<String>,
        root_identity: LinuxInstallRootIdentity,
        initial_package: PackageSnapshot,
    ) -> Result<Self, LinuxInstallReceiptError> {
        let receipt = Self {
            receipt_format: LINUX_INSTALL_RECEIPT_FORMAT.to_owned(),
            product_id: LINUX_INSTALL_PRODUCT_ID.to_owned(),
            distribution_identity: LINUX_DISTRIBUTION_IDENTITY.to_owned(),
            operation_chain,
            operation_id: request.operation_id,
            operation_kind: request.operation_kind,
            version_relation: request.relation,
            root_identity,
            source_artifact: request.source,
            target_artifact: request.target,
            initial_package,
            state: LinuxInstallState::Prepared,
            failure_code: None,
            failure_after_state: None,
            manual_recovery_required: false,
            staged_artifacts: Vec::new(),
            target_proof: None,
            source_proof: None,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn operation_chain(&self) -> &[String] {
        &self.operation_chain
    }

    pub const fn operation_kind(&self) -> LinuxOperationKind {
        self.operation_kind
    }

    pub const fn version_relation(&self) -> ArtifactVersionRelation {
        self.version_relation
    }

    pub const fn state(&self) -> LinuxInstallState {
        self.state
    }

    pub fn root_identity(&self) -> &LinuxInstallRootIdentity {
        &self.root_identity
    }

    pub fn source_artifact(&self) -> Option<&LinuxArtifactIdentity> {
        self.source_artifact.as_ref()
    }

    pub fn target_artifact(&self) -> Option<&LinuxArtifactIdentity> {
        self.target_artifact.as_ref()
    }

    pub fn initial_package(&self) -> &PackageSnapshot {
        &self.initial_package
    }

    pub const fn failure_code(&self) -> Option<LinuxFailureCode> {
        self.failure_code
    }

    pub const fn manual_recovery_required(&self) -> bool {
        self.manual_recovery_required
    }

    pub fn target_proof(&self) -> Option<&PackageSnapshot> {
        self.target_proof.as_ref()
    }

    pub fn source_proof(&self) -> Option<&PackageSnapshot> {
        self.source_proof.as_ref()
    }

    pub fn staged_artifact(&self, slot: ArtifactSlot) -> Option<&StagedArtifactEvidence> {
        self.staged_artifacts.iter().find(|item| item.slot == slot)
    }

    pub fn installed_artifact(&self) -> Option<&LinuxArtifactIdentity> {
        match self.state {
            LinuxInstallState::Completed => self.target_artifact.as_ref(),
            LinuxInstallState::AbortedPreserved | LinuxInstallState::RolledBack => {
                self.source_artifact.as_ref()
            }
            _ => None,
        }
    }

    pub fn record_staged_artifact(
        &mut self,
        evidence: StagedArtifactEvidence,
    ) -> Result<(), LinuxInstallReceiptError> {
        if self.state != LinuxInstallState::Prepared {
            return Err(LinuxInstallReceiptError::invalid(
                "staged_artifacts",
                "artifacts can only be recorded while prepared",
            ));
        }
        if self
            .staged_artifacts
            .iter()
            .any(|existing| existing.slot == evidence.slot)
        {
            return Err(LinuxInstallReceiptError::invalid(
                "staged_artifacts",
                "artifact slot is already recorded",
            ));
        }
        if !required_slots(self.operation_kind).contains(&evidence.slot) {
            return Err(LinuxInstallReceiptError::invalid(
                "staged_artifacts",
                "artifact slot is not required by this operation",
            ));
        }
        let expected = self.artifact_for_slot(evidence.slot).ok_or_else(|| {
            LinuxInstallReceiptError::invalid(
                "staged_artifacts",
                "artifact slot is not used by this operation",
            )
        })?;
        if expected != &evidence.artifact {
            return Err(LinuxInstallReceiptError::invalid(
                "staged_artifacts",
                "staged artifact differs from operation identity",
            ));
        }
        self.staged_artifacts.push(evidence);
        self.staged_artifacts.sort_by_key(|item| item.slot);
        self.validate()
    }

    pub fn advance(&mut self, next: LinuxInstallState) -> Result<(), LinuxInstallReceiptError> {
        if self.state.is_terminal() || !normal_transition(self.state, next) {
            return Err(LinuxInstallReceiptError::invalid(
                "state",
                "Linux install state transition is not allowed",
            ));
        }
        let previous = self.state;
        self.state = next;
        if let Err(error) = self.validate() {
            self.state = previous;
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn finish_rollback(&mut self) -> Result<(), LinuxInstallReceiptError> {
        if self.state != LinuxInstallState::SourceVerified || self.source_proof.is_none() {
            return Err(LinuxInstallReceiptError::invalid(
                "state",
                "rollback completion requires a verified source package",
            ));
        }
        self.manual_recovery_required = false;
        self.state = LinuxInstallState::RolledBack;
        self.validate()
    }

    pub fn abort_preserved(
        &mut self,
        failure: LinuxFailureCode,
    ) -> Result<(), LinuxInstallReceiptError> {
        if !self.state.is_before_package_mutation() {
            return Err(LinuxInstallReceiptError::invalid(
                "state",
                "package mutation uncertainty requires rollback",
            ));
        }
        self.failure_after_state = Some(self.state);
        self.failure_code = Some(failure);
        self.manual_recovery_required = false;
        self.state = LinuxInstallState::AbortedPreserved;
        self.validate()
    }

    pub fn require_rollback(
        &mut self,
        failure: LinuxFailureCode,
    ) -> Result<(), LinuxInstallReceiptError> {
        if !matches!(
            self.state,
            LinuxInstallState::PackageMutating | LinuxInstallState::PackageVerified
        ) {
            return Err(LinuxInstallReceiptError::invalid(
                "state",
                "rollback requires package mutation uncertainty",
            ));
        }
        self.failure_after_state = Some(self.state);
        self.failure_code = Some(failure);
        self.manual_recovery_required = true;
        self.state = LinuxInstallState::RollbackRequired;
        self.validate()
    }

    pub(crate) fn record_target_proof(
        &mut self,
        snapshot: PackageSnapshot,
    ) -> Result<(), LinuxInstallReceiptError> {
        if self.state != LinuxInstallState::PackageMutating
            || self.target_proof.is_some()
            || !self.matches_target(&snapshot)
        {
            return Err(LinuxInstallReceiptError::invalid(
                "target_proof",
                "target package proof is inconsistent",
            ));
        }
        self.target_proof = Some(snapshot);
        self.validate()
    }

    pub(crate) fn record_source_proof(
        &mut self,
        snapshot: PackageSnapshot,
    ) -> Result<(), LinuxInstallReceiptError> {
        if self.state != LinuxInstallState::SourceRestoring
            || self.source_proof.is_some()
            || !self.matches_source(&snapshot)
        {
            return Err(LinuxInstallReceiptError::invalid(
                "source_proof",
                "restored source package proof is inconsistent",
            ));
        }
        self.source_proof = Some(snapshot);
        self.validate()
    }

    pub fn encode(&self) -> Result<Vec<u8>, LinuxInstallReceiptError> {
        self.validate()?;
        let mut value = serde_json::to_vec(self)
            .map_err(|_| LinuxInstallReceiptError::invalid("receipt", "cannot encode receipt"))?;
        value.push(b'\n');
        if value.len() > MAX_LINUX_INSTALL_RECEIPT_BYTES {
            return Err(LinuxInstallReceiptError::invalid(
                "receipt",
                "receipt exceeds size limit",
            ));
        }
        Ok(value)
    }

    pub fn decode(value: &[u8]) -> Result<Self, LinuxInstallReceiptError> {
        if value.is_empty()
            || value.len() > MAX_LINUX_INSTALL_RECEIPT_BYTES
            || !value.ends_with(b"\n")
            || value[..value.len() - 1].contains(&b'\n')
        {
            return Err(LinuxInstallReceiptError::invalid(
                "receipt",
                "receipt bytes are not canonical",
            ));
        }
        let receipt: Self = serde_json::from_slice(&value[..value.len() - 1])
            .map_err(|_| LinuxInstallReceiptError::invalid("receipt", "receipt JSON is invalid"))?;
        receipt.validate()?;
        if receipt.encode()? != value {
            return Err(LinuxInstallReceiptError::invalid(
                "receipt",
                "receipt JSON is not canonical",
            ));
        }
        Ok(receipt)
    }

    pub fn can_replace(&self, current: &Self) -> bool {
        if self.validate().is_err() || current.validate().is_err() {
            return false;
        }
        if self.operation_id != current.operation_id {
            return current.state.is_terminal()
                && self.state == LinuxInstallState::Prepared
                && self.root_identity == current.root_identity
                && self.operation_chain.len() == current.operation_chain.len() + 1
                && self.operation_chain.starts_with(&current.operation_chain)
                && self.operation_chain.last() == Some(&self.operation_id)
                && self.source_artifact.as_ref() == current.installed_artifact()
                && self.staged_artifacts.is_empty()
                && self.failure_code.is_none();
        }
        self.static_fields_equal(current)
            && staged_is_append_only(&current.staged_artifacts, &self.staged_artifacts)
            && proof_is_append_only(&current.target_proof, &self.target_proof)
            && proof_is_append_only(&current.source_proof, &self.source_proof)
            && replacement_transition(current, self)
    }

    pub(crate) fn matches_target(&self, snapshot: &PackageSnapshot) -> bool {
        match self.operation_kind {
            LinuxOperationKind::Remove => snapshot.matches_absent(),
            _ => self
                .target_artifact
                .as_ref()
                .is_some_and(|target| snapshot.matches_installed(target)),
        }
    }

    pub(crate) fn matches_source(&self, snapshot: &PackageSnapshot) -> bool {
        self.source_artifact.as_ref().map_or_else(
            || snapshot.matches_absent(),
            |source| snapshot.matches_installed(source),
        )
    }

    pub(crate) fn source_snapshot_is_mutation_precondition(
        &self,
        snapshot: &PackageSnapshot,
    ) -> bool {
        match self.operation_kind {
            LinuxOperationKind::Install => snapshot.matches_absent(),
            LinuxOperationKind::Repair => self.source_artifact.as_ref().is_some_and(|source| {
                snapshot.state.is_recoverable() && snapshot.artifact.as_ref() == Some(source)
            }),
            LinuxOperationKind::Upgrade
            | LinuxOperationKind::Remove
            | LinuxOperationKind::Rollback => self
                .source_artifact
                .as_ref()
                .is_some_and(|source| snapshot.matches_installed(source)),
        }
    }

    pub(crate) fn restore_staged_artifact(&self) -> Option<&StagedArtifactEvidence> {
        match self.operation_kind {
            LinuxOperationKind::Install => None,
            LinuxOperationKind::Repair => self.staged_artifact(ArtifactSlot::Target),
            LinuxOperationKind::Upgrade
            | LinuxOperationKind::Remove
            | LinuxOperationKind::Rollback => self.staged_artifact(ArtifactSlot::Source),
        }
    }

    pub(crate) fn artifact_for_slot(&self, slot: ArtifactSlot) -> Option<&LinuxArtifactIdentity> {
        match slot {
            ArtifactSlot::Source => self.source_artifact.as_ref(),
            ArtifactSlot::Target => self.target_artifact.as_ref(),
        }
    }

    pub(crate) fn required_slots(&self) -> &'static [ArtifactSlot] {
        required_slots(self.operation_kind)
    }

    fn static_fields_equal(&self, other: &Self) -> bool {
        self.receipt_format == other.receipt_format
            && self.product_id == other.product_id
            && self.distribution_identity == other.distribution_identity
            && self.operation_chain == other.operation_chain
            && self.operation_kind == other.operation_kind
            && self.version_relation == other.version_relation
            && self.root_identity == other.root_identity
            && self.source_artifact == other.source_artifact
            && self.target_artifact == other.target_artifact
            && self.initial_package == other.initial_package
    }

    fn validate(&self) -> Result<(), LinuxInstallReceiptError> {
        if self.receipt_format != LINUX_INSTALL_RECEIPT_FORMAT
            || self.product_id != LINUX_INSTALL_PRODUCT_ID
            || self.distribution_identity != LINUX_DISTRIBUTION_IDENTITY
        {
            return Err(LinuxInstallReceiptError::invalid(
                "receipt_identity",
                "unsupported Linux install receipt identity",
            ));
        }
        validate_operation_chain(&self.operation_chain, &self.operation_id)?;
        self.root_identity.validate()?;
        self.initial_package.validate()?;
        LinuxOperationRequest {
            operation_id: self.operation_id.clone(),
            operation_kind: self.operation_kind,
            relation: self.version_relation,
            source: self.source_artifact.clone(),
            target: self.target_artifact.clone(),
        }
        .validate_products()?;
        self.validate_initial_package()?;
        self.validate_staged_artifacts()?;
        self.validate_failure()?;
        self.validate_proofs()
    }

    fn validate_initial_package(&self) -> Result<(), LinuxInstallReceiptError> {
        if self.initial_package.state == DpkgPackageState::Unknown {
            return Err(LinuxInstallReceiptError::invalid(
                "initial_package",
                "unknown dpkg state cannot start an operation",
            ));
        }
        let valid = match self.operation_kind {
            LinuxOperationKind::Install => self.initial_package.matches_absent(),
            LinuxOperationKind::Repair => self.source_artifact.as_ref().is_some_and(|source| {
                self.initial_package.state.is_recoverable()
                    && self.initial_package.artifact.as_ref() == Some(source)
            }),
            LinuxOperationKind::Upgrade
            | LinuxOperationKind::Remove
            | LinuxOperationKind::Rollback => self
                .source_artifact
                .as_ref()
                .is_some_and(|source| self.initial_package.matches_installed(source)),
        };
        if !valid {
            return Err(LinuxInstallReceiptError::invalid(
                "initial_package",
                "initial dpkg state does not match the operation source",
            ));
        }
        Ok(())
    }

    fn validate_staged_artifacts(&self) -> Result<(), LinuxInstallReceiptError> {
        let mut slots = BTreeSet::new();
        for evidence in &self.staged_artifacts {
            if !required_slots(self.operation_kind).contains(&evidence.slot)
                || !slots.insert(evidence.slot)
                || self.artifact_for_slot(evidence.slot) != Some(&evidence.artifact)
            {
                return Err(LinuxInstallReceiptError::invalid(
                    "staged_artifacts",
                    "staged artifact inventory is inconsistent",
                ));
            }
            evidence.package_file.validate()?;
            evidence.evidence_file.validate()?;
        }
        let required = required_slots(self.operation_kind);
        let staged_complete = required.iter().all(|slot| slots.contains(slot));
        if self.state != LinuxInstallState::Prepared && !staged_complete {
            return Err(LinuxInstallReceiptError::invalid(
                "staged_artifacts",
                "operation state requires every artifact to be staged",
            ));
        }
        Ok(())
    }

    fn validate_failure(&self) -> Result<(), LinuxInstallReceiptError> {
        match self.state {
            LinuxInstallState::AbortedPreserved => {
                if self.failure_code.is_none()
                    || !self
                        .failure_after_state
                        .is_some_and(LinuxInstallState::is_before_package_mutation)
                    || self.manual_recovery_required
                {
                    return Err(LinuxInstallReceiptError::invalid(
                        "failure",
                        "aborted receipt failure evidence is invalid",
                    ));
                }
            }
            LinuxInstallState::RollbackRequired
            | LinuxInstallState::SourceRestoring
            | LinuxInstallState::SourceVerified => {
                if self.failure_code.is_none()
                    || !matches!(
                        self.failure_after_state,
                        Some(
                            LinuxInstallState::PackageMutating | LinuxInstallState::PackageVerified
                        )
                    )
                    || !self.manual_recovery_required
                {
                    return Err(LinuxInstallReceiptError::invalid(
                        "failure",
                        "rollback failure evidence is invalid",
                    ));
                }
            }
            LinuxInstallState::RolledBack => {
                if self.failure_code.is_none()
                    || self.failure_after_state.is_none()
                    || self.manual_recovery_required
                {
                    return Err(LinuxInstallReceiptError::invalid(
                        "failure",
                        "rolled-back failure evidence is invalid",
                    ));
                }
            }
            _ => {
                if self.failure_code.is_some()
                    || self.failure_after_state.is_some()
                    || self.manual_recovery_required
                {
                    return Err(LinuxInstallReceiptError::invalid(
                        "failure",
                        "normal state cannot carry failure evidence",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_proofs(&self) -> Result<(), LinuxInstallReceiptError> {
        if let Some(proof) = &self.target_proof {
            proof.validate()?;
            if !self.matches_target(proof) {
                return Err(LinuxInstallReceiptError::invalid(
                    "target_proof",
                    "target proof differs from expected package state",
                ));
            }
        }
        if let Some(proof) = &self.source_proof {
            proof.validate()?;
            if !self.matches_source(proof) {
                return Err(LinuxInstallReceiptError::invalid(
                    "source_proof",
                    "source proof differs from expected package state",
                ));
            }
        }
        if matches!(
            self.state,
            LinuxInstallState::PackageVerified | LinuxInstallState::Completed
        ) && self.target_proof.is_none()
        {
            return Err(LinuxInstallReceiptError::invalid(
                "target_proof",
                "verified package state requires target proof",
            ));
        }
        if matches!(
            self.state,
            LinuxInstallState::SourceVerified | LinuxInstallState::RolledBack
        ) && self.source_proof.is_none()
        {
            return Err(LinuxInstallReceiptError::invalid(
                "source_proof",
                "restored package state requires source proof",
            ));
        }
        Ok(())
    }
}

impl LinuxOperationRequest {
    fn validate_products(&self) -> Result<(), LinuxInstallReceiptError> {
        if let Some(source) = &self.source {
            source.validate()?;
        }
        if let Some(target) = &self.target {
            target.validate()?;
        }
        let valid = match (
            self.operation_kind,
            self.relation,
            self.source.as_ref(),
            self.target.as_ref(),
        ) {
            (
                LinuxOperationKind::Install,
                ArtifactVersionRelation::NotApplicable,
                None,
                Some(_),
            ) => true,
            (
                LinuxOperationKind::Upgrade,
                ArtifactVersionRelation::TargetNewer,
                Some(source),
                Some(target),
            ) => source != target && source.compatible_with(target),
            (
                LinuxOperationKind::Repair,
                ArtifactVersionRelation::SameRelease,
                Some(source),
                Some(target),
            ) => source == target,
            (LinuxOperationKind::Remove, ArtifactVersionRelation::NotApplicable, Some(_), None) => {
                true
            }
            (
                LinuxOperationKind::Rollback,
                ArtifactVersionRelation::TargetOlder,
                Some(source),
                Some(target),
            ) => source != target && source.compatible_with(target),
            _ => false,
        };
        if !valid {
            return Err(LinuxInstallReceiptError::invalid(
                "operation_products",
                "source, target, and version relation do not match operation kind",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxInstallReceiptError {
    field: &'static str,
    message: &'static str,
}

impl LinuxInstallReceiptError {
    const fn invalid(field: &'static str, message: &'static str) -> Self {
        Self { field, message }
    }

    pub const fn field(&self) -> &'static str {
        self.field
    }
}

impl fmt::Display for LinuxInstallReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for LinuxInstallReceiptError {}

fn required_slots(kind: LinuxOperationKind) -> &'static [ArtifactSlot] {
    match kind {
        LinuxOperationKind::Install | LinuxOperationKind::Repair => &[ArtifactSlot::Target],
        LinuxOperationKind::Upgrade | LinuxOperationKind::Rollback => {
            &[ArtifactSlot::Source, ArtifactSlot::Target]
        }
        LinuxOperationKind::Remove => &[ArtifactSlot::Source],
    }
}

fn normal_transition(current: LinuxInstallState, next: LinuxInstallState) -> bool {
    matches!(
        (current, next),
        (
            LinuxInstallState::Prepared,
            LinuxInstallState::ArtifactsStaged
        ) | (
            LinuxInstallState::ArtifactsStaged,
            LinuxInstallState::Quiesced
        ) | (
            LinuxInstallState::Quiesced,
            LinuxInstallState::PackageMutating
        ) | (
            LinuxInstallState::PackageMutating,
            LinuxInstallState::PackageVerified
        ) | (
            LinuxInstallState::PackageVerified,
            LinuxInstallState::Completed
        ) | (
            LinuxInstallState::RollbackRequired,
            LinuxInstallState::SourceRestoring
        ) | (
            LinuxInstallState::SourceRestoring,
            LinuxInstallState::SourceVerified
        ) | (
            LinuxInstallState::SourceVerified,
            LinuxInstallState::RolledBack
        )
    )
}

fn replacement_transition(current: &LinuxInstallReceipt, next: &LinuxInstallReceipt) -> bool {
    if current.state == next.state {
        return current.failure_code == next.failure_code
            && current.failure_after_state == next.failure_after_state
            && current.manual_recovery_required == next.manual_recovery_required
            && (next.staged_artifacts.len() > current.staged_artifacts.len()
                || current.target_proof != next.target_proof
                || current.source_proof != next.source_proof);
    }
    if normal_transition(current.state, next.state) {
        return current.failure_code == next.failure_code
            && current.failure_after_state == next.failure_after_state;
    }
    match next.state {
        LinuxInstallState::AbortedPreserved => current.state.is_before_package_mutation(),
        LinuxInstallState::RollbackRequired => matches!(
            current.state,
            LinuxInstallState::PackageMutating | LinuxInstallState::PackageVerified
        ),
        _ => false,
    }
}

fn staged_is_append_only(
    current: &[StagedArtifactEvidence],
    next: &[StagedArtifactEvidence],
) -> bool {
    current
        .iter()
        .all(|item| next.iter().any(|candidate| candidate == item))
        && next.len() >= current.len()
}

fn proof_is_append_only(current: &Option<PackageSnapshot>, next: &Option<PackageSnapshot>) -> bool {
    current.is_none() || current == next
}

fn validate_operation_chain(
    chain: &[String],
    current: &str,
) -> Result<(), LinuxInstallReceiptError> {
    if chain.is_empty()
        || chain.len() > MAX_OPERATION_CHAIN_LENGTH
        || chain.last().map(String::as_str) != Some(current)
    {
        return Err(LinuxInstallReceiptError::invalid(
            "operation_chain",
            "operation chain is invalid",
        ));
    }
    let mut unique = BTreeSet::new();
    for operation_id in chain {
        validate_operation_id(operation_id, "operation_chain")?;
        if !unique.insert(operation_id) {
            return Err(LinuxInstallReceiptError::invalid(
                "operation_chain",
                "operation chain contains a duplicate",
            ));
        }
    }
    Ok(())
}

fn validate_operation_id(value: &str, field: &'static str) -> Result<(), LinuxInstallReceiptError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(LinuxInstallReceiptError::invalid(
            field,
            "operation ID must be 32 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &'static str) -> Result<(), LinuxInstallReceiptError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(LinuxInstallReceiptError::invalid(
            field,
            "SHA-256 must use 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_debian_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b':' | b'~' | b'_' | b'-')
        })
}
