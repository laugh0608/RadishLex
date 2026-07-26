use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    resume_install_finalization, InstallFailureCode, InstallFinalizationError,
    InstallFinalizationPort, InstallFinalizationValidationStage, InstallOperationKind,
    InstallReceipt, ProductArtifactIdentity, ProductRelease, ProgramBundleIdentity,
    ProgramComponent, VerifiedInstallRoot, INSTALL_PRODUCT_ID,
};

use super::*;

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    container: PathBuf,
    receipt_store: InstallReceiptStore,
    guard: Option<InstallProcessGuard>,
    manager: ProgramSwitchStore,
    input_method: ProgramSwitchStore,
    receipt: InstallReceipt,
}

impl Fixture {
    fn upgrade_ready() -> Self {
        Self::ready(InstallOperationKind::Upgrade)
    }

    fn ready(operation_kind: InstallOperationKind) -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp directory")
            .join(format!(
                "radishlex-program-switch-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        create_directory(&container, 0o700);
        let owner_id = fs::metadata(&container).expect("container metadata").uid();
        let data_root = container.join("data");
        let manager_parent = container.join("Applications");
        let input_method_parent = container.join("Input Methods");
        for path in [&data_root, &manager_parent, &input_method_parent] {
            create_directory(path, 0o700);
        }

        let receipt_store = InstallReceiptStore::open(
            VerifiedInstallRoot::verify(&data_root, owner_id).expect("verified data root"),
        )
        .expect("receipt store");
        let guard = receipt_store.acquire_guard().expect("install guard");
        let operation_id = "00112233445566778899aabbccddeeff";
        let manager = ProgramSwitchStore::open(
            VerifiedProgramTarget::verify(&manager_parent, ProgramComponent::Manager, owner_id)
                .expect("Manager target"),
            operation_id,
        )
        .expect("Manager switch store");
        let input_method = ProgramSwitchStore::open(
            VerifiedProgramTarget::verify(
                &input_method_parent,
                ProgramComponent::InputMethod,
                owner_id,
            )
            .expect("InputMethod target"),
            operation_id,
        )
        .expect("InputMethod switch store");

        if operation_kind != InstallOperationKind::FirstInstall {
            for path in [manager.target_path(), input_method.target_path()] {
                create_directory(path, 0o755);
            }
        }
        if operation_kind != InstallOperationKind::RemovePrograms {
            for path in [
                manager.staged_bundle_path(),
                input_method.staged_bundle_path(),
            ] {
                create_directory(path, 0o755);
            }
        }

        let source =
            (operation_kind != InstallOperationKind::FirstInstall).then(|| product("0.1.0", 35, 0));
        let target = match operation_kind {
            InstallOperationKind::FirstInstall | InstallOperationKind::Upgrade => {
                Some(product("0.2.0", 36, 1))
            }
            InstallOperationKind::Repair => Some(product("0.1.0", 35, 1)),
            InstallOperationKind::RemovePrograms => None,
        };
        let mut receipt = InstallReceipt::new(
            operation_id,
            None,
            operation_kind,
            receipt_store.root_identity().clone(),
            source,
            target,
        )
        .expect("program receipt");
        receipt_store
            .persist(&guard, &receipt)
            .expect("prepared receipt");
        receipt.advance(InstallState::Quiesced).expect("quiesced");
        receipt_store
            .persist(&guard, &receipt)
            .expect("quiesced receipt");
        if operation_kind != InstallOperationKind::FirstInstall {
            for store in [&manager, &input_method] {
                record_program_source(&receipt_store, &guard, store, &mut receipt)
                    .expect("record source");
            }
        }
        if operation_kind != InstallOperationKind::RemovePrograms {
            for store in [&manager, &input_method] {
                record_staged_program(&receipt_store, &guard, store, &mut receipt)
                    .expect("record staging");
            }
            finish_target_staging(
                &receipt_store,
                &guard,
                &manager,
                &input_method,
                &mut receipt,
            )
            .expect("finish staging");
        }

        Self {
            container,
            receipt_store,
            guard: Some(guard),
            manager,
            input_method,
            receipt,
        }
    }

    fn preserve_both(&mut self) {
        preserve_program_source(
            &self.receipt_store,
            self.guard.as_ref().expect("guard"),
            &self.manager,
            &mut self.receipt,
        )
        .expect("preserve Manager");
        preserve_program_source(
            &self.receipt_store,
            self.guard.as_ref().expect("guard"),
            &self.input_method,
            &mut self.receipt,
        )
        .expect("preserve InputMethod");
        finish_source_preservation(
            &self.receipt_store,
            self.guard.as_ref().expect("guard"),
            &self.manager,
            &self.input_method,
            &mut self.receipt,
        )
        .expect("finish preservation");
    }

    fn commit_both(&mut self) {
        commit_program_target(
            &self.receipt_store,
            self.guard.as_ref().expect("guard"),
            &self.manager,
            &mut self.receipt,
        )
        .expect("commit Manager");
        commit_program_target(
            &self.receipt_store,
            self.guard.as_ref().expect("guard"),
            &self.input_method,
            &mut self.receipt,
        )
        .expect("commit InputMethod");
    }

    fn require_rollback(&mut self) {
        self.receipt
            .require_rollback(InstallFailureCode::DataCoordinationFailed)
            .expect("rollback required");
        self.receipt_store
            .persist(self.guard.as_ref().expect("guard"), &self.receipt)
            .expect("persist rollback");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.guard.take());
        let _ = fs::remove_dir_all(&self.container);
    }
}

#[derive(Debug)]
struct FailOnce {
    point: ProgramSwitchFaultPoint,
    fired: bool,
}

impl ProgramSwitchFaultInjector for FailOnce {
    fn should_fail(&mut self, point: ProgramSwitchFaultPoint) -> bool {
        if !self.fired && point == self.point {
            self.fired = true;
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
struct FinalizationPort {
    calls: Vec<InstallFinalizationValidationStage>,
    reject: Option<InstallFinalizationValidationStage>,
}

impl InstallFinalizationPort for FinalizationPort {
    fn validate_final_state(
        &mut self,
        manager: &ProgramSwitchStore,
        input_method: &ProgramSwitchStore,
        receipt: &InstallReceipt,
        stage: InstallFinalizationValidationStage,
    ) -> bool {
        self.calls.push(stage);
        if self.reject == Some(stage) {
            return false;
        }
        let manager_exists = fs::symlink_metadata(manager.target_path()).is_ok();
        let input_method_exists = fs::symlink_metadata(input_method.target_path()).is_ok();
        if receipt.operation_kind() == InstallOperationKind::RemovePrograms {
            !manager_exists && !input_method_exists
        } else {
            manager_exists && input_method_exists
        }
    }
}

fn prepare_for_finalization(fixture: &mut Fixture) {
    match fixture.receipt.operation_kind() {
        InstallOperationKind::FirstInstall => fixture.commit_both(),
        InstallOperationKind::Upgrade => {
            fixture.preserve_both();
            fixture.commit_both();
            fixture
                .receipt
                .advance(InstallState::DataCoordinating)
                .expect("data coordinating");
            fixture
                .receipt_store
                .persist(fixture.guard.as_ref().expect("guard"), &fixture.receipt)
                .expect("persist data coordinating");
            fixture
                .receipt
                .advance(InstallState::DataSettled)
                .expect("data settled");
            fixture
                .receipt_store
                .persist(fixture.guard.as_ref().expect("guard"), &fixture.receipt)
                .expect("persist data settled");
        }
        InstallOperationKind::Repair => {
            fixture.preserve_both();
            fixture.commit_both();
        }
        InstallOperationKind::RemovePrograms => {
            fixture.preserve_both();
            for store in [&fixture.manager, &fixture.input_method] {
                commit_program_removal(
                    &fixture.receipt_store,
                    fixture.guard.as_ref().expect("guard"),
                    store,
                    &mut fixture.receipt,
                )
                .expect("commit removal");
            }
        }
    }
}

#[test]
fn finalization_uses_one_two_checkpoint_path_for_every_operation() {
    for kind in [
        InstallOperationKind::FirstInstall,
        InstallOperationKind::Upgrade,
        InstallOperationKind::Repair,
        InstallOperationKind::RemovePrograms,
    ] {
        let mut fixture = Fixture::ready(kind);
        prepare_for_finalization(&mut fixture);
        let mut port = FinalizationPort::default();
        resume_install_finalization(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            &mut fixture.receipt,
            &fixture.manager,
            &fixture.input_method,
            &mut port,
        )
        .expect("finalization");
        assert_eq!(fixture.receipt.state(), InstallState::Completed);
        assert_eq!(
            port.calls,
            [
                InstallFinalizationValidationStage::BeforeFinalVerified,
                InstallFinalizationValidationStage::BeforeCompleted,
            ]
        );
        fixture
            .receipt_store
            .verify_current(fixture.guard.as_ref().expect("guard"), &fixture.receipt)
            .expect("completed receipt persisted");
    }
}

#[test]
fn finalization_persists_final_verified_and_resumes_idempotently() {
    let mut fixture = Fixture::upgrade_ready();
    prepare_for_finalization(&mut fixture);
    let mut interrupted = FinalizationPort {
        reject: Some(InstallFinalizationValidationStage::BeforeCompleted),
        ..FinalizationPort::default()
    };
    assert_eq!(
        resume_install_finalization(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            &mut fixture.receipt,
            &fixture.manager,
            &fixture.input_method,
            &mut interrupted,
        ),
        Err(InstallFinalizationError::FinalStateNotProven(
            InstallFinalizationValidationStage::BeforeCompleted
        ))
    );
    assert_eq!(fixture.receipt.state(), InstallState::FinalVerified);
    fixture
        .receipt_store
        .verify_current(fixture.guard.as_ref().expect("guard"), &fixture.receipt)
        .expect("final_verified persisted");

    let mut resumed = FinalizationPort::default();
    resume_install_finalization(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &mut fixture.receipt,
        &fixture.manager,
        &fixture.input_method,
        &mut resumed,
    )
    .expect("resume finalization");
    assert_eq!(
        resumed.calls,
        [InstallFinalizationValidationStage::BeforeCompleted]
    );
    assert_eq!(fixture.receipt.state(), InstallState::Completed);
    resume_install_finalization(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &mut fixture.receipt,
        &fixture.manager,
        &fixture.input_method,
        &mut resumed,
    )
    .expect("completed finalization is idempotent");
    assert_eq!(
        resumed.calls,
        [
            InstallFinalizationValidationStage::BeforeCompleted,
            InstallFinalizationValidationStage::BeforeCompleted,
        ]
    );
}

#[test]
fn upgrade_recovers_a_partial_program_switch_back_to_exact_source_inodes() {
    let mut fixture = Fixture::upgrade_ready();
    let manager_source = fixture
        .receipt
        .artifact(InstallArtifactSlot::SourceManager)
        .expect("Manager source")
        .filesystem_identity()
        .clone();
    let input_source = fixture
        .receipt
        .artifact(InstallArtifactSlot::SourceInputMethod)
        .expect("InputMethod source")
        .filesystem_identity()
        .clone();

    fixture.preserve_both();
    commit_program_target(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &fixture.manager,
        &mut fixture.receipt,
    )
    .expect("commit only Manager");
    assert_eq!(fixture.receipt.state(), InstallState::ManagerCommitted);
    fixture
        .receipt
        .require_rollback(InstallFailureCode::InputMethodCommitFailed)
        .expect("rollback required");
    fixture
        .receipt_store
        .persist(fixture.guard.as_ref().expect("guard"), &fixture.receipt)
        .expect("persist rollback");

    restore_program_source(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &fixture.manager,
        &fixture.receipt,
    )
    .expect("restore Manager");
    restore_program_source(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &fixture.input_method,
        &fixture.receipt,
    )
    .expect("restore InputMethod");
    finish_program_restore(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &fixture.manager,
        &fixture.input_method,
        &mut fixture.receipt,
    )
    .expect("finish restore");

    assert_eq!(fixture.receipt.state(), InstallState::ProgramsRestored);
    assert_eq!(
        filesystem_identity(fixture.manager.target_path()),
        manager_source
    );
    assert_eq!(
        filesystem_identity(fixture.input_method.target_path()),
        input_source
    );
    assert!(fs::symlink_metadata(fixture.manager.source_backup_bundle_path()).is_err());
    assert!(fs::symlink_metadata(fixture.input_method.source_backup_bundle_path()).is_err());
}

#[test]
fn first_install_rollback_removes_the_partially_committed_programs() {
    let mut fixture = Fixture::ready(InstallOperationKind::FirstInstall);
    commit_program_target(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &fixture.manager,
        &mut fixture.receipt,
    )
    .expect("commit only Manager");
    fixture
        .receipt
        .require_rollback(InstallFailureCode::InputMethodCommitFailed)
        .expect("rollback required");
    fixture
        .receipt_store
        .persist(fixture.guard.as_ref().expect("guard"), &fixture.receipt)
        .expect("persist rollback");
    for store in [&fixture.manager, &fixture.input_method] {
        restore_program_source(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            store,
            &fixture.receipt,
        )
        .expect("restore first-install state");
    }
    finish_program_restore(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &fixture.manager,
        &fixture.input_method,
        &mut fixture.receipt,
    )
    .expect("finish restore");
    assert!(fs::symlink_metadata(fixture.manager.target_path()).is_err());
    assert!(fs::symlink_metadata(fixture.input_method.target_path()).is_err());
}

#[test]
fn remove_programs_can_restore_both_original_program_inodes() {
    let mut fixture = Fixture::ready(InstallOperationKind::RemovePrograms);
    let manager_source = fixture
        .receipt
        .artifact(InstallArtifactSlot::SourceManager)
        .expect("Manager source")
        .filesystem_identity()
        .clone();
    let input_source = fixture
        .receipt
        .artifact(InstallArtifactSlot::SourceInputMethod)
        .expect("InputMethod source")
        .filesystem_identity()
        .clone();
    fixture.preserve_both();
    for store in [&fixture.manager, &fixture.input_method] {
        commit_program_removal(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            store,
            &mut fixture.receipt,
        )
        .expect("commit program removal");
    }
    fixture.require_rollback();
    for store in [&fixture.manager, &fixture.input_method] {
        restore_program_source(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            store,
            &fixture.receipt,
        )
        .expect("restore removed program");
    }
    finish_program_restore(
        &fixture.receipt_store,
        fixture.guard.as_ref().expect("guard"),
        &fixture.manager,
        &fixture.input_method,
        &mut fixture.receipt,
    )
    .expect("finish restore");
    assert_eq!(
        filesystem_identity(fixture.manager.target_path()),
        manager_source
    );
    assert_eq!(
        filesystem_identity(fixture.input_method.target_path()),
        input_source
    );
}

#[test]
fn preserve_and_commit_retry_every_rename_and_directory_sync_boundary() {
    for boundary in boundaries() {
        let mut fixture = Fixture::upgrade_ready();
        let mut faults = FailOnce {
            point: ProgramSwitchFaultPoint::new(ProgramSwitchAction::PreserveSource, boundary),
            fired: false,
        };
        let error = preserve_program_source_with_faults(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            &fixture.manager,
            &mut fixture.receipt,
            &mut faults,
        )
        .expect_err("injected preserve failure");
        assert_eq!(error.code(), ProgramSwitchErrorCode::FaultInjected);
        preserve_program_source(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            &fixture.manager,
            &mut fixture.receipt,
        )
        .expect("retry preserve");
    }

    for boundary in boundaries() {
        let mut fixture = Fixture::upgrade_ready();
        fixture.preserve_both();
        let mut faults = FailOnce {
            point: ProgramSwitchFaultPoint::new(ProgramSwitchAction::CommitTarget, boundary),
            fired: false,
        };
        let error = commit_program_target_with_faults(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            &fixture.manager,
            &mut fixture.receipt,
            &mut faults,
        )
        .expect_err("injected commit failure");
        assert_eq!(error.code(), ProgramSwitchErrorCode::FaultInjected);
        commit_program_target(
            &fixture.receipt_store,
            fixture.guard.as_ref().expect("guard"),
            &fixture.manager,
            &mut fixture.receipt,
        )
        .expect("retry commit");
        assert_eq!(fixture.receipt.state(), InstallState::ManagerCommitted);
    }
}

#[test]
fn rollback_retries_both_rename_actions_at_every_durability_boundary() {
    for action in [
        ProgramSwitchAction::ReturnTargetToStage,
        ProgramSwitchAction::RestoreSource,
    ] {
        for boundary in boundaries() {
            let mut fixture = Fixture::upgrade_ready();
            fixture.preserve_both();
            fixture.commit_both();
            fixture.require_rollback();
            let mut faults = FailOnce {
                point: ProgramSwitchFaultPoint::new(action, boundary),
                fired: false,
            };
            let error = restore_program_source_with_faults(
                &fixture.receipt_store,
                fixture.guard.as_ref().expect("guard"),
                &fixture.manager,
                &fixture.receipt,
                &mut faults,
            )
            .expect_err("injected rollback failure");
            assert_eq!(error.code(), ProgramSwitchErrorCode::FaultInjected);
            restore_program_source(
                &fixture.receipt_store,
                fixture.guard.as_ref().expect("guard"),
                &fixture.manager,
                &fixture.receipt,
            )
            .expect("retry Manager restore");
            restore_program_source(
                &fixture.receipt_store,
                fixture.guard.as_ref().expect("guard"),
                &fixture.input_method,
                &fixture.receipt,
            )
            .expect("restore InputMethod");
            finish_program_restore(
                &fixture.receipt_store,
                fixture.guard.as_ref().expect("guard"),
                &fixture.manager,
                &fixture.input_method,
                &mut fixture.receipt,
            )
            .expect("finish restore");
        }
    }
}

#[test]
fn target_and_transaction_roots_reject_aliases_permissions_and_unknown_entries() {
    let container = fs::canonicalize(std::env::temp_dir())
        .expect("temp directory")
        .join(format!(
            "radishlex-program-target-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
    create_directory(&container, 0o700);
    let owner_id = fs::metadata(&container).expect("metadata").uid();
    let unsafe_parent = container.join("unsafe");
    create_directory(&unsafe_parent, 0o777);
    assert_eq!(
        VerifiedProgramTarget::verify(&unsafe_parent, ProgramComponent::Manager, owner_id)
            .expect_err("unsafe permissions")
            .code(),
        ProgramSwitchErrorCode::UnsafeTargetParent
    );

    let parent = container.join("Applications");
    create_directory(&parent, 0o700);
    let target = VerifiedProgramTarget::verify(&parent, ProgramComponent::Manager, owner_id)
        .expect("target");
    let store =
        ProgramSwitchStore::open(target, "00112233445566778899aabbccddeeff").expect("switch store");
    create_directory(&store.transaction_directory.join("unexpected"), 0o700);
    assert_eq!(
        store.revalidate().expect_err("unknown entry").code(),
        ProgramSwitchErrorCode::UnexpectedTransactionObject
    );
    fs::remove_dir_all(&container).expect("remove fixture");
}

fn boundaries() -> [ProgramSwitchBoundary; 4] {
    [
        ProgramSwitchBoundary::BeforeRename,
        ProgramSwitchBoundary::AfterRename,
        ProgramSwitchBoundary::AfterTargetDirectorySync,
        ProgramSwitchBoundary::AfterSourceDirectorySync,
    ]
}

fn create_directory(path: &Path, mode: u32) {
    DirBuilder::new()
        .mode(mode)
        .create(path)
        .expect("create directory");
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set permissions");
}

fn filesystem_identity(path: &Path) -> ProgramFilesystemIdentity {
    let metadata = fs::symlink_metadata(path).expect("artifact metadata");
    ProgramFilesystemIdentity::new(
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.permissions().mode() & 0o7777,
    )
    .expect("filesystem identity")
}

fn hash(byte: char) -> String {
    byte.to_string().repeat(64)
}

fn bundle(
    component: ProgramComponent,
    bundle_id: &str,
    tree: char,
    code: char,
) -> ProgramBundleIdentity {
    ProgramBundleIdentity::new(component, bundle_id, hash(tree), hash(code))
        .expect("bundle identity")
}

fn product(version: &str, build: u64, marker: u8) -> ProductArtifactIdentity {
    let hex =
        |offset: u8| char::from_digit(u32::from(marker * 5 + offset), 16).expect("hex marker");
    ProductArtifactIdentity::new(
        INSTALL_PRODUCT_ID,
        ProductRelease::new(version, build).expect("release"),
        hash(hex(0)),
        bundle(
            ProgramComponent::Manager,
            "org.radishlex.manager",
            hex(1),
            hex(2),
        ),
        bundle(
            ProgramComponent::InputMethod,
            "org.radishlex.inputmethod",
            hex(3),
            hex(4),
        ),
    )
    .expect("product identity")
}
