//! Read-only installed-product observation for the cancellation release bridge.
//! No Installer action or qualification issuer is enabled by this module.
use radishlex_ime_product_install::{InstallRootIdentity, ProductArtifactIdentity};
use radishlex_ime_product_upgrade::{CancellationReleaseCheckpoint, CancellationReleaseReceipt};
use radishlex_macos_product_install::MacOsProductInstallAdapter;
use radishlex_macos_product_install_coordinator::CancellationReleaseProductAuthority;

/// The implementation must freshly bind task authorization and BOTH programs'
/// process quiescence to this proof's root, source and operation. A target-only
/// preflight result, cached UI permission or successful qualification run does
/// not establish this fact. It must not stop/start programs or open user data.
pub trait CancellationQuiescenceAuthority {
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        checkpoint: CancellationReleaseCheckpoint,
    ) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationProductObservationFailure {
    InstalledProduct,
    AuthorityOrQuiescence,
}

/// Uses the existing signature/tree/manifest verifier for both source bundles
/// and the sealed target. Fresh authority and quiescence remain an explicit
/// service; there is deliberately no default or always-true implementation.
pub struct MacOsCancellationProductAuthority<'a, Q> {
    programs: &'a MacOsProductInstallAdapter,
    quiescence: &'a mut Q,
    failure: Option<CancellationProductObservationFailure>,
}

impl<'a, Q> MacOsCancellationProductAuthority<'a, Q> {
    pub fn new(programs: &'a MacOsProductInstallAdapter, quiescence: &'a mut Q) -> Self {
        Self {
            programs,
            quiescence,
            failure: None,
        }
    }

    pub fn failure(&self) -> Option<CancellationProductObservationFailure> {
        self.failure
    }

    fn verify_programs(
        &self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
    ) -> bool {
        let root = &proof.preparation().binding().data_root;
        let Ok(root) =
            InstallRootIdentity::new(root.device_id, root.inode, root.owner_id, root.mode)
        else {
            return false;
        };
        self.programs.verify_data_root(&root).is_ok()
            && self.programs.revalidate_target_product().as_ref() == Ok(target)
            && self.programs.verify_installed_product(Some(source)).is_ok()
            && self.programs.verify_data_root(&root).is_ok()
    }
}

impl<Q: CancellationQuiescenceAuthority> CancellationReleaseProductAuthority
    for MacOsCancellationProductAuthority<'_, Q>
{
    fn confirm_source_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        checkpoint: CancellationReleaseCheckpoint,
    ) -> bool {
        self.failure = None;
        if !self.verify_programs(proof, source, target) {
            self.failure = Some(CancellationProductObservationFailure::InstalledProduct);
            return false;
        }
        if !self
            .quiescence
            .confirm_authority_and_quiescence(proof, source, checkpoint)
        {
            self.failure = Some(CancellationProductObservationFailure::AuthorityOrQuiescence);
            return false;
        }
        if !self.verify_programs(proof, source, target) {
            self.failure = Some(CancellationProductObservationFailure::InstalledProduct);
            return false;
        }
        true
    }
}
