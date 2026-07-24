use std::fmt;
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Component, Path, PathBuf};

use crate::{
    UpgradeArtifactIdentity, UpgradeArtifactSlot, UpgradeReceipt, UpgradeState,
    MAX_UPGRADE_RECEIPT_BYTES,
};

const STATE_DIRECTORY_NAME: &str = ".radishlex-upgrade-v1";
const RECEIPT_FILE_NAME: &str = "receipt.json";
const STAGED_RECEIPT_FILE_NAME: &str = "receipt.json.tmp";
const SNAPSHOT_FILE_NAME: &str = "source-snapshot.sqlite3";
const STAGED_SNAPSHOT_FILE_NAME: &str = "source-snapshot.sqlite3.tmp";
const CANDIDATE_FILE_NAME: &str = "migration-candidate.sqlite3";
const STAGED_CANDIDATE_FILE_NAME: &str = "migration-candidate.sqlite3.tmp";
const SOURCE_BACKUP_FILE_NAME: &str = "source-backup.sqlite3";
const SETTINGS_BACKUP_FILE_NAME: &str = "source-settings.json";
const STAGED_SETTINGS_BACKUP_FILE_NAME: &str = "source-settings.json.tmp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeFilesystemErrorCode {
    UnsafeDataRoot,
    UnsafeStateDirectory,
    UnexpectedStateObject,
    InvalidReceipt,
    InvalidReceiptReplacement,
    InterruptedReceiptWrite,
    InterruptedSnapshot,
    InterruptedCandidate,
    InterruptedSwitch,
    InterruptedSettingsBackup,
    OperationAlreadyActive,
    InvalidSnapshotState,
    InvalidCandidateState,
    InvalidCandidateValidation,
    InvalidSwitchState,
    InvalidSettingsState,
    InsufficientSpace,
    SnapshotFailed,
    CandidateMigrationFailed,
    SwitchFailed,
    SettingsBackupFailed,
    IdentityChanged,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeFilesystemError {
    code: UpgradeFilesystemErrorCode,
}

impl UpgradeFilesystemError {
    const fn new(code: UpgradeFilesystemErrorCode) -> Self {
        Self { code }
    }

    pub const fn code(self) -> UpgradeFilesystemErrorCode {
        self.code
    }
}

impl fmt::Display for UpgradeFilesystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            UpgradeFilesystemErrorCode::UnsafeDataRoot => "upgrade data root is unsafe",
            UpgradeFilesystemErrorCode::UnsafeStateDirectory => "upgrade state directory is unsafe",
            UpgradeFilesystemErrorCode::UnexpectedStateObject => {
                "upgrade state directory contains an unexpected object"
            }
            UpgradeFilesystemErrorCode::InvalidReceipt => "upgrade receipt is invalid",
            UpgradeFilesystemErrorCode::InvalidReceiptReplacement => {
                "upgrade receipt replacement is invalid"
            }
            UpgradeFilesystemErrorCode::InterruptedReceiptWrite => {
                "upgrade receipt write requires recovery"
            }
            UpgradeFilesystemErrorCode::InterruptedSnapshot => "upgrade snapshot requires recovery",
            UpgradeFilesystemErrorCode::InterruptedCandidate => {
                "upgrade migration candidate requires recovery"
            }
            UpgradeFilesystemErrorCode::InterruptedSwitch => {
                "upgrade database switch requires recovery"
            }
            UpgradeFilesystemErrorCode::InterruptedSettingsBackup => {
                "upgrade settings backup requires recovery"
            }
            UpgradeFilesystemErrorCode::OperationAlreadyActive => {
                "another upgrade operation is active"
            }
            UpgradeFilesystemErrorCode::InvalidSnapshotState => "upgrade snapshot state is invalid",
            UpgradeFilesystemErrorCode::InvalidCandidateState => {
                "upgrade migration candidate state is invalid"
            }
            UpgradeFilesystemErrorCode::InvalidCandidateValidation => {
                "upgrade candidate validation evidence is invalid"
            }
            UpgradeFilesystemErrorCode::InvalidSwitchState => {
                "upgrade database switch state is invalid"
            }
            UpgradeFilesystemErrorCode::InvalidSettingsState => {
                "upgrade settings backup state is invalid"
            }
            UpgradeFilesystemErrorCode::InsufficientSpace => {
                "upgrade snapshot has insufficient free space"
            }
            UpgradeFilesystemErrorCode::SnapshotFailed => "upgrade snapshot failed",
            UpgradeFilesystemErrorCode::CandidateMigrationFailed => {
                "upgrade migration candidate failed"
            }
            UpgradeFilesystemErrorCode::SwitchFailed => "upgrade database switch failed",
            UpgradeFilesystemErrorCode::SettingsBackupFailed => "upgrade settings backup failed",
            UpgradeFilesystemErrorCode::IdentityChanged => "upgrade filesystem identity changed",
            UpgradeFilesystemErrorCode::Io => "upgrade filesystem operation failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for UpgradeFilesystemError {}

pub struct VerifiedDataRoot {
    path: PathBuf,
    expected_owner_id: u32,
    identity: UpgradeArtifactIdentity,
}

impl fmt::Debug for VerifiedDataRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedDataRoot")
            .field("expected_owner_id", &self.expected_owner_id)
            .field("identity", &self.identity)
            .finish()
    }
}

impl VerifiedDataRoot {
    pub fn verify(
        path: impl AsRef<Path>,
        expected_owner_id: u32,
    ) -> Result<Self, UpgradeFilesystemError> {
        let path = path.as_ref();
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(error(UpgradeFilesystemErrorCode::UnsafeDataRoot));
        }
        let canonical = fs::canonicalize(path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::UnsafeDataRoot))?;
        if canonical != path {
            return Err(error(UpgradeFilesystemErrorCode::UnsafeDataRoot));
        }
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::UnsafeDataRoot))?;
        verify_private_directory(&metadata, expected_owner_id, 0o700)
            .map_err(|_| error(UpgradeFilesystemErrorCode::UnsafeDataRoot))?;
        let identity = directory_artifact(UpgradeArtifactSlot::DataRoot, &metadata)
            .map_err(|_| error(UpgradeFilesystemErrorCode::UnsafeDataRoot))?;
        Ok(Self {
            path: path.to_path_buf(),
            expected_owner_id,
            identity,
        })
    }

    pub fn identity(&self) -> &UpgradeArtifactIdentity {
        &self.identity
    }

    fn revalidate(&self) -> Result<(), UpgradeFilesystemError> {
        let canonical = fs::canonicalize(&self.path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        if canonical != self.path {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        verify_private_directory(&metadata, self.expected_owner_id, 0o700)
            .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        if !artifact_matches_metadata(&self.identity, &metadata) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        Ok(())
    }
}

pub struct UpgradeReceiptStore {
    root: VerifiedDataRoot,
    state_directory: PathBuf,
    state_directory_identity: DirectoryIdentity,
    guard_socket_path: PathBuf,
}

impl UpgradeReceiptStore {
    pub fn open(mut root: VerifiedDataRoot) -> Result<Self, UpgradeFilesystemError> {
        root.revalidate()?;
        let state_directory = root.path.join(STATE_DIRECTORY_NAME);
        let state_directory_created = match DirBuilder::new().mode(0o700).create(&state_directory) {
            Ok(()) => {
                fs::set_permissions(&state_directory, fs::Permissions::from_mode(0o700))
                    .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
                sync_directory(&root.path)?;
                true
            }
            Err(io_error) if io_error.kind() == ErrorKind::AlreadyExists => false,
            Err(_) => return Err(error(UpgradeFilesystemErrorCode::Io)),
        };
        if state_directory_created {
            let root_metadata = fs::symlink_metadata(&root.path)
                .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
            verify_private_directory(&root_metadata, root.expected_owner_id, 0o700)
                .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
            root.identity = directory_artifact(UpgradeArtifactSlot::DataRoot, &root_metadata)
                .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        }
        let metadata = fs::symlink_metadata(&state_directory)
            .map_err(|_| error(UpgradeFilesystemErrorCode::UnsafeStateDirectory))?;
        verify_private_directory(&metadata, root.expected_owner_id, 0o700)
            .map_err(|_| error(UpgradeFilesystemErrorCode::UnsafeStateDirectory))?;
        let state_directory_identity = DirectoryIdentity::from_metadata(&metadata);
        let guard_socket_path = build_guard_socket_path(&root.identity, root.expected_owner_id)?;
        let store = Self {
            root,
            state_directory,
            state_directory_identity,
            guard_socket_path,
        };
        store.revalidate()?;
        store.validate_known_entries()?;
        Ok(store)
    }

    pub fn data_root_identity(&self) -> &UpgradeArtifactIdentity {
        &self.root.identity
    }

    pub fn acquire_guard(&self) -> Result<UpgradeProcessGuard, UpgradeFilesystemError> {
        self.revalidate()?;
        self.validate_known_entries()?;
        let socket_path = self.guard_path();
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(io_error) if io_error.kind() == ErrorKind::AddrInUse => {
                let existing_metadata = fs::symlink_metadata(&socket_path)
                    .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
                verify_private_socket(&existing_metadata, self.root.expected_owner_id)?;
                match UnixStream::connect(&socket_path) {
                    Ok(_) => {
                        return Err(error(UpgradeFilesystemErrorCode::OperationAlreadyActive));
                    }
                    Err(connect_error) if connect_error.kind() == ErrorKind::ConnectionRefused => {}
                    Err(_) => {
                        return Err(error(UpgradeFilesystemErrorCode::OperationAlreadyActive));
                    }
                }
                self.remove_stale_guard(&socket_path)?;
                UnixListener::bind(&socket_path).map_err(|bind_error| {
                    if bind_error.kind() == ErrorKind::AddrInUse {
                        error(UpgradeFilesystemErrorCode::OperationAlreadyActive)
                    } else {
                        error(UpgradeFilesystemErrorCode::Io)
                    }
                })?
            }
            Err(_) => return Err(error(UpgradeFilesystemErrorCode::Io)),
        };
        let initial_metadata = fs::symlink_metadata(&socket_path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        if !initial_metadata.file_type().is_socket()
            || initial_metadata.uid() != self.root.expected_owner_id
        {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        let initial_identity = FileIdentity::from_metadata(&initial_metadata);
        if fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)).is_err() {
            remove_socket_if_identity(&socket_path, initial_identity);
            return Err(error(UpgradeFilesystemErrorCode::Io));
        }
        let metadata = fs::symlink_metadata(&socket_path).map_err(|_| {
            remove_socket_if_identity(&socket_path, initial_identity);
            error(UpgradeFilesystemErrorCode::IdentityChanged)
        })?;
        if metadata.dev() != initial_identity.device_id || metadata.ino() != initial_identity.inode
        {
            remove_socket_if_identity(&socket_path, initial_identity);
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        verify_private_socket(&metadata, self.root.expected_owner_id)?;
        Ok(UpgradeProcessGuard {
            _listener: listener,
            socket_path,
            socket_identity: FileIdentity::from_metadata(&metadata),
            root_identity: self.root.identity.clone(),
            state_directory_identity: self.state_directory_identity,
        })
    }

    pub fn load(&self) -> Result<Option<UpgradeReceipt>, UpgradeFilesystemError> {
        self.revalidate()?;
        self.validate_known_entries()?;
        if path_exists(&self.staged_receipt_path())? {
            return Err(error(UpgradeFilesystemErrorCode::InterruptedReceiptWrite));
        }
        if path_exists(&self.guard_path())? {
            let metadata = fs::symlink_metadata(self.guard_path())
                .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
            verify_private_socket(&metadata, self.root.expected_owner_id)?;
            return Err(error(UpgradeFilesystemErrorCode::OperationAlreadyActive));
        }
        let receipt = self.load_current_internal()?.map(|(receipt, _, _)| receipt);
        snapshot::validate_snapshot_state(self, receipt.as_ref())?;
        candidate::validate_candidate_state(self, receipt.as_ref())?;
        settings::validate_settings_backup_state(self, receipt.as_ref())?;
        switch::validate_switch_state(self, receipt.as_ref())?;
        Ok(receipt)
    }

    pub fn persist(
        &self,
        guard: &UpgradeProcessGuard,
        receipt: &UpgradeReceipt,
    ) -> Result<(), UpgradeFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        if path_exists(&self.staged_receipt_path())? {
            return Err(error(UpgradeFilesystemErrorCode::InterruptedReceiptWrite));
        }
        validate_receipt_root(receipt, &self.root.identity)?;
        let next_bytes = receipt
            .encode()
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidReceipt))?;
        let current = self.load_current_internal()?;
        if let Some((current_receipt, current_bytes, _)) = &current {
            if current_bytes == &next_bytes {
                return Ok(());
            }
            if !receipt.can_replace(current_receipt) {
                return Err(error(UpgradeFilesystemErrorCode::InvalidReceiptReplacement));
            }
        } else if receipt.state() != UpgradeState::Preflighted {
            return Err(error(UpgradeFilesystemErrorCode::InvalidReceiptReplacement));
        }

        let staged_path = self.staged_receipt_path();
        let mut staged = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged_path)
            .map_err(|io_error| {
                if io_error.kind() == ErrorKind::AlreadyExists {
                    error(UpgradeFilesystemErrorCode::InterruptedReceiptWrite)
                } else {
                    error(UpgradeFilesystemErrorCode::Io)
                }
            })?;
        fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        staged
            .write_all(&next_bytes)
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        staged
            .sync_all()
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        let staged_metadata = staged
            .metadata()
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        verify_private_file(&staged_metadata, self.root.expected_owner_id)?;
        if staged_metadata.len() != next_bytes.len() as u64 {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        drop(staged);

        self.revalidate()?;
        guard.revalidate()?;
        match (&current, self.load_current_internal()?) {
            (None, None) => {}
            (
                Some((_, expected_bytes, expected_identity)),
                Some((_, actual_bytes, actual_identity)),
            ) if expected_bytes == &actual_bytes && expected_identity == &actual_identity => {}
            _ => return Err(error(UpgradeFilesystemErrorCode::IdentityChanged)),
        }
        fs::rename(&staged_path, self.receipt_path())
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        sync_directory(&self.state_directory)?;

        let (stored, stored_bytes, _) = self
            .load_current_internal()?
            .ok_or_else(|| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        if stored != *receipt || stored_bytes != next_bytes {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        Ok(())
    }

    fn revalidate(&self) -> Result<(), UpgradeFilesystemError> {
        self.root.revalidate()?;
        let metadata = fs::symlink_metadata(&self.state_directory)
            .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        verify_private_directory(&metadata, self.root.expected_owner_id, 0o700)
            .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        if self.state_directory_identity != DirectoryIdentity::from_metadata(&metadata) {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        Ok(())
    }

    fn validate_known_entries(&self) -> Result<(), UpgradeFilesystemError> {
        let entries = fs::read_dir(&self.state_directory)
            .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
        for entry in entries {
            let entry = entry.map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
            let name = entry.file_name();
            if name != RECEIPT_FILE_NAME
                && name != STAGED_RECEIPT_FILE_NAME
                && name != SNAPSHOT_FILE_NAME
                && name != STAGED_SNAPSHOT_FILE_NAME
                && name != CANDIDATE_FILE_NAME
                && name != STAGED_CANDIDATE_FILE_NAME
                && name != SOURCE_BACKUP_FILE_NAME
                && name != SETTINGS_BACKUP_FILE_NAME
                && name != STAGED_SETTINGS_BACKUP_FILE_NAME
            {
                return Err(error(UpgradeFilesystemErrorCode::UnexpectedStateObject));
            }
        }
        Ok(())
    }

    fn load_current_internal(
        &self,
    ) -> Result<Option<(UpgradeReceipt, Vec<u8>, FileIdentity)>, UpgradeFilesystemError> {
        let path = self.receipt_path();
        let Some((bytes, identity)) = read_private_file(&path, self.root.expected_owner_id)? else {
            return Ok(None);
        };
        let receipt = UpgradeReceipt::decode(&bytes)
            .map_err(|_| error(UpgradeFilesystemErrorCode::InvalidReceipt))?;
        validate_receipt_root(&receipt, &self.root.identity)?;
        Ok(Some((receipt, bytes, identity)))
    }

    fn remove_stale_guard(&self, socket_path: &Path) -> Result<(), UpgradeFilesystemError> {
        let metadata = fs::symlink_metadata(socket_path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        verify_private_socket(&metadata, self.root.expected_owner_id)?;
        fs::remove_file(socket_path).map_err(|_| error(UpgradeFilesystemErrorCode::Io))
    }

    fn receipt_path(&self) -> PathBuf {
        self.state_directory.join(RECEIPT_FILE_NAME)
    }

    fn staged_receipt_path(&self) -> PathBuf {
        self.state_directory.join(STAGED_RECEIPT_FILE_NAME)
    }

    fn guard_path(&self) -> PathBuf {
        self.guard_socket_path.clone()
    }
}

pub struct UpgradeProcessGuard {
    _listener: UnixListener,
    socket_path: PathBuf,
    socket_identity: FileIdentity,
    root_identity: UpgradeArtifactIdentity,
    state_directory_identity: DirectoryIdentity,
}

impl fmt::Debug for UpgradeProcessGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpgradeProcessGuard")
            .field("socket_identity", &self.socket_identity)
            .field("root_identity", &self.root_identity)
            .field("state_directory_identity", &self.state_directory_identity)
            .finish()
    }
}

impl UpgradeProcessGuard {
    fn belongs_to(&self, store: &UpgradeReceiptStore) -> bool {
        self.root_identity == store.root.identity
            && self.state_directory_identity == store.state_directory_identity
            && self.socket_path == store.guard_path()
    }

    fn revalidate(&self) -> Result<(), UpgradeFilesystemError> {
        let metadata = fs::symlink_metadata(&self.socket_path)
            .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
        if self.socket_identity != FileIdentity::from_metadata(&metadata)
            || !metadata.file_type().is_socket()
        {
            return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
        }
        Ok(())
    }
}

impl Drop for UpgradeProcessGuard {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.socket_path) else {
            return;
        };
        if metadata.file_type().is_socket()
            && FileIdentity::from_metadata(&metadata) == self.socket_identity
        {
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectoryIdentity {
    device_id: u64,
    inode: u64,
    owner_id: u32,
    mode: u32,
}

impl DirectoryIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device_id: metadata.dev(),
            inode: metadata.ino(),
            owner_id: metadata.uid(),
            mode: metadata.mode() & 0o7777,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device_id: u64,
    inode: u64,
    owner_id: u32,
    mode: u32,
    link_count: u64,
    byte_len: u64,
}

impl FileIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device_id: metadata.dev(),
            inode: metadata.ino(),
            owner_id: metadata.uid(),
            mode: metadata.mode() & 0o7777,
            link_count: metadata.nlink(),
            byte_len: metadata.len(),
        }
    }
}

fn validate_receipt_root(
    receipt: &UpgradeReceipt,
    root_identity: &UpgradeArtifactIdentity,
) -> Result<(), UpgradeFilesystemError> {
    let matches = receipt
        .artifacts()
        .iter()
        .any(|artifact| directory_artifacts_match(artifact, root_identity));
    if matches {
        Ok(())
    } else {
        Err(error(UpgradeFilesystemErrorCode::IdentityChanged))
    }
}

fn verify_private_directory(
    metadata: &Metadata,
    expected_owner_id: u32,
    expected_mode: u32,
) -> Result<(), UpgradeFilesystemError> {
    if !metadata.file_type().is_dir()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o7777 != expected_mode
        || metadata.nlink() == 0
    {
        return Err(error(UpgradeFilesystemErrorCode::UnsafeStateDirectory));
    }
    Ok(())
}

fn verify_private_file(
    metadata: &Metadata,
    expected_owner_id: u32,
) -> Result<(), UpgradeFilesystemError> {
    if !metadata.file_type().is_file()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() > MAX_UPGRADE_RECEIPT_BYTES as u64
    {
        return Err(error(UpgradeFilesystemErrorCode::InvalidReceipt));
    }
    Ok(())
}

fn verify_private_socket(
    metadata: &Metadata,
    expected_owner_id: u32,
) -> Result<(), UpgradeFilesystemError> {
    if !metadata.file_type().is_socket()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
    }
    Ok(())
}

fn directory_artifact(
    slot: UpgradeArtifactSlot,
    metadata: &Metadata,
) -> Result<UpgradeArtifactIdentity, UpgradeFilesystemError> {
    UpgradeArtifactIdentity::private_directory(
        slot,
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.nlink(),
    )
    .map_err(|_| error(UpgradeFilesystemErrorCode::UnsafeDataRoot))
}

fn artifact_matches_metadata(identity: &UpgradeArtifactIdentity, metadata: &Metadata) -> bool {
    identity.device_id() == metadata.dev()
        && identity.inode() == metadata.ino()
        && identity.owner_id() == metadata.uid()
        && identity.mode() == metadata.mode() & 0o7777
        && identity.byte_len() == 0
}

fn directory_artifacts_match(
    left: &UpgradeArtifactIdentity,
    right: &UpgradeArtifactIdentity,
) -> bool {
    left.slot() == UpgradeArtifactSlot::DataRoot
        && right.slot() == UpgradeArtifactSlot::DataRoot
        && left.device_id() == right.device_id()
        && left.inode() == right.inode()
        && left.owner_id() == right.owner_id()
        && left.mode() == right.mode()
}

fn build_guard_socket_path(
    root_identity: &UpgradeArtifactIdentity,
    expected_owner_id: u32,
) -> Result<PathBuf, UpgradeFilesystemError> {
    let guard_root = [Path::new("/private/tmp"), Path::new("/tmp")]
        .into_iter()
        .find_map(|candidate| {
            let canonical = fs::canonicalize(candidate).ok()?;
            if canonical != candidate {
                return None;
            }
            let metadata = fs::symlink_metadata(candidate).ok()?;
            if metadata.file_type().is_dir()
                && metadata.uid() == 0
                && metadata.mode() & 0o7777 == 0o1777
            {
                Some(candidate.to_path_buf())
            } else {
                None
            }
        })
        .ok_or_else(|| error(UpgradeFilesystemErrorCode::UnsafeStateDirectory))?;
    let file_name = format!(
        "rlx-upgrade-{expected_owner_id}-{:x}-{:x}.sock",
        root_identity.device_id(),
        root_identity.inode()
    );
    Ok(guard_root.join(file_name))
}

fn remove_socket_if_identity(path: &Path, expected: FileIdentity) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_socket() && FileIdentity::from_metadata(&metadata) == expected {
        let _ = fs::remove_file(path);
    }
}

fn read_private_file(
    path: &Path,
    expected_owner_id: u32,
) -> Result<Option<(Vec<u8>, FileIdentity)>, UpgradeFilesystemError> {
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(error(UpgradeFilesystemErrorCode::Io)),
    };
    verify_private_file(&before, expected_owner_id)?;
    let before_identity = FileIdentity::from_metadata(&before);
    let mut file = File::open(path).map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
    let opened = file
        .metadata()
        .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
    if FileIdentity::from_metadata(&opened) != before_identity {
        return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_UPGRADE_RECEIPT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error(UpgradeFilesystemErrorCode::Io))?;
    if bytes.len() > MAX_UPGRADE_RECEIPT_BYTES {
        return Err(error(UpgradeFilesystemErrorCode::InvalidReceipt));
    }
    let after = fs::symlink_metadata(path)
        .map_err(|_| error(UpgradeFilesystemErrorCode::IdentityChanged))?;
    if FileIdentity::from_metadata(&after) != before_identity {
        return Err(error(UpgradeFilesystemErrorCode::IdentityChanged));
    }
    Ok(Some((bytes, before_identity)))
}

fn sync_directory(path: &Path) -> Result<(), UpgradeFilesystemError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| error(UpgradeFilesystemErrorCode::Io))
}

fn path_exists(path: &Path) -> Result<bool, UpgradeFilesystemError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(error(UpgradeFilesystemErrorCode::Io)),
    }
}

const fn error(code: UpgradeFilesystemErrorCode) -> UpgradeFilesystemError {
    UpgradeFilesystemError::new(code)
}

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;

#[path = "snapshot.rs"]
mod snapshot;
pub use snapshot::{UpgradeSnapshotSpaceBudget, UpgradeSnapshotSummary};

#[path = "candidate.rs"]
mod candidate;
pub use candidate::UpgradeCandidateSummary;

#[path = "validation.rs"]
mod validation;
pub use validation::{
    UpgradeCandidateValidationDisposition, UpgradeCandidateValidationReport,
    UpgradeCandidateValidationSummary, UpgradeCompletionDisposition,
    UpgradeInputMethodValidationEvidence, UpgradeManagerValidationEvidence,
    UpgradePostSwitchValidationDisposition, UpgradePostSwitchValidationReport,
    UpgradePostSwitchValidationSummary, UPGRADE_VALIDATION_EVIDENCE_VERSION,
};

#[path = "settings.rs"]
mod settings;
pub use settings::UpgradeSettingsBackupSummary;

#[path = "switch.rs"]
mod switch;
pub use switch::{UpgradeSwitchDisposition, UpgradeSwitchSummary};

#[path = "rollback.rs"]
mod rollback;
pub use rollback::{
    UpgradeRollbackRestoreDisposition, UpgradeRollbackRestoreSummary,
    UpgradeRollbackValidationDisposition, UpgradeRollbackValidationEvidence,
    UpgradeRollbackValidationSummary, UPGRADE_ROLLBACK_VALIDATION_EVIDENCE_VERSION,
};

#[path = "startup_gate.rs"]
mod startup_gate;
pub use startup_gate::{
    inspect_startup_gate, StartupGateDecision, StartupGateErrorCode, StartupGateResult,
};
