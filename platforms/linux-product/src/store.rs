use std::fmt;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::{
    ArtifactFileIdentity, ArtifactSlot, LinuxInstallReceipt, LinuxInstallReceiptError,
    LinuxInstallRootIdentity, LinuxInstallState, StagedArtifactEvidence,
    MAX_LINUX_INSTALL_RECEIPT_BYTES,
};

pub const SYSTEM_STATE_ROOT: &str = "/var/lib/radishlex/install-v1";
pub const SYSTEM_GUARD_PATH: &str = "/run/lock/radishlex-install-v1.lock";

const RECEIPT_FILENAME: &str = "receipt.json";
const RECEIPT_TMP_FILENAME: &str = "receipt.json.tmp";
const OPERATIONS_DIRECTORY: &str = "operations";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxInstallStoreErrorCode {
    Io,
    PermissionDenied,
    PathInvalid,
    IdentityChanged,
    ReceiptInvalid,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedArtifactPaths {
    package_path: PathBuf,
    evidence_path: PathBuf,
}

impl StagedArtifactPaths {
    pub fn package_path(&self) -> &Path {
        &self.package_path
    }

    pub fn evidence_path(&self) -> &Path {
        &self.evidence_path
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
        let operations = state_root.join(OPERATIONS_DIRECTORY);
        ensure_directory(&operations, 0o755)?;
        validate_directory(&operations, 0o755, expected_owner_id, expected_group_id)?;
        validate_root_entries(state_root)?;
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
        self.verify_root()?;
        if let Ok(metadata) = fs::symlink_metadata(&self.guard_path) {
            if UnixStream::connect(&self.guard_path).is_ok() {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::GuardActive,
                    "another Linux package transaction owns the guard",
                ));
            }
            if !metadata.file_type().is_socket()
                || metadata.uid() != self.expected_owner_id
                || metadata.gid() != self.expected_group_id
                || metadata.mode() & 0o7777 != 0o600
                || metadata.nlink() != 1
            {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::GuardInvalid,
                    "stale guard path has an unexpected identity",
                ));
            }
            fs::remove_file(&self.guard_path)
                .map_err(|error| LinuxInstallStoreError::io("remove stale guard", error))?;
        }

        let listener = UnixListener::bind(&self.guard_path)
            .map_err(|error| LinuxInstallStoreError::io("bind Linux install guard", error))?;
        fs::set_permissions(&self.guard_path, fs::Permissions::from_mode(0o600))
            .map_err(|error| LinuxInstallStoreError::io("set Linux install guard mode", error))?;
        let metadata = validate_socket(
            &self.guard_path,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        Ok(LinuxInstallGuard {
            listener,
            path: self.guard_path.clone(),
            device_id: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    pub fn verify_guard(&self, guard: &LinuxInstallGuard) -> Result<(), LinuxInstallStoreError> {
        self.verify_root()?;
        if guard.path != self.guard_path {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::GuardInvalid,
                "guard belongs to a different Linux install store",
            ));
        }
        let metadata = validate_socket(
            &self.guard_path,
            self.expected_owner_id,
            self.expected_group_id,
        )?;
        if metadata.dev() != guard.device_id || metadata.ino() != guard.inode {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::GuardInvalid,
                "Linux install guard identity changed",
            ));
        }
        let _ = guard.listener.local_addr().map_err(|error| {
            LinuxInstallStoreError::io("inspect Linux install guard listener", error)
        })?;
        Ok(())
    }

    pub fn load_receipt(&self) -> Result<Option<LinuxInstallReceipt>, LinuxInstallStoreError> {
        self.verify_root()?;
        let path = self.state_root.join(RECEIPT_FILENAME);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                validate_regular_metadata(
                    &metadata,
                    0o644,
                    self.expected_owner_id,
                    self.expected_group_id,
                    MAX_LINUX_INSTALL_RECEIPT_BYTES as u64,
                )?;
                let mut value = Vec::with_capacity(metadata.len() as usize);
                File::open(&path)
                    .and_then(|mut file| file.read_to_end(&mut value))
                    .map_err(|error| LinuxInstallStoreError::io("read Linux receipt", error))?;
                let receipt = LinuxInstallReceipt::decode(&value)?;
                if receipt.root_identity() != &self.root_identity {
                    return Err(LinuxInstallStoreError::new(
                        LinuxInstallStoreErrorCode::IdentityChanged,
                        "receipt belongs to a different state root inode",
                    ));
                }
                self.validate_operation_entries(&receipt)?;
                Ok(Some(receipt))
            }
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
        fs::set_permissions(&temporary_path, fs::Permissions::from_mode(0o644))
            .map_err(|error| LinuxInstallStoreError::io("set receipt mode", error))?;
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
        let paths = staged_paths(&operation_directory, slot);
        stage_file(package_source, &paths.package_path)?;
        stage_file(evidence_source, &paths.evidence_path)?;
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
        for slot in receipt.required_slots() {
            if receipt.staged_artifact(*slot).is_none() {
                return Err(LinuxInstallStoreError::new(
                    LinuxInstallStoreErrorCode::ArtifactInvalid,
                    "required artifact has not been staged",
                ));
            }
            self.staged_artifact_paths(receipt.operation_id(), *slot)?;
        }
        receipt.advance(LinuxInstallState::ArtifactsStaged)?;
        self.persist_receipt(guard, &receipt)?;
        Ok(receipt)
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
        validate_root_entries(&self.state_root)?;
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
            )?;
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
    listener: UnixListener,
    path: PathBuf,
    device_id: u64,
    inode: u64,
}

impl Drop for LinuxInstallGuard {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::symlink_metadata(&self.path) {
            if metadata.file_type().is_socket()
                && metadata.dev() == self.device_id
                && metadata.ino() == self.inode
            {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

fn ensure_directory(path: &Path, mode: u32) -> Result<(), LinuxInstallStoreError> {
    match fs::create_dir(path) {
        Ok(()) => fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|error| LinuxInstallStoreError::io("set directory mode", error)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(LinuxInstallStoreError::io("create directory", error)),
    }
}

fn validate_absolute_leaf(path: &Path, label: &'static str) -> Result<(), LinuxInstallStoreError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PathInvalid,
            format!("{label} must be an absolute leaf path"),
        ));
    }
    Ok(())
}

fn validate_secure_parent(
    path: &Path,
    owner_id: u32,
    group_id: u32,
    allow_group_write: bool,
) -> Result<(), LinuxInstallStoreError> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| LinuxInstallStoreError::io("canonicalize parent directory", error))?;
    if canonical != path {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PathInvalid,
            "parent directory path contains a symlink or lexical alias",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect parent directory", error))?;
    let mode = metadata.mode() & 0o7777;
    let writable = if allow_group_write {
        mode & 0o002
    } else {
        mode & 0o022
    };
    if !metadata.file_type().is_dir()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || writable != 0
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::PermissionDenied,
            "parent directory ownership or write permissions are unsafe",
        ));
    }
    Ok(())
}

fn validate_directory(
    path: &Path,
    mode: u32,
    owner_id: u32,
    group_id: u32,
) -> Result<fs::Metadata, LinuxInstallStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect directory", error))?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || metadata.mode() & 0o7777 != mode
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "directory identity, ownership, or mode is invalid",
        ));
    }
    Ok(metadata)
}

fn validate_socket(
    path: &Path,
    owner_id: u32,
    group_id: u32,
) -> Result<fs::Metadata, LinuxInstallStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect guard socket", error))?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::GuardInvalid,
            "guard socket identity, ownership, or mode is invalid",
        ));
    }
    Ok(metadata)
}

fn validate_root_entries(state_root: &Path) -> Result<(), LinuxInstallStoreError> {
    for entry in fs::read_dir(state_root)
        .map_err(|error| LinuxInstallStoreError::io("read Linux install state root", error))?
    {
        let entry = entry.map_err(|error| LinuxInstallStoreError::io("read root entry", error))?;
        let allowed = matches!(
            entry.file_name().to_str(),
            Some(OPERATIONS_DIRECTORY | RECEIPT_FILENAME)
        );
        if !allowed {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::PathInvalid,
                "Linux install state root contains an unexpected entry",
            ));
        }
    }
    Ok(())
}

fn validate_operation_files(
    path: &Path,
    owner_id: u32,
    group_id: u32,
) -> Result<(), LinuxInstallStoreError> {
    for entry in fs::read_dir(path)
        .map_err(|error| LinuxInstallStoreError::io("read operation directory", error))?
    {
        let entry = entry
            .map_err(|error| LinuxInstallStoreError::io("read staged artifact entry", error))?;
        let valid = matches!(
            entry.file_name().to_str(),
            Some("source.deb" | "source.evidence.json" | "target.deb" | "target.evidence.json")
        );
        if !valid {
            return Err(LinuxInstallStoreError::new(
                LinuxInstallStoreErrorCode::PathInvalid,
                "operation directory contains an unexpected entry",
            ));
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| LinuxInstallStoreError::io("inspect staged operation file", error))?;
        validate_regular_metadata(&metadata, 0o600, owner_id, group_id, 512 * 1024 * 1024)?;
    }
    Ok(())
}

fn validate_regular_metadata(
    metadata: &fs::Metadata,
    mode: u32,
    owner_id: u32,
    group_id: u32,
    maximum_size: u64,
) -> Result<(), LinuxInstallStoreError> {
    if !metadata.file_type().is_file()
        || metadata.uid() != owner_id
        || metadata.gid() != group_id
        || metadata.mode() & 0o7777 != mode
        || metadata.nlink() != 1
        || metadata.len() == 0
        || metadata.len() > maximum_size
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::IdentityChanged,
            "regular file identity, ownership, mode, links, or size is invalid",
        ));
    }
    Ok(())
}

fn validate_source_artifact(
    path: &Path,
    expected_name: &str,
    expected_size: u64,
    expected_sha256: &str,
    state_root: &Path,
) -> Result<(), LinuxInstallStoreError> {
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name) {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source filename differs from the committed identity",
        ));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| LinuxInstallStoreError::io("canonicalize artifact source", error))?;
    if canonical != path || canonical.starts_with(state_root) {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source cannot contain symlinks or be inside the install state root",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect artifact source", error))?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.mode() & 0o022 != 0
        || metadata.len() != expected_size
        || sha256_file(path)? != expected_sha256
    {
        return Err(LinuxInstallStoreError::new(
            LinuxInstallStoreErrorCode::ArtifactInvalid,
            "artifact source identity, mode, size, or digest is invalid",
        ));
    }
    Ok(())
}

fn stage_file(source: &Path, target: &Path) -> Result<(), LinuxInstallStoreError> {
    if target.exists() {
        return Ok(());
    }
    let mut input = File::open(source)
        .map_err(|error| LinuxInstallStoreError::io("open artifact source", error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)
        .map_err(|error| LinuxInstallStoreError::io("create staged artifact", error))?;
    io::copy(&mut input, &mut output)
        .and_then(|_| output.sync_all())
        .map_err(|error| LinuxInstallStoreError::io("copy staged artifact", error))?;
    fs::set_permissions(target, fs::Permissions::from_mode(0o600))
        .map_err(|error| LinuxInstallStoreError::io("set staged artifact mode", error))
}

fn artifact_file_identity(
    path: &Path,
    owner_id: u32,
    group_id: u32,
) -> Result<ArtifactFileIdentity, LinuxInstallStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| LinuxInstallStoreError::io("inspect staged artifact", error))?;
    validate_regular_metadata(&metadata, 0o600, owner_id, group_id, 512 * 1024 * 1024)?;
    Ok(ArtifactFileIdentity::new(
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.gid(),
        metadata.mode() & 0o7777,
        metadata.nlink(),
        metadata.len(),
        sha256_file(path)?,
    )?)
}

fn staged_paths(operation_directory: &Path, slot: ArtifactSlot) -> StagedArtifactPaths {
    let prefix = match slot {
        ArtifactSlot::Source => "source",
        ArtifactSlot::Target => "target",
    };
    StagedArtifactPaths {
        package_path: operation_directory.join(format!("{prefix}.deb")),
        evidence_path: operation_directory.join(format!("{prefix}.evidence.json")),
    }
}

fn sha256_file(path: &Path) -> Result<String, LinuxInstallStoreError> {
    let mut file = File::open(path)
        .map_err(|error| LinuxInstallStoreError::io("open file for SHA-256", error))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| LinuxInstallStoreError::io("read file for SHA-256", error))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let mut value = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(value)
}

fn sync_directory(path: &Path) -> Result<(), LinuxInstallStoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| LinuxInstallStoreError::io("sync directory", error))
}
