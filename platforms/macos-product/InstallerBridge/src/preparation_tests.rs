//! Uses real persisted records to test the selector consumed by production.
use super::*;
use crate::preparation::{authorize_task, PreparationTask};
use radishlex_ime_product_upgrade::{
    PreparationBinding, PreparationDirectoryIdentity, PreparationFileIdentity, PreparationHasher,
    PreparationInstallEvidence, PreparationJournalStore, PreparationReceipt, UpgradeReceiptStore,
    VerifiedDataRoot,
};
use radishlex_macos_installer_executor::{
    inspect_source_preparation, SourcePreparationObservation,
};
use radishlex_macos_upgrade_coordinator::MacOsPreparationHasher;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

const OLD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NEW: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn confirmation() -> InstallerUserAuthorization {
    InstallerUserAuthorization {
        explicit_action_confirmed: true,
        data_retention_acknowledged: true,
        neutral_input_source_selected: true,
        manager_closed: true,
    }
}
fn write_private(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn identity(path: &Path) -> PreparationDirectoryIdentity {
    let m = fs::symlink_metadata(path).unwrap();
    PreparationDirectoryIdentity {
        device_id: m.dev(),
        inode: m.ino(),
        owner_id: m.uid(),
        mode: m.mode() & 0o7777,
    }
}
fn previous(f: &TestFixture) -> InstallReceipt {
    let mut receipt = InstallReceipt::new(
        OLD,
        None,
        InstallOperationKind::Upgrade,
        f.store.root_identity().clone(),
        Some(product_at("26.7.1", 39, 'a')),
        Some(product_at("26.7.1", 41, 'b')),
    )
    .unwrap();
    let guard = f.store.acquire_guard().unwrap();
    f.store.persist(&guard, &receipt).unwrap();
    receipt
        .abort_preserved(radishlex_ime_product_install::InstallFailureCode::SourceArtifactInvalid)
        .unwrap();
    f.store.persist(&guard, &receipt).unwrap();
    receipt
}
fn begin(f: &TestFixture) -> SourcePreparationObservation {
    inspect_source_preparation(&f.store, &product_at("26.7.1", 41, 'b'))
        .unwrap()
        .unwrap()
}
fn reserve(f: &TestFixture, previous: &InstallReceipt) {
    let _outer = f.store.acquire_guard().unwrap();
    write_private(
        &f.data_root.join("userdb.sqlite3"),
        b"synthetic-source-physical-fixture",
    );
    let history = f.data_root.join(".radishlex-install-history-v1");
    let directory = history.join(OLD);
    create_directory(&directory, 0o700);
    let path = directory.join("receipt.json");
    let bytes = previous.encode().unwrap();
    write_private(&path, &bytes);
    let digest: String = MacOsPreparationHasher
        .sha256(&mut &bytes[..])
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let m = fs::symlink_metadata(&path).unwrap();
    let store =
        UpgradeReceiptStore::open(VerifiedDataRoot::verify(&f.data_root, f.owner_id).unwrap())
            .unwrap();
    let guard = store.acquire_guard().unwrap();
    let journal = PreparationJournalStore::attach(store, &guard).unwrap();
    let mut record = PreparationReceipt::new(
        PreparationBinding {
            operation_id: NEW.into(),
            previous_install_operation_id: Some(OLD.into()),
            previous_data_operation_id: None,
            source_release: UpgradeProductRelease::new("26.7.1", 39).unwrap(),
            target_release: UpgradeProductRelease::new("26.7.1", 41).unwrap(),
            source_product_sha256: hash('a'),
            target_product_sha256: hash('b'),
            previous_install_receipt_sha256: Some(digest.clone()),
            previous_data_receipt_sha256: None,
            previous_inventory_sha256: None,
            // Matches the existing synthetic product/schema contract in tests.rs.
            target_schema_version: 9,
            data_root: journal.data_root_identity(),
            state_directory: journal.state_directory_identity(),
        },
        journal
            .observe_source_family(&guard, &MacOsPreparationHasher)
            .unwrap(),
    )
    .unwrap();
    record
        .bind_previous_install(PreparationInstallEvidence {
            state_directory: identity(&f.data_root.join(".radishlex-install-v1")),
            history_root: identity(&history),
            previous_directory: identity(&directory),
            previous_receipt: PreparationFileIdentity {
                device_id: m.dev(),
                inode: m.ino(),
                owner_id: m.uid(),
                mode: m.mode() & 0o7777,
                byte_len: m.len(),
                link_count: m.nlink(),
                sha256: digest,
            },
        })
        .unwrap();
    journal.persist(&guard, None, &record).unwrap();
}

#[test]
fn production_task_selector_routes_real_begin_resume_and_cancellation_records() {
    let f = TestFixture::new();
    let previous = previous(&f);
    let start = begin(&f);
    assert!(matches!(
        authorize_task(
            &start,
            InstallerAction::BeginSourcePreparation,
            confirmation()
        )
        .unwrap(),
        PreparationTask::Prepare(_, _)
    ));
    for action in [
        InstallerAction::ResumeSourcePreparation,
        InstallerAction::PreservePreparationCancellation,
        InstallerAction::ContinuePreparedUpgrade,
        InstallerAction::BeginUpgrade,
    ] {
        assert!(matches!(
            authorize_task(&start, action, confirmation()),
            Err(InstallerBridgeError::Authorization(
                InstallerAuthorizationError::ActionNotOffered
            ))
        ));
    }
    reserve(&f, &previous);
    let selected = begin(&f);
    assert!(matches!(
        authorize_task(
            &selected,
            InstallerAction::ResumeSourcePreparation,
            confirmation()
        )
        .unwrap(),
        PreparationTask::Prepare(_, _)
    ));
    assert!(matches!(
        authorize_task(
            &selected,
            InstallerAction::PreservePreparationCancellation,
            confirmation()
        )
        .unwrap(),
        PreparationTask::Cancel(_, _)
    ));
    assert!(authorize_task(
        &selected,
        InstallerAction::BeginSourcePreparation,
        confirmation()
    )
    .is_err());
    for (state, action) in [
        (&start, InstallerAction::BeginSourcePreparation),
        (&selected, InstallerAction::ResumeSourcePreparation),
        (&selected, InstallerAction::PreservePreparationCancellation),
    ] {
        for flag in 0..4 {
            let mut auth = confirmation();
            match flag {
                0 => auth.explicit_action_confirmed = false,
                1 => auth.data_retention_acknowledged = false,
                2 => auth.neutral_input_source_selected = false,
                3 => auth.manager_closed = false,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    authorize_task(state, action, auth),
                    Err(InstallerBridgeError::Authorization(_))
                ),
                "{action:?} / {flag}"
            );
        }
    }
}

#[test]
fn ordinary_bootstrap_executor_rejects_dedicated_intents_without_touching_stores() {
    let mut f = TestFixture::new();
    let previous = previous(&f);
    let base = inspect_installer_view(
        &f.data_root,
        f.owner_id,
        InstallerProductSituation::OlderReleaseInstalled,
    );
    let prepared = InstallReceipt::new(
        NEW,
        Some(OLD.into()),
        InstallOperationKind::Upgrade,
        f.store.root_identity().clone(),
        Some(product_at("26.7.1", 39, 'a')),
        Some(product_at("26.7.1", 41, 'b')),
    )
    .unwrap();
    let cases = [
        (
            base.with_prepared_continuation(&prepared),
            InstallerAction::ContinuePreparedUpgrade,
        ),
        (
            base.with_source_preparation(false, false),
            InstallerAction::BeginSourcePreparation,
        ),
        (
            base.with_source_preparation(true, true),
            InstallerAction::ResumeSourcePreparation,
        ),
        (
            base.with_source_preparation(true, true),
            InstallerAction::PreservePreparationCancellation,
        ),
    ];
    // A live guard makes accidental admission observable even before a program port is touched.
    let guard = f.store.acquire_guard().unwrap();
    for (view, action) in cases {
        let intent = authorize_installer_action(view, action, confirmation()).unwrap();
        let result =
            radishlex_macos_installer_executor::execute_authorized_intent_with_upgrade_bootstrap(
                intent,
                &f.store,
                &f.data_root,
                f.owner_id,
                &mut f.programs,
                &mut f.preflight,
                &mut f.operation_ids,
                &mut UnusedUpgradePort,
            );
        assert!(
            matches!(result, Err(InstallerExecutionError::InvalidIntent)),
            "{action:?}: {result:?}"
        );
        assert_eq!(
            f.store.load_guarded(&guard).unwrap(),
            Some(previous.clone())
        );
        assert!(!f.data_root.join(".radishlex-upgrade-v1").exists());
    }
}
