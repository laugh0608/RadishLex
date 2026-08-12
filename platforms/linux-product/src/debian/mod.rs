mod command;
mod relationship;

pub use command::{
    project_debian_lifecycle, validate_dpkg_configuration, BoundedDiagnostics,
    DebianCommandContractError, DebianCommandInvocation, DebianInvocationPhase,
    DebianLifecycleObservation, DebianLifecycleProjection, DebianVersion, DebianVersionComparison,
    DpkgConfigurationError, DpkgConfigurationErrorKind, PrivateStagedDeb, StagedDebSlot,
    StandardInputPolicy, ValidatedDpkgConfiguration, DPKG_ARCHITECTURE, DPKG_PACKAGE, DPKG_PROGRAM,
    DPKG_STATE_ROOT, MAX_DIAGNOSTIC_BYTES, MAX_DPKG_CONFIG_BYTES, MAX_DPKG_CONFIG_LINES,
    MAX_DPKG_CONFIG_LINE_BYTES, NULL_DEVICE,
};
#[cfg(test)]
pub(crate) use relationship::tests::helper::{status_snapshot, ArtifactFixture};
pub(crate) use relationship::validate_radishlex_package_release;
pub use relationship::{
    compare_debian_versions, validate_operation_relation, BinaryControlSnapshot,
    DebianRelationshipError, DebianRelationshipErrorCode, DebianVersionConstraint,
    DebianVersionOperator, DirectDependency, DpkgCurrentState, DpkgDesiredState, DpkgErrorState,
    DpkgMultiArch, DpkgPackageRecord, DpkgStatusSnapshot, ProductDataContract,
    VerifiedArtifactRelationship,
};
#[cfg(test)]
pub(crate) use relationship::{
    ArchiveFixture, ARCHIVE_EVIDENCE_FILENAME, ARCHIVE_PACKAGE_FILENAME,
};
