use std::fmt;

use radishlex_linux_product_install::LinuxMaintenanceCommand;

use crate::scenario::L6CrashScenario;

const CONTROLLER_VERB: &str = "crash";
const WORKER_VERB: &str = "__checkpoint_worker";

#[derive(Clone)]
pub struct ControllerCommand {
    scenario: L6CrashScenario,
    repository_commit: String,
    guest_identity_sha256: String,
    snapshot_identity_sha256: String,
    maintenance: LinuxMaintenanceCommand,
    maintenance_arguments: Vec<String>,
}

impl ControllerCommand {
    pub fn parse(arguments: &[String]) -> Result<Self, ControllerCommandError> {
        let (verb, remainder) = arguments
            .split_first()
            .ok_or(ControllerCommandError::ArgumentInvalid)?;
        if verb != CONTROLLER_VERB {
            return Err(ControllerCommandError::ArgumentInvalid);
        }
        let (controller_arguments, maintenance_arguments) = split_maintenance(remainder)?;
        let mut scenario = None;
        let mut repository_commit = None;
        let mut guest_identity_sha256 = None;
        let mut snapshot_identity_sha256 = None;
        let mut authorized_l6_crash = false;
        let mut index = 0;
        while index < controller_arguments.len() {
            match controller_arguments[index].as_str() {
                "--authorized-l6-crash" if !authorized_l6_crash => {
                    authorized_l6_crash = true;
                    index += 1;
                }
                option @ ("--scenario"
                | "--repository-commit"
                | "--guest-identity-sha256"
                | "--snapshot-identity-sha256") => {
                    let value = controller_arguments
                        .get(index + 1)
                        .ok_or(ControllerCommandError::ArgumentInvalid)?;
                    let destination = match option {
                        "--scenario" => &mut scenario,
                        "--repository-commit" => &mut repository_commit,
                        "--guest-identity-sha256" => &mut guest_identity_sha256,
                        "--snapshot-identity-sha256" => &mut snapshot_identity_sha256,
                        _ => unreachable!(),
                    };
                    if destination.replace(value.clone()).is_some() {
                        return Err(ControllerCommandError::ArgumentInvalid);
                    }
                    index += 2;
                }
                _ => return Err(ControllerCommandError::ArgumentInvalid),
            }
        }
        if !authorized_l6_crash {
            return Err(ControllerCommandError::AuthorizationRequired);
        }
        let scenario = L6CrashScenario::parse(
            scenario
                .as_deref()
                .ok_or(ControllerCommandError::ArgumentInvalid)?,
        )
        .map_err(|_| ControllerCommandError::ArgumentInvalid)?;
        let repository_commit = repository_commit.ok_or(ControllerCommandError::ArgumentInvalid)?;
        validate_lower_hex(&repository_commit, 40)?;
        let guest_identity_sha256 =
            guest_identity_sha256.ok_or(ControllerCommandError::ArgumentInvalid)?;
        validate_lower_hex(&guest_identity_sha256, 64)?;
        let snapshot_identity_sha256 =
            snapshot_identity_sha256.ok_or(ControllerCommandError::ArgumentInvalid)?;
        validate_lower_hex(&snapshot_identity_sha256, 64)?;
        let maintenance = LinuxMaintenanceCommand::parse(maintenance_arguments)
            .map_err(|_| ControllerCommandError::MaintenanceCommandInvalid)?;
        if !maintenance.is_start()
            || maintenance.operation_kind() != Some(scenario.operation_kind())
        {
            return Err(ControllerCommandError::ScenarioOperationMismatch);
        }
        Ok(Self {
            scenario,
            repository_commit,
            guest_identity_sha256,
            snapshot_identity_sha256,
            maintenance,
            maintenance_arguments: maintenance_arguments.to_vec(),
        })
    }

    pub const fn scenario(&self) -> L6CrashScenario {
        self.scenario
    }

    pub fn repository_commit(&self) -> &str {
        &self.repository_commit
    }

    pub fn guest_identity_sha256(&self) -> &str {
        &self.guest_identity_sha256
    }

    pub fn snapshot_identity_sha256(&self) -> &str {
        &self.snapshot_identity_sha256
    }

    pub fn maintenance(&self) -> &LinuxMaintenanceCommand {
        &self.maintenance
    }

    pub fn maintenance_arguments(&self) -> &[String] {
        &self.maintenance_arguments
    }

    pub fn worker_arguments(&self) -> Vec<String> {
        let mut arguments = vec![
            WORKER_VERB.to_owned(),
            "--scenario".to_owned(),
            self.scenario.as_str().to_owned(),
            "--authorized-l6-crash".to_owned(),
            "--".to_owned(),
        ];
        arguments.extend(self.maintenance_arguments.iter().cloned());
        arguments
    }
}

impl fmt::Debug for ControllerCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControllerCommand")
            .field("scenario", &self.scenario)
            .field("repository_commit", &self.repository_commit)
            .field("guest_identity_sha256", &self.guest_identity_sha256)
            .field("snapshot_identity_sha256", &self.snapshot_identity_sha256)
            .field("maintenance", &"[redacted]")
            .finish()
    }
}

#[derive(Clone)]
pub struct WorkerCommand {
    scenario: L6CrashScenario,
    maintenance: LinuxMaintenanceCommand,
}

impl WorkerCommand {
    pub fn parse(arguments: &[String]) -> Result<Self, ControllerCommandError> {
        let (verb, remainder) = arguments
            .split_first()
            .ok_or(ControllerCommandError::ArgumentInvalid)?;
        if verb != WORKER_VERB {
            return Err(ControllerCommandError::ArgumentInvalid);
        }
        let (worker_arguments, maintenance_arguments) = split_maintenance(remainder)?;
        if worker_arguments.len() != 3
            || worker_arguments[0] != "--scenario"
            || worker_arguments[2] != "--authorized-l6-crash"
        {
            return Err(ControllerCommandError::AuthorizationRequired);
        }
        let scenario = L6CrashScenario::parse(&worker_arguments[1])
            .map_err(|_| ControllerCommandError::ArgumentInvalid)?;
        let maintenance = LinuxMaintenanceCommand::parse(maintenance_arguments)
            .map_err(|_| ControllerCommandError::MaintenanceCommandInvalid)?;
        if !maintenance.is_start()
            || maintenance.operation_kind() != Some(scenario.operation_kind())
        {
            return Err(ControllerCommandError::ScenarioOperationMismatch);
        }
        Ok(Self {
            scenario,
            maintenance,
        })
    }

    pub const fn scenario(&self) -> L6CrashScenario {
        self.scenario
    }

    pub fn maintenance(&self) -> &LinuxMaintenanceCommand {
        &self.maintenance
    }
}

impl fmt::Debug for WorkerCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerCommand")
            .field("scenario", &self.scenario)
            .field("maintenance", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerCommandError {
    AuthorizationRequired,
    ArgumentInvalid,
    MaintenanceCommandInvalid,
    ScenarioOperationMismatch,
}

impl fmt::Display for ControllerCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::AuthorizationRequired => "explicit L6 crash authorization is required",
            Self::ArgumentInvalid => "L6 controller arguments are invalid",
            Self::MaintenanceCommandInvalid => "authorized maintenance command is invalid",
            Self::ScenarioOperationMismatch => "L6 scenario and maintenance operation differ",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ControllerCommandError {}

fn split_maintenance(
    arguments: &[String],
) -> Result<(&[String], &[String]), ControllerCommandError> {
    let separator = arguments
        .iter()
        .position(|argument| argument == "--")
        .ok_or(ControllerCommandError::ArgumentInvalid)?;
    let (controller, maintenance_with_separator) = arguments.split_at(separator);
    let maintenance = maintenance_with_separator
        .get(1..)
        .ok_or(ControllerCommandError::ArgumentInvalid)?;
    if maintenance.is_empty() || maintenance.iter().any(|argument| argument == "--") {
        return Err(ControllerCommandError::ArgumentInvalid);
    }
    Ok((controller, maintenance))
}

fn validate_lower_hex(value: &str, length: usize) -> Result<(), ControllerCommandError> {
    if value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(ControllerCommandError::ArgumentInvalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPERATION_ID: &str = "0123456789abcdef0123456789abcdef";

    fn maintenance(kind: &str) -> Vec<String> {
        let mut arguments = vec![
            "start".to_owned(),
            "--operation-id".to_owned(),
            OPERATION_ID.to_owned(),
            "--kind".to_owned(),
            kind.to_owned(),
            "--authorized-system-mutation".to_owned(),
            "--preserve-user-data".to_owned(),
        ];
        match kind {
            "install" => arguments.extend([
                "--target-deb".to_owned(),
                "/var/tmp/radishlex_1.0-1_arm64.deb".to_owned(),
                "--target-evidence".to_owned(),
                "/var/tmp/radishlex_1.0-1_arm64.deb.evidence.json".to_owned(),
            ]),
            "upgrade" => arguments.extend([
                "--source-deb".to_owned(),
                "/var/tmp/radishlex_1.0-1_arm64.deb".to_owned(),
                "--source-evidence".to_owned(),
                "/var/tmp/radishlex_1.0-1_arm64.deb.evidence.json".to_owned(),
                "--target-deb".to_owned(),
                "/var/tmp/radishlex_1.0-2_arm64.deb".to_owned(),
                "--target-evidence".to_owned(),
                "/var/tmp/radishlex_1.0-2_arm64.deb.evidence.json".to_owned(),
            ]),
            _ => unreachable!(),
        }
        arguments
    }

    fn controller(scenario: &str, kind: &str) -> Vec<String> {
        let mut arguments = vec![
            "crash".to_owned(),
            "--scenario".to_owned(),
            scenario.to_owned(),
            "--repository-commit".to_owned(),
            "a".repeat(40),
            "--guest-identity-sha256".to_owned(),
            "b".repeat(64),
            "--snapshot-identity-sha256".to_owned(),
            "c".repeat(64),
            "--authorized-l6-crash".to_owned(),
            "--".to_owned(),
        ];
        arguments.extend(maintenance(kind));
        arguments
    }

    #[test]
    fn controller_requires_all_authorizations_and_matching_operation() {
        let parsed = ControllerCommand::parse(&controller("install_prepared", "install"))
            .expect("parse authorized controller");
        assert_eq!(parsed.scenario(), L6CrashScenario::InstallPrepared);
        assert!(!format!("{parsed:?}").contains(OPERATION_ID));

        for authorization in [
            "--authorized-l6-crash",
            "--authorized-system-mutation",
            "--preserve-user-data",
        ] {
            let mut unauthorized = controller("install_prepared", "install");
            unauthorized.retain(|argument| argument != authorization);
            assert!(ControllerCommand::parse(&unauthorized).is_err());
        }
        assert_eq!(
            ControllerCommand::parse(&controller("upgrade_after_dpkg", "install")).unwrap_err(),
            ControllerCommandError::ScenarioOperationMismatch
        );
    }

    #[test]
    fn worker_is_only_formed_from_the_acceptance_identity_arguments() {
        let command = ControllerCommand::parse(&controller("upgrade_after_dpkg", "upgrade"))
            .expect("parse controller");
        let worker = WorkerCommand::parse(&command.worker_arguments()).expect("parse worker");
        assert_eq!(worker.scenario(), L6CrashScenario::UpgradeAfterDpkg);
        let mut unauthorized = command.worker_arguments();
        unauthorized.retain(|argument| argument != "--authorized-l6-crash");
        assert!(WorkerCommand::parse(&unauthorized).is_err());
    }
}
