//! Real guarded moves and SQLite, with explicitly synthetic product/native ports.
use super::*;
use crate::preparation_execution::tests::fixture::{confirmation, Fixture, Observer, NEW};
use crate::tests::TestPrograms;
use radishlex_ime_product_install::{
    InstallArtifactSlot as Slot, ProgramSwitchAction, ProgramSwitchBoundary,
    ProgramSwitchFaultPoint,
};
use radishlex_ime_product_upgrade::*;
use radishlex_ime_userdb::{SelectionEventDraft, UserDb};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

impl ContinuationPrograms for TestPrograms {
    fn verify_material(
        &mut self,
        install: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        stores: [&ProgramSwitchStore; 2],
        r: &InstallReceipt,
    ) -> Result<()> {
        for s in stores {
            s.verify_binding(install, guard, r).map_err(switch_error)?;
            let (source, staged, source_marker, target_marker) = match s.component() {
                ProgramComponent::Manager => (
                    Slot::SourceManager,
                    Slot::StagedManager,
                    "source-manager",
                    "target-manager",
                ),
                ProgramComponent::InputMethod => (
                    Slot::SourceInputMethod,
                    Slot::StagedInputMethod,
                    "source-input-method",
                    "target-input-method",
                ),
            };
            let mut source_found = false;
            let mut target_found = false;
            for path in [
                s.target_path(),
                s.source_backup_bundle_path(),
                s.staged_bundle_path(),
            ] {
                let Ok(m) = fs::symlink_metadata(path) else {
                    continue;
                };
                let marker = fs::read_to_string(path.join("marker.txt"))
                    .map_err(|_| Error::ProductObservation)?;
                let slot = if marker == source_marker {
                    if source_found {
                        return Err(Error::ProductObservation);
                    }
                    source_found = true;
                    source
                } else if marker == target_marker {
                    if target_found {
                        return Err(Error::ProductObservation);
                    }
                    target_found = true;
                    staged
                } else {
                    return Err(Error::ProductObservation);
                };
                if let Some(e) = r.artifact(slot) {
                    let id = e.filesystem_identity();
                    if (m.dev(), m.ino(), m.uid(), m.mode() & 0o7777)
                        != (id.device_id(), id.inode(), id.owner_id(), id.mode())
                    {
                        return Err(Error::ProductObservation);
                    }
                } else if slot == staged
                    || !matches!(r.state(), InstallState::Prepared | InstallState::Quiesced)
                {
                    return Err(Error::ProductObservation);
                }
            }
            if !source_found || (r.artifact(staged).is_some() && !target_found) {
                return Err(Error::ProductObservation);
            }
        }
        Ok(())
    }
}
#[derive(Default)]
struct Native {
    seen: Vec<Point>,
    stop: Option<Point>,
    exit: bool,
    mutation: Option<(Point, Box<dyn FnOnce()>)>,
}
impl ContinuationObserver for Native {
    fn observe(&mut self, _: &PreparationReceipt, _: &InstallReceipt, p: Point) -> Result<u64> {
        self.seen.push(p);
        if self.mutation.as_ref().is_some_and(|(at, _)| *at == p) {
            (self.mutation.take().unwrap().1)();
        }
        if self.stop == Some(p) {
            if self.exit {
                std::process::exit(83);
            }
            return Err(Error::NativeObservation);
        }
        Ok(u64::MAX)
    }
}
#[derive(Debug, Clone, Copy)]
enum Outcome {
    Completed,
    Aborted,
    RolledBack,
}
struct DataPort(Outcome);
fn manager(s: i64) -> UpgradeManagerValidationEvidence {
    UpgradeManagerValidationEvidence::new(UPGRADE_VALIDATION_EVIDENCE_VERSION, s, 1, 1)
}
fn imk(s: i64) -> UpgradeInputMethodValidationEvidence {
    UpgradeInputMethodValidationEvidence::new(UPGRADE_VALIDATION_EVIDENCE_VERSION, s, 1, 1)
}
impl UpgradeCoordinatorPort for DataPort {
    fn confirm_quiescence(&mut self, _: UpgradeCoordinatorCheckpoint) -> bool {
        true
    }
    fn validate_candidate(
        &mut self,
        _: &ProductRelease,
        s: i64,
    ) -> UpgradeCandidateValidationReport {
        if matches!(self.0, Outcome::Aborted) {
            UpgradeCandidateValidationReport::manager_failed()
        } else {
            UpgradeCandidateValidationReport::passed(manager(s), imk(s))
        }
    }
    fn validate_post_switch(
        &mut self,
        _: &ProductRelease,
        s: i64,
    ) -> UpgradePostSwitchValidationReport {
        if matches!(self.0, Outcome::RolledBack) {
            UpgradePostSwitchValidationReport::manager_failed()
        } else {
            UpgradePostSwitchValidationReport::passed(manager(s), imk(s))
        }
    }
    fn validate_restored_source(
        &mut self,
        _: &ProductRelease,
        s: i64,
    ) -> Option<UpgradeRollbackValidationEvidence> {
        Some(UpgradeRollbackValidationEvidence::new(
            UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
            s,
            1,
            1,
        ))
    }
}
fn seed() -> (Fixture, TestPrograms, PreparationReceipt) {
    let f = Fixture::new();
    let p = TestPrograms::for_prepared_upgrade(&f.path, f.target.clone());
    f.run(&mut Observer::default()).unwrap();
    let prep = f.preparation();
    (f, p, prep)
}
fn initial<'a>(
    f: &Fixture,
    prep: &PreparationReceipt,
    install: &'a InstallReceiptStore,
) -> (Task<'a>, PreparationTaskControl) {
    let current = install.load_for_recovery_inspection().unwrap().unwrap();
    let (auth, control) = crate::authorize_prepared_continuation(
        &current,
        prep,
        confirmation(),
        Duration::from_secs(300),
    )
    .unwrap();
    let task = crate::preparation_continuation::bind(auth, install).unwrap();
    assert_eq!(task.data_receipt().state(), UpgradeState::Preflighted);
    let _ = f;
    (Task::from(task), control)
}
fn resumed<'a>(
    prep: &PreparationReceipt,
    install: &'a InstallReceiptStore,
) -> Result<(Task<'a>, PreparationTaskControl)> {
    let outer = install
        .load_for_recovery_inspection()?
        .ok_or(Error::InvalidBinding)?;
    let data = if install
        .data_root_path()
        .join(".radishlex-upgrade-v1/terminal-release.json")
        .exists()
        || install
            .data_root_path()
            .join(".radishlex-upgrade-history-v1")
            .join(NEW)
            .join("terminal-release.json")
            .exists()
    {
        let s = TerminalReleaseStore::open_existing(root(install)?)?;
        let g = s.acquire_guard()?;
        PreparedUpgradeData::Releasing(Box::new(s.inspect_terminal_release(
            &g,
            NEW,
            &MacOsPreparationHasher,
        )?))
    } else {
        let g = install.acquire_guard()?;
        let (_, _, r) = crate::rebind_prepared_upgrade(install, &g, prep)?;
        PreparedUpgradeData::Active(Box::new(r))
    };
    if outer.state() == InstallState::Prepared {
        return Err(Error::InvalidPhase);
    }
    let (auth, c) = authorize_prepared_upgrade_resume(
        &outer,
        prep,
        data,
        confirmation(),
        Duration::from_secs(300),
    )
    .map_err(|_| Error::TaskAuthorization)?;
    Ok((bind_resume(auth, install)?, c))
}
fn run_again(
    f: &Fixture,
    prep: &PreparationReceipt,
    install: &InstallReceiptStore,
    p: &mut TestPrograms,
    o: &mut Native,
    outcome: Outcome,
) -> Result<PreparedUpgradeSummary> {
    let current = install
        .load_for_recovery_inspection()?
        .ok_or(Error::InvalidBinding)?;
    let (task, _control) = if current.state() == InstallState::Prepared {
        let (auth, control) = crate::authorize_prepared_continuation(
            &current,
            prep,
            confirmation(),
            Duration::from_secs(300),
        )
        .map_err(|_| Error::TaskAuthorization)?;
        (
            Task::from(crate::preparation_continuation::bind(auth, install)?),
            control,
        )
    } else {
        resumed(prep, install)?
    };
    let _ = f;
    execute(task, p, o, &mut DataPort(outcome))
}
fn assert_released(f: &Fixture, s: &PreparedUpgradeSummary, outcome: Outcome) {
    let (outer, data, marker) = match outcome {
        Outcome::Completed => (
            InstallState::Completed,
            UpgradeState::Completed,
            "target-manager",
        ),
        Outcome::Aborted => (
            InstallState::RolledBack,
            UpgradeState::AbortedPreserved,
            "source-manager",
        ),
        Outcome::RolledBack => (
            InstallState::RolledBack,
            UpgradeState::RolledBack,
            "source-manager",
        ),
    };
    assert_eq!(s.install_receipt().state(), outer);
    assert_eq!(s.data_receipt().state(), data);
    assert_eq!(s.install_receipt().operation_id(), NEW);
    assert_eq!(
        fs::read_to_string(
            f.path
                .join("synthetic-programs/Applications/RadishLex Manager.app/marker.txt")
        )
        .unwrap(),
        marker
    );
    let active = f.path.join(".radishlex-upgrade-v1");
    assert!(fs::read_dir(&active).unwrap().next().is_none());
    let store = UpgradeReceiptStore::open_existing(root(&f.install()).unwrap()).unwrap();
    assert_eq!(store.load().unwrap(), None);
    let mut db = UserDb::open(f.path.join("userdb.sqlite3")).unwrap();
    db.record_selection(SelectionEventDraft::new(
        "synthetic-after-release",
        "xin",
        "新",
        0,
        8,
    ))
    .unwrap();
    db.delete_term("xin", "新", None).unwrap();
    drop(db);
    assert_eq!(
        store.load().unwrap(),
        None,
        "normal WAL must not be judged against historical v1 lengths"
    );
}

#[test]
fn complete_abort_and_rollback_all_preserve_history_and_release_last() {
    for outcome in [Outcome::Completed, Outcome::Aborted, Outcome::RolledBack] {
        let (f, mut p, prep) = seed();
        let install = f.install();
        let old = fs::read(
            f.path
                .join(".radishlex-install-history-v1")
                .join(
                    prep.binding()
                        .previous_install_operation_id
                        .as_ref()
                        .unwrap(),
                )
                .join("receipt.json"),
        )
        .unwrap();
        let snapshot = fs::read(
            f.path
                .join(".radishlex-upgrade-history-v1")
                .join(NEW)
                .join("preparation-snapshot.sqlite3"),
        )
        .unwrap();
        let mut observer = Native::default();
        let summary = run_again(&f, &prep, &install, &mut p, &mut observer, outcome)
            .unwrap_or_else(|e| {
                panic!(
                    "{outcome:?}: {e}; last points {:?}",
                    &observer.seen[observer.seen.len().saturating_sub(8)..]
                )
            });
        assert_released(&f, &summary, outcome);
        assert_eq!(
            fs::read(
                f.path
                    .join(".radishlex-install-history-v1")
                    .join(
                        prep.binding()
                            .previous_install_operation_id
                            .as_ref()
                            .unwrap()
                    )
                    .join("receipt.json")
            )
            .unwrap(),
            old
        );
        assert_eq!(
            fs::read(
                f.path
                    .join(".radishlex-upgrade-history-v1")
                    .join(NEW)
                    .join("preparation-snapshot.sqlite3")
            )
            .unwrap(),
            snapshot
        );
        assert_eq!(observer.seen.last(), Some(&Point::Finished));
    }
}

#[test]
fn program_data_and_release_restarts_require_fresh_exact_authorization() {
    let points = [
        Point::Programs(InstallState::TargetStaged),
        Point::Programs(InstallState::SourcePreserved),
        Point::Programs(InstallState::ManagerCommitted),
        Point::Programs(InstallState::ProgramsCommitted),
        Point::DataCheckpoint(UpgradeCoordinatorCheckpoint::BeforeCandidateValidation),
        Point::DataCheckpoint(UpgradeCoordinatorCheckpoint::BeforePostSwitchValidation),
        Point::DataSettled,
        Point::Release(TerminalReleaseCheckpoint::Moved(
            PreparationArchiveSlot::Receipt,
        )),
        Point::BeforeOuterTerminal,
        Point::Release(TerminalReleaseCheckpoint::BeforeMarkerMove),
        Point::Release(TerminalReleaseCheckpoint::MarkerMoved),
    ];
    for stop in points {
        let (f, mut p, prep) = seed();
        let install = f.install();
        let result = run_again(
            &f,
            &prep,
            &install,
            &mut p,
            &mut Native {
                stop: Some(stop),
                ..Native::default()
            },
            Outcome::Completed,
        );
        assert!(result.is_err(), "{stop:?}");
        let mut reopened = TestPrograms::reopen_prepared_upgrade(&f.path, f.target.clone());
        let summary = run_again(
            &f,
            &prep,
            &install,
            &mut reopened,
            &mut Native::default(),
            Outcome::Completed,
        )
        .unwrap_or_else(|e| panic!("{stop:?}: {e}"));
        assert_released(&f, &summary, Outcome::Completed);
    }
}

#[test]
fn revocation_and_callback_inode_replacement_stop_before_further_mutation() {
    for stop in [
        Point::Admission,
        Point::Staged(ProgramComponent::Manager),
        Point::Commit(ProgramComponent::Manager),
        Point::Candidate,
        Point::BeforeOuterTerminal,
        Point::Release(TerminalReleaseCheckpoint::BeforeMarkerMove),
    ] {
        let (f, mut p, prep) = seed();
        let install = f.install();
        let (task, control) = initial(&f, &prep, &install);
        let native = Native {
            mutation: Some((stop, Box::new(move || control.revoke()))),
            ..Native::default()
        };
        let result = execute(
            task,
            &mut p,
            &mut { native },
            &mut DataPort(Outcome::Completed),
        );
        assert_eq!(result, Err(Error::TaskAuthorization), "{stop:?}");
    }
    for which in 0..3 {
        let (f, mut p, prep) = seed();
        let install = f.install();
        let (task, _control) = initial(&f, &prep, &install);
        let path = if which == 0 {
            f.path.join(".radishlex-install-v1/receipt.json")
        } else if which == 1 {
            f.path.join(".radishlex-upgrade-v1/receipt.json")
        } else {
            f.path
                .join(".radishlex-upgrade-history-v1")
                .join(NEW)
                .join("preparation.json")
        };
        let retained = f.path.join("retained-synthetic-original");
        let mut native = Native {
            mutation: Some((
                Point::Stage(ProgramComponent::Manager),
                Box::new(move || {
                    let bytes = fs::read(&path).unwrap();
                    fs::rename(&path, &retained).unwrap();
                    fs::write(&path, bytes).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                }),
            )),
            ..Native::default()
        };
        assert!(execute(task, &mut p, &mut native, &mut DataPort(Outcome::Completed)).is_err());
    }
}

#[test]
fn equal_product_unbound_staging_and_unknown_material_are_preserved_and_blocked() {
    for unknown in ["staged.app", "unknown.tmp"] {
        let (f, mut p, prep) = seed();
        let install = f.install();
        let stop = Point::Stage(ProgramComponent::Manager);
        assert!(run_again(
            &f,
            &prep,
            &install,
            &mut p,
            &mut Native {
                stop: Some(stop),
                ..Native::default()
            },
            Outcome::Completed
        )
        .is_err());
        let path = p
            .target_path(ProgramComponent::Manager)
            .parent()
            .unwrap()
            .join(format!(".radishlex-install-{NEW}"))
            .join(unknown);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("marker.txt"), "target-manager").unwrap();
        assert!(run_again(
            &f,
            &prep,
            &install,
            &mut p,
            &mut Native::default(),
            Outcome::Completed
        )
        .is_err());
        assert!(path.is_dir());
    }
}

#[path = "prepared_fault_tests.rs"]
mod faults;
#[path = "prepared_projection_tests.rs"]
mod projection;

const EXIT_POINTS: [Point; 5] = [
    Point::Switch(
        ProgramComponent::Manager,
        ProgramSwitchFaultPoint::new(
            ProgramSwitchAction::PreserveSource,
            ProgramSwitchBoundary::AfterRename,
        ),
    ),
    Point::Switch(
        ProgramComponent::Manager,
        ProgramSwitchFaultPoint::new(
            ProgramSwitchAction::CommitTarget,
            ProgramSwitchBoundary::AfterRename,
        ),
    ),
    Point::DataCheckpoint(UpgradeCoordinatorCheckpoint::BeforePostSwitchValidation),
    Point::Release(TerminalReleaseCheckpoint::Moved(
        PreparationArchiveSlot::Receipt,
    )),
    Point::Release(TerminalReleaseCheckpoint::MarkerMoved),
];
#[test]
fn exited_process_reopens_moved_programs_data_and_last_marker() {
    for (index, point) in EXIT_POINTS.iter().enumerate() {
        let (f, mut p, prep) = seed();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "prepared_execution::tests::prepared_exit_child",
                "--ignored",
                "--nocapture",
            ])
            .env("RADISHLEX_PREPARED_TEST_ROOT", &f.path)
            .env("RADISHLEX_PREPARED_TEST_POINT", index.to_string())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(83),
            "{point:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let install = f.install();
        let summary = run_again(
            &f,
            &prep,
            &install,
            &mut p,
            &mut Native::default(),
            Outcome::Completed,
        )
        .unwrap_or_else(|e| panic!("{point:?}: {e}"));
        assert_released(&f, &summary, Outcome::Completed);
    }
}
#[test]
#[ignore = "invoked by the prepared upgrade process-exit matrix"]
fn prepared_exit_child() {
    let f = Fixture::reopen(
        std::env::var_os("RADISHLEX_PREPARED_TEST_ROOT")
            .unwrap()
            .into(),
    );
    let prep = f.preparation();
    let install = f.install();
    let mut p = TestPrograms::reopen_prepared_upgrade(&f.path, f.target.clone());
    let index: usize = std::env::var("RADISHLEX_PREPARED_TEST_POINT")
        .unwrap()
        .parse()
        .unwrap();
    run_again(
        &f,
        &prep,
        &install,
        &mut p,
        &mut Native {
            stop: Some(EXIT_POINTS[index]),
            exit: true,
            ..Native::default()
        },
        Outcome::Completed,
    )
    .unwrap();
    panic!("exit checkpoint not reached");
}
