//! Isolated synthetic preparation/cancellation; no native program is launched.
use super::*;
use radishlex_ime_product_install::{
    InstallFailureCode, InstallOperationKind, InstallReceipt, InstallReceiptStore, ProductRelease,
    ProgramBundleIdentity, ProgramComponent, VerifiedInstallRoot,
};
use radishlex_ime_product_upgrade::*;
use radishlex_macos_upgrade_coordinator::MacOsPreparationHasher;
use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

static NEXT: AtomicU64 = AtomicU64::new(0);
const OLD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NEW: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub(super) struct Fixture {
    path: PathBuf,
    pub(super) proof: CancellationReleaseReceipt,
    pub(super) source: ProductArtifactIdentity,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let path = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-cancel-authorization-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        let owner = fs::metadata(&path).unwrap().uid();
        drop(radishlex_ime_userdb::UserDb::open(path.join("userdb.sqlite3")).unwrap());
        let install =
            InstallReceiptStore::open(VerifiedInstallRoot::verify(&path, owner).unwrap()).unwrap();
        let outer = install.acquire_guard().unwrap();
        let source = product(39, 'a');
        let target = product(41, 'b');
        let mut old = InstallReceipt::new(
            OLD,
            None,
            InstallOperationKind::Upgrade,
            install.root_identity().clone(),
            Some(source.clone()),
            Some(target.clone()),
        )
        .unwrap();
        install.persist(&outer, &old).unwrap();
        old.abort_preserved(InstallFailureCode::SourceArtifactInvalid)
            .unwrap();
        install.persist(&outer, &old).unwrap();
        let history = path.join(".radishlex-install-history-v1");
        let operation = history.join(OLD);
        for directory in [&history, &operation] {
            DirBuilder::new().mode(0o700).create(directory).unwrap();
        }
        let bytes = old.encode().unwrap();
        let receipt = operation.join("receipt.json");
        fs::write(&receipt, &bytes).unwrap();
        fs::set_permissions(receipt, fs::Permissions::from_mode(0o600)).unwrap();
        let root = || VerifiedDataRoot::verify(&path, owner).unwrap();
        let data = UpgradeReceiptStore::open(root()).unwrap();
        let inner = data.acquire_guard().unwrap();
        let journal = PreparationJournalStore::attach(data, &inner).unwrap();
        let digest = MacOsPreparationHasher
            .sha256(&mut &bytes[..])
            .unwrap()
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect();
        let prep = PreparationReceipt::new(
            PreparationBinding {
                operation_id: NEW.into(),
                previous_install_operation_id: Some(OLD.into()),
                previous_data_operation_id: None,
                source_release: radishlex_ime_product_upgrade::ProductRelease::new("26.7.1", 39)
                    .unwrap(),
                target_release: radishlex_ime_product_upgrade::ProductRelease::new("26.7.1", 41)
                    .unwrap(),
                source_product_sha256: source.product_manifest_sha256().into(),
                target_product_sha256: target.product_manifest_sha256().into(),
                previous_install_receipt_sha256: Some(digest),
                previous_data_receipt_sha256: None,
                previous_inventory_sha256: None,
                target_schema_version: 9,
                data_root: journal.data_root_identity(),
                state_directory: journal.state_directory_identity(),
            },
            journal
                .observe_source_family(&inner, &MacOsPreparationHasher)
                .unwrap(),
        )
        .unwrap();
        journal.persist(&inner, None, &prep).unwrap();
        let cancel = PreparationCancellationStore::attach(journal, &inner).unwrap();
        cancel
            .request_cancellation(&inner, NEW, &mut Synthetic, &MacOsPreparationHasher)
            .unwrap();
        cancel
            .finish_source(&inner, &mut Synthetic, &MacOsPreparationHasher)
            .unwrap();
        drop(inner);
        let archive = CancellationArchiveStore::open_existing(root()).unwrap();
        let inner = archive.acquire_guard().unwrap();
        let binding = archive
            .observe_outer(&inner, &MacOsPreparationHasher)
            .unwrap();
        archive
            .preserve_cancellation(&inner, &binding, &mut Synthetic, &MacOsPreparationHasher)
            .unwrap();
        drop(inner);
        let release = CancellationReleaseStore::open_existing(root()).unwrap();
        let inner = release.acquire_guard().unwrap();
        let proof = release
            .prepare_cancellation_release(&inner, NEW, &mut Synthetic, &MacOsPreparationHasher)
            .unwrap();
        drop(inner);
        drop(outer);
        Self {
            path,
            proof,
            source,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}
fn product(build: u64, marker: char) -> ProductArtifactIdentity {
    let bundle = |component, id| {
        ProgramBundleIdentity::new(
            component,
            id,
            marker.to_string().repeat(64),
            marker.to_string().repeat(64),
        )
        .unwrap()
    };
    ProductArtifactIdentity::new(
        "radishlex-macos",
        ProductRelease::new("26.7.1", build).unwrap(),
        marker.to_string().repeat(64),
        bundle(ProgramComponent::Manager, "dev.radishlex.radishlexManager"),
        bundle(
            ProgramComponent::InputMethod,
            "org.radishlex.inputmethod.macos",
        ),
    )
    .unwrap()
}
struct Synthetic;
impl PreparationCancellationPort for Synthetic {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: PreparationCancellationCheckpoint,
    ) -> bool {
        true
    }
}
impl CancellationSourcePort for Synthetic {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: &CancellationSourceReceipt,
        _: CancellationSourceCheckpoint,
    ) -> bool {
        true
    }
    fn available_bytes(&mut self) -> Option<u64> {
        Some(u64::MAX)
    }
}
impl CancellationArchivePort for Synthetic {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &CancellationArchiveReceipt,
        _: CancellationArchiveCheckpoint,
    ) -> bool {
        true
    }
}
impl CancellationReleasePort for Synthetic {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &CancellationReleaseReceipt,
        _: CancellationReleaseCheckpoint,
        requirement: CancellationReleaseRequirement,
    ) -> bool {
        // This fixture never has old-program qualification or releases a marker.
        requirement == CancellationReleaseRequirement::Preserve
    }
}
