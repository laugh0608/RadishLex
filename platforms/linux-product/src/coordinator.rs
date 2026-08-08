use std::fmt;
use std::path::Path;

use crate::model::{
    ArtifactSlot, DpkgPackageState, LinuxArtifactIdentity, LinuxFailureCode, LinuxInstallReceipt,
    LinuxInstallReceiptError, LinuxInstallState, LinuxOperationKind, LinuxOperationRequest,
    PackageSnapshot,
};
use crate::store::{LinuxInstallGuard, LinuxInstallStore, LinuxInstallStoreError};

pub trait DpkgTransactionPort {
    fn validate_operation(&mut self, request: &LinuxOperationRequest) -> Result<(), DpkgPortError>;

    fn prove_quiescent(&mut self) -> Result<(), DpkgPortError>;

    fn inspect_package(&mut self) -> Result<PackageSnapshot, DpkgPortError>;

    fn apply_package(
        &mut self,
        operation: LinuxOperationKind,
        artifact: &LinuxArtifactIdentity,
        package_path: &Path,
        evidence_path: &Path,
    ) -> Result<(), DpkgPortError>;

    fn remove_package(&mut self) -> Result<(), DpkgPortError>;

    fn validate_target(
        &mut self,
        operation: LinuxOperationKind,
        snapshot: &PackageSnapshot,
    ) -> Result<bool, DpkgPortError>;

    fn restore_source(
        &mut self,
        source: Option<&LinuxArtifactIdentity>,
        package_path: Option<&Path>,
        evidence_path: Option<&Path>,
    ) -> Result<(), DpkgPortError>;

    fn validate_source(&mut self, snapshot: &PackageSnapshot) -> Result<bool, DpkgPortError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpkgPortError {
    message: String,
}

impl DpkgPortError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DpkgPortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DpkgPortError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionOutcome {
    Completed,
    AbortedPreserved,
    RolledBack,
}

#[derive(Debug)]
pub enum TransactionError {
    Store(LinuxInstallStoreError),
    Receipt(LinuxInstallReceiptError),
    Port {
        failure: LinuxFailureCode,
        source: DpkgPortError,
    },
    RecoveryBlocked {
        failure: LinuxFailureCode,
        source: DpkgPortError,
    },
    OperationNotStaged,
}

impl TransactionError {
    pub const fn failure_code(&self) -> Option<LinuxFailureCode> {
        match self {
            Self::Port { failure, .. } | Self::RecoveryBlocked { failure, .. } => Some(*failure),
            Self::Store(_) | Self::Receipt(_) | Self::OperationNotStaged => None,
        }
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "Linux install store: {error}"),
            Self::Receipt(error) => write!(formatter, "Linux install receipt: {error}"),
            Self::Port { failure, source } => {
                write!(formatter, "dpkg port failed ({failure:?}): {source}")
            }
            Self::RecoveryBlocked { failure, source } => write!(
                formatter,
                "source recovery remains blocked ({failure:?}): {source}"
            ),
            Self::OperationNotStaged => {
                formatter.write_str("Linux install operation is still awaiting artifact staging")
            }
        }
    }
}

impl std::error::Error for TransactionError {}

impl From<LinuxInstallStoreError> for TransactionError {
    fn from(value: LinuxInstallStoreError) -> Self {
        Self::Store(value)
    }
}

impl From<LinuxInstallReceiptError> for TransactionError {
    fn from(value: LinuxInstallReceiptError) -> Self {
        Self::Receipt(value)
    }
}

pub fn prepare_operation(
    store: &LinuxInstallStore,
    guard: &LinuxInstallGuard,
    request: LinuxOperationRequest,
    port: &mut impl DpkgTransactionPort,
) -> Result<LinuxInstallReceipt, TransactionError> {
    store.verify_guard(guard)?;
    port.validate_operation(&request)
        .map_err(|source| TransactionError::Port {
            failure: LinuxFailureCode::ArtifactInvalid,
            source,
        })?;
    let previous = store.load_receipt()?;
    let mut operation_chain = previous
        .as_ref()
        .map_or_else(Vec::new, |receipt| receipt.operation_chain().to_vec());
    operation_chain.push(request.operation_id().to_owned());

    let initial_package = port
        .inspect_package()
        .map_err(|source| TransactionError::Port {
            failure: LinuxFailureCode::PackageStateUnavailable,
            source,
        })?;
    if initial_package.state() == DpkgPackageState::Unknown {
        return Err(TransactionError::Port {
            failure: LinuxFailureCode::PackageStateUnknown,
            source: DpkgPortError::new("dpkg returned an unknown package state"),
        });
    }

    let receipt = LinuxInstallReceipt::from_request(
        request,
        operation_chain,
        store.root_identity().clone(),
        initial_package,
    )?;
    store.persist_receipt(guard, &receipt)?;
    Ok(receipt)
}

pub fn resume_operation(
    store: &LinuxInstallStore,
    guard: &LinuxInstallGuard,
    port: &mut impl DpkgTransactionPort,
) -> Result<TransactionOutcome, TransactionError> {
    store.verify_guard(guard)?;
    let mut receipt = store
        .load_receipt()?
        .ok_or(TransactionError::OperationNotStaged)?;

    loop {
        match receipt.state() {
            LinuxInstallState::Prepared => return Err(TransactionError::OperationNotStaged),
            LinuxInstallState::ArtifactsStaged => {
                if port.prove_quiescent().is_err() {
                    receipt.abort_preserved(LinuxFailureCode::ProgramsRunning)?;
                    store.persist_receipt(guard, &receipt)?;
                    return Ok(TransactionOutcome::AbortedPreserved);
                }
                receipt.advance(LinuxInstallState::Quiesced)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::Quiesced => {
                receipt.advance(LinuxInstallState::PackageMutating)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::PackageMutating => {
                if let Err(failure) = drive_target(store, port, &mut receipt) {
                    receipt.require_rollback(failure)?;
                    store.persist_receipt(guard, &receipt)?;
                    continue;
                }
                store.persist_receipt(guard, &receipt)?;
                receipt.advance(LinuxInstallState::PackageVerified)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::PackageVerified => {
                let failure = match port.inspect_package() {
                    Err(_) => Some(LinuxFailureCode::PackageStateUnavailable),
                    Ok(snapshot) if !receipt.matches_target(&snapshot) => {
                        Some(LinuxFailureCode::TargetValidationFailed)
                    }
                    Ok(snapshot) => match port.validate_target(receipt.operation_kind(), &snapshot)
                    {
                        Ok(true) => None,
                        Ok(false) | Err(_) => Some(LinuxFailureCode::TargetValidationFailed),
                    },
                };
                if let Some(failure) = failure {
                    receipt.require_rollback(failure)?;
                    store.persist_receipt(guard, &receipt)?;
                    continue;
                }
                receipt.advance(LinuxInstallState::Completed)?;
                store.persist_receipt(guard, &receipt)?;
                return Ok(TransactionOutcome::Completed);
            }
            LinuxInstallState::RollbackRequired => {
                receipt.advance(LinuxInstallState::SourceRestoring)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::SourceRestoring => {
                if let Err((failure, source)) = drive_source(store, port, &mut receipt) {
                    return Err(TransactionError::RecoveryBlocked { failure, source });
                }
                store.persist_receipt(guard, &receipt)?;
                receipt.advance(LinuxInstallState::SourceVerified)?;
                store.persist_receipt(guard, &receipt)?;
            }
            LinuxInstallState::SourceVerified => {
                let snapshot =
                    port.inspect_package()
                        .map_err(|source| TransactionError::RecoveryBlocked {
                            failure: LinuxFailureCode::PackageStateUnavailable,
                            source,
                        })?;
                let valid = receipt.matches_source(&snapshot)
                    && port.validate_source(&snapshot).map_err(|source| {
                        TransactionError::RecoveryBlocked {
                            failure: LinuxFailureCode::SourceValidationFailed,
                            source,
                        }
                    })?;
                if !valid {
                    return Err(TransactionError::RecoveryBlocked {
                        failure: LinuxFailureCode::SourceValidationFailed,
                        source: DpkgPortError::new("restored source validation no longer holds"),
                    });
                }
                receipt.finish_rollback()?;
                store.persist_receipt(guard, &receipt)?;
                return Ok(TransactionOutcome::RolledBack);
            }
            LinuxInstallState::Completed => return Ok(TransactionOutcome::Completed),
            LinuxInstallState::AbortedPreserved => return Ok(TransactionOutcome::AbortedPreserved),
            LinuxInstallState::RolledBack => return Ok(TransactionOutcome::RolledBack),
        }
    }
}

fn drive_target(
    store: &LinuxInstallStore,
    port: &mut impl DpkgTransactionPort,
    receipt: &mut LinuxInstallReceipt,
) -> Result<(), LinuxFailureCode> {
    let snapshot = port
        .inspect_package()
        .map_err(|_| LinuxFailureCode::PackageStateUnavailable)?;
    if snapshot.state() == DpkgPackageState::Unknown {
        return Err(LinuxFailureCode::PackageStateUnknown);
    }
    if receipt.matches_target(&snapshot) {
        let valid = port
            .validate_target(receipt.operation_kind(), &snapshot)
            .map_err(|_| LinuxFailureCode::TargetValidationFailed)?;
        if valid {
            if receipt.target_proof().is_none() {
                receipt
                    .record_target_proof(snapshot)
                    .map_err(|_| LinuxFailureCode::ReceiptInconsistent)?;
            }
            return Ok(());
        }
        if receipt.operation_kind() != LinuxOperationKind::Repair {
            return Err(LinuxFailureCode::TargetValidationFailed);
        }
    }

    if receipt.operation_kind() == LinuxOperationKind::Remove {
        if !receipt.source_snapshot_is_mutation_precondition(&snapshot) {
            return Err(LinuxFailureCode::PackageStateUnexpected);
        }
        port.remove_package()
            .map_err(|_| LinuxFailureCode::PackageMutationFailed)?;
    } else {
        let target = receipt
            .target_artifact()
            .ok_or(LinuxFailureCode::ReceiptInconsistent)?;
        let target_in_progress =
            snapshot.state().is_recoverable() && snapshot.artifact() == Some(target);
        if !receipt.source_snapshot_is_mutation_precondition(&snapshot) && !target_in_progress {
            return Err(LinuxFailureCode::PackageStateUnexpected);
        }
        let paths = store
            .staged_artifact_paths(receipt.operation_id(), ArtifactSlot::Target)
            .map_err(|_| LinuxFailureCode::ArtifactInvalid)?;
        port.apply_package(
            receipt.operation_kind(),
            target,
            paths.package_path(),
            paths.evidence_path(),
        )
        .map_err(|_| LinuxFailureCode::PackageMutationFailed)?;
    }

    let target_snapshot = port
        .inspect_package()
        .map_err(|_| LinuxFailureCode::PackageStateUnavailable)?;
    let valid = receipt.matches_target(&target_snapshot)
        && port
            .validate_target(receipt.operation_kind(), &target_snapshot)
            .map_err(|_| LinuxFailureCode::TargetValidationFailed)?;
    if !valid {
        return Err(LinuxFailureCode::TargetValidationFailed);
    }
    receipt
        .record_target_proof(target_snapshot)
        .map_err(|_| LinuxFailureCode::ReceiptInconsistent)
}

fn drive_source(
    store: &LinuxInstallStore,
    port: &mut impl DpkgTransactionPort,
    receipt: &mut LinuxInstallReceipt,
) -> Result<(), (LinuxFailureCode, DpkgPortError)> {
    if receipt.source_proof().is_some() {
        return Ok(());
    }
    let mut snapshot = port
        .inspect_package()
        .map_err(|error| (LinuxFailureCode::PackageStateUnavailable, error))?;
    let source_valid = receipt.matches_source(&snapshot)
        && port
            .validate_source(&snapshot)
            .map_err(|error| (LinuxFailureCode::SourceValidationFailed, error))?;
    if !source_valid {
        let staged = receipt
            .restore_staged_artifact()
            .map(|evidence| store.staged_artifact_paths(receipt.operation_id(), evidence.slot()))
            .transpose()
            .map_err(|error| {
                (
                    LinuxFailureCode::ArtifactInvalid,
                    DpkgPortError::new(error.to_string()),
                )
            })?;
        port.restore_source(
            receipt.source_artifact(),
            staged.as_ref().map(|paths| paths.package_path()),
            staged.as_ref().map(|paths| paths.evidence_path()),
        )
        .map_err(|error| (LinuxFailureCode::SourceRestoreFailed, error))?;
        snapshot = port
            .inspect_package()
            .map_err(|error| (LinuxFailureCode::PackageStateUnavailable, error))?;
    }
    let validated = port
        .validate_source(&snapshot)
        .map_err(|error| (LinuxFailureCode::SourceValidationFailed, error))?;
    if !receipt.matches_source(&snapshot) || !validated {
        return Err((
            LinuxFailureCode::SourceValidationFailed,
            DpkgPortError::new(
                "source package state or product validation does not match the receipt",
            ),
        ));
    }
    receipt.record_source_proof(snapshot).map_err(|error| {
        (
            LinuxFailureCode::ReceiptInconsistent,
            DpkgPortError::new(error.to_string()),
        )
    })
}
