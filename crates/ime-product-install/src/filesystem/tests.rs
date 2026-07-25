use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    InstallArtifactEvidence, InstallArtifactSlot, InstallFailureCode, InstallOperationKind,
    InstallReceipt, InstallStartupGateDecision, InstallStartupGateErrorCode, InstallState,
    InstallStatusDecision, ProductArtifactIdentity, ProductRelease, ProgramBundleIdentity,
    ProgramComponent, ProgramFilesystemIdentity, RunningProgramIdentity, INSTALL_PRODUCT_ID,
};

use super::*;

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    container: PathBuf,
    data_root: PathBuf,
    owner_id: u32,
}

impl Fixture {
    fn new(create_root: bool) -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-install-core-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new()
            .mode(0o700)
            .create(&container)
            .expect("container");
        let owner_id = fs::metadata(&container).expect("metadata").uid();
        let data_root = container.join("RadishLex");
        if create_root {
            DirBuilder::new()
                .mode(0o700)
                .create(&data_root)
                .expect("data root");
        }
        Self {
            container,
            data_root,
            owner_id,
        }
    }

    fn store(&self) -> InstallReceiptStore {
        InstallReceiptStore::open(
            VerifiedInstallRoot::verify(&self.data_root, self.owner_id).expect("verified root"),
        )
        .expect("receipt store")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

fn hash(byte: char) -> String {
    byte.to_string().repeat(64)
}

fn product(version: &str, build: u64, marker: u8) -> ProductArtifactIdentity {
    let hex =
        |offset: u8| char::from_digit(u32::from(marker * 5 + offset), 16).expect("hex marker");
    ProductArtifactIdentity::new(
        INSTALL_PRODUCT_ID,
        ProductRelease::new(version, build).expect("release"),
        hash(hex(0)),
        ProgramBundleIdentity::new(
            ProgramComponent::Manager,
            "org.radishlex.manager",
            hash(hex(1)),
            hash(hex(2)),
        )
        .expect("Manager"),
        ProgramBundleIdentity::new(
            ProgramComponent::InputMethod,
            "org.radishlex.inputmethod",
            hash(hex(3)),
            hash(hex(4)),
        )
        .expect("InputMethod"),
    )
    .expect("product")
}

fn running(
    product: &ProductArtifactIdentity,
    component: ProgramComponent,
) -> RunningProgramIdentity {
    RunningProgramIdentity::new(
        product.release().clone(),
        product.program(component).clone(),
    )
    .expect("running")
}

fn first_install_receipt(store: &InstallReceiptStore) -> InstallReceipt {
    InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::FirstInstall,
        store.root_identity().clone(),
        None,
        Some(product("0.1.0", 35, 1)),
    )
    .expect("receipt")
}

fn evidence(receipt: &InstallReceipt, slot: InstallArtifactSlot) -> InstallArtifactEvidence {
    let target = receipt.target_product().expect("target");
    let inode = match slot {
        InstallArtifactSlot::StagedManager | InstallArtifactSlot::InstalledManager => 200,
        InstallArtifactSlot::StagedInputMethod | InstallArtifactSlot::InstalledInputMethod => 201,
        InstallArtifactSlot::SourceManager | InstallArtifactSlot::BackupManager => 100,
        InstallArtifactSlot::SourceInputMethod | InstallArtifactSlot::BackupInputMethod => 101,
    };
    InstallArtifactEvidence::new(
        slot,
        target.program(slot.component()).clone(),
        ProgramFilesystemIdentity::new(10, inode, receipt.root_identity().owner_id(), 0o755)
            .expect("filesystem identity"),
    )
    .expect("evidence")
}

fn complete_first_install(
    store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    receipt: &mut InstallReceipt,
) {
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    store.persist(guard, receipt).expect("persist quiesced");
    for slot in [
        InstallArtifactSlot::StagedManager,
        InstallArtifactSlot::StagedInputMethod,
    ] {
        receipt
            .record_artifact(evidence(receipt, slot))
            .expect("staging");
        store.persist(guard, receipt).expect("persist staging");
    }
    receipt
        .advance(InstallState::TargetStaged)
        .expect("target staged");
    store
        .persist(guard, receipt)
        .expect("persist target staged");
    receipt
        .record_artifact(evidence(receipt, InstallArtifactSlot::InstalledManager))
        .expect("installed Manager");
    store
        .persist(guard, receipt)
        .expect("persist installed Manager");
    receipt
        .advance(InstallState::ManagerCommitted)
        .expect("Manager committed");
    store
        .persist(guard, receipt)
        .expect("persist Manager committed");
    receipt
        .record_artifact(evidence(receipt, InstallArtifactSlot::InstalledInputMethod))
        .expect("installed InputMethod");
    store
        .persist(guard, receipt)
        .expect("persist installed InputMethod");
    for state in [
        InstallState::ProgramsCommitted,
        InstallState::FinalVerified,
        InstallState::Completed,
    ] {
        receipt.advance(state).expect("state advances");
        store.persist(guard, receipt).expect("persist state");
    }
}

fn write_private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .expect("private file");
    file.write_all(bytes).expect("write");
    file.sync_all().expect("sync");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("mode");
}

#[test]
fn store_persists_canonical_receipt_and_guard_rejects_concurrency() {
    let fixture = Fixture::new(true);
    let store = fixture.store();
    let receipt = first_install_receipt(&store);
    let guard = store.acquire_guard().expect("guard");
    assert_eq!(
        store.acquire_guard().expect_err("concurrent guard").code(),
        InstallFilesystemErrorCode::OperationAlreadyActive
    );
    store.persist(&guard, &receipt).expect("persist");
    assert_eq!(
        store.load().expect_err("active guard").code(),
        InstallFilesystemErrorCode::OperationAlreadyActive
    );
    drop(guard);
    assert_eq!(store.load().expect("load"), Some(receipt));
}

#[test]
fn store_accepts_only_append_only_single_operation_replacements() {
    let fixture = Fixture::new(true);
    let store = fixture.store();
    let mut receipt = first_install_receipt(&store);
    let guard = store.acquire_guard().expect("guard");
    store.persist(&guard, &receipt).expect("initial");
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    store.persist(&guard, &receipt).expect("state replacement");
    receipt
        .record_artifact(evidence(&receipt, InstallArtifactSlot::StagedManager))
        .expect("artifact");
    store
        .persist(&guard, &receipt)
        .expect("evidence replacement");

    let different = InstallReceipt::new(
        "ffeeddccbbaa99887766554433221100",
        None,
        InstallOperationKind::FirstInstall,
        store.root_identity().clone(),
        None,
        Some(product("0.1.0", 35, 1)),
    )
    .expect("different receipt");
    assert_eq!(
        store
            .persist(&guard, &different)
            .expect_err("operation replacement")
            .code(),
        InstallFilesystemErrorCode::InvalidReceiptReplacement
    );
}

#[test]
fn startup_gate_is_read_only_for_absent_and_empty_state() {
    let target = product("0.1.0", 35, 1);
    let running = running(&target, ProgramComponent::Manager);
    let absent = Fixture::new(false);
    assert_eq!(
        inspect_install_startup_gate(&absent.data_root, absent.owner_id, &running).decision(),
        InstallStartupGateDecision::AllowedFirstLaunch
    );
    assert!(!absent.data_root.exists());

    let empty = Fixture::new(true);
    assert_eq!(
        inspect_install_startup_gate(&empty.data_root, empty.owner_id, &running).decision(),
        InstallStartupGateDecision::AllowedNoInstallState
    );
    assert!(!empty.data_root.join(STATE_DIRECTORY_NAME).exists());
}

#[test]
fn status_inspection_projects_restart_state_without_creating_or_mutating() {
    let absent = Fixture::new(false);
    let first = inspect_install_status(&absent.data_root, absent.owner_id);
    assert_eq!(first.decision(), InstallStatusDecision::ReadyFirstLaunch);
    assert!(!absent.data_root.exists());

    let fixture = Fixture::new(true);
    let empty = inspect_install_status(&fixture.data_root, fixture.owner_id);
    assert_eq!(empty.decision(), InstallStatusDecision::ReadyNoInstallState);
    assert!(!fixture.data_root.join(STATE_DIRECTORY_NAME).exists());

    let store = fixture.store();
    let mut receipt = first_install_receipt(&store);
    let guard = store.acquire_guard().expect("guard");
    store.persist(&guard, &receipt).expect("persist prepared");
    let active = inspect_install_status(&fixture.data_root, fixture.owner_id);
    assert_eq!(
        active.decision(),
        InstallStatusDecision::OperationInProgress
    );
    assert_eq!(
        active.error_code(),
        InstallStartupGateErrorCode::ActiveGuard
    );
    assert_eq!(active.operation_kind(), None);
    drop(guard);

    let stale_guard = UnixListener::bind(store.guard_path()).expect("stale guard");
    fs::set_permissions(store.guard_path(), fs::Permissions::from_mode(0o600))
        .expect("stale guard permissions");
    drop(stale_guard);
    let resumable = inspect_install_status(&fixture.data_root, fixture.owner_id);
    assert_eq!(
        resumable.decision(),
        InstallStatusDecision::OperationInProgress
    );
    assert_eq!(
        resumable.operation_kind(),
        Some(InstallOperationKind::FirstInstall)
    );
    assert_eq!(resumable.receipt_state(), Some(InstallState::Prepared));
    assert_eq!(resumable.failure_code(), None);
    assert!(!resumable.manual_recovery_required());
    assert!(store.guard_path().exists());
    fs::remove_file(store.guard_path()).expect("remove stale guard");

    let guard = store.acquire_guard().expect("completion guard");
    complete_first_install(&store, &guard, &mut receipt);
    drop(guard);
    let completed = inspect_install_status(&fixture.data_root, fixture.owner_id);
    assert_eq!(completed.decision(), InstallStatusDecision::TerminalReceipt);
    assert_eq!(completed.receipt_state(), Some(InstallState::Completed));
    assert_eq!(
        completed.operation_kind(),
        Some(InstallOperationKind::FirstInstall)
    );
}

#[test]
fn startup_gate_blocks_guard_and_nonterminal_then_matches_completed_target() {
    let fixture = Fixture::new(true);
    let store = fixture.store();
    let mut receipt = first_install_receipt(&store);
    let target = receipt.target_product().expect("target").clone();
    let running_input_method = running(&target, ProgramComponent::InputMethod);
    let guard = store.acquire_guard().expect("guard");
    store.persist(&guard, &receipt).expect("persist");
    assert_eq!(
        inspect_install_startup_gate(&fixture.data_root, fixture.owner_id, &running_input_method)
            .error_code(),
        InstallStartupGateErrorCode::ActiveGuard
    );
    assert_eq!(
        inspect_install_startup_gate_with(&fixture.data_root, fixture.owner_id, || {
            panic!("active guard must block before runtime identity")
        })
        .error_code(),
        InstallStartupGateErrorCode::ActiveGuard
    );
    drop(guard);
    assert_eq!(
        inspect_install_startup_gate(&fixture.data_root, fixture.owner_id, &running_input_method)
            .decision(),
        InstallStartupGateDecision::BlockedInstallInProgress
    );
    assert_eq!(
        inspect_install_startup_gate_with(&fixture.data_root, fixture.owner_id, || {
            panic!("nonterminal receipt must block before runtime identity")
        })
        .decision(),
        InstallStartupGateDecision::BlockedInstallInProgress
    );

    let guard = store.acquire_guard().expect("guard");
    complete_first_install(&store, &guard, &mut receipt);
    drop(guard);
    assert!(inspect_install_startup_gate(
        &fixture.data_root,
        fixture.owner_id,
        &running_input_method
    )
    .is_allowed());
    let drifted = running(&product("0.1.0", 35, 2), ProgramComponent::InputMethod);
    assert_eq!(
        inspect_install_startup_gate(&fixture.data_root, fixture.owner_id, &drifted).error_code(),
        InstallStartupGateErrorCode::ProgramIdentityChanged
    );
}

#[test]
fn startup_gate_rejects_interrupted_unknown_and_corrupt_state() {
    let target = product("0.1.0", 35, 1);
    let running = running(&target, ProgramComponent::Manager);

    let interrupted = Fixture::new(true);
    let interrupted_store = interrupted.store();
    write_private(&interrupted_store.staged_receipt_path(), b"interrupted\n");
    assert_eq!(
        inspect_install_startup_gate(&interrupted.data_root, interrupted.owner_id, &running)
            .error_code(),
        InstallStartupGateErrorCode::InterruptedReceipt
    );

    let unknown = Fixture::new(true);
    let unknown_store = unknown.store();
    write_private(&unknown_store.state_directory.join("unexpected"), b"x\n");
    assert_eq!(
        inspect_install_startup_gate(&unknown.data_root, unknown.owner_id, &running).error_code(),
        InstallStartupGateErrorCode::UnexpectedStateObject
    );

    let corrupt = Fixture::new(true);
    let corrupt_store = corrupt.store();
    write_private(&corrupt_store.receipt_path(), b"broken\n");
    assert_eq!(
        inspect_install_startup_gate(&corrupt.data_root, corrupt.owner_id, &running).error_code(),
        InstallStartupGateErrorCode::InvalidReceipt
    );
}

#[test]
fn startup_gate_rejects_receipt_bound_to_another_root() {
    let fixture = Fixture::new(true);
    let store = fixture.store();
    let target = product("0.1.0", 35, 1);
    let receipt = InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::FirstInstall,
        crate::InstallRootIdentity::new(
            store.root_identity().device_id(),
            store.root_identity().inode() + 1,
            store.root_identity().owner_id(),
            0o700,
        )
        .expect("wrong root identity"),
        None,
        Some(target.clone()),
    )
    .expect("receipt");
    write_private(
        &store.receipt_path(),
        &receipt.encode().expect("receipt bytes"),
    );
    assert_eq!(
        inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &running(&target, ProgramComponent::Manager)
        )
        .error_code(),
        InstallStartupGateErrorCode::RootIdentityChanged
    );
}

#[test]
fn precommit_terminal_without_source_fails_closed_for_a_running_program() {
    let fixture = Fixture::new(true);
    let store = fixture.store();
    let mut receipt = first_install_receipt(&store);
    let target = receipt.target_product().expect("target").clone();
    let guard = store.acquire_guard().expect("guard");
    store.persist(&guard, &receipt).expect("persist prepared");
    receipt
        .abort_preserved(InstallFailureCode::TargetArtifactInvalid)
        .expect("abort");
    store.persist(&guard, &receipt).expect("persist");
    drop(guard);
    assert_eq!(
        inspect_install_startup_gate(
            &fixture.data_root,
            fixture.owner_id,
            &running(&target, ProgramComponent::Manager)
        )
        .error_code(),
        InstallStartupGateErrorCode::ProgramIdentityChanged
    );
}

#[test]
fn store_replaces_a_terminal_receipt_only_with_its_chained_operation() {
    let fixture = Fixture::new(true);
    let store = fixture.store();
    let mut completed = first_install_receipt(&store);
    let guard = store.acquire_guard().expect("guard");
    store.persist(&guard, &completed).expect("persist prepared");
    complete_first_install(&store, &guard, &mut completed);
    drop(guard);

    let next = InstallReceipt::new(
        "ffeeddccbbaa99887766554433221100",
        Some(completed.operation_id().to_owned()),
        InstallOperationKind::Upgrade,
        store.root_identity().clone(),
        completed.target_product().cloned(),
        Some(product("0.2.0", 36, 2)),
    )
    .expect("next receipt");
    let guard = store.acquire_guard().expect("guard");
    store
        .persist(&guard, &next)
        .expect("persist next operation");
    drop(guard);
    assert_eq!(store.load().expect("load"), Some(next));
}
