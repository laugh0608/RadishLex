//! Explicit cancellation tasks. Preservation never releases the last marker;
//! final release requires a separately confirmed task and an explicit qualifier.
use std::fs;
use std::io::ErrorKind;
use std::time::Duration;

use radishlex_ime_product_install::{
    InstallFilesystemErrorCode, InstallReceiptStore, ProductArtifactIdentity,
};
use radishlex_ime_product_upgrade::{
    inspect_maintenance, CancellationArchiveStore, CancellationReleaseReceipt,
    CancellationReleaseStore, MaintenanceInspection, MaintenancePhase,
    PreparationCancellationStore, PreparationPhase, PreparationReceipt, SourcePreparationError,
    VerifiedDataRoot,
};
use radishlex_macos_installer_driver::InstallerUserAuthorization;
use radishlex_macos_product_install::MacOsProductInstallAdapter;
use radishlex_macos_product_install_coordinator::{
    CancellationArchiveAuthority, CancellationQualificationError, CancellationReleaseAuthority,
    CancellationReleaseAuthorityFailure, CancellationSourceQualificationAuthority,
};
use radishlex_macos_upgrade_coordinator::{MacOsPreparationHasher, MacOsProductPreflightAdapter};

use super::cancellation_authorization::CancellationTaskGrant;
use super::{
    CancellationTaskAuthorization, CancellationTaskAuthorizationError, CancellationTaskControl,
};

#[path = "cancellation_execution_authority.rs"]
mod authority;
use authority::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationExecutionError {
    TaskAuthorization,
    InvalidBinding,
    InvalidPhase,
    InstallFilesystem(InstallFilesystemErrorCode),
    Preparation(SourcePreparationError),
    ProductObservation,
    NativeObservation,
    ReleaseAuthority(CancellationReleaseAuthorityFailure),
}
impl CancellationExecutionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TaskAuthorization => "cancellation_task_authorization",
            Self::InvalidBinding => "cancellation_binding_changed",
            Self::InvalidPhase => "cancellation_phase_rejected",
            Self::InstallFilesystem(_) => "cancellation_install_filesystem",
            Self::Preparation(_) => "cancellation_preparation_rejected",
            Self::ProductObservation => "cancellation_product_observation",
            Self::NativeObservation => "cancellation_native_observation",
            Self::ReleaseAuthority(_) => "cancellation_release_authority",
        }
    }
}
impl std::fmt::Display for CancellationExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for CancellationExecutionError {}
impl From<SourcePreparationError> for CancellationExecutionError {
    fn from(error: SourcePreparationError) -> Self {
        Self::Preparation(error)
    }
}
type Result<T> = std::result::Result<T, CancellationExecutionError>;

/// A bounded task for one immutable preparation and exact program identities.
/// It cannot be converted to final-release authorization or serialized.
pub struct CancellationPreservationAuthorization {
    preparation: PreparationReceipt,
    source: ProductArtifactIdentity,
    target: ProductArtifactIdentity,
    grant: CancellationTaskGrant,
}

pub fn authorize_cancellation_preservation(
    observation: &MaintenanceInspection,
    source: &ProductArtifactIdentity,
    target: &ProductArtifactIdentity,
    confirmation: InstallerUserAuthorization,
    duration: Duration,
) -> std::result::Result<
    (
        CancellationPreservationAuthorization,
        CancellationTaskControl,
    ),
    CancellationTaskAuthorizationError,
> {
    let preparation = observation.preparation();
    if observation.phase() == MaintenancePhase::CancellationReleased
        || preparation.handoff_started()
        || preparation.phase() == PreparationPhase::HandoffReady
        || preparation
            .binding()
            .previous_install_operation_id
            .is_none()
        || !products_match(preparation, source, target)
    {
        return Err(CancellationTaskAuthorizationError::InvalidBinding);
    }
    let (grant, control) = CancellationTaskGrant::new(confirmation, duration)?;
    Ok((
        CancellationPreservationAuthorization {
            preparation: preparation.clone(),
            source: source.clone(),
            target: target.clone(),
            grant,
        },
        control,
    ))
}

/// Owns the outer-before-inner lock sequence, admits or resumes the same
/// cancellation, finishes its bounded source work and publishes its proof.
/// Success means only "preserved, awaiting qualification". No final marker
/// movement, new operation, program switch or cleanup occurs in this entry.
pub fn preserve_preparation_cancellation(
    authorization: CancellationPreservationAuthorization,
    install: &InstallReceiptStore,
    programs: &MacOsProductInstallAdapter,
    preflight: &mut MacOsProductPreflightAdapter,
) -> Result<CancellationReleaseReceipt> {
    preserve_with_observer(
        authorization,
        install,
        &mut NativeObserver {
            programs,
            preflight,
        },
    )
}

fn preserve_with_observer(
    authorization: CancellationPreservationAuthorization,
    install: &InstallReceiptStore,
    observer: &mut impl ExecutionObserver,
) -> Result<CancellationReleaseReceipt> {
    authorization.require_active()?;
    verify_root(install, &authorization.preparation)?;
    let outer_guard = install
        .acquire_guard()
        .map_err(|e| CancellationExecutionError::InstallFilesystem(e.code()))?;
    // Read the exact current preparation before taking the inner lock. Every
    // mutating core entry then rechecks its physical bindings under that lock.
    let current = inspect_maintenance(
        install.data_root_path(),
        install.root_identity().owner_id(),
        &MacOsPreparationHasher,
    )?
    .ok_or(CancellationExecutionError::InvalidPhase)?;
    if current.preparation() != &authorization.preparation
        || current.phase() == MaintenancePhase::CancellationReleased
    {
        return Err(CancellationExecutionError::InvalidBinding);
    }
    authorization.require_active()?;
    let root = || {
        VerifiedDataRoot::verify(install.data_root_path(), install.root_identity().owner_id())
            .map_err(SourcePreparationError::from)
    };
    let archive = CancellationArchiveStore::open_existing(root()?)?;
    let inner_guard = archive.acquire_guard()?;
    let operation = &authorization.preparation.binding().operation_id;
    let proof_path = install
        .data_root_path()
        .join(".radishlex-upgrade-history-v1")
        .join(operation)
        .join("cancellation-release.json");
    let has_proof = present(&proof_path)?;
    let mut authority = PreservationAuthority {
        authorization: &authorization,
        install,
        outer_guard: &outer_guard,
        inner_guard: &inner_guard,
        archive: &archive,
        observer,
        original_outer: None,
        failure: None,
    };
    if !has_proof {
        let record = archive.load_guarded(&inner_guard, &MacOsPreparationHasher)?;
        let binding = if let Some(record) = record {
            if record.request().preparation() != &authorization.preparation {
                return Err(CancellationExecutionError::InvalidBinding);
            }
            record.outer().clone()
        } else {
            let binding = archive.observe_preparation_outer(
                &inner_guard,
                &authorization.preparation,
                &MacOsPreparationHasher,
            )?;
            authority.original_outer = Some(binding.clone());
            authority.verify_admission_binding()?;
            let cancellation = PreparationCancellationStore::open_existing(root()?)?;
            if current.phase() == MaintenancePhase::Preparing {
                let result = cancellation.request_cancellation(
                    &inner_guard,
                    operation,
                    &mut authority,
                    &MacOsPreparationHasher,
                );
                authority.resolve(result)?;
            }
            let result =
                cancellation.finish_source(&inner_guard, &mut authority, &MacOsPreparationHasher);
            authority.resolve(result)?;
            binding
        };
        let mut port = CancellationArchiveAuthority {
            install,
            guard: &outer_guard,
            source: &authorization.source,
            target: &authorization.target,
            product: &mut authority,
        };
        let result = archive.preserve_cancellation(
            &inner_guard,
            &binding,
            &mut port,
            &MacOsPreparationHasher,
        );
        authority.resolve(result)?;
    }
    let release = CancellationReleaseStore::open_existing(root()?)?;
    let mut qualification = PreservationOnly;
    let mut port = CancellationReleaseAuthority::new(
        install,
        &outer_guard,
        &authorization.source,
        &authorization.target,
        &mut authority,
        &mut qualification,
    );
    let result = release.prepare_cancellation_release(
        &inner_guard,
        operation,
        &mut port,
        &MacOsPreparationHasher,
    );
    let failure = port.failure();
    resolve_release(result, failure, authority.failure)
}

/// The caller supplies independent qualification explicitly. There is no
/// default issuer. Persisted intent and the preservation task cannot authorize
/// this final step; all guards, exact proof, products and quiescence are fresh.
pub fn finish_preparation_cancellation<Q: CancellationSourceQualificationAuthority>(
    authorization: CancellationTaskAuthorization,
    install: &InstallReceiptStore,
    programs: &MacOsProductInstallAdapter,
    preflight: &mut MacOsProductPreflightAdapter,
    qualification: &mut Q,
) -> Result<CancellationReleaseReceipt> {
    let target = programs.target_product().clone();
    finish_with_observer(
        authorization,
        install,
        &target,
        &mut NativeObserver {
            programs,
            preflight,
        },
        qualification,
    )
}

fn finish_with_observer(
    authorization: CancellationTaskAuthorization,
    install: &InstallReceiptStore,
    target: &ProductArtifactIdentity,
    observer: &mut impl ExecutionObserver,
    qualification: &mut impl CancellationSourceQualificationAuthority,
) -> Result<CancellationReleaseReceipt> {
    if !authorization.verify(&authorization.proof, &authorization.source) {
        return Err(CancellationExecutionError::TaskAuthorization);
    }
    verify_root(install, authorization.proof.preparation())?;
    if !products_match(
        authorization.proof.preparation(),
        &authorization.source,
        target,
    ) {
        return Err(CancellationExecutionError::InvalidBinding);
    }
    let outer_guard = install
        .acquire_guard()
        .map_err(|e| CancellationExecutionError::InstallFilesystem(e.code()))?;
    let root =
        VerifiedDataRoot::verify(install.data_root_path(), install.root_identity().owner_id())
            .map_err(SourcePreparationError::from)?;
    let release = CancellationReleaseStore::open_existing(root)?;
    let inner_guard = release.acquire_guard()?;
    let mut product = FinalReleaseObserver {
        authorization: &authorization,
        observer,
        failure: None,
    };
    let mut port = CancellationReleaseAuthority::new(
        install,
        &outer_guard,
        &authorization.source,
        target,
        &mut product,
        qualification,
    );
    let result = release.finish_cancellation_release(
        &inner_guard,
        authorization.proof.operation_id(),
        &mut port,
        &MacOsPreparationHasher,
    );
    let failure = port.failure();
    let (_, proof) = resolve_release(result, failure, product.failure)?;
    Ok(proof)
}

impl CancellationPreservationAuthorization {
    fn require_active(&self) -> Result<()> {
        if self.grant.active() {
            Ok(())
        } else {
            Err(CancellationExecutionError::TaskAuthorization)
        }
    }
}
fn verify_root(install: &InstallReceiptStore, preparation: &PreparationReceipt) -> Result<()> {
    let actual = install.root_identity();
    let expected = &preparation.binding().data_root;
    if actual.device_id() != expected.device_id
        || actual.inode() != expected.inode
        || actual.owner_id() != expected.owner_id
        || actual.mode() != expected.mode
    {
        return Err(CancellationExecutionError::InvalidBinding);
    }
    Ok(())
}
fn products_match(
    preparation: &PreparationReceipt,
    source: &ProductArtifactIdentity,
    target: &ProductArtifactIdentity,
) -> bool {
    let binding = preparation.binding();
    source.product_manifest_sha256() == binding.source_product_sha256
        && target.product_manifest_sha256() == binding.target_product_sha256
        && source.release().product_version() == binding.source_release.product_version()
        && source.release().build_number() == binding.source_release.build_number()
        && target.release().product_version() == binding.target_release.product_version()
        && target.release().build_number() == binding.target_release.build_number()
}
fn present(path: &std::path::Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(SourcePreparationError::Io.into()),
    }
}
fn resolve_release<T>(
    result: std::result::Result<T, SourcePreparationError>,
    release: Option<CancellationReleaseAuthorityFailure>,
    observation: Option<CancellationExecutionError>,
) -> Result<T> {
    result.map_err(|error| {
        if error == SourcePreparationError::AuthorityNotProven {
            if let Some(error) = observation {
                return error;
            }
            if let Some(error) = release {
                return CancellationExecutionError::ReleaseAuthority(error);
            }
        }
        CancellationExecutionError::Preparation(error)
    })
}

#[cfg(test)]
#[path = "cancellation_execution_tests.rs"]
mod tests;
