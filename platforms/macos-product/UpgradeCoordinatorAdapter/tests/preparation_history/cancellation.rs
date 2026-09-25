use super::*;
use radishlex_ime_product_upgrade::{
    PreparationCancellationCheckpoint, PreparationCancellationPort, PreparationCancellationRequest,
    PreparationCancellationStore,
};

struct Admission(PreparationBinding);
impl PreparationCancellationPort for Admission {
    fn confirm_authority_and_quiescence(
        &mut self,
        request: &PreparationCancellationRequest,
        _: PreparationCancellationCheckpoint,
    ) -> bool {
        assert_eq!(request.preparation().binding(), &self.0);
        true // Synthetic outer authorization; private material checks are real.
    }
}

#[test]
fn cancellation_preserves_three_old_terminal_inventories_at_every_archive_position() {
    for state in [State::Completed, State::AbortedPreserved, State::RolledBack] {
        for stage in 0..3 {
            let fixture = Fixture::new();
            let (store, guard, mut port) = fixture.reserve(Some(state));
            if stage == 1 {
                port.fail = Some(Point::ArchiveSlotRecorded(Slot::Receipt));
                assert!(store
                    .archive_previous_upgrade(&guard, &mut port, &Hasher)
                    .is_err());
            } else if stage == 2 {
                store
                    .archive_previous_upgrade(&guard, &mut port, &Hasher)
                    .unwrap();
            }
            let prep = store.load_guarded(&guard).unwrap().unwrap();
            let before: Vec<_> = FILES
                .iter()
                .flat_map(|(_, name)| [fixture.state(name), fixture.history(name)])
                .filter(|path| path.exists())
                .map(|path| {
                    (
                        path.clone(),
                        fs::metadata(&path).unwrap().ino(),
                        fs::read(path).unwrap(),
                    )
                })
                .collect();
            let source = fs::read(fixture.source()).unwrap();
            let cancel = PreparationCancellationStore::attach(store, &guard).unwrap();
            let request = cancel
                .request_cancellation(&guard, NEW, &mut Admission(port.binding), &Hasher)
                .unwrap();
            assert_eq!(request.preparation(), &prep);
            assert_eq!(cancel.load_guarded(&guard, &Hasher).unwrap(), Some(request));
            for (path, inode, bytes) in &before {
                assert_eq!(fs::metadata(path).unwrap().ino(), *inode);
                assert_eq!(fs::read(path).unwrap(), *bytes);
            }
            assert_eq!(fs::read(fixture.source()).unwrap(), source);
            // A caller's authorization cannot waive drift in an old private slot.
            let path = &before
                .iter()
                .find(|(path, _, _)| path.file_name().unwrap() == "source-snapshot.sqlite3")
                .unwrap()
                .0;
            fs::write(path, b"changed old snapshot").unwrap();
            assert!(cancel.load_guarded(&guard, &Hasher).is_err());
        }
    }
}
