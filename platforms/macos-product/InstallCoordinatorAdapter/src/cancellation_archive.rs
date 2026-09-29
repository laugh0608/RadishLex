//! Typed outer authority for the cancellation material coordinator. This does
//! not implement installed-bundle observation, Installer actions or release.
use radishlex_ime_product_install::{
    validate_preparation_cancellation_outer, InstallProcessGuard, InstallReceiptStore,
    ProductArtifactIdentity,
};
use radishlex_ime_product_upgrade::{
    CancellationArchiveCheckpoint, CancellationArchivePort, CancellationArchiveReceipt,
};

/// Implementations must freshly inspect exact installed source programs,
/// their quiescence and current task authority on every call. Never use a cached
/// UI flag. Target is the sealed intended product, not a program to launch.
pub trait CancellationProductAuthority {
    fn confirm_source_and_quiescence(
        &mut self,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        checkpoint: CancellationArchiveCheckpoint,
    ) -> bool;
}

/// Keeps the real outer guard across the complete archive/restore call. Acquire
/// it before the upgrade guard. The archive core verifies that inner guard and
/// physical evidence on both sides of every callback; this port adds v1 types,
/// product/root bindings and live outer-guard checks.
pub struct CancellationArchiveAuthority<'a, P> {
    pub install: &'a InstallReceiptStore,
    pub guard: &'a InstallProcessGuard,
    pub source: &'a ProductArtifactIdentity,
    pub target: &'a ProductArtifactIdentity,
    pub product: &'a mut P,
}

impl<P: CancellationProductAuthority> CancellationArchivePort
    for CancellationArchiveAuthority<'_, P>
{
    fn confirm_authority_and_quiescence(
        &mut self,
        record: &CancellationArchiveReceipt,
        checkpoint: CancellationArchiveCheckpoint,
    ) -> bool {
        let verify = || {
            let binding = record.request().preparation().binding();
            let root = self.install.root_identity();
            self.install.verify_guard(self.guard).is_ok()
                && root.device_id() == binding.data_root.device_id
                && root.inode() == binding.data_root.inode
                && root.owner_id() == binding.data_root.owner_id
                && root.mode() == binding.data_root.mode
                && self.source.product_manifest_sha256() == binding.source_product_sha256
                && self.target.product_manifest_sha256() == binding.target_product_sha256
                && self.source.release().product_version()
                    == binding.source_release.product_version()
                && self.source.release().build_number() == binding.source_release.build_number()
                && self.target.release().product_version()
                    == binding.target_release.product_version()
                && self.target.release().build_number() == binding.target_release.build_number()
                && binding
                    .previous_install_operation_id
                    .as_deref()
                    .is_some_and(|previous| {
                        validate_preparation_cancellation_outer(
                            record.outer().previous_bytes(),
                            record.outer().new_bytes(),
                            &binding.operation_id,
                            previous,
                            root,
                            self.source,
                            self.target,
                        )
                        .is_ok()
                    })
        };
        verify()
            && self
                .product
                .confirm_source_and_quiescence(self.source, self.target, checkpoint)
            && verify()
    }
}
