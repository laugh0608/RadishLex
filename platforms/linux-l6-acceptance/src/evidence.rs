use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::command::ControllerCommand;
use crate::controller::{ProcessGroupProof, WorkerTermination};

pub const L6_CHECKPOINT_EVIDENCE_FORMAT: &str = "radishlex-linux-l6-checkpoint-evidence-v1";
pub const L6_ACCEPTANCE_BUILD_IDENTITY: &str = "radishlex-linux-l6-acceptance-v1";
pub const L6_EVIDENCE_ROOT: &str = "/var/tmp/radishlex-l6-evidence";
const MATRIX_PROFILE: &str = "debian13-arm64-ephemeral-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointEvidenceEnvelopeV1 {
    format: String,
    build_identity: String,
    matrix_format_version: u32,
    matrix_profile: String,
    repository_commit: String,
    guest_identity_sha256: String,
    snapshot_identity_sha256: String,
    scenario: String,
    checkpoint: String,
    operation: OperationEvidenceV1,
    authorization: AuthorizationEvidenceV1,
    fault: FaultEvidenceV1,
    termination: TerminationEvidenceV1,
    expected_terminal: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationEvidenceV1 {
    matrix_operation: String,
    operation_id_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorizationEvidenceV1 {
    l6_crash: String,
    system_mutation: String,
    user_data_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FaultEvidenceV1 {
    requested: String,
    target_validation_rejection: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminationEvidenceV1 {
    checkpoint_notification: String,
    process_group: String,
    signal: String,
    worker: String,
    process_group_member_count: u32,
    dpkg_child: String,
    process_inspection: String,
}

impl CheckpointEvidenceEnvelopeV1 {
    pub fn new(
        command: &ControllerCommand,
        termination: WorkerTermination,
        proof: ProcessGroupProof,
    ) -> Result<Self, CheckpointEvidenceError> {
        if termination.signal() != Some(9) || !proof.proves_empty_without_dpkg() {
            return Err(CheckpointEvidenceError::Invalid);
        }
        let scenario = command.scenario();
        let envelope = Self {
            format: L6_CHECKPOINT_EVIDENCE_FORMAT.to_owned(),
            build_identity: L6_ACCEPTANCE_BUILD_IDENTITY.to_owned(),
            matrix_format_version: 1,
            matrix_profile: MATRIX_PROFILE.to_owned(),
            repository_commit: command.repository_commit().to_owned(),
            guest_identity_sha256: command.guest_identity_sha256().to_owned(),
            snapshot_identity_sha256: command.snapshot_identity_sha256().to_owned(),
            scenario: scenario.as_str().to_owned(),
            checkpoint: scenario.checkpoint().as_str().to_owned(),
            operation: OperationEvidenceV1 {
                matrix_operation: scenario.operation_id().to_owned(),
                operation_id_sha256: sha256_text(command.maintenance().operation_id()),
            },
            authorization: AuthorizationEvidenceV1 {
                l6_crash: "explicit".to_owned(),
                system_mutation: "explicit".to_owned(),
                user_data_policy: "preserve".to_owned(),
            },
            fault: FaultEvidenceV1 {
                requested: scenario.fault().to_owned(),
                target_validation_rejection: scenario.reject_target_validation(),
            },
            termination: TerminationEvidenceV1 {
                checkpoint_notification: "inherited-pipe-v1".to_owned(),
                process_group: "terminated".to_owned(),
                signal: "sigkill".to_owned(),
                worker: "signaled".to_owned(),
                process_group_member_count: 0,
                dpkg_child: "absent".to_owned(),
                process_inspection: "complete".to_owned(),
            },
            expected_terminal: scenario.expected_terminal().to_owned(),
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CheckpointEvidenceError> {
        self.validate()?;
        let mut bytes =
            serde_json::to_vec_pretty(self).map_err(|_| CheckpointEvidenceError::Json)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, CheckpointEvidenceError> {
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| CheckpointEvidenceError::Json)?;
        value.validate()?;
        if value.canonical_bytes()? != bytes {
            return Err(CheckpointEvidenceError::NonCanonical);
        }
        Ok(value)
    }

    pub fn operation_id_sha256(&self) -> &str {
        &self.operation.operation_id_sha256
    }

    pub fn scenario(&self) -> &str {
        &self.scenario
    }

    fn validate(&self) -> Result<(), CheckpointEvidenceError> {
        if self.format != L6_CHECKPOINT_EVIDENCE_FORMAT
            || self.build_identity != L6_ACCEPTANCE_BUILD_IDENTITY
            || self.matrix_format_version != 1
            || self.matrix_profile != MATRIX_PROFILE
            || !is_lower_hex(&self.repository_commit, 40)
            || !is_lower_hex(&self.guest_identity_sha256, 64)
            || !is_lower_hex(&self.snapshot_identity_sha256, 64)
            || !is_lower_hex(&self.operation.operation_id_sha256, 64)
            || self.authorization.l6_crash != "explicit"
            || self.authorization.system_mutation != "explicit"
            || self.authorization.user_data_policy != "preserve"
            || self.termination.checkpoint_notification != "inherited-pipe-v1"
            || self.termination.process_group != "terminated"
            || self.termination.signal != "sigkill"
            || self.termination.worker != "signaled"
            || self.termination.process_group_member_count != 0
            || self.termination.dpkg_child != "absent"
            || self.termination.process_inspection != "complete"
        {
            return Err(CheckpointEvidenceError::Invalid);
        }
        let scenario = crate::scenario::L6CrashScenario::parse(&self.scenario)
            .map_err(|_| CheckpointEvidenceError::Invalid)?;
        if self.checkpoint != scenario.checkpoint().as_str()
            || self.operation.matrix_operation != scenario.operation_id()
            || self.fault.requested != scenario.fault()
            || self.fault.target_validation_rejection != scenario.reject_target_validation()
            || self.expected_terminal != scenario.expected_terminal()
        {
            return Err(CheckpointEvidenceError::Invalid);
        }
        Ok(())
    }
}

#[cfg(any(target_os = "linux", test))]
pub fn write_system_checkpoint_evidence(
    envelope: &CheckpointEvidenceEnvelopeV1,
) -> Result<(), CheckpointEvidenceError> {
    use std::fs::{self, DirBuilder, File, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
    use std::path::Path;

    fn ensure_root_directory(path: &Path) -> Result<(), CheckpointEvidenceError> {
        match fs::symlink_metadata(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                DirBuilder::new()
                    .mode(0o700)
                    .create(path)
                    .map_err(|_| CheckpointEvidenceError::Store)?;
            }
            Err(_) => return Err(CheckpointEvidenceError::Store),
        }
        let metadata = fs::symlink_metadata(path).map_err(|_| CheckpointEvidenceError::Store)?;
        let canonical = fs::canonicalize(path).map_err(|_| CheckpointEvidenceError::Store)?;
        if !metadata.file_type().is_dir()
            || canonical != path
            || metadata.uid() != 0
            || metadata.gid() != 0
            || metadata.mode() & 0o7777 != 0o700
        {
            return Err(CheckpointEvidenceError::Store);
        }
        Ok(())
    }

    fn sync_directory(path: &Path) -> Result<(), CheckpointEvidenceError> {
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| CheckpointEvidenceError::Store)
    }

    let root = Path::new(L6_EVIDENCE_ROOT);
    ensure_root_directory(root)?;
    let checkpoint_root = root.join("checkpoints");
    ensure_root_directory(&checkpoint_root)?;
    sync_directory(root)?;
    let operation_hash = envelope.operation_id_sha256();
    let filename = format!(
        "{}-{}.json",
        envelope.scenario(),
        operation_hash
            .get(..16)
            .ok_or(CheckpointEvidenceError::Invalid)?
    );
    let path = checkpoint_root.join(filename);
    let bytes = envelope.canonical_bytes()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| CheckpointEvidenceError::Store)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| CheckpointEvidenceError::Store)?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| CheckpointEvidenceError::Store)?;
    if !metadata.file_type().is_file()
        || metadata.uid() != 0
        || metadata.gid() != 0
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() != bytes.len() as u64
    {
        return Err(CheckpointEvidenceError::Store);
    }
    sync_directory(&checkpoint_root)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointEvidenceError {
    Invalid,
    Json,
    NonCanonical,
    Store,
    EnvironmentUnsupported,
}

impl fmt::Display for CheckpointEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Invalid => "L6 checkpoint evidence is invalid",
            Self::Json => "L6 checkpoint evidence JSON failed",
            Self::NonCanonical => "L6 checkpoint evidence is not canonical",
            Self::Store => "L6 checkpoint evidence store failed",
            Self::EnvironmentUnsupported => "L6 checkpoint evidence requires Linux",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CheckpointEvidenceError {}

pub(crate) fn sha256_text(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::command::ControllerCommand;

    const OPERATION_ID: &str = "0123456789abcdef0123456789abcdef";

    fn command() -> ControllerCommand {
        let arguments = vec![
            "crash".to_owned(),
            "--scenario".to_owned(),
            "upgrade_after_source_restore".to_owned(),
            "--repository-commit".to_owned(),
            "a".repeat(40),
            "--guest-identity-sha256".to_owned(),
            "b".repeat(64),
            "--snapshot-identity-sha256".to_owned(),
            "c".repeat(64),
            "--authorized-l6-crash".to_owned(),
            "--".to_owned(),
            "start".to_owned(),
            "--operation-id".to_owned(),
            OPERATION_ID.to_owned(),
            "--kind".to_owned(),
            "upgrade".to_owned(),
            "--source-deb".to_owned(),
            "/var/tmp/radishlex_1.0-1_arm64.deb".to_owned(),
            "--source-evidence".to_owned(),
            "/var/tmp/radishlex_1.0-1_arm64.deb.evidence.json".to_owned(),
            "--target-deb".to_owned(),
            "/var/tmp/radishlex_1.0-2_arm64.deb".to_owned(),
            "--target-evidence".to_owned(),
            "/var/tmp/radishlex_1.0-2_arm64.deb.evidence.json".to_owned(),
            "--authorized-system-mutation".to_owned(),
            "--preserve-user-data".to_owned(),
        ];
        ControllerCommand::parse(&arguments).expect("parse controller")
    }

    #[test]
    fn envelope_is_versioned_canonical_and_contains_only_the_operation_hash() {
        let envelope = CheckpointEvidenceEnvelopeV1::new(
            &command(),
            WorkerTermination::signaled(9),
            ProcessGroupProof::new(0, 0, true),
        )
        .expect("build envelope");
        let bytes = envelope.canonical_bytes().expect("encode envelope");
        assert_eq!(
            CheckpointEvidenceEnvelopeV1::decode_canonical(&bytes).expect("decode envelope"),
            envelope
        );
        let text = String::from_utf8(bytes).expect("UTF-8 evidence");
        assert!(!text.contains(OPERATION_ID));
        assert!(!text.contains("/var/tmp/radishlex_1.0-1_arm64.deb"));
        assert!(!text.contains("/proc/"));
        assert!(!text.contains("dpkg stdout"));
        assert!(!text.contains("synthetic-user-term"));
        assert_eq!(envelope.operation_id_sha256(), sha256_text(OPERATION_ID));
    }

    #[test]
    fn unknown_noncanonical_and_raw_identifier_fields_are_rejected() {
        let envelope = CheckpointEvidenceEnvelopeV1::new(
            &command(),
            WorkerTermination::signaled(9),
            ProcessGroupProof::new(0, 0, true),
        )
        .expect("build envelope");
        let bytes = envelope.canonical_bytes().expect("encode envelope");
        let mut value: Value = serde_json::from_slice(&bytes).expect("parse JSON");
        value["pid"] = Value::from(4242);
        let mut unknown = serde_json::to_vec_pretty(&value).expect("encode unknown");
        unknown.push(b'\n');
        assert_eq!(
            CheckpointEvidenceEnvelopeV1::decode_canonical(&unknown).unwrap_err(),
            CheckpointEvidenceError::Json
        );
        let compact = serde_json::to_vec(&envelope).expect("compact JSON");
        assert_eq!(
            CheckpointEvidenceEnvelopeV1::decode_canonical(&compact).unwrap_err(),
            CheckpointEvidenceError::NonCanonical
        );
    }
}
