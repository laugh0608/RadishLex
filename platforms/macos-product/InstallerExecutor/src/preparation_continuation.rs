//! Fresh, guarded admission to a handed-off upgrade. Admission reaffirms
//! durability but does not advance v1 state or authorize a later unlocked call.
use std::time::Duration;

use radishlex_ime_product_install::{
    InstallProcessGuard, InstallReceipt, InstallReceiptStore, InstallState,
};
use radishlex_ime_product_upgrade::{
    PreparationFileIdentity, PreparationJournalStore, PreparationPhase, PreparationReceipt,
    SourcePreparationCheckpoint, SourcePreparationPort, UpgradeProcessGuard, UpgradeReceipt,
    UpgradeReceiptStore, UpgradeState, VerifiedDataRoot,
};
use radishlex_macos_installer_driver::InstallerUserAuthorization;
use radishlex_macos_product_install::MacOsProductInstallAdapter;
use radishlex_macos_upgrade_coordinator::{MacOsPreparationHasher, MacOsProductPreflightAdapter};

use crate::cancellation_authorization::CancellationTaskGrant;
use crate::preparation_execution::outer;
use crate::{
    PreparationExecutionError as Error, PreparationTaskAuthorizationError as AuthError,
    PreparationTaskControl,
};

type Result<T> = std::result::Result<T, Error>;

/// Consumed by admission. A recorded intent or a previous admission cannot
/// recreate this grant. The task owner retains the revocation control.
pub struct PreparedContinuationAuthorization {
    outer: InstallReceipt,
    preparation: PreparationReceipt,
    grant: CancellationTaskGrant,
}

pub fn authorize_prepared_continuation(
    current: &InstallReceipt,
    preparation: &PreparationReceipt,
    confirmation: InstallerUserAuthorization,
    duration: Duration,
) -> std::result::Result<(PreparedContinuationAuthorization, PreparationTaskControl), AuthError> {
    let binding = preparation.binding();
    let root = current.root_identity();
    let matches = |source: bool| {
        let (product, release, digest) = if source {
            (
                current.source_product(),
                &binding.source_release,
                &binding.source_product_sha256,
            )
        } else {
            (
                current.target_product(),
                &binding.target_release,
                &binding.target_product_sha256,
            )
        };
        product.is_some_and(|product| {
            product.release().product_version() == release.product_version()
                && product.release().build_number() == release.build_number()
                && product.product_manifest_sha256() == digest
        })
    };
    if current.encode().is_err()
        || preparation.encode().is_err()
        || current.operation_kind() != radishlex_ime_product_install::InstallOperationKind::Upgrade
        || current.state() != InstallState::Prepared
        || current.manual_recovery_required()
        || !current.artifacts().is_empty()
        || preparation.phase() != PreparationPhase::HandoffReady
        || preparation.previous_install_evidence().is_none()
        || current.operation_id() != binding.operation_id
        || current.previous_operation_id() != binding.previous_install_operation_id.as_deref()
        || (root.device_id(), root.inode(), root.owner_id(), root.mode())
            != (
                binding.data_root.device_id,
                binding.data_root.inode,
                binding.data_root.owner_id,
                binding.data_root.mode,
            )
        || !matches(true)
        || !matches(false)
    {
        return Err(AuthError::InvalidBinding);
    }
    let (grant, control) = CancellationTaskGrant::new(confirmation, duration)?;
    Ok((
        PreparedContinuationAuthorization {
            outer: current.clone(),
            preparation: preparation.clone(),
            grant,
        },
        control,
    ))
}

/// Both locks remain held until this task is dropped. Stores, guards and the
/// grant are deliberately not exported as a reusable execution capability.
/// Subsequent execution must be integrated here with checks at every mutation.
pub struct PreparedContinuationTask<'a> {
    install: &'a InstallReceiptStore,
    outer_guard: InstallProcessGuard,
    data: UpgradeReceiptStore,
    inner_guard: UpgradeProcessGuard,
    receipt: UpgradeReceipt,
    authorization: PreparedContinuationAuthorization,
    outer_identity: PreparationFileIdentity,
    proof_identity: PreparationFileIdentity,
}

impl PreparedContinuationTask<'_> {
    pub fn install_receipt(&self) -> &InstallReceipt {
        &self.authorization.outer
    }
    pub fn data_receipt(&self) -> &UpgradeReceipt {
        &self.receipt
    }

    /// Repeats native observations and durability checks under the same locks.
    /// This is required again after any pause; expiry requires a new task.
    pub fn revalidate(
        &self,
        programs: &MacOsProductInstallAdapter,
        preflight: &mut MacOsProductPreflightAdapter,
    ) -> Result<()> {
        self.revalidate_with(&mut |_| self.observe(programs, preflight))
    }

    fn observe(
        &self,
        programs: &MacOsProductInstallAdapter,
        preflight: &mut MacOsProductPreflightAdapter,
    ) -> Result<()> {
        let root = &self.authorization.preparation.binding().data_root;
        let source = self
            .authorization
            .outer
            .source_product()
            .ok_or(Error::InvalidBinding)?;
        let target = self
            .authorization
            .outer
            .target_product()
            .ok_or(Error::InvalidBinding)?;
        let verify = || {
            crate::cancellation_observation::verify_cancellation_programs(
                programs, root, source, target,
            )
        };
        if !verify() {
            return Err(Error::ProductObservation);
        }
        preflight
            .inspect_cancellation_quiescence(root, target.product_manifest_sha256())
            .map_err(|_| Error::NativeObservation)?;
        if !verify() {
            return Err(Error::ProductObservation);
        }
        Ok(())
    }

    fn verify(&self) -> Result<()> {
        if !self.authorization.grant.active() {
            return Err(Error::TaskAuthorization);
        }
        if self.install.load_guarded(&self.outer_guard)?.as_ref() != Some(&self.authorization.outer)
            || outer::active_identity(self.install)? != self.outer_identity
            || outer::preparation_proof_identity(self.install, &self.authorization.preparation)?
                != self.proof_identity
        {
            return Err(Error::InvalidBinding);
        }
        crate::preparation_rebinding::verify_outer(
            self.install,
            &self.authorization.preparation,
            &self.authorization.outer,
        )?;
        self.data.verify_current(&self.inner_guard, &self.receipt)?;
        let (_, current) = self.journal()?.rebind_handed_off_upgrade(
            &self.inner_guard,
            &self.authorization.preparation,
            &MacOsPreparationHasher,
        )?;
        if current != self.receipt || current.state() != UpgradeState::Preflighted {
            return Err(Error::InvalidPhase);
        }
        if !self.authorization.grant.active() {
            return Err(Error::TaskAuthorization);
        }
        if outer::preparation_proof_identity(self.install, &self.authorization.preparation)?
            != self.proof_identity
        {
            return Err(Error::InvalidBinding);
        }
        Ok(())
    }

    fn journal(&self) -> Result<PreparationJournalStore> {
        Ok(PreparationJournalStore::open_existing(
            VerifiedDataRoot::verify(
                self.install.data_root_path(),
                self.install.root_identity().owner_id(),
            )?,
        )?)
    }

    fn checkpoint(
        &self,
        observer: &mut impl FnMut(Checkpoint) -> Result<()>,
        point: Checkpoint,
    ) -> Result<()> {
        self.verify()?;
        observer(point)?;
        self.verify()
    }

    fn revalidate_with(&self, observer: &mut impl FnMut(Checkpoint) -> Result<()>) -> Result<()> {
        self.checkpoint(observer, Checkpoint::Admission)?;
        outer::sync_preparation_history(self.install, &self.authorization.preparation, || {
            self.checkpoint(observer, Checkpoint::OuterHistory)
        })?;
        self.checkpoint(observer, Checkpoint::BeforeOuterSync)?;
        outer::sync_active(self.install, &self.authorization.outer)?;
        self.checkpoint(observer, Checkpoint::OuterSynced)?;
        let mut authority = Authority {
            task: self,
            observer,
            failure: None,
        };
        // The completed handoff is replayed using its exact recorded moves.
        // This re-syncs receipt, protection snapshot, proof and BOTH parents,
        // and repeats SQLite source/snapshot equivalence under fresh authority.
        let result = self.journal()?.handoff_userdb_source(
            &self.inner_guard,
            self.receipt.operation_id(),
            &mut authority,
            &MacOsPreparationHasher,
        );
        let (_, receipt) =
            result.map_err(|error| authority.failure.unwrap_or(Error::Preparation(error)))?;
        if receipt != self.receipt {
            return Err(Error::InvalidBinding);
        }
        self.checkpoint(observer, Checkpoint::Ready)
    }
}

pub fn admit_prepared_continuation<'a>(
    authorization: PreparedContinuationAuthorization,
    install: &'a InstallReceiptStore,
    programs: &MacOsProductInstallAdapter,
    preflight: &mut MacOsProductPreflightAdapter,
) -> Result<PreparedContinuationTask<'a>> {
    let task = bind(authorization, install)?;
    task.revalidate(programs, preflight)?;
    Ok(task)
}

fn bind(
    authorization: PreparedContinuationAuthorization,
    install: &InstallReceiptStore,
) -> Result<PreparedContinuationTask<'_>> {
    if !authorization.grant.active() {
        return Err(Error::TaskAuthorization);
    }
    if authorization.outer.root_identity() != install.root_identity() {
        return Err(Error::InvalidBinding);
    }
    let outer_guard = install.acquire_guard()?;
    let outer_identity = outer::active_identity(install)?;
    let proof_identity = outer::preparation_proof_identity(install, &authorization.preparation)?;
    if install.load_guarded(&outer_guard)?.as_ref() != Some(&authorization.outer) {
        return Err(Error::InvalidBinding);
    }
    let (data, inner_guard, receipt) =
        crate::rebind_prepared_upgrade(install, &outer_guard, &authorization.preparation)?;
    let task = PreparedContinuationTask {
        install,
        outer_guard,
        data,
        inner_guard,
        receipt,
        authorization,
        outer_identity,
        proof_identity,
    };
    task.verify()?;
    Ok(task)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Checkpoint {
    Admission,
    OuterHistory,
    BeforeOuterSync,
    OuterSynced,
    Handoff(SourcePreparationCheckpoint),
    Ready,
}

struct Authority<'a, 'b, O> {
    task: &'a PreparedContinuationTask<'b>,
    observer: &'a mut O,
    failure: Option<Error>,
}
impl<O: FnMut(Checkpoint) -> Result<()>> SourcePreparationPort for Authority<'_, '_, O> {
    fn confirm_authority_and_quiescence(
        &mut self,
        record: &PreparationReceipt,
        point: SourcePreparationCheckpoint,
    ) -> bool {
        self.failure = if record != &self.task.authorization.preparation {
            Some(Error::InvalidBinding)
        } else {
            self.task
                .checkpoint(self.observer, Checkpoint::Handoff(point))
                .err()
        };
        self.failure.is_none()
    }
    fn available_bytes(&mut self) -> Option<u64> {
        None
    }
}

#[cfg(test)]
#[path = "preparation_continuation_tests.rs"]
mod tests;
