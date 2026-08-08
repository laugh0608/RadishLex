use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::coordinator::{
    prepare_operation, resume_operation, TransactionError, TransactionOutcome,
};
use crate::debian::{validate_operation_relation, VerifiedArtifactRelationship};
use crate::model::{ArtifactSlot, LinuxFailureCode, LinuxOperationKind, LinuxOperationRequest};
use crate::store::{LinuxInstallStore, LinuxInstallStoreError, SYSTEM_STATE_ROOT};

use super::observer::{
    validate_effective_root, verify_root_owned_input_artifact, LinuxSystemObservationError,
};
use super::port::LinuxDpkgTransactionPort;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxMaintenanceHostErrorCode {
    AuthorizationRequired,
    ArgumentInvalid,
    ArtifactInvalid,
    VersionRelationInvalid,
    OperationIdentityChanged,
    EnvironmentUnsupported,
    PermissionDenied,
    Io,
    Store,
    Transaction,
}

#[derive(Debug)]
pub struct LinuxMaintenanceHostError {
    code: LinuxMaintenanceHostErrorCode,
    message: &'static str,
    failure_code: Option<LinuxFailureCode>,
}

impl LinuxMaintenanceHostError {
    const fn new(code: LinuxMaintenanceHostErrorCode, message: &'static str) -> Self {
        Self {
            code,
            message,
            failure_code: None,
        }
    }

    pub const fn code(&self) -> LinuxMaintenanceHostErrorCode {
        self.code
    }

    pub const fn failure_code(&self) -> Option<LinuxFailureCode> {
        self.failure_code
    }
}

impl fmt::Display for LinuxMaintenanceHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for LinuxMaintenanceHostError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxMaintenanceArtifactInput {
    package_path: PathBuf,
    evidence_path: PathBuf,
}

impl LinuxMaintenanceArtifactInput {
    fn new(
        package_path: PathBuf,
        evidence_path: PathBuf,
    ) -> Result<Self, LinuxMaintenanceHostError> {
        validate_input_path(&package_path)?;
        validate_input_path(&evidence_path)?;
        let package_name = package_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(argument_invalid)?;
        if !package_name.ends_with("_arm64.deb")
            || evidence_path.parent() != package_path.parent()
            || evidence_path.file_name().and_then(|name| name.to_str())
                != Some(&format!("{package_name}.evidence.json"))
        {
            return Err(argument_invalid());
        }
        Ok(Self {
            package_path,
            evidence_path,
        })
    }

    pub fn package_path(&self) -> &Path {
        &self.package_path
    }

    pub fn evidence_path(&self) -> &Path {
        &self.evidence_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxMaintenanceCommand {
    action: LinuxMaintenanceAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LinuxMaintenanceAction {
    Start {
        operation_id: String,
        kind: LinuxOperationKind,
        source: Option<LinuxMaintenanceArtifactInput>,
        target: Option<LinuxMaintenanceArtifactInput>,
    },
    Resume {
        operation_id: String,
    },
}

impl LinuxMaintenanceCommand {
    pub fn parse(arguments: &[String]) -> Result<Self, LinuxMaintenanceHostError> {
        let (verb, options) = arguments.split_first().ok_or_else(argument_invalid)?;
        let mut authorized = false;
        let mut preserve_user_data = false;
        let mut operation_id = None;
        let mut kind = None;
        let mut source_package = None;
        let mut source_evidence = None;
        let mut target_package = None;
        let mut target_evidence = None;
        let mut index = 0;
        while index < options.len() {
            match options[index].as_str() {
                "--authorized-system-mutation" if !authorized => {
                    authorized = true;
                    index += 1;
                }
                "--preserve-user-data" if !preserve_user_data => {
                    preserve_user_data = true;
                    index += 1;
                }
                option @ ("--operation-id" | "--kind" | "--source-deb" | "--source-evidence"
                | "--target-deb" | "--target-evidence") => {
                    let value = options.get(index + 1).ok_or_else(argument_invalid)?;
                    let destination = match option {
                        "--operation-id" => &mut operation_id,
                        "--kind" => &mut kind,
                        "--source-deb" => &mut source_package,
                        "--source-evidence" => &mut source_evidence,
                        "--target-deb" => &mut target_package,
                        "--target-evidence" => &mut target_evidence,
                        _ => unreachable!(),
                    };
                    if destination.replace(value.clone()).is_some() {
                        return Err(argument_invalid());
                    }
                    index += 2;
                }
                _ => return Err(argument_invalid()),
            }
        }
        if !authorized || !preserve_user_data {
            return Err(LinuxMaintenanceHostError::new(
                LinuxMaintenanceHostErrorCode::AuthorizationRequired,
                "explicit system-mutation and user-data-preservation authorization is required",
            ));
        }
        let operation_id = operation_id.ok_or_else(argument_invalid)?;
        validate_operation_id(&operation_id)?;
        match verb.as_str() {
            "resume" => {
                if kind.is_some()
                    || source_package.is_some()
                    || source_evidence.is_some()
                    || target_package.is_some()
                    || target_evidence.is_some()
                {
                    return Err(argument_invalid());
                }
                Ok(Self {
                    action: LinuxMaintenanceAction::Resume { operation_id },
                })
            }
            "start" => {
                let kind = parse_operation_kind(kind.as_deref().ok_or_else(argument_invalid)?)?;
                let source = parse_artifact_pair(source_package, source_evidence)?;
                let target = parse_artifact_pair(target_package, target_evidence)?;
                let shape_is_valid = match kind {
                    LinuxOperationKind::Install | LinuxOperationKind::Repair => {
                        source.is_none() && target.is_some()
                    }
                    LinuxOperationKind::Upgrade | LinuxOperationKind::Rollback => {
                        source.is_some() && target.is_some()
                    }
                    LinuxOperationKind::Remove => source.is_some() && target.is_none(),
                };
                if !shape_is_valid {
                    return Err(argument_invalid());
                }
                Ok(Self {
                    action: LinuxMaintenanceAction::Start {
                        operation_id,
                        kind,
                        source,
                        target,
                    },
                })
            }
            _ => Err(argument_invalid()),
        }
    }
}

pub fn run_linux_maintenance(
    command: LinuxMaintenanceCommand,
) -> Result<TransactionOutcome, LinuxMaintenanceHostError> {
    validate_effective_root().map_err(observation_error)?;
    match command.action {
        LinuxMaintenanceAction::Start {
            operation_id,
            kind,
            source,
            target,
        } => start_operation(operation_id, kind, source, target),
        LinuxMaintenanceAction::Resume { operation_id } => resume_existing(operation_id),
    }
}

fn start_operation(
    operation_id: String,
    kind: LinuxOperationKind,
    source: Option<LinuxMaintenanceArtifactInput>,
    target: Option<LinuxMaintenanceArtifactInput>,
) -> Result<TransactionOutcome, LinuxMaintenanceHostError> {
    let source_relationship = source.as_ref().map(verify_input).transpose()?;
    let target_relationship = target.as_ref().map(verify_input).transpose()?;
    let effective_source = if kind == LinuxOperationKind::Repair {
        target_relationship.as_ref()
    } else {
        source_relationship.as_ref()
    };
    let relation =
        validate_operation_relation(kind, effective_source, target_relationship.as_ref()).map_err(
            |_| {
                LinuxMaintenanceHostError::new(
                    LinuxMaintenanceHostErrorCode::VersionRelationInvalid,
                    "maintenance artifact version relation is invalid",
                )
            },
        )?;
    let source_identity = effective_source
        .map(VerifiedArtifactRelationship::to_linux_artifact_identity)
        .transpose()
        .map_err(|_| artifact_invalid())?;
    let target_identity = target_relationship
        .as_ref()
        .map(|value| value.to_linux_artifact_identity())
        .transpose()
        .map_err(|_| artifact_invalid())?;
    let request = LinuxOperationRequest::new(
        operation_id,
        kind,
        relation,
        source_identity,
        target_identity,
    )
    .map_err(|_| argument_invalid())?;
    let mut relationships = Vec::new();
    if let Some(value) = source_relationship {
        relationships.push(value);
    }
    if let Some(value) = target_relationship {
        if !relationships.contains(&value) {
            relationships.push(value);
        }
    }
    let store = LinuxInstallStore::bootstrap_system().map_err(store_error)?;
    let guard = store.acquire_guard().map_err(store_error)?;
    let mut port = LinuxDpkgTransactionPort::system(relationships);
    prepare_operation(&store, &guard, request, &mut port).map_err(transaction_error)?;
    if matches!(
        kind,
        LinuxOperationKind::Upgrade | LinuxOperationKind::Rollback | LinuxOperationKind::Remove
    ) {
        let input = source.as_ref().ok_or_else(argument_invalid)?;
        store
            .stage_artifact(
                &guard,
                ArtifactSlot::Source,
                input.package_path(),
                input.evidence_path(),
            )
            .map_err(store_error)?;
    }
    if kind != LinuxOperationKind::Remove {
        let input = target.as_ref().ok_or_else(argument_invalid)?;
        store
            .stage_artifact(
                &guard,
                ArtifactSlot::Target,
                input.package_path(),
                input.evidence_path(),
            )
            .map_err(store_error)?;
    }
    store.finish_staging(&guard).map_err(store_error)?;
    resume_operation(&store, &guard, &mut port).map_err(transaction_error)
}

fn resume_existing(operation_id: String) -> Result<TransactionOutcome, LinuxMaintenanceHostError> {
    match fs::symlink_metadata(SYSTEM_STATE_ROOT) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(LinuxMaintenanceHostError::new(
                LinuxMaintenanceHostErrorCode::OperationIdentityChanged,
                "maintenance state is unavailable",
            ))
        }
        Err(error) => {
            return Err(LinuxMaintenanceHostError::new(
                if error.kind() == io::ErrorKind::PermissionDenied {
                    LinuxMaintenanceHostErrorCode::PermissionDenied
                } else {
                    LinuxMaintenanceHostErrorCode::Io
                },
                "maintenance state inspection failed",
            ))
        }
    }
    let store = LinuxInstallStore::bootstrap_system().map_err(store_error)?;
    let guard = store.acquire_guard().map_err(store_error)?;
    let receipt = store.load_receipt().map_err(store_error)?.ok_or_else(|| {
        LinuxMaintenanceHostError::new(
            LinuxMaintenanceHostErrorCode::OperationIdentityChanged,
            "maintenance receipt is unavailable",
        )
    })?;
    if receipt.operation_id() != operation_id {
        return Err(LinuxMaintenanceHostError::new(
            LinuxMaintenanceHostErrorCode::OperationIdentityChanged,
            "maintenance operation identity differs from the receipt",
        ));
    }
    let mut port = LinuxDpkgTransactionPort::system(Vec::new());
    resume_operation(&store, &guard, &mut port).map_err(transaction_error)
}

fn verify_input(
    input: &LinuxMaintenanceArtifactInput,
) -> Result<VerifiedArtifactRelationship, LinuxMaintenanceHostError> {
    verify_root_owned_input_artifact(input.package_path(), input.evidence_path())
        .map_err(observation_error)
}

fn parse_artifact_pair(
    package: Option<String>,
    evidence: Option<String>,
) -> Result<Option<LinuxMaintenanceArtifactInput>, LinuxMaintenanceHostError> {
    match (package, evidence) {
        (None, None) => Ok(None),
        (Some(package), Some(evidence)) => {
            LinuxMaintenanceArtifactInput::new(PathBuf::from(package), PathBuf::from(evidence))
                .map(Some)
        }
        _ => Err(argument_invalid()),
    }
}

fn parse_operation_kind(value: &str) -> Result<LinuxOperationKind, LinuxMaintenanceHostError> {
    match value {
        "install" => Ok(LinuxOperationKind::Install),
        "upgrade" => Ok(LinuxOperationKind::Upgrade),
        "repair" => Ok(LinuxOperationKind::Repair),
        "remove" => Ok(LinuxOperationKind::Remove),
        "rollback" => Ok(LinuxOperationKind::Rollback),
        _ => Err(argument_invalid()),
    }
}

fn validate_operation_id(value: &str) -> Result<(), LinuxMaintenanceHostError> {
    if value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(argument_invalid())
    }
}

fn validate_input_path(path: &Path) -> Result<(), LinuxMaintenanceHostError> {
    if path.is_absolute()
        && path.file_name().is_some()
        && path
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir))
    {
        Ok(())
    } else {
        Err(argument_invalid())
    }
}

fn argument_invalid() -> LinuxMaintenanceHostError {
    LinuxMaintenanceHostError::new(
        LinuxMaintenanceHostErrorCode::ArgumentInvalid,
        "maintenance command arguments are invalid",
    )
}

fn artifact_invalid() -> LinuxMaintenanceHostError {
    LinuxMaintenanceHostError::new(
        LinuxMaintenanceHostErrorCode::ArtifactInvalid,
        "maintenance artifact identity is invalid",
    )
}

fn observation_error(error: LinuxSystemObservationError) -> LinuxMaintenanceHostError {
    use super::observer::LinuxSystemObservationErrorCode as Code;
    let code = match error.code() {
        Code::EnvironmentUnsupported => LinuxMaintenanceHostErrorCode::EnvironmentUnsupported,
        Code::PermissionDenied => LinuxMaintenanceHostErrorCode::PermissionDenied,
        Code::Io => LinuxMaintenanceHostErrorCode::Io,
        _ => LinuxMaintenanceHostErrorCode::ArtifactInvalid,
    };
    LinuxMaintenanceHostError::new(code, "maintenance artifact observation failed")
}

fn store_error(_error: LinuxInstallStoreError) -> LinuxMaintenanceHostError {
    LinuxMaintenanceHostError::new(
        LinuxMaintenanceHostErrorCode::Store,
        "maintenance state store failed",
    )
}

fn transaction_error(error: TransactionError) -> LinuxMaintenanceHostError {
    let mut result = LinuxMaintenanceHostError::new(
        LinuxMaintenanceHostErrorCode::Transaction,
        "maintenance transaction failed",
    );
    result.failure_code = error.failure_code();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef";

    fn authorized(arguments: &[&str]) -> Vec<String> {
        arguments
            .iter()
            .chain(["--authorized-system-mutation", "--preserve-user-data"].iter())
            .map(|value| (*value).to_owned())
            .collect()
    }

    #[test]
    fn parser_requires_two_explicit_authorizations_and_exact_operation_identity() {
        for arguments in [
            vec!["resume", "--operation-id", ID],
            vec![
                "resume",
                "--operation-id",
                ID,
                "--authorized-system-mutation",
            ],
            vec![
                "resume",
                "--operation-id",
                "ABCDEF0123456789ABCDEF0123456789",
                "--authorized-system-mutation",
                "--preserve-user-data",
            ],
        ] {
            assert!(LinuxMaintenanceCommand::parse(
                &arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()
            )
            .is_err());
        }
        assert_eq!(
            LinuxMaintenanceCommand::parse(&authorized(&["resume", "--operation-id", ID]))
                .expect("authorized resume"),
            LinuxMaintenanceCommand {
                action: LinuxMaintenanceAction::Resume {
                    operation_id: ID.to_owned()
                }
            }
        );
    }

    #[test]
    fn parser_enforces_artifact_shape_for_each_operation_kind() {
        let source = "/var/tmp/radishlex_26.7.1+38-1_arm64.deb";
        let source_evidence = "/var/tmp/radishlex_26.7.1+38-1_arm64.deb.evidence.json";
        let target = "/var/tmp/radishlex_26.7.1+39-1_arm64.deb";
        let target_evidence = "/var/tmp/radishlex_26.7.1+39-1_arm64.deb.evidence.json";
        for (kind, artifact_arguments) in [
            (
                "install",
                vec!["--target-deb", target, "--target-evidence", target_evidence],
            ),
            (
                "repair",
                vec!["--target-deb", source, "--target-evidence", source_evidence],
            ),
            (
                "remove",
                vec!["--source-deb", source, "--source-evidence", source_evidence],
            ),
            (
                "upgrade",
                vec![
                    "--source-deb",
                    source,
                    "--source-evidence",
                    source_evidence,
                    "--target-deb",
                    target,
                    "--target-evidence",
                    target_evidence,
                ],
            ),
            (
                "rollback",
                vec![
                    "--source-deb",
                    target,
                    "--source-evidence",
                    target_evidence,
                    "--target-deb",
                    source,
                    "--target-evidence",
                    source_evidence,
                ],
            ),
        ] {
            let mut arguments = vec!["start", "--operation-id", ID, "--kind", kind];
            arguments.extend(artifact_arguments);
            LinuxMaintenanceCommand::parse(&authorized(&arguments))
                .expect("valid operation artifact shape");
        }
        let invalid = authorized(&[
            "start",
            "--operation-id",
            ID,
            "--kind",
            "install",
            "--source-deb",
            source,
            "--source-evidence",
            source_evidence,
            "--target-deb",
            target,
            "--target-evidence",
            target_evidence,
        ]);
        assert!(LinuxMaintenanceCommand::parse(&invalid).is_err());
    }

    #[test]
    fn parser_rejects_relative_mismatched_and_duplicate_inputs() {
        for arguments in [
            authorized(&[
                "start",
                "--operation-id",
                ID,
                "--kind",
                "install",
                "--target-deb",
                "relative.deb",
                "--target-evidence",
                "relative.deb.evidence.json",
            ]),
            authorized(&[
                "start",
                "--operation-id",
                ID,
                "--kind",
                "install",
                "--target-deb",
                "/var/tmp/radishlex_26.7.1+39-1_arm64.deb",
                "--target-evidence",
                "/var/tmp/unrelated.evidence.json",
            ]),
            authorized(&["resume", "--operation-id", ID, "--operation-id", ID]),
        ] {
            assert!(LinuxMaintenanceCommand::parse(&arguments).is_err());
        }
    }
}
