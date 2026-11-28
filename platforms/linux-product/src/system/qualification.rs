//! Desktop startup qualification while the system maintenance guard is held.
use super::host::{
    observation_error, store_error, LinuxMaintenanceHostError, LinuxMaintenanceHostErrorCode,
};
use super::observer::DpkgSystemObserver;
use crate::coordinator::DpkgStagedPackage;
use crate::model::{LinuxInstallReceipt, LinuxOperationKind, LinuxOperationRequest};
use crate::startup::record::{sync_parent, temporary_record_present, StartupRecord};
use crate::startup::{
    public_receipt, LinuxStartupPaths, LinuxStartupPortError, LinuxStartupPortErrorCode,
};
use crate::store::{LinuxInstallGuard, LinuxInstallStore};

fn invalid() -> LinuxMaintenanceHostError {
    LinuxMaintenanceHostError::new(
        LinuxMaintenanceHostErrorCode::StartupQualification,
        "desktop startup qualification is unavailable",
    )
}

fn record_error(e: LinuxStartupPortError) -> LinuxMaintenanceHostError {
    match e.code() {
        LinuxStartupPortErrorCode::Io => LinuxMaintenanceHostError::new(
            LinuxMaintenanceHostErrorCode::Io,
            "startup record I/O failed",
        ),
        LinuxStartupPortErrorCode::PermissionDenied => LinuxMaintenanceHostError::new(
            LinuxMaintenanceHostErrorCode::PermissionDenied,
            "startup record permission denied",
        ),
        _ => invalid(),
    }
}

pub(super) fn before_start(
    paths: &LinuxStartupPaths,
    store: &LinuxInstallStore,
    guard: &LinuxInstallGuard,
    request: &LinuxOperationRequest,
    observer: &mut impl DpkgSystemObserver,
) -> Result<(), LinuxMaintenanceHostError> {
    store.verify_guard(guard).map_err(store_error)?;
    let previous = public_receipt(paths).map_err(record_error)?;
    if previous.as_ref().is_some_and(|r| {
        !r.state().is_terminal()
            || r.installed_artifact() != request.source_artifact()
            || r.operation_chain()
                .iter()
                .any(|id| id == request.operation_id())
    }) || temporary_record_present(paths).map_err(record_error)?
    {
        return Err(invalid());
    }
    if let Some(old) = StartupRecord::read(paths).map_err(record_error)? {
        old.dependencies_for(previous.as_ref().ok_or_else(invalid)?)
            .map_err(record_error)?;
        old.revoke(paths).map_err(record_error)?;
    }
    if let Some(previous) = previous {
        // Repair accepts damage to installed files. Its private recovery material
        // must still match the original operation before a new receipt can replace it.
        verify_terminal(
            store,
            &previous,
            request.operation_kind() != LinuxOperationKind::Repair,
            observer,
        )?;
    }
    Ok(())
}

pub(super) fn before_resume(paths: &LinuxStartupPaths) -> Result<(), LinuxMaintenanceHostError> {
    if StartupRecord::read(paths).map_err(record_error)?.is_some()
        || temporary_record_present(paths).map_err(record_error)?
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn finish(
    paths: &LinuxStartupPaths,
    store: &LinuxInstallStore,
    guard: &LinuxInstallGuard,
    receipt: &LinuxInstallReceipt,
    observer: &mut impl DpkgSystemObserver,
) -> Result<(), LinuxMaintenanceHostError> {
    store.verify_guard(guard).map_err(store_error)?;
    let existing = StartupRecord::read(paths).map_err(record_error)?;
    let temporary = temporary_record_present(paths).map_err(record_error)?;
    if let Some(record) = &existing {
        record.dependencies_for(receipt).map_err(record_error)?;
        if temporary {
            return Err(invalid());
        }
    }
    let expected = match verify_terminal(store, receipt, true, observer) {
        Ok(record) => record,
        Err(error) => {
            if let Some(record) = existing {
                record.revoke(paths).map_err(record_error)?;
            }
            return Err(error);
        }
    };
    store.verify_guard(guard).map_err(store_error)?;
    match (&existing, &expected) {
        (None, Some(record)) => record.publish(paths).map_err(record_error)?,
        (Some(old), Some(record)) if old == record => sync_parent(paths).map_err(record_error)?,
        (Some(old), Some(_)) => {
            old.revoke(paths).map_err(record_error)?;
            return Err(invalid());
        }
        (None, None) if !temporary => (),
        _ => return Err(invalid()),
    }
    if public_receipt(paths).map_err(record_error)?.as_ref() != Some(receipt)
        || StartupRecord::read(paths).map_err(record_error)? != expected
        || temporary_record_present(paths).map_err(record_error)?
    {
        return Err(invalid());
    }
    Ok(())
}

fn verify_terminal(
    store: &LinuxInstallStore,
    receipt: &LinuxInstallReceipt,
    validate_installed: bool,
    observer: &mut impl DpkgSystemObserver,
) -> Result<Option<StartupRecord>, LinuxMaintenanceHostError> {
    if !receipt.state().is_terminal()
        || store.load_receipt().map_err(store_error)?.as_ref() != Some(receipt)
    {
        return Err(invalid());
    }
    let mut selected = None;
    let mut absent_package = None;
    for slot in receipt.required_slots() {
        let artifact = receipt
            .staged_artifact(*slot)
            .ok_or_else(invalid)?
            .artifact();
        let files = store
            .staged_artifact_paths(receipt.operation_id(), *slot)
            .map_err(store_error)?;
        let package =
            DpkgStagedPackage::from_paths(artifact, files.package_path(), files.evidence_path());
        let relationship = observer
            .verify_staged_package(package)
            .map_err(observation_error)?;
        if Some(artifact) == receipt.installed_artifact() {
            selected = Some(relationship);
        } else {
            absent_package = Some(relationship);
        }
    }
    if !validate_installed {
        return Ok(None);
    }
    let status = observer.inspect_status().map_err(observation_error)?;
    if let Some(artifact) = receipt.installed_artifact() {
        let relationship = selected.ok_or_else(invalid)?;
        observer
            .validate_installed_product(artifact, &relationship, &status)
            .map_err(observation_error)?;
        Ok(Some(
            StartupRecord::new(receipt, relationship.dependencies()).map_err(record_error)?,
        ))
    } else {
        observer
            .validate_absent_product(absent_package.as_ref(), &status)
            .map_err(observation_error)?;
        Ok(None)
    }
}

pub(super) fn current_receipt(
    paths: &LinuxStartupPaths,
) -> Result<LinuxInstallReceipt, LinuxMaintenanceHostError> {
    public_receipt(paths)
        .map_err(record_error)?
        .ok_or_else(invalid)
}
