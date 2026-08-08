//! Linux Debian package transaction contracts for RadishLex.

#![forbid(unsafe_code)]

mod coordinator;
mod model;
#[cfg(any(target_os = "linux", all(test, unix)))]
mod startup;
#[cfg(unix)]
mod store;

pub use coordinator::{
    prepare_operation, resume_operation, DpkgPortError, DpkgTransactionPort, TransactionError,
    TransactionOutcome,
};
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
#[cfg(unix)]
pub use store::{
    LinuxInstallGuard, LinuxInstallStore, LinuxInstallStoreError, LinuxInstallStoreErrorCode,
    StagedArtifactPaths, SYSTEM_GUARD_PATH, SYSTEM_STATE_ROOT,
};

#[cfg(test)]
mod tests;
