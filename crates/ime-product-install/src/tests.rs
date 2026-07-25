use super::*;

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

fn root() -> InstallRootIdentity {
    InstallRootIdentity::new(10, 20, 501, 0o700).expect("root identity")
}

fn receipt(kind: InstallOperationKind) -> InstallReceipt {
    let source = match kind {
        InstallOperationKind::FirstInstall => None,
        InstallOperationKind::Upgrade
        | InstallOperationKind::Repair
        | InstallOperationKind::RemovePrograms => Some(product("0.0.9", 34, 0)),
    };
    let target = match kind {
        InstallOperationKind::FirstInstall | InstallOperationKind::Upgrade => {
            Some(product("0.1.0", 35, 1))
        }
        InstallOperationKind::Repair => Some(product("0.0.9", 34, 1)),
        InstallOperationKind::RemovePrograms => None,
    };
    InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        kind,
        root(),
        source,
        target,
    )
    .expect("install receipt")
}

fn evidence(receipt: &InstallReceipt, slot: InstallArtifactSlot) -> InstallArtifactEvidence {
    let product = if slot.is_target() {
        receipt.target_product().expect("target product")
    } else {
        receipt.source_product().expect("source product")
    };
    InstallArtifactEvidence::new(slot, product.program(slot.component()).clone())
        .expect("artifact evidence")
}

fn record_target_staging(receipt: &mut InstallReceipt) {
    receipt
        .record_artifact(evidence(receipt, InstallArtifactSlot::StagedManager))
        .expect("staged Manager");
    receipt
        .record_artifact(evidence(receipt, InstallArtifactSlot::StagedInputMethod))
        .expect("staged InputMethod");
}

fn record_source_backups(receipt: &mut InstallReceipt) {
    receipt
        .record_artifact(evidence(receipt, InstallArtifactSlot::BackupManager))
        .expect("backup Manager");
    receipt
        .record_artifact(evidence(receipt, InstallArtifactSlot::BackupInputMethod))
        .expect("backup InputMethod");
}

fn record_installed_manager(receipt: &mut InstallReceipt) {
    receipt
        .record_artifact(evidence(receipt, InstallArtifactSlot::InstalledManager))
        .expect("installed Manager");
}

fn record_installed_input_method(receipt: &mut InstallReceipt) {
    receipt
        .record_artifact(evidence(receipt, InstallArtifactSlot::InstalledInputMethod))
        .expect("installed InputMethod");
}

fn running(
    product: &ProductArtifactIdentity,
    component: ProgramComponent,
) -> RunningProgramIdentity {
    RunningProgramIdentity::new(
        product.release().clone(),
        product.product_manifest_sha256(),
        product.program(component).clone(),
    )
    .expect("running program identity")
}

#[test]
fn receipt_round_trips_only_canonical_json_without_paths() {
    let receipt = receipt(InstallOperationKind::Upgrade);
    let bytes = receipt.encode().expect("encode");
    let decoded = InstallReceipt::decode(&bytes).expect("decode");
    assert_eq!(decoded, receipt);
    assert!(bytes.ends_with(b"\n"));
    let text = String::from_utf8(bytes.clone()).expect("UTF-8");
    assert!(!text.contains("/Users/"));
    assert!(!text.contains("TeamIdentifier"));

    let mut non_canonical = bytes;
    non_canonical.insert(0, b' ');
    assert!(InstallReceipt::decode(&non_canonical).is_err());

    let changed = text.replacen("{", "{\"plaintext_user_term\":\"must-not-be-accepted\",", 1);
    assert!(InstallReceipt::decode(changed.as_bytes()).is_err());
}

#[test]
fn operation_kinds_require_exact_source_target_relationships() {
    let source = product("0.1.0", 35, 0);
    let target = product("0.1.0", 36, 1);
    assert!(InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::FirstInstall,
        root(),
        Some(source.clone()),
        Some(target.clone()),
    )
    .is_err());
    assert!(InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::Upgrade,
        root(),
        Some(target.clone()),
        Some(source.clone()),
    )
    .is_err());
    assert!(InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::Repair,
        root(),
        Some(source.clone()),
        Some(target.clone()),
    )
    .is_err());
    assert!(InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::RemovePrograms,
        root(),
        Some(source),
        Some(target),
    )
    .is_err());
}

#[test]
fn program_identity_rejects_unstable_hashes_and_component_drift() {
    assert!(ProgramBundleIdentity::new(
        ProgramComponent::Manager,
        "not-a-bundle-id",
        hash('a'),
        hash('b')
    )
    .is_err());
    assert!(ProgramBundleIdentity::new(
        ProgramComponent::Manager,
        "-invalid.example",
        hash('a'),
        hash('b')
    )
    .is_err());
    assert!(ProgramBundleIdentity::new(
        ProgramComponent::Manager,
        "org.radishlex.manager",
        "ABC",
        hash('b')
    )
    .is_err());
    let manager = bundle(
        ProgramComponent::InputMethod,
        "org.radishlex.manager",
        'a',
        'b',
    );
    let input_method = bundle(
        ProgramComponent::Manager,
        "org.radishlex.inputmethod",
        'c',
        'd',
    );
    assert!(ProductArtifactIdentity::new(
        INSTALL_PRODUCT_ID,
        ProductRelease::new("0.1.0", 35).expect("release"),
        hash('e'),
        manager,
        input_method,
    )
    .is_err());
}

#[test]
fn first_install_requires_staging_and_installed_evidence_in_order() {
    let mut receipt = receipt(InstallOperationKind::FirstInstall);
    assert!(receipt.advance(InstallState::TargetStaged).is_err());
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    record_target_staging(&mut receipt);
    receipt
        .advance(InstallState::TargetStaged)
        .expect("target staged");
    assert!(receipt.advance(InstallState::ManagerCommitted).is_err());
    record_installed_manager(&mut receipt);
    receipt
        .advance(InstallState::ManagerCommitted)
        .expect("Manager committed");
    record_installed_input_method(&mut receipt);
    for state in [
        InstallState::ProgramsCommitted,
        InstallState::FinalVerified,
        InstallState::Completed,
    ] {
        receipt.advance(state).expect("state advances");
    }
    let target = receipt.target_product().expect("target");
    assert!(evaluate_install_startup_receipt(
        &receipt,
        &running(target, ProgramComponent::Manager)
    )
    .is_allowed());
    let drifted = product("0.1.0", 35, 2);
    assert_eq!(
        evaluate_install_startup_receipt(&receipt, &running(&drifted, ProgramComponent::Manager))
            .error_code(),
        InstallStartupGateErrorCode::ProgramIdentityChanged
    );
}

#[test]
fn upgrade_data_failure_requires_source_program_rollback() {
    let mut receipt = receipt(InstallOperationKind::Upgrade);
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    record_target_staging(&mut receipt);
    receipt
        .advance(InstallState::TargetStaged)
        .expect("target staged");
    record_source_backups(&mut receipt);
    receipt
        .advance(InstallState::SourcePreserved)
        .expect("source preserved");
    record_installed_manager(&mut receipt);
    receipt
        .advance(InstallState::ManagerCommitted)
        .expect("Manager committed");
    record_installed_input_method(&mut receipt);
    receipt
        .advance(InstallState::ProgramsCommitted)
        .expect("programs committed");
    assert!(receipt.advance(InstallState::FinalVerified).is_err());
    receipt
        .advance(InstallState::DataCoordinating)
        .expect("data coordinating");
    receipt
        .require_rollback(InstallFailureCode::DataCoordinationFailed)
        .expect("rollback required");
    assert!(receipt.manual_recovery_required());
    receipt.mark_programs_restored().expect("programs restored");
    receipt.mark_rolled_back().expect("rolled back");
    let source = receipt.source_product().expect("source");
    assert!(evaluate_install_startup_receipt(
        &receipt,
        &running(source, ProgramComponent::InputMethod)
    )
    .is_allowed());
}

#[test]
fn remove_programs_has_no_target_and_blocks_a_removed_program_start() {
    let mut receipt = receipt(InstallOperationKind::RemovePrograms);
    receipt.advance(InstallState::Quiesced).expect("quiesced");
    assert!(receipt
        .record_artifact(
            InstallArtifactEvidence::new(
                InstallArtifactSlot::StagedManager,
                receipt
                    .source_product()
                    .expect("source")
                    .program(ProgramComponent::Manager)
                    .clone(),
            )
            .expect("syntactic evidence")
        )
        .is_err());
    record_source_backups(&mut receipt);
    for state in [
        InstallState::SourcePreserved,
        InstallState::ManagerCommitted,
        InstallState::ProgramsCommitted,
        InstallState::FinalVerified,
        InstallState::Completed,
    ] {
        receipt.advance(state).expect("state advances");
    }
    let source = receipt.source_product().expect("source");
    let result =
        evaluate_install_startup_receipt(&receipt, &running(source, ProgramComponent::Manager));
    assert_eq!(result.decision(), InstallStartupGateDecision::FailedClosed);
    assert_eq!(
        result.error_code(),
        InstallStartupGateErrorCode::RemovedProgram
    );
}

#[test]
fn replacement_is_single_operation_and_artifact_evidence_is_append_only() {
    let current = receipt(InstallOperationKind::FirstInstall);
    let mut quiesced = current.clone();
    quiesced.advance(InstallState::Quiesced).expect("quiesced");
    assert!(quiesced.can_replace(&current));

    let mut with_manager = quiesced.clone();
    with_manager
        .record_artifact(evidence(&with_manager, InstallArtifactSlot::StagedManager))
        .expect("staged Manager");
    assert!(with_manager.can_replace(&quiesced));
    assert!(!quiesced.can_replace(&with_manager));

    let mut different_operation = with_manager.clone();
    different_operation.operation_id = "ffeeddccbbaa99887766554433221100".to_owned();
    assert!(!different_operation.can_replace(&with_manager));
}

#[test]
fn failure_branch_depends_on_the_first_committed_program_boundary() {
    let mut before_commit = receipt(InstallOperationKind::FirstInstall);
    before_commit
        .advance(InstallState::Quiesced)
        .expect("quiesced");
    before_commit
        .abort_preserved(InstallFailureCode::StagingFailed)
        .expect("aborted");
    assert_eq!(before_commit.state(), InstallState::AbortedPreserved);
    assert!(!before_commit.manual_recovery_required());

    let mut after_commit = receipt(InstallOperationKind::FirstInstall);
    after_commit
        .advance(InstallState::Quiesced)
        .expect("quiesced");
    record_target_staging(&mut after_commit);
    after_commit
        .advance(InstallState::TargetStaged)
        .expect("staged");
    record_installed_manager(&mut after_commit);
    after_commit
        .advance(InstallState::ManagerCommitted)
        .expect("Manager committed");
    assert!(after_commit
        .abort_preserved(InstallFailureCode::InputMethodCommitFailed)
        .is_err());
    after_commit
        .require_rollback(InstallFailureCode::InputMethodCommitFailed)
        .expect("rollback required");
    assert!(after_commit.manual_recovery_required());
}

#[test]
fn decoded_receipt_rejects_artifacts_that_appear_before_their_stage() {
    let mut forged = receipt(InstallOperationKind::FirstInstall);
    forged
        .artifacts
        .push(evidence(&forged, InstallArtifactSlot::InstalledManager));
    assert!(forged.encode().is_err());
}

#[test]
fn terminal_receipt_allows_only_a_chained_operation_with_matching_source() {
    let mut completed = receipt(InstallOperationKind::FirstInstall);
    completed.advance(InstallState::Quiesced).expect("quiesced");
    record_target_staging(&mut completed);
    completed
        .advance(InstallState::TargetStaged)
        .expect("target staged");
    record_installed_manager(&mut completed);
    completed
        .advance(InstallState::ManagerCommitted)
        .expect("Manager committed");
    record_installed_input_method(&mut completed);
    for state in [
        InstallState::ProgramsCommitted,
        InstallState::FinalVerified,
        InstallState::Completed,
    ] {
        completed.advance(state).expect("state advances");
    }

    let next = InstallReceipt::new(
        "ffeeddccbbaa99887766554433221100",
        Some(completed.operation_id().to_owned()),
        InstallOperationKind::Upgrade,
        completed.root_identity().clone(),
        completed.target_product().cloned(),
        Some(product("0.2.0", 36, 2)),
    )
    .expect("next operation");
    assert!(next.can_replace(&completed));

    let wrong_previous = InstallReceipt::new(
        "ffeeddccbbaa99887766554433221100",
        Some("11111111111111111111111111111111".to_owned()),
        InstallOperationKind::Upgrade,
        completed.root_identity().clone(),
        completed.target_product().cloned(),
        Some(product("0.2.0", 36, 2)),
    )
    .expect("wrong previous operation");
    assert!(!wrong_previous.can_replace(&completed));

    let wrong_source = InstallReceipt::new(
        "ffeeddccbbaa99887766554433221100",
        Some(completed.operation_id().to_owned()),
        InstallOperationKind::Upgrade,
        completed.root_identity().clone(),
        Some(product("0.1.0", 35, 0)),
        Some(product("0.2.0", 36, 2)),
    )
    .expect("wrong source operation");
    assert!(!wrong_source.can_replace(&completed));
}
