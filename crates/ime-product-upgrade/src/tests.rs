use super::*;

fn data_root() -> UpgradeArtifactIdentity {
    UpgradeArtifactIdentity::private_directory(UpgradeArtifactSlot::DataRoot, 10, 20, 501, 7)
        .expect("data root identity")
}

fn private_file(slot: UpgradeArtifactSlot, inode: u64) -> UpgradeArtifactIdentity {
    UpgradeArtifactIdentity::private_file(slot, 10, inode, 501, 100).expect("private file identity")
}

fn receipt() -> UpgradeReceipt {
    UpgradeReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        ProductRelease::new("0.0.9", 34).expect("source release"),
        ProductRelease::new("0.1.0", 35).expect("target release"),
        Some(8),
        9,
        vec![
            data_root(),
            private_file(UpgradeArtifactSlot::SourceDatabase, 21),
        ],
    )
    .expect("receipt")
}

#[test]
fn receipt_round_trips_only_canonical_json() {
    let receipt = receipt();
    let bytes = receipt.encode().expect("receipt encodes");
    let decoded = UpgradeReceipt::decode(&bytes).expect("receipt decodes");

    assert_eq!(decoded, receipt);
    assert!(bytes.ends_with(b"\n"));
    assert!(!String::from_utf8(bytes.clone())
        .expect("receipt is UTF-8")
        .contains("/Users/"));

    let mut non_canonical = bytes;
    non_canonical.insert(0, b' ');
    assert!(UpgradeReceipt::decode(&non_canonical).is_err());
}

#[test]
fn unknown_receipt_fields_fail_closed() {
    let bytes = receipt().encode().expect("receipt encodes");
    let text = String::from_utf8(bytes).expect("receipt is UTF-8");
    let changed = text.replacen("{", "{\"plaintext_user_term\":\"must-not-be-accepted\",", 1);

    assert!(UpgradeReceipt::decode(changed.as_bytes()).is_err());
}

#[test]
fn normal_states_advance_in_order_and_terminal_state_is_closed() {
    let mut receipt = receipt();
    receipt
        .advance(UpgradeState::Quiesced)
        .expect("state advances");
    receipt
        .record_artifact(private_file(UpgradeArtifactSlot::SnapshotDatabase, 22))
        .expect("snapshot identity is recorded");
    receipt
        .advance(UpgradeState::SnapshotReady)
        .expect("state advances");
    receipt
        .record_artifact(private_file(UpgradeArtifactSlot::CandidateDatabase, 23))
        .expect("candidate identity is recorded");
    receipt
        .advance(UpgradeState::CandidateMigrated)
        .expect("state advances");
    receipt
        .advance(UpgradeState::CandidateVerified)
        .expect("state advances");
    receipt
        .record_artifact(private_file(UpgradeArtifactSlot::BackupDatabase, 24))
        .expect("backup identity is recorded");
    for state in [
        UpgradeState::SwitchPrepared,
        UpgradeState::Switched,
        UpgradeState::PostSwitchVerified,
        UpgradeState::Completed,
    ] {
        receipt.advance(state).expect("state advances");
    }

    assert_eq!(receipt.state(), UpgradeState::Completed);
    assert!(receipt.advance(UpgradeState::Quiesced).is_err());
    assert!(receipt.record_artifact(data_root()).is_err());
}

#[test]
fn states_cannot_skip_required_evidence() {
    let mut receipt = receipt();

    assert!(receipt.advance(UpgradeState::SnapshotReady).is_err());
    assert_eq!(receipt.state(), UpgradeState::Preflighted);
    receipt
        .advance(UpgradeState::Quiesced)
        .expect("quiesced state is allowed");
    assert!(receipt.advance(UpgradeState::SnapshotReady).is_err());
    assert_eq!(receipt.state(), UpgradeState::Quiesced);
    assert!(receipt
        .require_rollback(UpgradeFailureCode::PostSwitchValidationFailed)
        .is_err());
}

#[test]
fn pre_switch_failure_preserves_original_as_terminal_result() {
    let mut receipt = receipt();
    receipt
        .advance(UpgradeState::Quiesced)
        .expect("quiescence is recorded");
    receipt
        .abort_preserved(UpgradeFailureCode::SnapshotFailed, false)
        .expect("pre-switch failure is recorded");

    assert_eq!(receipt.state(), UpgradeState::AbortedPreserved);
    assert_eq!(
        receipt.failure_code(),
        Some(UpgradeFailureCode::SnapshotFailed)
    );
    assert!(!receipt.manual_recovery_required());
    assert!(receipt.advance(UpgradeState::SnapshotReady).is_err());
    UpgradeReceipt::decode(&receipt.encode().expect("receipt encodes"))
        .expect("aborted receipt remains valid");
}

#[test]
fn post_switch_failure_requires_explicit_rollback_completion() {
    let mut receipt = receipt();
    receipt
        .advance(UpgradeState::Quiesced)
        .expect("state advances");
    receipt
        .record_artifact(private_file(UpgradeArtifactSlot::SnapshotDatabase, 22))
        .expect("snapshot identity is recorded");
    receipt
        .advance(UpgradeState::SnapshotReady)
        .expect("state advances");
    receipt
        .record_artifact(private_file(UpgradeArtifactSlot::CandidateDatabase, 23))
        .expect("candidate identity is recorded");
    receipt
        .advance(UpgradeState::CandidateMigrated)
        .expect("state advances");
    receipt
        .advance(UpgradeState::CandidateVerified)
        .expect("state advances");
    receipt
        .record_artifact(private_file(UpgradeArtifactSlot::BackupDatabase, 24))
        .expect("backup identity is recorded");
    receipt
        .advance(UpgradeState::SwitchPrepared)
        .expect("state advances");
    receipt
        .advance(UpgradeState::Switched)
        .expect("state advances");
    receipt
        .require_rollback(UpgradeFailureCode::PostSwitchValidationFailed)
        .expect("rollback is required");
    assert!(receipt.manual_recovery_required());
    receipt.mark_rolled_back().expect("rollback completes");

    assert_eq!(receipt.state(), UpgradeState::RolledBack);
    assert_eq!(
        receipt.failure_code(),
        Some(UpgradeFailureCode::PostSwitchValidationFailed)
    );
    assert!(!receipt.manual_recovery_required());
}

#[test]
fn receipt_rejects_unsafe_identity_and_unstable_identifiers() {
    assert!(UpgradeArtifactIdentity::new(
        UpgradeArtifactSlot::SourceDatabase,
        10,
        20,
        501,
        0o644,
        1,
        100,
    )
    .is_err());
    assert!(UpgradeArtifactIdentity::new(
        UpgradeArtifactSlot::SourceDatabase,
        10,
        20,
        501,
        0o600,
        2,
        100,
    )
    .is_err());
    assert!(UpgradeReceipt::new(
        "NOT-A-STABLE-ID",
        None,
        ProductRelease::new("0.0.9", 34).expect("source release"),
        ProductRelease::new("0.1.0", 35).expect("target release"),
        Some(8),
        9,
        vec![
            data_root(),
            private_file(UpgradeArtifactSlot::SourceDatabase, 21),
        ],
    )
    .is_err());
}
