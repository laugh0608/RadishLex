use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::model::{
    ArtifactSlot, LinuxInstallReceipt, LinuxInstallReceiptError, LinuxInstallRootIdentity,
    LinuxInstallState, StagedArtifactEvidence, MAX_LINUX_INSTALL_RECEIPT_BYTES,
};

mod filesystem;
mod guard;
mod mode;
mod operation;

pub use filesystem::StagedArtifactPaths;

use filesystem::{
    artifact_file_identity, ensure_directory, ensure_state_parent, read_stable_regular_file,
    remove_exact_regular_file, stage_file, staged_paths, sync_directory, validate_absolute_leaf,
    validate_directory, validate_operation_files, validate_root_entries, validate_secure_parent,
    validate_source_artifact,
};
use guard::{acquire_guard_lock, remove_locked_guard_path, verify_guard_lock, GuardLock};
use mode::set_regular_file_mode_and_sync;
use operation::validate_current_operation_slots;

pub const SYSTEM_STATE_ROOT: &str = "/var/lib/radishlex/install-v1";
pub const SYSTEM_GUARD_PATH: &str = "/run/lock/radishlex-install-v1.lock";

const RECEIPT_FILENAME: &str = "receipt.json";
const RECEIPT_TMP_FILENAME: &str = "receipt.json.tmp";
const OPERATIONS_DIRECTORY: &str = "operations";
const STAGED_TEMP_SUFFIX: &str = ".tmp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxInstallStoreErrorCode {
    Io,
    PermissionDenied,
    PathInvalid,
    IdentityChanged,
    ReceiptInvalid,
    InterruptedWrite,
    ReceiptReplacementDenied,
    GuardActive,
    GuardInvalid,
    ArtifactInvalid,
}

#[derive(Debug)]
pub struct LinuxInstallStoreError {
    code: LinuxInstallStoreErrorCode,
    message: String,
    source: Option<io::Error>,
}

impl LinuxInstallStoreError {
    fn new(code: LinuxInstallStoreErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }

    fn io(context: &'static str, source: io::Error) -> Self {
        let code = if source.kind() == io::ErrorKind::PermissionDenied {
            LinuxInstallStoreErrorCode::PermissionDenied
        } else {
            LinuxInstallStoreErrorCode::Io
        };
        Self {
            code,
            message: context.to_owned(),
            source: Some(source),
        }
    }

    pub const fn code(&self) -> LinuxInstallStoreErrorCode {
        self.code
    }
}

impl fmt::Display for LinuxInstallStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(formatter, "{}: {}", self.message, source)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl std::error::Error for LinuxInstallStoreError {}

impl From<LinuxInstallReceiptError> for LinuxInstallStoreError {
    fn from(value: LinuxInstallReceiptError) -> Self {
        Self::new(
            LinuxInstallStoreErrorCode::ReceiptInvalid,
            value.to_string(),
        )
    }
}

#[derive(Debug)]
pub struct LinuxInstallStore {
    state_root: PathBuf,
    guard_path: PathBuf,
    expected_owner_id: u32,
    expected_group_id: u32,
    root_identity: LinuxInstallRootIdentity,
}

impl LinuxInstallStore {
    pub fn bootstrap_system() -> Result<Self, LinuxInstallStoreError> {
        let state_parent = Path::new(SYSTEM_STATE_ROOT).parent().ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::PathInvalid,
                "system state root has no parent",
            )
        })?;
        ensure_state_parent(state_parent, 0, 0)?;
        Self::bootstrap_at(
            Path::new(SYSTEM_STATE_ROOT),
            Path::new(SYSTEM_GUARD_PATH),
            0,
            0,
        )
    }

    pub(crate) fn bootstrap_at(
        state_root: &Path,
        guard_path: &Path,
        expected_owner_id: u32,
        expected_group_id: u32,
    ) -> Result<Self, LinuxInstallStoreError> {
        validate_absolute_leaf(state_root, "state root")?;
        validate_absolute_leaf(guard_path, "guard path")?;
        validate_secure_parent(
            state_root.parent().ok_or_else(|| {
                LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::PathInvalid,
                    "state root has no parent",
                )
            })?,
            expected_owner_id,
            expected_group_id,
            false,
        )?;
        ensure_directory(state_root, 0o755)?;
        let root_metadata =
            validate_directory(state_root, 0o755, expected_owner_id, expected_group_id)?;
        sync_directory(state_root.parent().ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::PathInvalid,
                "state root has no parent",
            )
        })?)?;
        let operations = state_root.join(OPERATIONS_DIRECTORY);
        ensure_directory(&operations, 0o755)?;
        validate_directory(&operations, 0o755, expected_owner_id, expected_group_id)?;
        sync_directory(state_root)?;
        validate_root_entries(state_root, expected_owner_id, expected_group_id, true)?;
        validate_secure_parent(
            guard_path.parent().ok_or_else(|| {
                LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::PathInvalid,
                    "guard path has no parent",
                )
            })?,
            expected_owner_id,
            expected_group_id,
            true,
        )?;
        let root_identity = LinuxInstallRootIdentity::new(
            root_metadata.dev(),
            root_metadata.ino(),
            root_metadata.uid(),
            root_metadata.gid(),
            root_metadata.mode() & 0o7777,
        )?;
        Ok(Self {
            state_root: state_root.to_owned(),
            guard_path: guard_path.to_owned(),
            expected_owner_id,
            expected_group_id,
            root_identity,
        })
    }

    pub fn root_identity(&self) -> &LinuxInstallRootIdentity {
        &self.root_identity
    }

    pub fn acquire_guard(&self) -> Result<LinuxInstallGuard, LinuxInstallStoreError> {
        self.verify_root_allow_interrupted()?;
        let lock = acquire_guard_lock(
            &self.guard_path,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        let guard = LinuxInstallGuard {
            lock,
            path: self.guard_path.clone(),
        };
        self.reconcile_interrupted_receipt(&guard)?;
        self.verify_root()?;
        Ok(guard)
    }

    pub fn verify_guard(&self, guard: &LinuxInstallGuard) -> Result<(), LinuxInstallStoreError> {
        self.verify_root()?;
        self.verify_guard_identity(guard)
    }

    fn verify_guard_allow_interrupted(
        &self,
        guard: &LinuxInstallGuard,
    ) -> Result<(), LinuxInstallStoreError> {
        self.verify_root_allow_interrupted()?;
        self.verify_guard_identity(guard)
    }

    fn verify_guard_identity(
        &self,
        guard: &LinuxInstallGuard,
    ) -> Result<(), LinuxInstallStoreError> {
        if guard.path != self.guard_path {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::GuardInvalid,
                "guard belongs to a different Linux install store",
            ));
        }
        verify_guard_lock(
            &self.guard_path,
            &guard.lock,
            self.expected_owner_id,
            self.expected_group_id,
        )
    }

    fn reconcile_interrupted_receipt(
        &self,
        guard: &LinuxInstallGuard,
    ) -> Result<(), LinuxInstallStoreError> {
        self.verify_guard_allow_interrupted(guard)?;
        let temporary_path = self.state_root.join(RECEIPT_TMP_FILENAME);
        let temporary = match fs::symlink_metadata(&temporary_path) {
            Ok(_) => {
                set_regular_file_mode_and_sync(
                    &temporary_path,
                    &[0o000, 0o200, 0o400, 0o600, 0o644],
                    0o600,
                    self.expected_owner_id,
                    self.expected_group_id,
                    MAX_LINUX_INSTALL_RECEIPT_BYTES as u64,
                    true,
                )?;
                read_stable_regular_file(
                    &temporary_path,
                    &[0o600],
                    self.expected_owner_id,
                    self.expected_group_id,
                    MAX_LINUX_INSTALL_RECEIPT_BYTES as u64,
                    true,
                )?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(LinuxInstallStoreError::io(
                    "inspect interrupted receipt",
                    error,
                ))
            }
        };
        let candidate = LinuxInstallReceipt::decode(&temporary.value).map_err(|_| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::InterruptedWrite,
                "interrupted receipt is incomplete or non-canonical",
            )
        })?;
        if candidate.root_identity() != &self.root_identity {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::IdentityChanged,
                "interrupted receipt belongs to a different state root inode",
            ));
        }
        self.validate_operation_entries(&candidate)?;

        let receipt_path = self.state_root.join(RECEIPT_FILENAME);
        let current = match fs::symlink_metadata(&receipt_path) {
            Ok(_) => Some(self.read_receipt_path(&receipt_path)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(LinuxInstallStoreError::io(
                    "inspect current receipt during recovery",
                    error,
                ))
            }
        };
        if current.as_ref() == Some(&candidate) {
            remove_exact_regular_file(&temporary_path, &temporary.identity)?;
            sync_directory(&self.state_root)?;
            return Ok(());
        }
        let replacement_allowed = current.as_ref().map_or_else(
            || {
                candidate.state() == LinuxInstallState::Prepared
                    && candidate.operation_chain().len() == 1
            },
            |current| candidate.can_replace(current),
        );
        if !replacement_allowed {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::InterruptedWrite,
                "interrupted receipt is not an append-only replacement",
            ));
        }
        set_regular_file_mode_and_sync(
            &temporary_path,
            &[0o600],
            0o644,
            self.expected_owner_id,
            self.expected_group_id,
            MAX_LINUX_INSTALL_RECEIPT_BYTES as u64,
            false,
        )?;
        fs::rename(&temporary_path, &receipt_path)
            .map_err(|error| LinuxInstallStoreError::io("recover interrupted receipt", error))?;
        sync_directory(&self.state_root)?;
        let recovered = self.read_receipt_path(&receipt_path)?;
        if recovered != candidate {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptInvalid,
                "recovered receipt differs from interrupted receipt",
            ));
        }
        Ok(())
    }

    fn read_receipt_path(
        &self,
        path: &Path,
    ) -> Result<LinuxInstallReceipt, LinuxInstallStoreError> {
        let value = read_stable_regular_file(
            path,
            &[0o644],
            self.expected_owner_id,
            self.expected_group_id,
            MAX_LINUX_INSTALL_RECEIPT_BYTES as u64,
            false,
        )?
        .value;
        let receipt = LinuxInstallReceipt::decode(&value)?;
        if receipt.root_identity() != &self.root_identity {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::IdentityChanged,
                "receipt belongs to a different state root inode",
            ));
        }
        self.validate_operation_entries(&receipt)?;
        Ok(receipt)
    }

    pub fn load_receipt(&self) -> Result<Option<LinuxInstallReceipt>, LinuxInstallStoreError> {
        self.verify_root()?;
        self.load_receipt_without_root_check()
    }

    fn load_receipt_without_root_check(
        &self,
    ) -> Result<Option<LinuxInstallReceipt>, LinuxInstallStoreError> {
        let path = self.state_root.join(RECEIPT_FILENAME);
        match fs::symlink_metadata(&path) {
            Ok(_) => self.read_receipt_path(&path).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut operations = fs::read_dir(self.state_root.join(OPERATIONS_DIRECTORY))
                    .map_err(|error| {
                        LinuxInstallStoreError::io("read operations directory", error)
                    })?;
                if operations.next().is_some() {
                    return Err(LinuxInstallStoreError::new(
                        LinuxInstallStoreErrorCode::ReceiptInvalid,
                        "operation staging exists without a receipt",
                    ));
                }
                Ok(None)
            }
            Err(error) => Err(LinuxInstallStoreError::io("inspect Linux receipt", error)),
        }
    }

    pub fn persist_receipt(
        &self,
        guard: &LinuxInstallGuard,
        receipt: &LinuxInstallReceipt,
    ) -> Result<(), LinuxInstallStoreError> {
        self.verify_guard(guard)?;
        if receipt.root_identity() != &self.root_identity {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::IdentityChanged,
                "receipt state root identity differs from the current root",
            ));
        }
        match self.load_receipt()? {
            Some(current) if receipt == &current => return Ok(()),
            Some(current) if !receipt.can_replace(&current) => {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::ReceiptReplacementDenied,
                    "receipt update is not append-only",
                ));
            }
            None if receipt.state() != LinuxInstallState::Prepared
                || receipt.operation_chain().len() != 1 =>
            {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::ReceiptReplacementDenied,
                    "first receipt must be a single prepared operation",
                ));
            }
            Some(_) | None => {}
        }
        let value = receipt.encode()?;
        let temporary_path = self.state_root.join(RECEIPT_TMP_FILENAME);
        let receipt_path = self.state_root.join(RECEIPT_FILENAME);
        let mut temporary = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary_path)
            .map_err(|error| LinuxInstallStoreError::io("create receipt temporary file", error))?;
        temporary
            .write_all(&value)
            .and_then(|()| temporary.sync_all())
            .map_err(|error| LinuxInstallStoreError::io("write receipt temporary file", error))?;
        drop(temporary);
        set_regular_file_mode_and_sync(
            &temporary_path,
            &[0o000, 0o200, 0o400, 0o600],
            0o644,
            self.expected_owner_id,
            self.expected_group_id,
            MAX_LINUX_INSTALL_RECEIPT_BYTES as u64,
            false,
        )?;
        fs::rename(&temporary_path, &receipt_path)
            .map_err(|error| LinuxInstallStoreError::io("replace Linux receipt", error))?;
        sync_directory(&self.state_root)?;
        let persisted = self.load_receipt()?.ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptInvalid,
                "persisted receipt disappeared",
            )
        })?;
        if &persisted != receipt {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptInvalid,
                "persisted receipt differs from requested receipt",
            ));
        }
        Ok(())
    }

    pub fn stage_artifact(
        &self,
        guard: &LinuxInstallGuard,
        slot: ArtifactSlot,
        package_source: &Path,
        evidence_source: &Path,
    ) -> Result<LinuxInstallReceipt, LinuxInstallStoreError> {
        self.verify_guard(guard)?;
        let mut receipt = self.load_receipt()?.ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptInvalid,
                "artifact staging requires a prepared receipt",
            )
        })?;
        if receipt.state() != LinuxInstallState::Prepared {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptReplacementDenied,
                "artifact staging is closed after the prepared state",
            ));
        }
        if !receipt.required_slots().contains(&slot) {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ArtifactInvalid,
                "artifact slot is not required by this operation",
            ));
        }
        let artifact = receipt.artifact_for_slot(slot).cloned().ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ArtifactInvalid,
                "artifact slot is not required by this operation",
            )
        })?;
        validate_source_artifact(
            package_source,
            artifact.package_filename(),
            artifact.package_size(),
            artifact.package_sha256(),
            &self.state_root,
        )?;
        validate_source_artifact(
            evidence_source,
            artifact.evidence_filename(),
            artifact.evidence_size(),
            artifact.evidence_sha256(),
            &self.state_root,
        )?;

        let operation_directory = self.operation_directory(receipt.operation_id());
        ensure_directory(&operation_directory, 0o700)?;
        validate_directory(
            &operation_directory,
            0o700,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        sync_directory(&self.state_root.join(OPERATIONS_DIRECTORY))?;
        let paths = staged_paths(&operation_directory, slot);
        stage_file(
            package_source,
            &paths.package_path,
            artifact.package_size(),
            artifact.package_sha256(),
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        stage_file(
            evidence_source,
            &paths.evidence_path,
            artifact.evidence_size(),
            artifact.evidence_sha256(),
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        sync_directory(&operation_directory)?;
        let package_file = artifact_file_identity(
            &paths.package_path,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        let evidence_file = artifact_file_identity(
            &paths.evidence_path,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        let staged = StagedArtifactEvidence::new(slot, artifact, package_file, evidence_file)?;
        if receipt.staged_artifact(slot).is_none() {
            receipt.record_staged_artifact(staged)?;
            self.persist_receipt(guard, &receipt)?;
        } else if receipt.staged_artifact(slot) != Some(&staged) {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ArtifactInvalid,
                "staged artifact identity changed",
            ));
        }
        Ok(receipt)
    }

    pub fn finish_staging(
        &self,
        guard: &LinuxInstallGuard,
    ) -> Result<LinuxInstallReceipt, LinuxInstallStoreError> {
        self.verify_guard(guard)?;
        let mut receipt = self.load_receipt()?.ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptInvalid,
                "artifact staging requires a prepared receipt",
            )
        })?;
        if receipt.state() == LinuxInstallState::ArtifactsStaged {
            for slot in receipt.required_slots() {
                self.staged_artifact_paths(receipt.operation_id(), *slot)?;
            }
            return Ok(receipt);
        }
        if receipt.state() != LinuxInstallState::Prepared {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptReplacementDenied,
                "artifact staging cannot finish from the current operation state",
            ));
        }
        self.verify_prepared_staging(&receipt)?;
        receipt.advance(LinuxInstallState::ArtifactsStaged)?;
        self.persist_receipt(guard, &receipt)?;
        Ok(receipt)
    }

    pub(crate) fn verify_prepared_staging_complete(
        &self,
        guard: &LinuxInstallGuard,
    ) -> Result<LinuxInstallReceipt, LinuxInstallStoreError> {
        self.verify_guard(guard)?;
        let receipt = self.load_receipt()?.ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptInvalid,
                "artifact staging requires a prepared receipt",
            )
        })?;
        if receipt.state() != LinuxInstallState::Prepared {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptReplacementDenied,
                "prepared staging checkpoint requires the prepared state",
            ));
        }
        self.verify_prepared_staging(&receipt)?;
        Ok(receipt)
    }

    fn verify_prepared_staging(
        &self,
        receipt: &LinuxInstallReceipt,
    ) -> Result<(), LinuxInstallStoreError> {
        for slot in receipt.required_slots() {
            if receipt.staged_artifact(*slot).is_none() {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::ArtifactInvalid,
                    "required artifact has not been staged",
                ));
            }
            self.staged_artifact_paths(receipt.operation_id(), *slot)?;
        }
        Ok(())
    }

    pub fn staged_artifact_paths(
        &self,
        operation_id: &str,
        slot: ArtifactSlot,
    ) -> Result<StagedArtifactPaths, LinuxInstallStoreError> {
        let receipt = self.load_receipt()?.ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ReceiptInvalid,
                "staged paths require an existing receipt",
            )
        })?;
        if receipt.operation_id() != operation_id {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ArtifactInvalid,
                "operation ID does not match the active receipt",
            ));
        }
        let expected = receipt.staged_artifact(slot).ok_or_else(|| {
            LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ArtifactInvalid,
                "artifact slot has no staged proof",
            )
        })?;
        let paths = staged_paths(&self.operation_directory(operation_id), slot);
        let package = artifact_file_identity(
            &paths.package_path,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        let evidence = artifact_file_identity(
            &paths.evidence_path,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        let actual =
            StagedArtifactEvidence::new(slot, expected.artifact().clone(), package, evidence)
                .map_err(|error| {
                    LinuxInstallStoreError::new(
                        LinuxInstallStoreErrorCode::ArtifactInvalid,
                        error.to_string(),
                    )
                })?;
        if &actual != expected {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::ArtifactInvalid,
                "staged artifact no longer matches its receipt proof",
            ));
        }
        Ok(paths)
    }

    fn verify_root(&self) -> Result<(), LinuxInstallStoreError> {
        self.verify_root_with_policy(false)
    }

    fn verify_root_allow_interrupted(&self) -> Result<(), LinuxInstallStoreError> {
        self.verify_root_with_policy(true)
    }

    fn verify_root_with_policy(
        &self,
        allow_interrupted_receipt: bool,
    ) -> Result<(), LinuxInstallStoreError> {
        validate_root_entries(
            &self.state_root,
            self.expected_owner_id,
            self.expected_group_id,
            allow_interrupted_receipt,
        )?;
        let metadata = validate_directory(
            &self.state_root,
            0o755,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        let actual = LinuxInstallRootIdentity::new(
            metadata.dev(),
            metadata.ino(),
            metadata.uid(),
            metadata.gid(),
            metadata.mode() & 0o7777,
        )?;
        if actual != self.root_identity {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::IdentityChanged,
                "Linux install state root identity changed",
            ));
        }
        validate_directory(
            &self.state_root.join(OPERATIONS_DIRECTORY),
            0o755,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        Ok(())
    }

    fn validate_operation_entries(
        &self,
        receipt: &LinuxInstallReceipt,
    ) -> Result<(), LinuxInstallStoreError> {
        let operations_root = self.state_root.join(OPERATIONS_DIRECTORY);
        let mut recorded = std::collections::BTreeSet::new();
        for entry in fs::read_dir(&operations_root)
            .map_err(|error| LinuxInstallStoreError::io("read operations directory", error))?
        {
            let entry =
                entry.map_err(|error| LinuxInstallStoreError::io("read operation entry", error))?;
            let name = entry.file_name();
            let name = name.to_str().ok_or_else(|| {
                LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::PathInvalid,
                    "operation directory name is not UTF-8",
                )
            })?;
            if !receipt.operation_chain().iter().any(|item| item == name) {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::PathInvalid,
                    "operations directory contains an unrecorded entry",
                ));
            }
            recorded.insert(name.to_owned());
            validate_directory(
                &entry.path(),
                0o700,
                self.expected_owner_id,
                self.expected_group_id,
            )?;
            validate_operation_files(
                &entry.path(),
                self.expected_owner_id,
                self.expected_group_id,
                receipt.state() == LinuxInstallState::Prepared && name == receipt.operation_id(),
                receipt.state() != LinuxInstallState::Prepared || name != receipt.operation_id(),
            )?;
            if name == receipt.operation_id() {
                validate_current_operation_slots(
                    &entry.path(),
                    receipt,
                    self.expected_owner_id,
                    self.expected_group_id,
                )?;
            }
        }
        for operation_id in receipt.operation_chain() {
            let current_prepared_without_staging = operation_id == receipt.operation_id()
                && receipt.state() == LinuxInstallState::Prepared
                && receipt
                    .required_slots()
                    .iter()
                    .all(|slot| receipt.staged_artifact(*slot).is_none());
            if !recorded.contains(operation_id) && !current_prepared_without_staging {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::ReceiptInvalid,
                    "receipt operation history is missing its staging directory",
                ));
            }
        }
        Ok(())
    }

    fn operation_directory(&self, operation_id: &str) -> PathBuf {
        self.state_root
            .join(OPERATIONS_DIRECTORY)
            .join(operation_id)
    }
}

#[derive(Debug)]
pub struct LinuxInstallGuard {
    lock: GuardLock,
    path: PathBuf,
}

impl Drop for LinuxInstallGuard {
    fn drop(&mut self) {
        remove_locked_guard_path(&self.path, &self.lock);
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
