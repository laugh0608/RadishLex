use super::*;
use radishlex_ime_product_upgrade::{CancellationReleaseStore, VerifiedDataRoot};
use radishlex_macos_installer_driver::{
    authorize_installer_action, InstallerAction, InstallerUserAuthorization,
};
use std::fs;
use std::os::unix::fs::MetadataExt;

use crate::cancellation_fixture::Fixture;

fn query(fixture: &Fixture) -> InstallerViewSnapshot {
    let before = tree(&fixture.path);
    let result = super::super::inspect_installer_view_with_recovery(
        &fixture.path,
        fs::metadata(&fixture.path).unwrap().uid(),
        InstallerProductSituation::OlderReleaseInstalled,
    );
    assert_eq!(tree(&fixture.path), before);
    result
}

fn tree(root: &Path) -> Vec<(std::path::PathBuf, u64, u32, u64, Vec<u8>)> {
    let mut result = vec![];
    let mut remaining = vec![root.to_owned()];
    while let Some(path) = remaining.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap();
        let bytes = if metadata.is_file() {
            fs::read(&path).unwrap()
        } else {
            vec![]
        };
        result.push((
            path.strip_prefix(root).unwrap().to_owned(),
            metadata.ino(),
            metadata.mode(),
            metadata.nlink(),
            bytes,
        ));
        if metadata.is_dir() {
            remaining.extend(
                fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

struct SyntheticQualification;
impl radishlex_ime_product_upgrade::CancellationReleasePort for SyntheticQualification {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &radishlex_ime_product_upgrade::CancellationReleaseReceipt,
        _: radishlex_ime_product_upgrade::CancellationReleaseCheckpoint,
        _: radishlex_ime_product_upgrade::CancellationReleaseRequirement,
    ) -> bool {
        true
    }
}

#[test]
fn maintenance_query_overrides_old_terminal_and_requires_released_marker() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.source.product_manifest_sha256(),
        fixture.proof.preparation().binding().source_product_sha256
    );
    let view = query(&fixture);
    assert_eq!(
        view.phase(),
        InstallerViewPhase::CancellationAwaitingQualification
    );
    assert_eq!(view.receipt_state(), None);
    assert!(!view.offers(InstallerAction::RetryOperation));
    assert!(!view.offers(InstallerAction::RemovePrograms));
    assert!(authorize_installer_action(
        view,
        InstallerAction::RetryOperation,
        InstallerUserAuthorization {
            explicit_action_confirmed: true,
            data_retention_acknowledged: true,
            neutral_input_source_selected: true,
            manager_closed: true,
        }
    )
    .is_err());
    let owner = fs::metadata(&fixture.path).unwrap().uid();
    let outer = InstallReceiptStore::open_existing(
        VerifiedInstallRoot::verify(&fixture.path, owner).unwrap(),
    )
    .unwrap();
    let outer_guard = outer.acquire_guard().unwrap();
    assert!(
        require_no_maintenance(&outer).is_err(),
        "stale intent must stop under outer guard"
    );
    assert_eq!(
        query(&fixture).stable_error(),
        InstallerStableError::OperationActive
    );
    let release = CancellationReleaseStore::open_existing(
        VerifiedDataRoot::verify(&fixture.path, owner).unwrap(),
    )
    .unwrap();
    let inner_guard = release.acquire_guard().unwrap();
    release
        .finish_cancellation_release(
            &inner_guard,
            fixture.proof.operation_id(),
            &mut SyntheticQualification,
            &MacOsPreparationHasher,
        )
        .unwrap();
    drop(inner_guard);
    drop(outer_guard);
    assert_eq!(
        query(&fixture).phase(),
        InstallerViewPhase::CancellationCompleted
    );
    assert!(!query(&fixture).offers(InstallerAction::BeginUpgrade));
}

#[test]
fn maintenance_query_fails_closed_for_missing_proof_foreign_outer_and_live_inner_guard() {
    for mode in 0..3 {
        let fixture = Fixture::new();
        let owner = fs::metadata(&fixture.path).unwrap().uid();
        let mut held_guard = None;
        match mode {
            0 => {
                let proof = fixture
                    .path
                    .join(".radishlex-upgrade-history-v1")
                    .join(fixture.proof.operation_id())
                    .join("cancellation-release.json");
                fs::rename(&proof, fixture.path.join("saved-proof")).unwrap();
            }
            1 => {
                let foreign = Fixture::new();
                fs::write(
                    fixture.path.join(".radishlex-install-v1/receipt.json"),
                    fs::read(foreign.path.join(".radishlex-install-v1/receipt.json")).unwrap(),
                )
                .unwrap();
            }
            2 => {
                let release = CancellationReleaseStore::open_existing(
                    VerifiedDataRoot::verify(&fixture.path, owner).unwrap(),
                )
                .unwrap();
                held_guard = Some(release.acquire_guard().unwrap());
            }
            _ => unreachable!(),
        }
        let view = query(&fixture);
        assert_eq!(view.phase(), InstallerViewPhase::Blocked);
        assert_eq!(view.primary_action(), InstallerAction::Refresh);
        assert_eq!(view.secondary_action(), InstallerAction::None);
        assert_ne!(view.stable_error(), InstallerStableError::None);
        drop(held_guard);
    }
}
