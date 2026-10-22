//! Read-only projection covers active v1 and partially archived terminal slots.
use super::*;
use radishlex_macos_installer_driver::{
    InstallerAction, InstallerProductSituation, InstallerStableError,
};
use std::path::{Path, PathBuf};

type Entry = (PathBuf, u64, u64, u32, u32, Vec<u8>);
fn tree(root: &Path) -> Vec<Entry> {
    fn collect(root: &Path, path: &Path, out: &mut Vec<Entry>) {
        let m = fs::symlink_metadata(path).unwrap();
        out.push((
            path.strip_prefix(root).unwrap().to_path_buf(),
            m.dev(),
            m.ino(),
            m.uid(),
            m.mode(),
            if m.is_file() {
                fs::read(path).unwrap()
            } else {
                vec![]
            },
        ));
        if m.is_dir() {
            for e in fs::read_dir(path).unwrap() {
                collect(root, &e.unwrap().path(), out);
            }
        }
    }
    let mut out = vec![];
    collect(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
#[test]
fn active_and_partially_released_views_offer_only_the_special_fresh_action_without_writes() {
    for (outcome, point) in [
        (Outcome::Completed, Point::Admission),
        (
            Outcome::Completed,
            Point::Programs(InstallState::ManagerCommitted),
        ),
        (
            Outcome::Completed,
            Point::DataCheckpoint(UpgradeCoordinatorCheckpoint::BeforePostSwitchValidation),
        ),
        (
            Outcome::Completed,
            Point::Release(TerminalReleaseCheckpoint::Moved(
                PreparationArchiveSlot::Receipt,
            )),
        ),
        (Outcome::Completed, Point::BeforeOuterTerminal),
        (
            Outcome::Completed,
            Point::Release(TerminalReleaseCheckpoint::BeforeMarkerMove),
        ),
        (
            Outcome::Aborted,
            Point::Restore(ProgramComponent::InputMethod),
        ),
        (
            Outcome::RolledBack,
            Point::Programs(InstallState::ProgramsRestored),
        ),
    ] {
        let (f, mut p, prep) = seed();
        let install = f.install();
        run_again(
            &f,
            &prep,
            &install,
            &mut p,
            &mut Native {
                stop: Some(point),
                ..Native::default()
            },
            outcome,
        )
        .unwrap_err();
        let before = tree(&f.path);
        let state = inspect_prepared_upgrade_continuation(&install)
            .unwrap()
            .unwrap();
        assert_eq!(state.preparation(), &prep);
        let view = crate::inspect_installer_view_with_recovery(
            &f.path,
            f.owner(),
            InstallerProductSituation::IdentityUnavailable,
        );
        assert_eq!(
            view.primary_action(),
            InstallerAction::ContinuePreparedUpgrade,
            "{outcome:?}: {point:?}"
        );
        assert!(!view.offers(InstallerAction::ResumeOperation));
        assert!(!view.offers(InstallerAction::RetryOperation));
        assert!(!view.offers(InstallerAction::RemovePrograms));
        assert_eq!(tree(&f.path), before);
    }
}
#[test]
fn held_task_and_unknown_release_temporaries_block_projection_without_creating_or_cleaning() {
    let (f, _p, prep) = seed();
    let install = f.install();
    let (task, _c) = initial(&f, &prep, &install);
    let before = tree(&f.path);
    assert!(inspect_prepared_upgrade_continuation(&install).is_err());
    let view = crate::inspect_installer_view_with_recovery(
        &f.path,
        f.owner(),
        InstallerProductSituation::IdentityUnavailable,
    );
    assert_eq!(view.stable_error(), InstallerStableError::OperationActive);
    assert_eq!(tree(&f.path), before);
    drop(task);
    let (f, mut p, prep) = seed();
    let install = f.install();
    run_again(
        &f,
        &prep,
        &install,
        &mut p,
        &mut Native {
            stop: Some(Point::Release(TerminalReleaseCheckpoint::RecordCreated)),
            ..Native::default()
        },
        Outcome::Completed,
    )
    .unwrap_err();
    let before = tree(&f.path);
    assert!(inspect_prepared_upgrade_continuation(&install).is_err());
    assert_eq!(
        crate::inspect_installer_view_with_recovery(
            &f.path,
            f.owner(),
            InstallerProductSituation::IdentityUnavailable
        )
        .primary_action(),
        InstallerAction::Refresh
    );
    assert_eq!(tree(&f.path), before);
}
#[test]
fn ready_release_marker_inode_substitution_is_rejected_before_outer_terminal() {
    let (f, mut p, prep) = seed();
    let install = f.install();
    let path = f.path.join(".radishlex-upgrade-v1/terminal-release.json");
    let retained = f.path.join("retained-synthetic-release");
    let mut observer = Native {
        mutation: Some((
            Point::BeforeOuterTerminal,
            Box::new(move || {
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, &retained).unwrap();
                fs::write(&path, bytes).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            }),
        )),
        ..Native::default()
    };
    assert_eq!(
        run_again(
            &f,
            &prep,
            &install,
            &mut p,
            &mut observer,
            Outcome::Completed
        ),
        Err(Error::InvalidBinding)
    );
    assert_eq!(
        install
            .load_for_recovery_inspection()
            .unwrap()
            .unwrap()
            .state(),
        InstallState::DataSettled
    );
    assert!(f.path.join("retained-synthetic-release").exists());
}
