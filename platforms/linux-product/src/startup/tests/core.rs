use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use super::super::read_only_state::RECEIPT_TMP_FILENAME;
use super::super::*;
use super::helper::*;
use crate::model::{
    ArtifactSlot, DpkgPackageState, LinuxFailureCode, LinuxInstallState, LinuxOperationKind,
};

#[test]
fn development_absence_is_allowed_without_any_filesystem_write() {
    let site = TestSite::new("development-absent");
    let port = FakeStartupPort::new(LinuxPackageObservation::not_installed());
    let before = site.tree_fingerprint();
    let outcome = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DevelopmentStaged,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(outcome.decision(), LinuxStartupDecision::AllowedDevelopment);
    assert_eq!(outcome.reason(), LinuxStartupReason::DevelopmentStateAbsent);
    assert_eq!(port.package_calls.get(), 1);
    assert_eq!(port.component_calls.get(), 0);
    assert_eq!(site.tree_fingerprint(), before);
}

#[test]
fn development_rejects_an_empty_system_state_root() {
    let site = TestSite::new("development-empty-system-state");
    site.create_state_root();
    let port = FakeStartupPort::new(LinuxPackageObservation::not_installed());
    let before = site.tree_fingerprint();
    let outcome = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DevelopmentStaged,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
    assert_eq!(
        outcome.reason(),
        LinuxStartupReason::DevelopmentIsolationViolation
    );
    assert_eq!(port.package_calls.get(), 1);
    assert_eq!(port.component_calls.get(), 0);
    assert_eq!(site.tree_fingerprint(), before);
}

#[test]
fn product_without_receipt_fails_and_unmanaged_package_requires_maintenance() {
    let site = TestSite::new("missing-receipt");
    let absent = FakeStartupPort::new(LinuxPackageObservation::not_installed());
    let outcome = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DebianSystemProduct,
        LinuxStartupComponent::Manager,
        &absent,
    );
    assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
    assert_eq!(outcome.reason(), LinuxStartupReason::ReceiptMissing);

    let config_files = FakeStartupPort::new(
        LinuxPackageObservation::new(DpkgPackageState::ConfigFiles, None, None)
            .expect("valid config-files observation"),
    );
    let outcome = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DevelopmentStaged,
        LinuxStartupComponent::Manager,
        &config_files,
    );
    assert_eq!(
        outcome.decision(),
        LinuxStartupDecision::MaintenanceRequired
    );
    assert_eq!(outcome.reason(), LinuxStartupReason::UnmanagedPackageState);
}

#[test]
fn terminal_install_abort_and_rollback_use_the_installed_artifact() {
    let cases = ["completed", "aborted", "rolled-back"];
    for (index, case) in cases.into_iter().enumerate() {
        let site = TestSite::new(case);
        let root_identity = site.create_state_root();
        let old = artifact("26.6.1+37-1", 0x11);
        let new = artifact("26.7.1+38-1", 0x22);
        let receipt = match case {
            "completed" => {
                let mut receipt = prepared_receipt(
                    root_identity,
                    LinuxOperationKind::Install,
                    None,
                    Some(new.clone()),
                );
                advance_to(&site, &mut receipt, LinuxInstallState::Completed);
                receipt
            }
            "aborted" => {
                let mut receipt = prepared_receipt(
                    root_identity,
                    LinuxOperationKind::Upgrade,
                    Some(old.clone()),
                    Some(new.clone()),
                );
                stage_required(&site, &mut receipt);
                receipt
                    .abort_preserved(LinuxFailureCode::ProgramsRunning)
                    .expect("abort preserved");
                receipt
            }
            "rolled-back" => {
                rolled_back_receipt(&site, root_identity, Some(old.clone()), new.clone())
            }
            _ => unreachable!(),
        };
        let installed = receipt
            .installed_artifact()
            .expect("terminal receipt has installed artifact")
            .clone();
        site.write_receipt(&receipt);
        let port = FakeStartupPort::new(LinuxPackageObservation::installed(&installed));
        let before = site.tree_fingerprint();
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            if index % 2 == 0 {
                LinuxStartupComponent::Manager
            } else {
                LinuxStartupComponent::FcitxAddon
            },
            &port,
        );
        assert_eq!(outcome.decision(), LinuxStartupDecision::AllowedProduct);
        assert_eq!(
            outcome.reason(),
            LinuxStartupReason::InstalledReceiptVerified
        );
        assert_eq!(port.component_calls.get(), 1);
        assert_eq!(site.tree_fingerprint(), before);
    }
}

#[test]
fn removed_or_failed_first_install_never_produces_a_startup_permit() {
    for case in ["remove", "aborted-install", "rolled-back-install"] {
        let site = TestSite::new(case);
        let root_identity = site.create_state_root();
        let installed = artifact("26.7.1+38-1", 0x31);
        let receipt = match case {
            "remove" => {
                let mut receipt = prepared_receipt(
                    root_identity,
                    LinuxOperationKind::Remove,
                    Some(installed.clone()),
                    None,
                );
                advance_to(&site, &mut receipt, LinuxInstallState::Completed);
                receipt
            }
            "aborted-install" => {
                let mut receipt = prepared_receipt(
                    root_identity,
                    LinuxOperationKind::Install,
                    None,
                    Some(installed.clone()),
                );
                stage_required(&site, &mut receipt);
                receipt
                    .abort_preserved(LinuxFailureCode::ProgramsRunning)
                    .expect("abort first install");
                receipt
            }
            "rolled-back-install" => {
                rolled_back_receipt(&site, root_identity, None, installed.clone())
            }
            _ => unreachable!(),
        };
        site.write_receipt(&receipt);
        let port = FakeStartupPort::new(LinuxPackageObservation::not_installed());
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::Manager,
            &port,
        );
        assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
        assert_eq!(
            outcome.reason(),
            if case == "remove" {
                LinuxStartupReason::RemovedProgram
            } else {
                LinuxStartupReason::ProductNotInstalled
            }
        );
        assert_eq!(port.component_calls.get(), 0);
    }
}

#[test]
fn every_nonterminal_state_blocks_before_package_or_component_inspection() {
    for target in [
        LinuxInstallState::Prepared,
        LinuxInstallState::ArtifactsStaged,
        LinuxInstallState::Quiesced,
        LinuxInstallState::PackageMutating,
        LinuxInstallState::PackageVerified,
    ] {
        let site = TestSite::new(&format!("nonterminal-{target:?}"));
        let root_identity = site.create_state_root();
        let target_artifact = artifact("26.7.1+38-1", 0x42);
        let mut receipt = prepared_receipt(
            root_identity,
            LinuxOperationKind::Install,
            None,
            Some(target_artifact.clone()),
        );
        advance_to(&site, &mut receipt, target);
        site.write_receipt(&receipt);
        let port = FakeStartupPort::new(LinuxPackageObservation::installed(&target_artifact));
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::FcitxAddon,
            &port,
        );
        assert_eq!(
            outcome.decision(),
            LinuxStartupDecision::MaintenanceRequired
        );
        assert_eq!(outcome.reason(), LinuxStartupReason::OperationInProgress);
        assert_eq!(port.package_calls.get(), 0);
        assert_eq!(port.component_calls.get(), 0);
    }
}

#[test]
fn guard_and_temporary_receipt_have_stable_precedence_without_cleanup() {
    let site = TestSite::new("guard-precedence");
    site.create_state_root();
    let temporary = site.paths.state_root.join(RECEIPT_TMP_FILENAME);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .expect("create temporary receipt");
    file.write_all(b"partial").expect("write temporary receipt");
    file.set_permissions(fs::Permissions::from_mode(0o400))
        .expect("simulate temporary receipt before canonical mode commit");
    fs::write(&site.paths.guard_path, b"").expect("create startup guard");
    fs::set_permissions(&site.paths.guard_path, fs::Permissions::from_mode(0o400))
        .expect("simulate guard before canonical mode commit");
    let before = site.tree_fingerprint();
    let port = FakeStartupPort::new(LinuxPackageObservation::not_installed());
    let outcome = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DebianSystemProduct,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(
        outcome.decision(),
        LinuxStartupDecision::MaintenanceRequired
    );
    assert_eq!(outcome.reason(), LinuxStartupReason::ActiveGuard);
    assert_eq!(port.package_calls.get(), 0);
    assert_eq!(site.tree_fingerprint(), before);
    fs::remove_file(&site.paths.guard_path).expect("remove test guard");

    let outcome = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DebianSystemProduct,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(outcome.reason(), LinuxStartupReason::InterruptedReceipt);
    assert!(temporary.is_file());
}

#[test]
fn malformed_guard_tmp_receipt_and_state_entries_fail_closed() {
    for case in ["guard", "tmp", "extra"] {
        let site = TestSite::new(case);
        site.create_state_root();
        match case {
            "guard" => {
                fs::write(&site.paths.guard_path, b"not an empty guard").expect("write guard")
            }
            "tmp" => {
                let path = site.paths.state_root.join(RECEIPT_TMP_FILENAME);
                fs::write(&path, b"unsafe tmp").expect("write tmp");
                fs::set_permissions(&path, fs::Permissions::from_mode(0o666))
                    .expect("set unsafe tmp mode");
            }
            "extra" => fs::write(site.paths.state_root.join("unexpected"), b"x")
                .expect("write unexpected state object"),
            _ => unreachable!(),
        }
        let port = FakeStartupPort::new(LinuxPackageObservation::not_installed());
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::Manager,
            &port,
        );
        assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
        assert_eq!(
            outcome.reason(),
            match case {
                "guard" => LinuxStartupReason::GuardInvalid,
                "tmp" => LinuxStartupReason::InterruptedReceiptInvalid,
                "extra" => LinuxStartupReason::UnexpectedStateObject,
                _ => unreachable!(),
            }
        );
        assert_eq!(port.package_calls.get(), 0);
    }
}

#[test]
fn guard_regular_file_identity_is_strictly_read_only_validated() {
    for case in ["directory", "mode", "hardlink", "size"] {
        let site = TestSite::new(&format!("guard-{case}"));
        site.create_state_root();
        match case {
            "directory" => fs::create_dir(&site.paths.guard_path).expect("create guard directory"),
            "mode" => {
                fs::write(&site.paths.guard_path, b"").expect("create guard file");
                fs::set_permissions(&site.paths.guard_path, fs::Permissions::from_mode(0o644))
                    .expect("set wide guard mode");
            }
            "hardlink" => {
                fs::write(&site.paths.guard_path, b"").expect("create guard file");
                fs::set_permissions(&site.paths.guard_path, fs::Permissions::from_mode(0o600))
                    .expect("set guard mode");
                fs::hard_link(&site.paths.guard_path, site.root.join("guard-alias"))
                    .expect("create guard hardlink");
            }
            "size" => {
                fs::write(&site.paths.guard_path, b"x").expect("create nonempty guard");
                fs::set_permissions(&site.paths.guard_path, fs::Permissions::from_mode(0o600))
                    .expect("set guard mode");
            }
            _ => unreachable!(),
        }
        let before = site.tree_fingerprint();
        let port = FakeStartupPort::new(LinuxPackageObservation::not_installed());
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::Manager,
            &port,
        );
        assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
        assert_eq!(outcome.reason(), LinuxStartupReason::GuardInvalid);
        assert_eq!(site.tree_fingerprint(), before);
        assert_eq!(port.package_calls.get(), 0);
    }
}

#[test]
fn guard_parent_uses_the_shared_lock_permission_contract() {
    let site = TestSite::new("guard-parent-permissions");
    fs::write(&site.paths.guard_path, b"").expect("create guard file");
    fs::set_permissions(&site.paths.guard_path, fs::Permissions::from_mode(0o600))
        .expect("set guard mode");
    let port = FakeStartupPort::new(LinuxPackageObservation::not_installed());

    for mode in [0o755, 0o775, 0o1777] {
        fs::set_permissions(&site.root, fs::Permissions::from_mode(mode))
            .expect("set accepted guard parent mode");
        let before = site.tree_fingerprint();
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::Manager,
            &port,
        );
        assert_eq!(
            outcome.decision(),
            LinuxStartupDecision::MaintenanceRequired
        );
        assert_eq!(outcome.reason(), LinuxStartupReason::ActiveGuard);
        assert_eq!(site.tree_fingerprint(), before);
    }

    for mode in [0o777, 0o1703, 0o1733, 0o1757] {
        fs::set_permissions(&site.root, fs::Permissions::from_mode(mode))
            .expect("set rejected guard parent mode");
        let before = site.tree_fingerprint();
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::Manager,
            &port,
        );
        assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
        assert_eq!(outcome.reason(), LinuxStartupReason::GuardInvalid);
        assert_eq!(site.tree_fingerprint(), before);
    }

    assert_eq!(port.package_calls.get(), 0);
    assert_eq!(port.component_calls.get(), 0);
}

#[test]
fn partial_unknown_and_identity_drift_never_reach_business_initialization() {
    for state in [
        DpkgPackageState::NotInstalled,
        DpkgPackageState::ConfigFiles,
        DpkgPackageState::Unpacked,
        DpkgPackageState::HalfConfigured,
        DpkgPackageState::HalfInstalled,
        DpkgPackageState::TriggersAwaited,
        DpkgPackageState::TriggersPending,
        DpkgPackageState::Unknown,
    ] {
        let site = TestSite::new(&format!("package-{state:?}"));
        let root_identity = site.create_state_root();
        let target = artifact("26.7.1+38-1", 0x51);
        let mut receipt = prepared_receipt(
            root_identity,
            LinuxOperationKind::Install,
            None,
            Some(target.clone()),
        );
        advance_to(&site, &mut receipt, LinuxInstallState::Completed);
        site.write_receipt(&receipt);
        let observation = if state.is_recoverable() {
            LinuxPackageObservation::new(
                state,
                Some(target.package_version().to_owned()),
                Some(target.architecture().to_owned()),
            )
            .expect("valid recoverable observation")
        } else {
            LinuxPackageObservation::new(state, None, None).expect("valid package observation")
        };
        let port = FakeStartupPort::new(observation);
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::Manager,
            &port,
        );
        assert_ne!(outcome.decision(), LinuxStartupDecision::AllowedProduct);
        assert_eq!(port.component_calls.get(), 0);
        if state == DpkgPackageState::Unknown {
            assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
            assert_eq!(outcome.reason(), LinuxStartupReason::PackageStateUnknown);
        } else {
            assert_eq!(
                outcome.decision(),
                LinuxStartupDecision::MaintenanceRequired
            );
        }
    }

    for error in [
        LinuxStartupPortErrorCode::PackageIdentityChanged,
        LinuxStartupPortErrorCode::ComponentIdentityChanged,
        LinuxStartupPortErrorCode::DependencyUnavailable,
    ] {
        let site = TestSite::new(&format!("component-{error:?}"));
        let root_identity = site.create_state_root();
        let target = artifact("26.7.1+38-1", 0x61);
        let mut receipt = prepared_receipt(
            root_identity,
            LinuxOperationKind::Install,
            None,
            Some(target.clone()),
        );
        advance_to(&site, &mut receipt, LinuxInstallState::Completed);
        site.write_receipt(&receipt);
        let port = FakeStartupPort::new(LinuxPackageObservation::installed(&target))
            .with_component_error(error);
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::FcitxAddon,
            &port,
        );
        assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
        assert_eq!(
            outcome.reason(),
            match error {
                LinuxStartupPortErrorCode::PackageIdentityChanged => {
                    LinuxStartupReason::PackageIdentityChanged
                }
                LinuxStartupPortErrorCode::ComponentIdentityChanged => {
                    LinuxStartupReason::ComponentIdentityChanged
                }
                LinuxStartupPortErrorCode::DependencyUnavailable => {
                    LinuxStartupReason::DependencyUnavailable
                }
                _ => unreachable!(),
            }
        );
        assert_eq!(port.relationship_calls.get(), 1);
        assert_eq!(port.component_calls.get(), 1);
    }
}

#[test]
fn package_relationship_failure_precedes_component_validation() {
    for (error, reason) in [
        (
            LinuxStartupPortErrorCode::PackageIdentityChanged,
            LinuxStartupReason::PackageIdentityChanged,
        ),
        (
            LinuxStartupPortErrorCode::DependencyUnavailable,
            LinuxStartupReason::DependencyUnavailable,
        ),
    ] {
        let site = TestSite::new(&format!("relationship-{error:?}"));
        let root_identity = site.create_state_root();
        let target = artifact("26.7.1+38-1", 0x68);
        let mut receipt = prepared_receipt(
            root_identity,
            LinuxOperationKind::Install,
            None,
            Some(target.clone()),
        );
        advance_to(&site, &mut receipt, LinuxInstallState::Completed);
        site.write_receipt(&receipt);
        let port = FakeStartupPort::new(LinuxPackageObservation::installed(&target))
            .with_relationship_error(error);
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::Manager,
            &port,
        );
        assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
        assert_eq!(outcome.reason(), reason);
        assert_eq!(port.relationship_calls.get(), 1);
        assert_eq!(port.component_calls.get(), 0);
    }
}

#[test]
fn development_build_cannot_consume_a_system_terminal_receipt() {
    let site = TestSite::new("development-isolation");
    let root_identity = site.create_state_root();
    let target = artifact("26.7.1+38-1", 0x71);
    let mut receipt = prepared_receipt(
        root_identity,
        LinuxOperationKind::Install,
        None,
        Some(target.clone()),
    );
    advance_to(&site, &mut receipt, LinuxInstallState::Completed);
    site.write_receipt(&receipt);
    let port = FakeStartupPort::new(LinuxPackageObservation::installed(&target));
    let outcome = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DevelopmentStaged,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
    assert_eq!(
        outcome.reason(),
        LinuxStartupReason::DevelopmentIsolationViolation
    );
    assert_eq!(port.package_calls.get(), 0);
    assert_eq!(port.component_calls.get(), 0);
}

#[test]
fn staged_artifact_identity_and_inventory_changes_fail_closed() {
    for case in [
        "missing",
        "tampered",
        "hardlinked",
        "replaced",
        "mode",
        "unexpected",
    ] {
        let site = TestSite::new(&format!("staged-{case}"));
        let root_identity = site.create_state_root();
        let target = artifact("26.7.1+38-1", 0x81);
        let mut receipt = prepared_receipt(
            root_identity,
            LinuxOperationKind::Install,
            None,
            Some(target.clone()),
        );
        advance_to(&site, &mut receipt, LinuxInstallState::Completed);
        site.write_receipt(&receipt);
        let (package_path, _) = staged_paths(&site, ArtifactSlot::Target);
        match case {
            "missing" => fs::remove_file(&package_path).expect("remove staged package"),
            "tampered" => fs::write(&package_path, vec![0x82; target.package_size() as usize])
                .expect("tamper staged package in place"),
            "hardlinked" => fs::hard_link(&package_path, site.root.join("staged-hardlink"))
                .expect("create staged package hardlink"),
            "replaced" => {
                let replacement = site.root.join("replacement.deb");
                write_mode(
                    &replacement,
                    &fs::read(&package_path).expect("read original staged package"),
                    0o600,
                );
                fs::remove_file(&package_path).expect("remove original staged package");
                fs::rename(replacement, &package_path).expect("replace staged package inode");
            }
            "mode" => fs::set_permissions(&package_path, fs::Permissions::from_mode(0o644))
                .expect("change staged package mode"),
            "unexpected" => write_mode(
                &package_path
                    .parent()
                    .expect("staged package parent")
                    .join("unexpected.deb"),
                b"unexpected",
                0o600,
            ),
            _ => unreachable!(),
        }

        let port = FakeStartupPort::new(LinuxPackageObservation::installed(&target));
        let before = site.tree_fingerprint();
        let outcome = inspect_linux_startup(
            &site.paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            LinuxStartupComponent::Manager,
            &port,
        );
        assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
        assert_eq!(
            outcome.reason(),
            if case == "unexpected" {
                LinuxStartupReason::UnexpectedStateObject
            } else {
                LinuxStartupReason::RootIdentityChanged
            }
        );
        assert_eq!(port.package_calls.get(), 0);
        assert_eq!(port.component_calls.get(), 0);
        assert_eq!(site.tree_fingerprint(), before);
    }
}

#[test]
fn terminal_source_rollback_material_cannot_disappear() {
    let site = TestSite::new("terminal-source-missing");
    let root_identity = site.create_state_root();
    let source = artifact("26.6.1+37-1", 0x83);
    let target = artifact("26.7.1+38-1", 0x84);
    let mut receipt = prepared_receipt(
        root_identity,
        LinuxOperationKind::Upgrade,
        Some(source),
        Some(target.clone()),
    );
    advance_to(&site, &mut receipt, LinuxInstallState::Completed);
    site.write_receipt(&receipt);
    let (_, source_evidence_path) = staged_paths(&site, ArtifactSlot::Source);
    fs::remove_file(source_evidence_path).expect("remove terminal source rollback evidence");

    let port = FakeStartupPort::new(LinuxPackageObservation::installed(&target));
    let before = site.tree_fingerprint();
    let outcome = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DebianSystemProduct,
        LinuxStartupComponent::FcitxAddon,
        &port,
    );
    assert_eq!(outcome.decision(), LinuxStartupDecision::FailedClosed);
    assert_eq!(outcome.reason(), LinuxStartupReason::RootIdentityChanged);
    assert_eq!(port.package_calls.get(), 0);
    assert_eq!(port.component_calls.get(), 0);
    assert_eq!(site.tree_fingerprint(), before);
}

#[test]
fn operation_chain_directories_are_required_and_reject_incomplete_history() {
    let site = TestSite::new("operation-chain");
    let root_identity = site.create_state_root();
    let target = artifact("26.7.1+38-1", 0x85);
    let historical_id = "00000000000000000000000000000000";
    let mut receipt = prepared_receipt_with_chain(
        root_identity,
        LinuxOperationKind::Install,
        None,
        Some(target.clone()),
        vec![
            historical_id.to_owned(),
            "11111111111111111111111111111111".to_owned(),
        ],
    );
    advance_to(&site, &mut receipt, LinuxInstallState::Completed);
    let historical_directory = site
        .paths
        .state_root
        .join(super::super::read_only_state::OPERATIONS_DIRECTORY)
        .join(historical_id);
    fs::create_dir(&historical_directory).expect("create historical operation directory");
    fs::set_permissions(&historical_directory, fs::Permissions::from_mode(0o700))
        .expect("set historical operation mode");
    write_mode(
        &historical_directory.join("target.deb"),
        b"historical",
        0o600,
    );
    write_mode(
        &historical_directory.join("target.evidence.json"),
        b"historical-evidence",
        0o600,
    );
    site.write_receipt(&receipt);

    let port = FakeStartupPort::new(LinuxPackageObservation::installed(&target));
    let allowed = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DebianSystemProduct,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(allowed.decision(), LinuxStartupDecision::AllowedProduct);

    let displaced_history = site.root.join("displaced-history");
    fs::rename(&historical_directory, &displaced_history)
        .expect("displace historical operation directory");
    let missing_before = site.tree_fingerprint();
    let missing = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DebianSystemProduct,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(missing.decision(), LinuxStartupDecision::FailedClosed);
    assert_eq!(missing.reason(), LinuxStartupReason::RootIdentityChanged);
    assert_eq!(site.tree_fingerprint(), missing_before);
    fs::rename(&displaced_history, &historical_directory)
        .expect("restore historical operation directory");

    fs::remove_file(historical_directory.join("target.evidence.json"))
        .expect("remove historical evidence pair member");
    let before = site.tree_fingerprint();
    let rejected = inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DebianSystemProduct,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(rejected.decision(), LinuxStartupDecision::FailedClosed);
    assert_eq!(rejected.reason(), LinuxStartupReason::RootIdentityChanged);
    assert_eq!(site.tree_fingerprint(), before);
}
