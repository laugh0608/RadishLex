//! Linux Debian package transaction contracts for RadishLex.

#![forbid(unsafe_code)]

#[cfg(any(target_os = "linux", all(test, unix)))]
mod coordinator;
#[cfg(any(target_os = "linux", all(test, unix)))]
mod debian;
#[cfg(any(target_os = "linux", all(test, unix)))]
mod model;
#[cfg(any(target_os = "linux", all(test, unix)))]
mod startup;
#[cfg(any(target_os = "linux", all(test, unix)))]
mod store;
#[cfg(any(target_os = "linux", all(test, unix)))]
mod system;

#[cfg(any(target_os = "linux", all(test, unix)))]
pub use coordinator::{
    prepare_operation, resume_operation, DpkgExpectedProductState, DpkgOperationContext,
    DpkgPortError, DpkgPortErrorCode, DpkgPortPhase, DpkgProductValidationPhase,
    DpkgQuiescencePhase, DpkgRestoreRequest, DpkgStagedOperation, DpkgStagedPackage,
    DpkgTransactionPort, TransactionError, TransactionOutcome,
};
#[cfg(any(target_os = "linux", all(test, unix)))]
pub use debian::{
    compare_debian_versions, project_debian_lifecycle, validate_dpkg_configuration,
    validate_operation_relation, BinaryControlSnapshot, BoundedDiagnostics,
    DebianCommandContractError, DebianCommandInvocation, DebianInvocationPhase,
    DebianLifecycleObservation, DebianLifecycleProjection, DebianRelationshipError,
    DebianRelationshipErrorCode, DebianVersion, DebianVersionComparison, DebianVersionConstraint,
    DebianVersionOperator, DirectDependency, DpkgConfigurationError, DpkgConfigurationErrorKind,
    DpkgCurrentState, DpkgDesiredState, DpkgErrorState, DpkgMultiArch, DpkgPackageRecord,
    DpkgStatusSnapshot, PrivateStagedDeb, ProductDataContract, StagedDebSlot, StandardInputPolicy,
    ValidatedDpkgConfiguration, VerifiedArtifactRelationship, DPKG_ARCHITECTURE, DPKG_PACKAGE,
    DPKG_PROGRAM, DPKG_STATE_ROOT, MAX_DIAGNOSTIC_BYTES, MAX_DPKG_CONFIG_BYTES,
    MAX_DPKG_CONFIG_LINES, MAX_DPKG_CONFIG_LINE_BYTES, NULL_DEVICE,
};
#[cfg(any(target_os = "linux", all(test, unix)))]
pub use model::{
    ArtifactFileIdentity, ArtifactSlot, ArtifactVersionRelation, DataContractIdentity,
    DpkgPackageState, LinuxArtifactIdentity, LinuxFailureCode, LinuxInstallReceipt,
    LinuxInstallReceiptError, LinuxInstallRootIdentity, LinuxInstallState, LinuxOperationKind,
    LinuxOperationRequest, PackageSnapshot, StagedArtifactEvidence, LINUX_DISTRIBUTION_IDENTITY,
    LINUX_INSTALL_PRODUCT_ID, LINUX_INSTALL_RECEIPT_FORMAT, MAX_LINUX_INSTALL_RECEIPT_BYTES,
};
#[cfg(any(target_os = "linux", all(test, unix)))]
pub use startup::{
    inspect_linux_startup, LinuxPackageObservation, LinuxStartupBuildIdentity,
    LinuxStartupComponent, LinuxStartupDecision, LinuxStartupOutcome, LinuxStartupPaths,
    LinuxStartupPort, LinuxStartupPortError, LinuxStartupPortErrorCode, LinuxStartupReason,
    LinuxSystemStartupPort,
};
#[cfg(any(target_os = "linux", all(test, unix)))]
pub use store::{
    LinuxInstallGuard, LinuxInstallStore, LinuxInstallStoreError, LinuxInstallStoreErrorCode,
    StagedArtifactPaths, SYSTEM_GUARD_PATH, SYSTEM_STATE_ROOT,
};
#[cfg(any(target_os = "linux", all(test, unix)))]
pub use system::{
    run_linux_maintenance, DebianCommandExecutor, DebianCommandOutput, DebianCommandTermination,
    DebianExecutionError, DebianExecutionErrorCode, DpkgSystemObserver, LinuxDpkgTransactionPort,
    LinuxMaintenanceArtifactInput, LinuxMaintenanceCommand, LinuxMaintenanceHostError,
    LinuxMaintenanceHostErrorCode, LinuxSystemCommandExecutor, LinuxSystemObservationError,
    LinuxSystemObservationErrorCode, LinuxSystemObserver, LinuxSystemQuiescencePermit,
};

#[cfg(all(test, unix))]
mod tests;
