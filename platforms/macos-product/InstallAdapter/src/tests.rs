use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{symlink, DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use radishlex_ime_product_install::{
    commit_program_target, finish_program_restore, finish_source_preservation,
    finish_target_staging, preserve_program_source, restore_program_source, InstallFailureCode,
    InstallFinalizationPort, InstallFinalizationValidationStage, InstallOperationKind,
    InstallReceipt, InstallState, ProgramComponent, VerifiedInstallRoot, INSTALL_PRODUCT_ID,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::copy::BundleCopier;
use super::*;

const COMMITTED_LAYOUT: &[u8] = include_bytes!("../../../../packaging/macos/install-layout.json");

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    container: PathBuf,
    home: PathBuf,
    payload: PathBuf,
    owner_id: u32,
}

impl Fixture {
    fn new(version: &str, build: &str, marker: &str) -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp directory")
            .join(format!(
                "radishlex-install-adapter-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        create_directory(&container, 0o700);
        let owner_id = fs::metadata(&container).expect("container metadata").uid();
        let home = container.join("home");
        let payload = container.join("payload");
        create_user_layout(&home);
        build_payload(&payload, version, build, marker);
        Self {
            container,
            home,
            payload,
            owner_id,
        }
    }

    fn adapter(
        &self,
        signature_state: Arc<SignatureState>,
        copy_state: Arc<CopyState>,
    ) -> Result<MacOsProductInstallAdapter, MacOsInstallAdapterError> {
        MacOsProductInstallAdapter::load_with_services(
            &self.payload,
            &self.home,
            self.owner_id,
            Box::new(FakeSignatureVerifier {
                state: signature_state,
            }),
            Box::new(FakeBundleCopier { state: copy_state }),
        )
    }

    fn data_root(&self) -> PathBuf {
        self.home.join(DATA_ROOT)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

#[derive(Default)]
struct SignatureState {
    reject: AtomicBool,
    drift: AtomicBool,
}

struct FakeSignatureVerifier {
    state: Arc<SignatureState>,
}

impl MacOsCodeSignatureVerifier for FakeSignatureVerifier {
    fn verify(
        &self,
        _bundle: &Path,
        component: ProgramComponent,
        expected_bundle_id: &str,
    ) -> Result<MacOsCodeIdentity, MacOsInstallAdapterError> {
        if self.state.reject.load(Ordering::SeqCst) {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        let marker = match (component, self.state.drift.load(Ordering::SeqCst)) {
            (ProgramComponent::Manager, false) => 'a',
            (ProgramComponent::InputMethod, false) => 'b',
            (ProgramComponent::Manager, true) => 'c',
            (ProgramComponent::InputMethod, true) => 'd',
        };
        MacOsCodeIdentity::new(expected_bundle_id, marker.to_string().repeat(64))
    }
}

#[derive(Default)]
struct CopyState {
    calls: AtomicUsize,
    fail_after_root: AtomicBool,
}

struct FakeBundleCopier {
    state: Arc<CopyState>,
}

impl BundleCopier for FakeBundleCopier {
    fn copy_bundle(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.state.calls.fetch_add(1, Ordering::SeqCst);
        if self.state.fail_after_root.load(Ordering::SeqCst) {
            create_directory(destination, 0o755);
            return Err(error(MacOsInstallAdapterErrorCode::CopyFailed));
        }
        copy_tree(source, destination)
    }
}

#[test]
fn first_install_binds_payload_staging_and_installed_targets() {
    let fixture = Fixture::new("0.1.0", "35", "target");
    let signature = Arc::new(SignatureState::default());
    let copy = Arc::new(CopyState::default());
    let mut adapter = fixture
        .adapter(signature, copy.clone())
        .expect("install adapter");
    let receipt_store = InstallReceiptStore::open(
        VerifiedInstallRoot::verify(fixture.data_root(), fixture.owner_id)
            .expect("verified data root"),
    )
    .expect("receipt store");
    let guard = receipt_store.acquire_guard().expect("guard");
    let foreign_data_root = fixture.container.join("foreign-data-root");
    create_directory(&foreign_data_root, 0o700);
    let foreign_store = InstallReceiptStore::open(
        VerifiedInstallRoot::verify(&foreign_data_root, fixture.owner_id)
            .expect("verified foreign root"),
    )
    .expect("foreign store");
    let foreign_receipt = InstallReceipt::new(
        "ffeeddccbbaa99887766554433221100",
        None,
        InstallOperationKind::FirstInstall,
        foreign_store.root_identity().clone(),
        None,
        Some(adapter.target_product().clone()),
    )
    .expect("foreign receipt");
    assert_eq!(
        adapter
            .open_program_store(
                &receipt_store,
                &guard,
                ProgramComponent::Manager,
                &foreign_receipt,
            )
            .expect_err("foreign receipt root")
            .code(),
        MacOsInstallAdapterErrorCode::InvalidOperation
    );
    let mut receipt = InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::FirstInstall,
        receipt_store.root_identity().clone(),
        None,
        Some(adapter.target_product().clone()),
    )
    .expect("receipt");
    receipt_store
        .persist(&guard, &receipt)
        .expect("prepared receipt");
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    receipt_store
        .persist(&guard, &receipt)
        .expect("quiesced receipt");
    let manager = adapter
        .open_program_store(&receipt_store, &guard, ProgramComponent::Manager, &receipt)
        .expect("Manager store");
    let input_method = adapter
        .open_program_store(
            &receipt_store,
            &guard,
            ProgramComponent::InputMethod,
            &receipt,
        )
        .expect("InputMethod store");

    adapter
        .prepare_and_record_staged(&receipt_store, &guard, &manager, &mut receipt)
        .expect("stage Manager");
    adapter
        .prepare_and_record_staged(&receipt_store, &guard, &input_method, &mut receipt)
        .expect("stage InputMethod");
    assert_eq!(copy.calls.load(Ordering::SeqCst), 2);
    finish_target_staging(
        &receipt_store,
        &guard,
        &manager,
        &input_method,
        &mut receipt,
    )
    .expect("finish staging");
    commit_program_target(&receipt_store, &guard, &manager, &mut receipt).expect("commit Manager");
    adapter
        .verify_installed_target(&manager, &receipt)
        .expect("verify Manager");
    commit_program_target(&receipt_store, &guard, &input_method, &mut receipt)
        .expect("commit InputMethod");
    adapter
        .verify_installed_target(&input_method, &receipt)
        .expect("verify InputMethod");
    assert!(adapter.validate_final_state(
        &manager,
        &input_method,
        &receipt,
        InstallFinalizationValidationStage::BeforeFinalVerified,
    ));
}

#[test]
fn valid_staging_without_receipt_evidence_is_revalidated_without_copying_again() {
    let fixture = Fixture::new("0.1.0", "35", "target");
    let signature = Arc::new(SignatureState::default());
    let copy = Arc::new(CopyState::default());
    let adapter = fixture
        .adapter(signature, copy.clone())
        .expect("install adapter");
    let receipt_store = InstallReceiptStore::open(
        VerifiedInstallRoot::verify(fixture.data_root(), fixture.owner_id)
            .expect("verified data root"),
    )
    .expect("receipt store");
    let guard = receipt_store.acquire_guard().expect("guard");
    let mut receipt = first_install_receipt(&adapter, &receipt_store);
    receipt_store
        .persist(&guard, &receipt)
        .expect("prepared receipt");
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    receipt_store
        .persist(&guard, &receipt)
        .expect("quiesced receipt");
    let manager = adapter
        .open_program_store(&receipt_store, &guard, ProgramComponent::Manager, &receipt)
        .expect("Manager store");
    copy_tree(
        &fixture
            .payload
            .join("Product/Components/radishlex_manager.app"),
        manager.staged_bundle_path(),
    )
    .expect("synthetic completed copy");

    adapter
        .prepare_and_record_staged(&receipt_store, &guard, &manager, &mut receipt)
        .expect("recover staged evidence");
    assert_eq!(copy.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn upgrade_revalidates_source_target_and_restored_programs() {
    let source_fixture = Fixture::new("0.1.0", "35", "source");
    let target_payload = source_fixture.container.join("target-payload");
    build_payload(&target_payload, "0.2.0", "36", "target");
    let signature = Arc::new(SignatureState::default());
    let copy = Arc::new(CopyState::default());
    let source_adapter = source_fixture
        .adapter(signature.clone(), copy.clone())
        .expect("source adapter");
    let target_adapter = MacOsProductInstallAdapter::load_with_services(
        &target_payload,
        &source_fixture.home,
        source_fixture.owner_id,
        Box::new(FakeSignatureVerifier {
            state: signature.clone(),
        }),
        Box::new(FakeBundleCopier {
            state: copy.clone(),
        }),
    )
    .expect("target adapter");
    copy_tree(
        &source_fixture
            .payload
            .join("Product/Components/radishlex_manager.app"),
        &source_fixture
            .home
            .join("Applications/RadishLex Manager.app"),
    )
    .expect("installed source Manager");
    copy_tree(
        &source_fixture
            .payload
            .join("Product/Components/RadishLexInputMethod.app"),
        &source_fixture
            .home
            .join("Library/Input Methods/RadishLexInputMethod.app"),
    )
    .expect("installed source InputMethod");

    let receipt_store = InstallReceiptStore::open(
        VerifiedInstallRoot::verify(source_fixture.data_root(), source_fixture.owner_id)
            .expect("verified data root"),
    )
    .expect("receipt store");
    let guard = receipt_store.acquire_guard().expect("guard");
    let mut receipt = InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::Upgrade,
        receipt_store.root_identity().clone(),
        Some(source_adapter.target_product().clone()),
        Some(target_adapter.target_product().clone()),
    )
    .expect("upgrade receipt");
    receipt_store
        .persist(&guard, &receipt)
        .expect("prepared receipt");
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    receipt_store
        .persist(&guard, &receipt)
        .expect("quiesced receipt");
    let manager = target_adapter
        .open_program_store(&receipt_store, &guard, ProgramComponent::Manager, &receipt)
        .expect("Manager store");
    let input_method = target_adapter
        .open_program_store(
            &receipt_store,
            &guard,
            ProgramComponent::InputMethod,
            &receipt,
        )
        .expect("InputMethod store");
    for store in [&manager, &input_method] {
        target_adapter
            .verify_and_record_source(&receipt_store, &guard, store, &mut receipt)
            .expect("record source");
        target_adapter
            .prepare_and_record_staged(&receipt_store, &guard, store, &mut receipt)
            .expect("record target staging");
    }
    finish_target_staging(
        &receipt_store,
        &guard,
        &manager,
        &input_method,
        &mut receipt,
    )
    .expect("finish staging");
    for store in [&manager, &input_method] {
        preserve_program_source(&receipt_store, &guard, store, &mut receipt)
            .expect("preserve source");
    }
    finish_source_preservation(
        &receipt_store,
        &guard,
        &manager,
        &input_method,
        &mut receipt,
    )
    .expect("finish preservation");
    commit_program_target(&receipt_store, &guard, &manager, &mut receipt).expect("commit Manager");
    target_adapter
        .verify_installed_target(&manager, &receipt)
        .expect("verify installed Manager");
    commit_program_target(&receipt_store, &guard, &input_method, &mut receipt)
        .expect("commit InputMethod");
    target_adapter
        .verify_installed_target(&input_method, &receipt)
        .expect("verify installed InputMethod");

    receipt
        .require_rollback(InstallFailureCode::DataCoordinationFailed)
        .expect("rollback required");
    receipt_store
        .persist(&guard, &receipt)
        .expect("persist rollback");
    for store in [&manager, &input_method] {
        restore_program_source(&receipt_store, &guard, store, &receipt).expect("restore source");
    }
    finish_program_restore(
        &receipt_store,
        &guard,
        &manager,
        &input_method,
        &mut receipt,
    )
    .expect("finish restore");
    for store in [&manager, &input_method] {
        target_adapter
            .verify_restored_source(store, &receipt)
            .expect("verify restored source");
    }
}

#[test]
fn layout_payload_and_product_mutations_fail_closed() {
    let layout_fixture = Fixture::new("0.1.0", "35", "target");
    fs::write(layout_fixture.payload.join("InstallLayout.json"), b"{}\n").expect("mutate layout");
    assert_eq!(
        layout_fixture
            .adapter(
                Arc::new(SignatureState::default()),
                Arc::new(CopyState::default())
            )
            .expect_err("layout mutation")
            .code(),
        MacOsInstallAdapterErrorCode::InvalidPayloadManifest
    );

    let payload_fixture = Fixture::new("0.1.0", "35", "target");
    let manifest_path = payload_fixture.payload.join("InstallPayloadManifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("payload manifest"))
            .expect("payload JSON");
    manifest
        .as_object_mut()
        .expect("payload object")
        .insert("unexpected".to_owned(), json!(true));
    write_json(&manifest_path, &manifest);
    assert_eq!(
        payload_fixture
            .adapter(
                Arc::new(SignatureState::default()),
                Arc::new(CopyState::default())
            )
            .expect_err("payload mutation")
            .code(),
        MacOsInstallAdapterErrorCode::InvalidPayloadManifest
    );

    let publisher_fixture = Fixture::new("0.1.0", "35", "target");
    let product_manifest_path = publisher_fixture
        .payload
        .join("Product/ProductManifest.json");
    let mut product_manifest: Value =
        serde_json::from_slice(&fs::read(&product_manifest_path).expect("product manifest"))
            .expect("product JSON");
    product_manifest["developer_team_id"] = json!("ABCDEFGHIJ");
    write_json(&product_manifest_path, &product_manifest);
    let payload_manifest_path = publisher_fixture
        .payload
        .join("InstallPayloadManifest.json");
    let mut payload_manifest: Value =
        serde_json::from_slice(&fs::read(&payload_manifest_path).expect("payload manifest"))
            .expect("payload JSON");
    payload_manifest["product_manifest"] =
        manifest_file_record(&product_manifest_path, "Product/ProductManifest.json");
    write_json(&payload_manifest_path, &payload_manifest);
    assert_eq!(
        publisher_fixture
            .adapter(
                Arc::new(SignatureState::default()),
                Arc::new(CopyState::default())
            )
            .expect_err("publisher Team drift")
            .code(),
        MacOsInstallAdapterErrorCode::InvalidProductManifest
    );

    let product_fixture = Fixture::new("0.1.0", "35", "target");
    let adapter = product_fixture
        .adapter(
            Arc::new(SignatureState::default()),
            Arc::new(CopyState::default()),
        )
        .expect("adapter");
    fs::write(
        product_fixture
            .payload
            .join("Product/Components/radishlex_manager.app/Contents/Info.plist"),
        b"changed",
    )
    .expect("mutate product");
    assert_eq!(
        adapter
            .revalidate_target_product()
            .expect_err("product mutation")
            .code(),
        MacOsInstallAdapterErrorCode::ProductChanged
    );
}

#[test]
fn historical_upgrade_sources_are_release_bound_and_revalidated() {
    let fixture = Fixture::new("0.1.0", "35", "target");
    add_upgrade_source(&fixture.payload, "0.0.9", "34", "source");
    let adapter = fixture
        .adapter(
            Arc::new(SignatureState::default()),
            Arc::new(CopyState::default()),
        )
        .expect("adapter with historical source");
    let source_release = ProductRelease::new("0.0.9", 34).expect("source release");
    let source_root = adapter
        .upgrade_source_product_root(&source_release)
        .expect("manifest-bound source");
    assert_eq!(source_root, fixture.payload.join("UpgradeSources/0.0.9-34"));
    assert!(adapter
        .upgrade_source_product_root(&ProductRelease::new("0.0.8", 33).expect("missing release"))
        .is_none());

    fs::write(
        source_root.join("Components/radishlex_manager.app/Contents/MacOS/program"),
        b"drifted source",
    )
    .expect("mutate source");
    assert_eq!(
        adapter
            .revalidate_target_product()
            .expect_err("historical source mutation")
            .code(),
        MacOsInstallAdapterErrorCode::ProductChanged
    );

    let duplicate = Fixture::new("0.1.0", "35", "target");
    add_upgrade_source(&duplicate.payload, "0.0.9", "34", "source");
    let manifest_path = duplicate.payload.join("InstallPayloadManifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("payload manifest"))
            .expect("payload JSON");
    let sources = manifest["upgrade_sources"]
        .as_array_mut()
        .expect("source array");
    sources.push(sources[0].clone());
    write_json(&manifest_path, &manifest);
    assert_eq!(
        duplicate
            .adapter(
                Arc::new(SignatureState::default()),
                Arc::new(CopyState::default())
            )
            .expect_err("duplicate release")
            .code(),
        MacOsInstallAdapterErrorCode::InvalidPayloadManifest
    );
}

#[test]
fn signature_rejection_and_identity_drift_never_reach_receipt_evidence() {
    let fixture = Fixture::new("0.1.0", "35", "target");
    let rejected = Arc::new(SignatureState::default());
    rejected.reject.store(true, Ordering::SeqCst);
    assert_eq!(
        fixture
            .adapter(rejected, Arc::new(CopyState::default()))
            .expect_err("signature rejected")
            .code(),
        MacOsInstallAdapterErrorCode::SignatureRejected
    );

    let state = Arc::new(SignatureState::default());
    let adapter = fixture
        .adapter(state.clone(), Arc::new(CopyState::default()))
        .expect("adapter");
    state.drift.store(true, Ordering::SeqCst);
    assert_eq!(
        adapter
            .revalidate_target_product()
            .expect_err("signature drift")
            .code(),
        MacOsInstallAdapterErrorCode::ProductChanged
    );
}

#[test]
fn partial_or_replaced_staging_is_preserved_and_not_overwritten() {
    let fixture = Fixture::new("0.1.0", "35", "target");
    let signature = Arc::new(SignatureState::default());
    let copy = Arc::new(CopyState::default());
    copy.fail_after_root.store(true, Ordering::SeqCst);
    let adapter = fixture.adapter(signature, copy.clone()).expect("adapter");
    let receipt_store = InstallReceiptStore::open(
        VerifiedInstallRoot::verify(fixture.data_root(), fixture.owner_id)
            .expect("verified data root"),
    )
    .expect("receipt store");
    let guard = receipt_store.acquire_guard().expect("guard");
    let mut receipt = first_install_receipt(&adapter, &receipt_store);
    receipt_store
        .persist(&guard, &receipt)
        .expect("prepared receipt");
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    receipt_store
        .persist(&guard, &receipt)
        .expect("quiesced receipt");
    let manager = adapter
        .open_program_store(&receipt_store, &guard, ProgramComponent::Manager, &receipt)
        .expect("Manager store");
    assert_eq!(
        adapter
            .prepare_and_record_staged(&receipt_store, &guard, &manager, &mut receipt)
            .expect_err("partial copy")
            .code(),
        MacOsInstallAdapterErrorCode::CopyFailed
    );
    copy.fail_after_root.store(false, Ordering::SeqCst);
    assert_eq!(
        adapter
            .prepare_and_record_staged(&receipt_store, &guard, &manager, &mut receipt)
            .expect_err("preserved partial staging")
            .code(),
        MacOsInstallAdapterErrorCode::ProductChanged
    );
    assert_eq!(copy.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn home_alias_permissions_and_bundle_hardlinks_are_rejected() {
    let fixture = Fixture::new("0.1.0", "35", "target");
    let alias = fixture.container.join("home-alias");
    symlink(&fixture.home, &alias).expect("home alias");
    assert_eq!(
        MacOsProductInstallAdapter::load_with_services(
            &fixture.payload,
            &alias,
            fixture.owner_id,
            Box::new(FakeSignatureVerifier {
                state: Arc::new(SignatureState::default())
            }),
            Box::new(FakeBundleCopier {
                state: Arc::new(CopyState::default())
            }),
        )
        .expect_err("home alias")
        .code(),
        MacOsInstallAdapterErrorCode::UnsafeUserHome
    );

    fs::set_permissions(
        fixture.home.join("Applications"),
        fs::Permissions::from_mode(0o777),
    )
    .expect("unsafe target permissions");
    assert_eq!(
        fixture
            .adapter(
                Arc::new(SignatureState::default()),
                Arc::new(CopyState::default())
            )
            .expect_err("unsafe target")
            .code(),
        MacOsInstallAdapterErrorCode::UnsafeTarget
    );

    let hardlink_fixture = Fixture::new("0.1.0", "35", "target");
    let info = hardlink_fixture
        .payload
        .join("Product/Components/radishlex_manager.app/Contents/Info.plist");
    fs::hard_link(&info, hardlink_fixture.container.join("external-link")).expect("hardlink");
    assert_eq!(
        hardlink_fixture
            .adapter(
                Arc::new(SignatureState::default()),
                Arc::new(CopyState::default())
            )
            .expect_err("hardlinked bundle file")
            .code(),
        MacOsInstallAdapterErrorCode::ProductChanged
    );
}

#[test]
fn first_install_layout_provisioning_creates_only_absent_fixed_directories() {
    let container = fs::canonicalize(std::env::temp_dir())
        .expect("temp directory")
        .join(format!(
            "radishlex-install-layout-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
    create_directory(&container, 0o700);
    let owner_id = fs::metadata(&container).expect("owner").uid();
    let home = container.join("home");
    create_directory(&home, 0o700);
    create_directory(&home.join("Library"), 0o700);
    create_directory(&home.join("Library/Application Support"), 0o700);

    assert!(
        MacOsProductInstallAdapter::first_install_targets_are_absent(&home, owner_id)
            .expect("read-only target inspection")
    );
    prepare_user_layout(&home, owner_id).expect("prepare fixed layout");
    for path in [
        home.join("Applications"),
        home.join("Library/Input Methods"),
        home.join(DATA_ROOT),
    ] {
        let metadata = fs::symlink_metadata(path).expect("prepared directory");
        assert!(metadata.is_dir());
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o700);
    }

    fs::set_permissions(home.join(DATA_ROOT), fs::Permissions::from_mode(0o755))
        .expect("make existing root unsafe");
    assert_eq!(
        prepare_user_layout(&home, owner_id)
            .expect_err("existing data root must not be chmodded")
            .code(),
        MacOsInstallAdapterErrorCode::UnsafeTarget
    );
    assert_eq!(
        fs::metadata(home.join(DATA_ROOT))
            .expect("root metadata")
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
    let _ = fs::remove_dir_all(container);
}

fn first_install_receipt(
    adapter: &MacOsProductInstallAdapter,
    store: &InstallReceiptStore,
) -> InstallReceipt {
    InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::FirstInstall,
        store.root_identity().clone(),
        None,
        Some(adapter.target_product().clone()),
    )
    .expect("first-install receipt")
}

fn create_user_layout(home: &Path) {
    create_directory(home, 0o700);
    create_directory(&home.join("Applications"), 0o700);
    create_directory(&home.join("Library"), 0o700);
    create_directory(&home.join("Library/Input Methods"), 0o700);
    create_directory(&home.join("Library/Application Support"), 0o700);
    create_directory(&home.join("Library/Application Support/RadishLex"), 0o700);
}

fn build_payload(root: &Path, version: &str, build: &str, marker: &str) {
    create_directory(root, 0o700);
    build_product_root(&root.join("Product"), version, build, marker);
    create_directory(&root.join("UpgradeSources"), 0o700);
    fs::write(root.join("InstallLayout.json"), COMMITTED_LAYOUT).expect("layout");
    let payload_manifest = json!({
        "format_version": 2,
        "product_id": INSTALL_PRODUCT_ID,
        "product_version": version,
        "build_number": build,
        "distribution_container": "dmg",
        "installer_kind": "dedicated-user-domain-app",
        "installation_scope": "current-user",
        "installer_bundle_id": "org.radishlex.installer.macos",
        "components": {
            "manager": {
                "product_path": "Components/radishlex_manager.app",
                "target_path": "Applications/RadishLex Manager.app"
            },
            "input_method": {
                "product_path": "Components/RadishLexInputMethod.app",
                "target_path": "Library/Input Methods/RadishLexInputMethod.app"
            }
        },
        "data": {
            "root_path": "Library/Application Support/RadishLex",
            "install_state_path": "Library/Application Support/RadishLex/.radishlex-install-v1",
            "default_removal": "programs-only",
            "data_removal": "separate-authorized-flow"
        },
        "install_layout": manifest_file_record(
            &root.join("InstallLayout.json"),
            "InstallLayout.json"
        ),
        "product_manifest": manifest_file_record(
            &root.join("Product/ProductManifest.json"),
            "Product/ProductManifest.json"
        ),
        "upgrade_sources": []
    });
    write_json(&root.join("InstallPayloadManifest.json"), &payload_manifest);
}

fn build_product_root(root: &Path, version: &str, build: &str, marker: &str) {
    create_directory(root, 0o700);
    create_directory(&root.join("Components"), 0o700);
    let manager = root.join("Components/radishlex_manager.app");
    let input_method = root.join("Components/RadishLexInputMethod.app");
    create_bundle(&manager, "manager", marker);
    create_bundle(&input_method, "input-method", marker);
    fs::write(root.join("LICENSE"), b"synthetic license\n").expect("license");
    let product_manifest = json!({
        "format_version": 2,
        "product_id": INSTALL_PRODUCT_ID,
        "product_version": version,
        "build_number": build,
        "minimum_macos": "13.0",
        "ffi_abi_version": 9,
        "userdb_schema_version": 9,
        "rime_data_manifest_version": 2,
        "native_libraries_manifest_version": 1,
        "data_layout": "application-support-v1",
        "developer_team_id": "WF9UUN335P",
        "rime_schema_id": "radishlex_pinyin",
        "components": [
            {
                "component": "input_method",
                "bundle_id": "org.radishlex.inputmethod.macos",
                "product_version": version,
                "build_number": build,
                "minimum_macos": "13.0",
                "files": bundle_file_records(&input_method),
            },
            {
                "component": "manager",
                "bundle_id": "dev.radishlex.radishlexManager",
                "product_version": version,
                "build_number": build,
                "minimum_macos": "13.0",
                "files": bundle_file_records(&manager),
            }
        ],
        "licenses": [{
            "path": "LICENSE",
            "size": fs::metadata(root.join("LICENSE")).expect("license metadata").len(),
            "sha256": file_sha256(&root.join("LICENSE")),
        }]
    });
    write_json(&root.join("ProductManifest.json"), &product_manifest);
}

fn add_upgrade_source(payload_root: &Path, version: &str, build: &str, marker: &str) {
    let relative = format!("UpgradeSources/{version}-{build}");
    let source_root = payload_root.join(&relative);
    build_product_root(&source_root, version, build, marker);
    let manifest_path = payload_root.join("InstallPayloadManifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("payload manifest"))
            .expect("payload JSON");
    manifest["upgrade_sources"]
        .as_array_mut()
        .expect("upgrade sources")
        .push(json!({
            "product_version": version,
            "build_number": build,
            "product_path": relative,
            "product_manifest": manifest_file_record(
                &source_root.join("ProductManifest.json"),
                &format!("UpgradeSources/{version}-{build}/ProductManifest.json"),
            ),
        }));
    write_json(&manifest_path, &manifest);
}

fn create_bundle(root: &Path, component: &str, marker: &str) {
    create_directory(root, 0o755);
    create_directory(&root.join("Contents"), 0o755);
    create_directory(&root.join("Contents/MacOS"), 0o755);
    fs::write(
        root.join("Contents/Info.plist"),
        format!("{component}:{marker}:info\n"),
    )
    .expect("Info.plist");
    write_executable(
        &root.join("Contents/MacOS/program"),
        format!("{component}:{marker}:program\n").as_bytes(),
    );
}

fn bundle_file_records(root: &Path) -> Vec<Value> {
    let mut paths = vec![
        root.join("Contents/Info.plist"),
        root.join("Contents/MacOS/program"),
    ];
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            json!({
                "path": path.strip_prefix(root).expect("relative").to_str().expect("UTF-8"),
                "type": "file",
                "size": fs::metadata(&path).expect("metadata").len(),
                "sha256": file_sha256(&path),
            })
        })
        .collect()
}

fn manifest_file_record(path: &Path, relative: &str) -> Value {
    json!({
        "path": relative,
        "size": fs::metadata(path).expect("metadata").len(),
        "sha256": file_sha256(path),
    })
}

fn write_json(path: &Path, value: &Value) {
    fs::write(
        path,
        serde_json::to_vec_pretty(value)
            .expect("JSON")
            .into_iter()
            .chain(std::iter::once(b'\n'))
            .collect::<Vec<_>>(),
    )
    .expect("write JSON");
}

fn file_sha256(path: &Path) -> String {
    format!(
        "{:x}",
        Sha256::digest(fs::read(path).expect("read hash input"))
    )
}

fn create_directory(path: &Path, mode: u32) {
    DirBuilder::new()
        .mode(mode)
        .create(path)
        .expect("create directory");
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set permissions");
}

fn write_executable(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o755)
        .open(path)
        .expect("create executable");
    file.write_all(bytes).expect("write executable");
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), MacOsInstallAdapterError> {
    let source_metadata = fs::symlink_metadata(source)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::CopyFailed))?;
    if source_metadata.file_type().is_symlink() {
        let target =
            fs::read_link(source).map_err(|_| error(MacOsInstallAdapterErrorCode::CopyFailed))?;
        symlink(target, destination)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CopyFailed))?;
        return Ok(());
    }
    if source_metadata.file_type().is_dir() {
        create_directory(destination, source_metadata.permissions().mode() & 0o7777);
        let mut entries: Vec<_> = fs::read_dir(source)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CopyFailed))?
            .collect::<Result<_, _>>()
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CopyFailed))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if source_metadata.file_type().is_file() {
        fs::copy(source, destination)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::CopyFailed))?;
        fs::set_permissions(
            destination,
            fs::Permissions::from_mode(source_metadata.permissions().mode() & 0o7777),
        )
        .map_err(|_| error(MacOsInstallAdapterErrorCode::CopyFailed))?;
        return Ok(());
    }
    Err(error(MacOsInstallAdapterErrorCode::CopyFailed))
}
