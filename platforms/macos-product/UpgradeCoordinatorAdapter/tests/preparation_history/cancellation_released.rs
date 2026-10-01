use super::*;
use radishlex_ime_product_upgrade::{
    CancellationArchiveCheckpoint, CancellationArchivePort, CancellationArchiveReceipt,
    CancellationArchiveSlot, CancellationArchiveStore, CancellationSourceCheckpoint,
    CancellationSourcePort, CancellationSourceReceipt, PreparationCancellationCheckpoint,
    PreparationCancellationPort, PreparationCancellationRequest, PreparationCancellationStore,
};
use sha2::{Digest, Sha256};

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
impl CancellationArchivePort for Admission {
    fn confirm_authority_and_quiescence(
        &mut self,
        _: &CancellationArchiveReceipt,
        _: CancellationArchiveCheckpoint,
    ) -> bool {
        true
    }
}

#[test]
fn cancellation_archive_keeps_v1_and_v2_released_history_and_index_unchanged() {
    for legacy in [true, false] {
        for state in [State::Completed, State::AbortedPreserved, State::RolledBack] {
            let fixture = Fixture::new();
            let prior = if legacy {
                lifecycle::legacy_release(&fixture, state).0
            } else {
                released(&fixture, state)
            };
            let history = fixture.0.join(HISTORY);
            let before = failures::tree(&history);
            let outer_history = fixture.0.join(".radishlex-install-history-v1");
            let old = outer_history.join(OUTER);
            let active = fixture.0.join(".radishlex-install-v1");
            for path in [&outer_history, &old, &active] {
                DirBuilder::new().mode(0o700).create(path).unwrap();
            }
            let old_bytes = b"{\"synthetic_outer\":\"released_terminal\"}\n";
            put(&old.join("receipt.json"), old_bytes);
            put(
                &active.join("receipt.json"),
                b"{\"synthetic_outer\":\"new_prepared\"}\n",
            );
            let (store, guard, _) = reserve_with_outer_digest(
                &fixture,
                &prior,
                NEXT,
                format!("{:x}", Sha256::digest(old_bytes)),
            );
            let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
            cancel
                .request_cancellation(&guard, NEXT, &mut Admission, &Hasher)
                .unwrap();
            cancel
                .finish_source(&guard, &mut Admission, &Hasher)
                .unwrap();
            drop(guard);
            let archive = CancellationArchiveStore::open_existing(fixture.verified()).unwrap();
            let guard = archive.acquire_guard().unwrap();
            let outer = archive.observe_outer(&guard, &Hasher).unwrap();
            let proof = archive
                .preserve_cancellation(&guard, &outer, &mut Admission, &Hasher)
                .unwrap();
            assert!(proof.preserved());
            assert!(!proof
                .moved_slots()
                .iter()
                .any(|slot| matches!(slot, CancellationArchiveSlot::Previous(_))));
            let after = failures::tree(&history);
            for evidence in before {
                assert!(after.contains(&evidence));
            }
            assert_eq!(fs::read(active.join("receipt.json")).unwrap(), old_bytes);
            let index = history.join("latest-release.json");
            let bytes = fs::read(&index).unwrap();
            fs::rename(&index, fixture.0.join("preserved-index")).unwrap();
            put(&index, &bytes);
            assert!(archive.load_guarded(&guard, &Hasher).is_err());
        }
    }
}
