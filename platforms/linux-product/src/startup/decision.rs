use super::read_only_state::same_file_identity;
use super::record::{temporary_record_present, StartupRecord};
use crate::model::{DpkgPackageState, LinuxInstallReceipt, LinuxInstallState, LinuxOperationKind};
use std::os::unix::fs::MetadataExt;
use std::{fs, io};

use super::read_only_state::{inspect_guard, ReadOnlyStateView};
use super::types::{
    LinuxPackageObservation, LinuxStartupBuildIdentity, LinuxStartupComponent,
    LinuxStartupDecision, LinuxStartupOutcome, LinuxStartupPaths, LinuxStartupPort,
    LinuxStartupPortErrorCode, LinuxStartupReason,
};

pub fn inspect_linux_startup(
    paths: &LinuxStartupPaths,
    build_identity: LinuxStartupBuildIdentity,
    component: LinuxStartupComponent,
    port: &impl LinuxStartupPort,
) -> LinuxStartupOutcome {
    let denied =
        |reason| LinuxStartupOutcome::new(LinuxStartupDecision::FailedClosed, reason, None);
    let before = match input_identities(paths) {
        Ok(values) => values,
        Err(reason) => return denied(reason),
    };
    let result = inspect_inputs(paths, build_identity, component, port);
    if !matches!(
        result.decision(),
        LinuxStartupDecision::AllowedProduct | LinuxStartupDecision::AllowedDevelopment
    ) {
        return result;
    }
    // A process maps its component before entering this gate. Once admitted it is
    // visible to the writer's guarded quiescence check before any package mutation.
    if let Some(blocked) = inspect_guard(paths) {
        return blocked;
    }
    let final_state = match ReadOnlyStateView::inspect(paths) {
        Ok(view) => view,
        Err(error) => return error.outcome(None),
    };
    if let Err(error) = final_state.read_receipt(paths) {
        return error.outcome(None);
    }
    let after = match input_identities(paths) {
        Ok(values) => values,
        Err(reason) => return denied(reason),
    };
    if before
        .iter()
        .zip(&after)
        .all(|(old, new)| unchanged(old.as_ref(), new.as_ref()))
    {
        result
    } else {
        denied(LinuxStartupReason::RootIdentityChanged)
    }
}

fn unchanged(old: Option<&fs::Metadata>, new: Option<&fs::Metadata>) -> bool {
    match (old, new) {
        (None, None) => true,
        (Some(old), Some(new)) => {
            same_file_identity(old, new)
                && old.ctime() == new.ctime()
                && old.ctime_nsec() == new.ctime_nsec()
        }
        _ => false,
    }
}

fn input_identities(
    paths: &LinuxStartupPaths,
) -> Result<Vec<Option<fs::Metadata>>, LinuxStartupReason> {
    [
        paths.state_root.clone(),
        paths.state_root.join("operations"),
        paths.state_root.join("receipt.json"),
        paths.state_root.join("receipt.json.tmp"),
        paths.record_path(),
        paths.record_tmp(),
        paths.dpkg_status_path.clone(),
    ]
    .iter()
    .map(|p| match fs::symlink_metadata(p) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            Err(LinuxStartupReason::PermissionDenied)
        }
        Err(_) => Err(LinuxStartupReason::Io),
    })
    .collect()
}

fn inspect_inputs(
    paths: &LinuxStartupPaths,
    build_identity: LinuxStartupBuildIdentity,
    component: LinuxStartupComponent,
    port: &impl LinuxStartupPort,
) -> LinuxStartupOutcome {
    let state_view = match ReadOnlyStateView::inspect(paths) {
        Ok(view) => view,
        Err(error) => return error.outcome(None),
    };
    let system_state_present = state_view.is_present();
    if let Some(outcome) = inspect_guard(paths) {
        return outcome;
    }
    if let Some(outcome) = state_view.inspect_temporary_receipt(paths) {
        return outcome;
    }
    match temporary_record_present(paths) {
        Ok(false) => (),
        Ok(true) => {
            return LinuxStartupOutcome::new(
                LinuxStartupDecision::MaintenanceRequired,
                LinuxStartupReason::InterruptedReceipt,
                None,
            )
        }
        Err(e) => return port_error_outcome(e.code(), None),
    }
    let receipt = match state_view.read_receipt(paths) {
        Ok(receipt) => receipt,
        Err(error) => return error.outcome(None),
    };
    if receipt
        .as_ref()
        .is_some_and(|value| !value.state().is_terminal())
    {
        return LinuxStartupOutcome::new(
            LinuxStartupDecision::MaintenanceRequired,
            LinuxStartupReason::OperationInProgress,
            receipt.as_ref().map(LinuxInstallReceipt::state),
        );
    }
    if build_identity == LinuxStartupBuildIdentity::DevelopmentStaged && receipt.is_some() {
        return LinuxStartupOutcome::new(
            LinuxStartupDecision::FailedClosed,
            LinuxStartupReason::DevelopmentIsolationViolation,
            receipt.as_ref().map(LinuxInstallReceipt::state),
        );
    }

    let receipt_state = receipt.as_ref().map(LinuxInstallReceipt::state);
    let record = match StartupRecord::read(paths) {
        Ok(record) => record,
        Err(error) => return port_error_outcome(error.code(), receipt_state),
    };
    let installed = receipt
        .as_ref()
        .and_then(LinuxInstallReceipt::installed_artifact);
    let observed = match (receipt.as_ref(), installed, record) {
        (Some(receipt), Some(artifact), Some(record)) => {
            let deps = match record.dependencies_for(receipt) {
                Ok(deps) => deps,
                Err(e) => return port_error_outcome(e.code(), receipt_state),
            };
            port.inspect_installed_package(receipt, artifact, &deps)
        }
        (_, None, Some(_)) => {
            return LinuxStartupOutcome::new(
                LinuxStartupDecision::FailedClosed,
                LinuxStartupReason::UnexpectedStateObject,
                receipt_state,
            )
        }
        (_, Some(_), None) => {
            return LinuxStartupOutcome::new(
                LinuxStartupDecision::MaintenanceRequired,
                LinuxStartupReason::UnmanagedPackageState,
                receipt_state,
            )
        }
        _ => port.inspect_package(),
    };
    let package = match observed {
        Ok(package) => package,
        Err(error) => return port_error_outcome(error.code(), receipt_state),
    };
    if package.state() == DpkgPackageState::Unknown {
        return LinuxStartupOutcome::new(
            LinuxStartupDecision::FailedClosed,
            LinuxStartupReason::PackageStateUnknown,
            receipt.as_ref().map(LinuxInstallReceipt::state),
        );
    }
    let Some(receipt) = receipt else {
        return inspect_without_receipt(build_identity, package, system_state_present);
    };
    let Some(installed_artifact) = receipt.installed_artifact() else {
        return inspect_terminal_without_product(&receipt, package);
    };
    if package.state() != DpkgPackageState::Installed {
        return LinuxStartupOutcome::new(
            LinuxStartupDecision::MaintenanceRequired,
            LinuxStartupReason::PackageStateIncomplete,
            receipt_state,
        );
    }
    if !package.matches(installed_artifact) {
        return LinuxStartupOutcome::new(
            LinuxStartupDecision::MaintenanceRequired,
            LinuxStartupReason::UnmanagedPackageState,
            receipt_state,
        );
    }
    if let Err(error) = port.validate_component(component, installed_artifact) {
        return port_error_outcome(error.code(), receipt_state);
    }
    LinuxStartupOutcome::new(
        LinuxStartupDecision::AllowedProduct,
        LinuxStartupReason::InstalledReceiptVerified,
        receipt_state,
    )
}

fn inspect_without_receipt(
    build_identity: LinuxStartupBuildIdentity,
    package: LinuxPackageObservation,
    system_state_present: bool,
) -> LinuxStartupOutcome {
    if package.state() != DpkgPackageState::NotInstalled {
        return LinuxStartupOutcome::new(
            LinuxStartupDecision::MaintenanceRequired,
            LinuxStartupReason::UnmanagedPackageState,
            None,
        );
    }
    match build_identity {
        LinuxStartupBuildIdentity::DevelopmentStaged if system_state_present => {
            LinuxStartupOutcome::new(
                LinuxStartupDecision::FailedClosed,
                LinuxStartupReason::DevelopmentIsolationViolation,
                None,
            )
        }
        LinuxStartupBuildIdentity::DevelopmentStaged => LinuxStartupOutcome::new(
            LinuxStartupDecision::AllowedDevelopment,
            LinuxStartupReason::DevelopmentStateAbsent,
            None,
        ),
        LinuxStartupBuildIdentity::DebianSystemProduct => LinuxStartupOutcome::new(
            LinuxStartupDecision::FailedClosed,
            LinuxStartupReason::ReceiptMissing,
            None,
        ),
    }
}

fn inspect_terminal_without_product(
    receipt: &LinuxInstallReceipt,
    package: LinuxPackageObservation,
) -> LinuxStartupOutcome {
    let receipt_state = Some(receipt.state());
    if !matches!(
        package.state(),
        DpkgPackageState::NotInstalled | DpkgPackageState::ConfigFiles
    ) {
        return LinuxStartupOutcome::new(
            LinuxStartupDecision::MaintenanceRequired,
            if package.state() == DpkgPackageState::Installed {
                LinuxStartupReason::UnmanagedPackageState
            } else {
                LinuxStartupReason::PackageStateIncomplete
            },
            receipt_state,
        );
    }
    LinuxStartupOutcome::new(
        LinuxStartupDecision::FailedClosed,
        if receipt.operation_kind() == LinuxOperationKind::Remove
            && receipt.state() == LinuxInstallState::Completed
        {
            LinuxStartupReason::RemovedProgram
        } else {
            LinuxStartupReason::ProductNotInstalled
        },
        receipt_state,
    )
}

fn port_error_outcome(
    code: LinuxStartupPortErrorCode,
    receipt_state: Option<LinuxInstallState>,
) -> LinuxStartupOutcome {
    let reason = match code {
        LinuxStartupPortErrorCode::PackageStateUnavailable => {
            LinuxStartupReason::PackageStateUnavailable
        }
        LinuxStartupPortErrorCode::PackageStateUnknown => LinuxStartupReason::PackageStateUnknown,
        LinuxStartupPortErrorCode::PackageIdentityChanged => {
            LinuxStartupReason::PackageIdentityChanged
        }
        LinuxStartupPortErrorCode::ComponentIdentityChanged => {
            LinuxStartupReason::ComponentIdentityChanged
        }
        LinuxStartupPortErrorCode::DependencyUnavailable => {
            LinuxStartupReason::DependencyUnavailable
        }
        LinuxStartupPortErrorCode::PermissionDenied => LinuxStartupReason::PermissionDenied,
        LinuxStartupPortErrorCode::Io => LinuxStartupReason::Io,
    };
    LinuxStartupOutcome::new(LinuxStartupDecision::FailedClosed, reason, receipt_state)
}
