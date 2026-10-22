//! A current, revocable task grant combined with a fresh native observation.
//! No serialization, environment bypass, UI action or qualification is provided.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use radishlex_ime_product_install::ProductArtifactIdentity;
use radishlex_ime_product_upgrade::{CancellationReleaseCheckpoint, CancellationReleaseReceipt};
use radishlex_macos_installer_driver::{InstallerAuthorizationError, InstallerUserAuthorization};
use radishlex_macos_upgrade_coordinator::MacOsProductPreflightAdapter;

use crate::CancellationQuiescenceAuthority;

const MAX_TASK_DURATION: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationTaskAuthorizationError {
    UserConfirmation(InstallerAuthorizationError),
    InvalidBinding,
    InvalidDuration,
}

/// Retained by the task owner, independently of the executor. Revoking or
/// dropping it invalidates the grant; renewal requires fresh confirmation.
pub struct CancellationTaskControl {
    active: Arc<AtomicBool>,
}

impl CancellationTaskControl {
    pub fn revoke(&self) {
        self.active.store(false, Ordering::SeqCst);
    }
}

impl Drop for CancellationTaskControl {
    fn drop(&mut self) {
        self.revoke();
    }
}

pub struct CancellationTaskAuthorization {
    pub(super) proof: CancellationReleaseReceipt,
    pub(super) source: ProductArtifactIdentity,
    pub(super) grant: CancellationTaskGrant,
}

/// Shared lifetime rules for preservation and final release. The bound
/// immutable evidence stays in each task's distinct authorization type.
pub(super) struct CancellationTaskGrant {
    active: Arc<AtomicBool>,
    pub(super) expires: Instant,
}

impl CancellationTaskGrant {
    pub(super) fn active(&self) -> bool {
        self.active.load(Ordering::SeqCst) && Instant::now() < self.expires
    }

    pub(super) fn new(
        confirmation: InstallerUserAuthorization,
        duration: Duration,
    ) -> Result<(Self, CancellationTaskControl), CancellationTaskAuthorizationError> {
        validate_confirmation(confirmation)?;
        if duration.is_zero() || duration > MAX_TASK_DURATION {
            return Err(CancellationTaskAuthorizationError::InvalidDuration);
        }
        let active = Arc::new(AtomicBool::new(true));
        Ok((
            Self {
                active: active.clone(),
                expires: Instant::now() + duration,
            },
            CancellationTaskControl { active },
        ))
    }
}

/// Called only for a freshly confirmed cancellation-release task. Persisted
/// user_requested intent alone is insufficient. This does not inspect files or
/// establish qualification; the release coordinator must still hold both guards.
pub fn authorize_cancellation_release(
    proof: &CancellationReleaseReceipt,
    source: &ProductArtifactIdentity,
    confirmation: InstallerUserAuthorization,
    duration: Duration,
) -> Result<
    (CancellationTaskAuthorization, CancellationTaskControl),
    CancellationTaskAuthorizationError,
> {
    use CancellationTaskAuthorizationError as Error;
    validate_confirmation(confirmation)?;
    let binding = proof.preparation().binding();
    if proof.encode().is_err()
        || source.product_manifest_sha256() != binding.source_product_sha256
        || source.release().product_version() != binding.source_release.product_version()
        || source.release().build_number() != binding.source_release.build_number()
    {
        return Err(Error::InvalidBinding);
    }
    let (grant, control) = CancellationTaskGrant::new(confirmation, duration)?;
    Ok((
        CancellationTaskAuthorization {
            proof: proof.clone(),
            source: source.clone(),
            grant,
        },
        control,
    ))
}

fn validate_confirmation(
    confirmation: InstallerUserAuthorization,
) -> Result<(), CancellationTaskAuthorizationError> {
    use CancellationTaskAuthorizationError as Error;
    use InstallerAuthorizationError as UserError;
    if !confirmation.explicit_action_confirmed {
        return Err(Error::UserConfirmation(
            UserError::ExplicitConfirmationRequired,
        ));
    }
    if !confirmation.data_retention_acknowledged {
        return Err(Error::UserConfirmation(
            UserError::DataRetentionAcknowledgementRequired,
        ));
    }
    if !confirmation.neutral_input_source_selected || !confirmation.manager_closed {
        return Err(Error::UserConfirmation(
            UserError::ManualQuiescenceAcknowledgementRequired,
        ));
    }
    Ok(())
}

impl CancellationTaskAuthorization {
    pub(super) fn verify(
        &self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
    ) -> bool {
        self.grant.active() && &self.proof == proof && &self.source == source
    }

    fn observe(
        &self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        inspect: impl FnOnce() -> bool,
    ) -> Result<(), CancellationQuiescenceFailure> {
        if !self.verify(proof, source) {
            return Err(CancellationQuiescenceFailure::TaskAuthorization);
        }
        if !inspect() {
            return Err(CancellationQuiescenceFailure::NativeObservation);
        }
        if !self.verify(proof, source) {
            return Err(CancellationQuiescenceFailure::TaskAuthorization);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationQuiescenceFailure {
    TaskAuthorization,
    NativeObservation,
}

pub struct MacOsCancellationQuiescenceAuthority<'a> {
    authorization: CancellationTaskAuthorization,
    preflight: &'a mut MacOsProductPreflightAdapter,
    failure: Option<CancellationQuiescenceFailure>,
}

impl<'a> MacOsCancellationQuiescenceAuthority<'a> {
    pub fn new(
        authorization: CancellationTaskAuthorization,
        preflight: &'a mut MacOsProductPreflightAdapter,
    ) -> Self {
        Self {
            authorization,
            preflight,
            failure: None,
        }
    }

    pub fn failure(&self) -> Option<CancellationQuiescenceFailure> {
        self.failure
    }
}

impl CancellationQuiescenceAuthority for MacOsCancellationQuiescenceAuthority<'_> {
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        _checkpoint: CancellationReleaseCheckpoint,
    ) -> bool {
        let binding = proof.preparation().binding();
        self.failure = self
            .authorization
            .observe(proof, source, || {
                self.preflight
                    .inspect_cancellation_quiescence(
                        &binding.data_root,
                        &binding.target_product_sha256,
                    )
                    .is_ok()
            })
            .err();
        self.failure.is_none()
    }
}

#[cfg(test)]
#[path = "cancellation_authorization_tests.rs"]
mod tests;
