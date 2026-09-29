#![cfg(all(unix, feature = "qualification-harness"))]
//! Real dual guards/v1 receipts/files with synthetic installed-product authority.
use std::fs::{self, DirBuilder, File};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_product_install::{
    InstallFailureCode, InstallOperationKind, InstallReceipt, InstallReceiptStore,
    ProductArtifactIdentity, ProductRelease, ProgramBundleIdentity, ProgramComponent,
    VerifiedInstallRoot,
};
use radishlex_ime_product_upgrade::{
    CancellationArchiveCheckpoint, CancellationArchiveStore, CancellationSourceCheckpoint,
    CancellationSourcePort, CancellationSourceReceipt, PreparationBinding,
    PreparationCancellationCheckpoint, PreparationCancellationPort, PreparationCancellationRequest,
    PreparationCancellationStore, PreparationHasher, PreparationJournalStore, PreparationReceipt,
    ProductRelease as DataRelease, UpgradeReceiptStore, VerifiedDataRoot,
};
use radishlex_ime_userdb::UserDb;
use radishlex_macos_product_install_coordinator::{
    CancellationArchiveAuthority, CancellationProductAuthority,
};
use radishlex_macos_upgrade_coordinator::MacOsPreparationHasher as Hasher;

const OLD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NEW: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-cancel-dual-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
    fn data(&self) -> VerifiedDataRoot {
        VerifiedDataRoot::verify(&self.0, fs::metadata(&self.0).unwrap().uid()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn put(path: &Path, bytes: &[u8]) {
    let mut file = File::create(path).unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn hash(bytes: &[u8]) -> String {
    Hasher
        .sha256(&mut &bytes[..])
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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
        bundle(ProgramComponent::Manager, "org.radishlex.manager"),
        bundle(ProgramComponent::InputMethod, "org.radishlex.inputmethod"),
    )
    .unwrap()
}
struct Admission;
impl PreparationCancellationPort for Admission {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &PreparationCancellationRequest,
        _: PreparationCancellationCheckpoint,
    ) -> bool {
        true
    }
}
impl CancellationSourcePort for Admission {
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
struct Products {
    calls: usize,
    reject: bool,
}
impl CancellationProductAuthority for Products {
    fn confirm_source_and_quiescence(
        &mut self,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        _: CancellationArchiveCheckpoint,
    ) -> bool {
        assert_eq!(source.release().build_number(), 39);
        assert_eq!(target.release().build_number(), 41);
        self.calls += 1;
        !self.reject
    }
}

#[test]
fn typed_outer_authority_keeps_both_guards_and_preserves_exact_legacy_receipt() {
    for new_outer in [false, true] {
        for reject in [false, true] {
            let fixture = Fixture::new();
            drop(UserDb::open(fixture.0.join("userdb.sqlite3")).unwrap());
            let source = product(39, 'a');
            let target = product(41, 'b');
            let history = fixture.0.join(".radishlex-install-history-v1");
            let old_directory = history.join(OLD);
            for path in [&history, &old_directory] {
                DirBuilder::new().mode(0o700).create(path).unwrap();
            }
            let install = InstallReceiptStore::open(
                VerifiedInstallRoot::verify(&fixture.0, fs::metadata(&fixture.0).unwrap().uid())
                    .unwrap(),
            )
            .unwrap();
            let outer_guard = install.acquire_guard().unwrap();
            let mut old = InstallReceipt::new(
                OLD,
                None,
                InstallOperationKind::Upgrade,
                install.root_identity().clone(),
                Some(source.clone()),
                Some(target.clone()),
            )
            .unwrap();
            install.persist(&outer_guard, &old).unwrap();
            old.abort_preserved(InstallFailureCode::SourceArtifactInvalid)
                .unwrap();
            install.persist(&outer_guard, &old).unwrap();
            let old_bytes = old.encode().unwrap();
            put(&old_directory.join("receipt.json"), &old_bytes);
            let old_inode = fs::metadata(old_directory.join("receipt.json"))
                .unwrap()
                .ino();
            let new = InstallReceipt::new(
                NEW,
                Some(OLD.into()),
                InstallOperationKind::Upgrade,
                install.root_identity().clone(),
                Some(source.clone()),
                Some(target.clone()),
            )
            .unwrap();
            if new_outer {
                install.persist(&outer_guard, &new).unwrap();
            }
            let data = UpgradeReceiptStore::open(fixture.data()).unwrap();
            let inner_guard = data.acquire_guard().unwrap();
            let journal = PreparationJournalStore::attach(data, &inner_guard).unwrap();
            let binding = PreparationBinding {
                operation_id: NEW.into(),
                previous_install_operation_id: Some(OLD.into()),
                previous_data_operation_id: None,
                source_release: DataRelease::new("26.7.1", 39).unwrap(),
                target_release: DataRelease::new("26.7.1", 41).unwrap(),
                source_product_sha256: source.product_manifest_sha256().into(),
                target_product_sha256: target.product_manifest_sha256().into(),
                previous_install_receipt_sha256: Some(hash(&old_bytes)),
                previous_data_receipt_sha256: None,
                previous_inventory_sha256: None,
                target_schema_version: 9,
                data_root: journal.data_root_identity(),
                state_directory: journal.state_directory_identity(),
            };
            let prep = PreparationReceipt::new(
                binding,
                journal
                    .observe_source_family(&inner_guard, &Hasher)
                    .unwrap(),
            )
            .unwrap();
            journal.persist(&inner_guard, None, &prep).unwrap();
            let cancel = PreparationCancellationStore::attach(journal, &inner_guard).unwrap();
            cancel
                .request_cancellation(&inner_guard, NEW, &mut Admission, &Hasher)
                .unwrap();
            cancel
                .finish_source(&inner_guard, &mut Admission, &Hasher)
                .unwrap();
            drop(inner_guard);
            let archive = CancellationArchiveStore::open_existing(fixture.data()).unwrap();
            let inner_guard = archive.acquire_guard().unwrap();
            let outer = archive.observe_outer(&inner_guard, &Hasher).unwrap();
            let mut products = Products { calls: 0, reject };
            let mut authority = CancellationArchiveAuthority {
                install: &install,
                guard: &outer_guard,
                source: &source,
                target: &target,
                product: &mut products,
            };
            let result =
                archive.preserve_cancellation(&inner_guard, &outer, &mut authority, &Hasher);
            if reject {
                assert!(result.is_err());
                assert!(!fixture.0.join(".radishlex-upgrade-history-v1").exists());
            } else {
                assert!(result.unwrap().preserved());
                assert_eq!(
                    install.load_guarded(&outer_guard).unwrap(),
                    Some(old.clone())
                );
                assert_eq!(
                    fs::read(old_directory.join("receipt.json")).unwrap(),
                    old_bytes
                );
                assert_eq!(
                    fs::metadata(old_directory.join("receipt.json"))
                        .unwrap()
                        .ino(),
                    old_inode
                );
                assert!(!old.can_replace(&new));
            }
            assert!(products.calls > 0);
            assert!(fixture
                .0
                .join(".radishlex-upgrade-v1/preparation-cancellation.json")
                .exists());
            drop(inner_guard);
            drop(outer_guard);
            let owner = fs::metadata(&fixture.0).unwrap().uid();
            // The data gate must remain blocked after both live guards vanish;
            // the restored outer alone may recognize the exact legacy source.
            assert!(
                !radishlex_ime_product_upgrade::inspect_startup_gate(&fixture.0, owner)
                    .is_allowed()
            );
            if !reject {
                for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
                    let running = radishlex_ime_product_install::RunningProgramIdentity::new(
                        source.release().clone(),
                        source.program(component).clone(),
                    )
                    .unwrap();
                    assert!(radishlex_ime_product_install::inspect_install_startup_gate(
                        &fixture.0, owner, &running,
                    )
                    .is_allowed());
                }
            }
        }
    }
}
