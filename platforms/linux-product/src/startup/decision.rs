use crate::model::{DpkgPackageState, LinuxInstallReceipt, LinuxInstallState, LinuxOperationKind};

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

    let package = match port.inspect_package() {
        Ok(package) => package,
        Err(error) => {
            return port_error_outcome(
                error.code(),
                receipt.as_ref().map(LinuxInstallReceipt::state),
            )
        }
    };
    if package.state() == DpkgPackageState::Unknown {
        return LinuxStartupOutcome::new(
            LinuxStartupDecision::FailedClosed,
            LinuxStartupReason::PackageStateUnknown,
            receipt.as_ref().map(LinuxInstallReceipt::state),
        );
    }
    let receipt_state = receipt.as_ref().map(LinuxInstallReceipt::state);
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
