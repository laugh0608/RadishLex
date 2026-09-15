use super::*;
use crate::{UpgradeArtifactIdentity, UpgradeArtifactSlot};

fn digest(byte: char) -> String {
    std::iter::repeat(byte).take(64).collect()
}
fn file(inode: u64) -> PreparationFileIdentity {
    PreparationFileIdentity {
        device_id: 1,
        inode,
        owner_id: 501,
        mode: 0o600,
        link_count: 1,
        byte_len: 4096,
        sha256: digest('a'),
    }
}
fn fixture() -> PreparationReceipt {
    PreparationReceipt::new(
        PreparationBinding {
            operation_id: "11111111111111111111111111111111".to_owned(),
            previous_install_operation_id: Some("22222222222222222222222222222222".to_owned()),
            previous_data_operation_id: Some("33333333333333333333333333333333".to_owned()),
            source_release: ProductRelease::new("26.7.1", 39).unwrap(),
            target_release: ProductRelease::new("26.7.1", 41).unwrap(),
            source_product_sha256: digest('a'),
            target_product_sha256: digest('b'),
            previous_install_receipt_sha256: Some(digest('c')),
            previous_data_receipt_sha256: Some(digest('d')),
            previous_inventory_sha256: Some(digest('e')),
            target_schema_version: 9,
            data_root: PreparationDirectoryIdentity {
                device_id: 1,
                inode: 10,
                owner_id: 501,
                mode: 0o700,
            },
            state_directory: PreparationDirectoryIdentity {
                device_id: 1,
                inode: 11,
                owner_id: 501,
                mode: 0o700,
            },
        },
        PreparationFamily {
            database: file(20),
            wal: Some(file(21)),
            shm: None,
            journal: None,
        },
    )
    .unwrap()
}
fn snapshot_ready() -> PreparationReceipt {
    let mut receipt = fixture();
    receipt
        .record_snapshot(file(22), receipt.initial_source.clone(), 9)
        .unwrap();
    receipt
}
fn prepared() -> PreparationReceipt {
    let mut receipt = snapshot_ready();
    receipt.begin_maintenance().unwrap();
    let mut source = file(20);
    source.byte_len = 8192;
    source.sha256 = digest('f');
    receipt.record_prepared_source(source).unwrap();
    receipt
}
fn upgrade(receipt: &PreparationReceipt) -> UpgradeReceipt {
    let binding = receipt.binding();
    let source = receipt.prepared_source().unwrap();
    let root = &binding.data_root;
    UpgradeReceipt::new(
        binding.operation_id.clone(),
        binding.previous_data_operation_id.clone(),
        binding.source_release.clone(),
        binding.target_release.clone(),
        Some(9),
        9,
        vec![
            UpgradeArtifactIdentity::private_directory(
                UpgradeArtifactSlot::DataRoot,
                root.device_id,
                root.inode,
                root.owner_id,
                2,
            )
            .unwrap(),
            UpgradeArtifactIdentity::private_file(
                UpgradeArtifactSlot::SourceDatabase,
                source.device_id,
                source.inode,
                source.owner_id,
                source.byte_len,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn phase_chain_preserves_original_identity_and_binds_distinct_predecessors() {
    let mut receipt = fixture();
    let original = receipt.clone();
    receipt
        .record_snapshot(file(22), receipt.initial_source.clone(), 9)
        .unwrap();
    receipt.begin_maintenance().unwrap();
    let mut source = file(20);
    source.byte_len = 8192;
    source.sha256 = digest('f');
    receipt.record_prepared_source(source.clone()).unwrap();
    receipt.record_previous_archive(digest('e')).unwrap();
    receipt
        .record_handoff(&upgrade(&receipt), digest('f'))
        .unwrap();
    assert_eq!(receipt.phase(), PreparationPhase::HandoffReady);
    assert_eq!(receipt.initial_source(), original.initial_source());
    assert_eq!(receipt.prepared_source(), Some(&source));
    assert_ne!(
        receipt.binding().previous_data_operation_id,
        receipt.binding().previous_install_operation_id
    );
    assert_eq!(
        PreparationReceipt::decode(&receipt.encode().unwrap()).unwrap(),
        receipt
    );
    assert!(crate::UpgradeReceipt::decode(&receipt.encode().unwrap()).is_err());
    let before = receipt.clone();
    assert_eq!(receipt.begin_maintenance(), Err(Error::InvalidPhase));
    assert_eq!(receipt, before);
}

#[test]
fn identity_and_content_changes_outside_maintenance_are_rejected_atomically() {
    for mutate in 0..6 {
        let mut receipt = fixture();
        let before = receipt.clone();
        let mut family = receipt.initial_source.clone();
        let mut snapshot = file(22);
        match mutate {
            0 => family.database.sha256 = digest('b'),
            1 => family.wal.as_mut().unwrap().byte_len += 1,
            2 => family.database.inode += 100,
            3 => snapshot = family.wal.clone().unwrap(),
            4 => snapshot.mode = 0o644,
            _ => snapshot.link_count = 2,
        }
        assert!(receipt.record_snapshot(snapshot, family, 9).is_err());
        assert_eq!(receipt, before);
    }
    let mut receipt = snapshot_ready();
    receipt.begin_maintenance().unwrap();
    let before = receipt.clone();
    let mut changed = file(20);
    changed.inode = 999;
    assert!(receipt.record_prepared_source(changed).is_err());
    assert_eq!(receipt, before);
}

#[test]
fn only_controlled_auxiliary_creation_is_accepted_after_read_only_snapshot() {
    let mut receipt = fixture();
    receipt.initial_source.wal = None;
    let mut family = receipt.initial_source.clone();
    family.shm = Some(file(23));
    let mut wal = file(21);
    wal.byte_len = 0;
    family.wal = Some(wal);
    receipt.record_snapshot(file(22), family, 9).unwrap();
    let mut receipt = fixture();
    receipt.initial_source.wal = None;
    let mut family = receipt.initial_source.clone();
    family.wal = Some(file(21));
    assert_eq!(
        receipt.record_snapshot(file(22), family, 9),
        Err(Error::InvalidIdentity)
    );
}

#[test]
fn canonical_decode_rejects_unknown_duplicate_truncated_or_inconsistent_fields() {
    let receipt = prepared();
    let bytes = receipt.encode().unwrap();
    let canonical = String::from_utf8(bytes.clone()).unwrap();
    for text in [
        canonical.replace(
            "radishlex-source-preparation-v1",
            "radishlex-source-preparation-v2",
        ),
        canonical.replacen("{", "{\"unknown\":true,", 1),
        canonical.replacen("{", "{\"phase\":\"source_prepared\",", 1),
        canonical.replace("\"mode\":384", "\"mode\":420"),
        canonical.replace("\"source_prepared\"", "\"handoff_ready\""),
        canonical.replace("\"source_prepared\"", "\"reserved\""),
        format!(" {canonical}"),
        canonical.trim_end().to_owned(),
        canonical.replace("\"owner_id\":501", "\"owner_id\":-1"),
    ] {
        assert!(PreparationReceipt::decode(text.as_bytes()).is_err());
    }
    assert!(PreparationReceipt::decode(&vec![b' '; MAX_PREPARATION_RECEIPT_BYTES + 1]).is_err());
}

#[test]
fn replacement_rejects_skipped_phases_cross_operation_and_evidence_rewrites() {
    let before = fixture();
    let snapshot = snapshot_ready();
    assert!(snapshot.can_replace(&before));
    assert!(!prepared().can_replace(&before));
    let mut next = snapshot.clone();
    next.begin_maintenance().unwrap();
    assert!(next.can_replace(&snapshot));
    for mutate in 0..4 {
        let mut drift = next.clone();
        match mutate {
            0 => drift.binding.operation_id = "44444444444444444444444444444444".to_owned(),
            1 => drift.snapshot.as_mut().unwrap().identity.sha256 = digest('f'),
            2 => drift.binding.state_directory.inode += 1,
            _ => drift.binding.previous_inventory_sha256 = Some(digest('f')),
        }
        assert!(!drift.can_replace(&snapshot));
    }
}

#[test]
fn handoff_requires_prepared_source_and_exact_v1_binding() {
    let mut receipt = prepared();
    receipt.record_previous_archive(digest('e')).unwrap();
    let correct = upgrade(&receipt);
    for mutate in 0..4 {
        let mut json = serde_json::to_value(&correct).unwrap();
        match mutate {
            0 => json["operation_id"] = "44444444444444444444444444444444".into(),
            1 => json["previous_operation_id"] = serde_json::Value::Null,
            2 => json["source_release"]["build_number"] = 38.into(),
            _ => json["source_schema_version"] = 8.into(),
        }
        // Re-encode in the receipt's canonical field order.
        let wrong: UpgradeReceipt = serde_json::from_value(json).unwrap();
        let wrong = UpgradeReceipt::decode(&wrong.encode().unwrap()).unwrap();
        let before = receipt.clone();
        assert_eq!(
            receipt.record_handoff(&wrong, digest('f')),
            Err(Error::InvalidBinding)
        );
        assert_eq!(receipt, before);
    }
    receipt.record_handoff(&correct, digest('f')).unwrap();
}

#[test]
fn first_data_upgrade_requires_explicit_empty_predecessor_evidence() {
    let mut binding = fixture().binding;
    binding.previous_data_operation_id = None;
    assert!(PreparationReceipt::new(binding.clone(), fixture().initial_source).is_err());
    binding.previous_data_receipt_sha256 = None;
    binding.previous_inventory_sha256 = None;
    assert!(PreparationReceipt::new(binding, fixture().initial_source).is_ok());
}
