use std::fmt;
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Component, Path, PathBuf};

use crate::{
    evaluate_install_startup_receipt, InstallReceipt, InstallRootIdentity,
    InstallStartupGateDecision, InstallStartupGateErrorCode, InstallStartupGateResult,
    InstallState, RunningProgramIdentity, MAX_INSTALL_RECEIPT_BYTES,
};

const STATE_DIRECTORY_NAME: &str = ".radishlex-install-v1";
const RECEIPT_FILE_NAME: &str = "receipt.json";
const STAGED_RECEIPT_FILE_NAME: &str = "receipt.json.tmp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFilesystemErrorCode {
    UnsafeDataRoot,
    UnsafeStateDirectory,
    UnexpectedStateObject,
    InvalidReceipt,
    InvalidReceiptReplacement,
    InterruptedReceiptWrite,
    OperationAlreadyActive,
    IdentityChanged,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallFilesystemError {
    code: InstallFilesystemErrorCode,
}

impl InstallFilesystemError {
    const fn new(code: InstallFilesystemErrorCode) -> Self {
        Self { code }
    }

    pub const fn code(self) -> InstallFilesystemErrorCode {
        self.code
    }
}

impl fmt::Display for InstallFilesystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            InstallFilesystemErrorCode::UnsafeDataRoot => "install data root is unsafe",
            InstallFilesystemErrorCode::UnsafeStateDirectory => "install state directory is unsafe",
            InstallFilesystemErrorCode::UnexpectedStateObject => {
                "install state directory contains an unexpected object"
            }
            InstallFilesystemErrorCode::InvalidReceipt => "install receipt is invalid",
            InstallFilesystemErrorCode::InvalidReceiptReplacement => {
                "install receipt replacement is invalid"
            }
            InstallFilesystemErrorCode::InterruptedReceiptWrite => {
                "install receipt write requires recovery"
            }
            InstallFilesystemErrorCode::OperationAlreadyActive => {
                "another install operation is active"
            }
            InstallFilesystemErrorCode::IdentityChanged => "install filesystem identity changed",
            InstallFilesystemErrorCode::Io => "install filesystem operation failed",
        })
    }
}

impl std::error::Error for InstallFilesystemError {}

pub struct VerifiedInstallRoot {
    path: PathBuf,
    expected_owner_id: u32,
    identity: InstallRootIdentity,
}

impl fmt::Debug for VerifiedInstallRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedInstallRoot")
            .field("expected_owner_id", &self.expected_owner_id)
            .field("identity", &self.identity)
            .finish()
    }
}

impl VerifiedInstallRoot {
    pub fn verify(
        path: impl AsRef<Path>,
        expected_owner_id: u32,
    ) -> Result<Self, InstallFilesystemError> {
        let path = path.as_ref();
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        {
            return Err(error(InstallFilesystemErrorCode::UnsafeDataRoot));
        }
        let canonical = fs::canonicalize(path)
            .map_err(|_| error(InstallFilesystemErrorCode::UnsafeDataRoot))?;
        if canonical != path {
            return Err(error(InstallFilesystemErrorCode::UnsafeDataRoot));
        }
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| error(InstallFilesystemErrorCode::UnsafeDataRoot))?;
        verify_private_directory(&metadata, expected_owner_id, 0o700)
            .map_err(|_| error(InstallFilesystemErrorCode::UnsafeDataRoot))?;
        let identity = root_identity(&metadata)
            .map_err(|_| error(InstallFilesystemErrorCode::UnsafeDataRoot))?;
        Ok(Self {
            path: path.to_path_buf(),
            expected_owner_id,
            identity,
        })
    }

    pub fn identity(&self) -> &InstallRootIdentity {
        &self.identity
    }

    fn revalidate(&self) -> Result<(), InstallFilesystemError> {
        let canonical = fs::canonicalize(&self.path)
            .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
        if canonical != self.path {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
        }
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
        verify_private_directory(&metadata, self.expected_owner_id, 0o700)
            .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
        if root_identity(&metadata).ok().as_ref() != Some(&self.identity) {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
        }
        Ok(())
    }
}

pub struct InstallReceiptStore {
    root: VerifiedInstallRoot,
    state_directory: PathBuf,
    state_directory_identity: DirectoryIdentity,
    guard_socket_path: PathBuf,
}

impl InstallReceiptStore {
    pub fn open(root: VerifiedInstallRoot) -> Result<Self, InstallFilesystemError> {
        root.revalidate()?;
        let state_directory = root.path.join(STATE_DIRECTORY_NAME);
        match DirBuilder::new().mode(0o700).create(&state_directory) {
            Ok(()) => {
                fs::set_permissions(&state_directory, fs::Permissions::from_mode(0o700))
                    .map_err(|_| error(InstallFilesystemErrorCode::Io))?;
                sync_directory(&root.path)?;
            }
            Err(io_error) if io_error.kind() == ErrorKind::AlreadyExists => {}
            Err(_) => return Err(error(InstallFilesystemErrorCode::Io)),
        }
        let metadata = fs::symlink_metadata(&state_directory)
            .map_err(|_| error(InstallFilesystemErrorCode::UnsafeStateDirectory))?;
        verify_private_directory(&metadata, root.expected_owner_id, 0o700)
            .map_err(|_| error(InstallFilesystemErrorCode::UnsafeStateDirectory))?;
        let guard_socket_path = build_guard_socket_path(&root.identity, root.expected_owner_id)?;
        let store = Self {
            root,
            state_directory,
            state_directory_identity: DirectoryIdentity::from_metadata(&metadata),
            guard_socket_path,
        };
        store.revalidate()?;
        store.validate_known_entries()?;
        Ok(store)
    }

    pub fn root_identity(&self) -> &InstallRootIdentity {
        &self.root.identity
    }

    pub fn acquire_guard(&self) -> Result<InstallProcessGuard, InstallFilesystemError> {
        self.revalidate()?;
        self.validate_known_entries()?;
        let socket_path = self.guard_path();
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(io_error) if io_error.kind() == ErrorKind::AddrInUse => {
                let metadata = fs::symlink_metadata(&socket_path)
                    .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
                verify_private_socket(&metadata, self.root.expected_owner_id)?;
                match UnixStream::connect(&socket_path) {
                    Ok(_) => {
                        return Err(error(InstallFilesystemErrorCode::OperationAlreadyActive));
                    }
                    Err(connect_error) if connect_error.kind() == ErrorKind::ConnectionRefused => {}
                    Err(_) => {
                        return Err(error(InstallFilesystemErrorCode::OperationAlreadyActive));
                    }
                }
                self.remove_stale_guard(&socket_path)?;
                UnixListener::bind(&socket_path).map_err(|bind_error| {
                    if bind_error.kind() == ErrorKind::AddrInUse {
                        error(InstallFilesystemErrorCode::OperationAlreadyActive)
                    } else {
                        error(InstallFilesystemErrorCode::Io)
                    }
                })?
            }
            Err(_) => return Err(error(InstallFilesystemErrorCode::Io)),
        };
        let initial = fs::symlink_metadata(&socket_path)
            .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
        if !initial.file_type().is_socket() || initial.uid() != self.root.expected_owner_id {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
        }
        let initial_identity = FileIdentity::from_metadata(&initial);
        if fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)).is_err() {
            remove_socket_if_identity(&socket_path, initial_identity);
            return Err(error(InstallFilesystemErrorCode::Io));
        }
        let metadata = fs::symlink_metadata(&socket_path).map_err(|_| {
            remove_socket_if_identity(&socket_path, initial_identity);
            error(InstallFilesystemErrorCode::IdentityChanged)
        })?;
        verify_private_socket(&metadata, self.root.expected_owner_id)?;
        Ok(InstallProcessGuard {
            _listener: listener,
            socket_path,
            socket_identity: FileIdentity::from_metadata(&metadata),
            root_identity: self.root.identity.clone(),
            state_directory_identity: self.state_directory_identity,
        })
    }

    pub fn load(&self) -> Result<Option<InstallReceipt>, InstallFilesystemError> {
        self.revalidate()?;
        self.validate_known_entries()?;
        if path_exists(&self.staged_receipt_path())? {
            return Err(error(InstallFilesystemErrorCode::InterruptedReceiptWrite));
        }
        if path_exists(&self.guard_path())? {
            let metadata = fs::symlink_metadata(self.guard_path())
                .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
            verify_private_socket(&metadata, self.root.expected_owner_id)?;
            return Err(error(InstallFilesystemErrorCode::OperationAlreadyActive));
        }
        self.load_current_internal()
            .map(|current| current.map(|(receipt, _, _)| receipt))
    }

    pub fn persist(
        &self,
        guard: &InstallProcessGuard,
        receipt: &InstallReceipt,
    ) -> Result<(), InstallFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()?;
        self.validate_known_entries()?;
        if path_exists(&self.staged_receipt_path())? {
            return Err(error(InstallFilesystemErrorCode::InterruptedReceiptWrite));
        }
        validate_receipt_root(receipt, &self.root.identity)?;
        let next_bytes = receipt
            .encode()
            .map_err(|_| error(InstallFilesystemErrorCode::InvalidReceipt))?;
        let current = self.load_current_internal()?;
        if let Some((current_receipt, current_bytes, _)) = &current {
            if current_bytes == &next_bytes {
                return Ok(());
            }
            if !receipt.can_replace(current_receipt) {
                return Err(error(InstallFilesystemErrorCode::InvalidReceiptReplacement));
            }
        } else if receipt.state() != InstallState::Prepared
            || receipt.previous_operation_id.is_some()
        {
            return Err(error(InstallFilesystemErrorCode::InvalidReceiptReplacement));
        }

        let staged_path = self.staged_receipt_path();
        let mut staged = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged_path)
            .map_err(|io_error| {
                if io_error.kind() == ErrorKind::AlreadyExists {
                    error(InstallFilesystemErrorCode::InterruptedReceiptWrite)
                } else {
                    error(InstallFilesystemErrorCode::Io)
                }
            })?;
        fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| error(InstallFilesystemErrorCode::Io))?;
        staged
            .write_all(&next_bytes)
            .map_err(|_| error(InstallFilesystemErrorCode::Io))?;
        staged
            .sync_all()
            .map_err(|_| error(InstallFilesystemErrorCode::Io))?;
        let staged_metadata = staged
            .metadata()
            .map_err(|_| error(InstallFilesystemErrorCode::Io))?;
        verify_private_file(&staged_metadata, self.root.expected_owner_id)?;
        if staged_metadata.len() != next_bytes.len() as u64 {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
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
            _ => return Err(error(InstallFilesystemErrorCode::IdentityChanged)),
        }
        fs::rename(&staged_path, self.receipt_path())
            .map_err(|_| error(InstallFilesystemErrorCode::Io))?;
        sync_directory(&self.state_directory)?;
        let (stored, stored_bytes, _) = self
            .load_current_internal()?
            .ok_or_else(|| error(InstallFilesystemErrorCode::IdentityChanged))?;
        if stored != *receipt || stored_bytes != next_bytes {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
        }
        Ok(())
    }

    pub fn verify_guard(&self, guard: &InstallProcessGuard) -> Result<(), InstallFilesystemError> {
        self.revalidate()?;
        if !guard.belongs_to(self) {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
        }
        guard.revalidate()
    }

    pub fn verify_current(
        &self,
        guard: &InstallProcessGuard,
        receipt: &InstallReceipt,
    ) -> Result<(), InstallFilesystemError> {
        self.verify_guard(guard)?;
        self.validate_known_entries()?;
        if path_exists(&self.staged_receipt_path())? {
            return Err(error(InstallFilesystemErrorCode::InterruptedReceiptWrite));
        }
        let stored = self
            .load_current_internal()?
            .map(|(stored, _, _)| stored)
            .ok_or_else(|| error(InstallFilesystemErrorCode::InvalidReceipt))?;
        if stored != *receipt {
            return Err(error(InstallFilesystemErrorCode::InvalidReceiptReplacement));
        }
        Ok(())
    }

    fn revalidate(&self) -> Result<(), InstallFilesystemError> {
        self.root.revalidate()?;
        let metadata = fs::symlink_metadata(&self.state_directory)
            .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
        verify_private_directory(&metadata, self.root.expected_owner_id, 0o700)
            .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
        if self.state_directory_identity != DirectoryIdentity::from_metadata(&metadata) {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
        }
        Ok(())
    }

    fn validate_known_entries(&self) -> Result<(), InstallFilesystemError> {
        for entry in fs::read_dir(&self.state_directory)
            .map_err(|_| error(InstallFilesystemErrorCode::Io))?
        {
            let name = entry
                .map_err(|_| error(InstallFilesystemErrorCode::Io))?
                .file_name();
            if name != RECEIPT_FILE_NAME && name != STAGED_RECEIPT_FILE_NAME {
                return Err(error(InstallFilesystemErrorCode::UnexpectedStateObject));
            }
        }
        Ok(())
    }

    fn load_current_internal(
        &self,
    ) -> Result<Option<(InstallReceipt, Vec<u8>, FileIdentity)>, InstallFilesystemError> {
        let Some((bytes, identity)) =
            read_private_file(&self.receipt_path(), self.root.expected_owner_id)?
        else {
            return Ok(None);
        };
        let receipt = InstallReceipt::decode(&bytes)
            .map_err(|_| error(InstallFilesystemErrorCode::InvalidReceipt))?;
        validate_receipt_root(&receipt, &self.root.identity)?;
        Ok(Some((receipt, bytes, identity)))
    }

    fn remove_stale_guard(&self, socket_path: &Path) -> Result<(), InstallFilesystemError> {
        let metadata = fs::symlink_metadata(socket_path)
            .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
        verify_private_socket(&metadata, self.root.expected_owner_id)?;
        fs::remove_file(socket_path).map_err(|_| error(InstallFilesystemErrorCode::Io))
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

pub struct InstallProcessGuard {
    _listener: UnixListener,
    socket_path: PathBuf,
    socket_identity: FileIdentity,
    root_identity: InstallRootIdentity,
    state_directory_identity: DirectoryIdentity,
}

impl fmt::Debug for InstallProcessGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstallProcessGuard")
            .field("socket_identity", &self.socket_identity)
            .field("root_identity", &self.root_identity)
            .field("state_directory_identity", &self.state_directory_identity)
            .finish()
    }
}

impl InstallProcessGuard {
    fn belongs_to(&self, store: &InstallReceiptStore) -> bool {
        self.root_identity == store.root.identity
            && self.state_directory_identity == store.state_directory_identity
            && self.socket_path == store.guard_path()
    }

    fn revalidate(&self) -> Result<(), InstallFilesystemError> {
        let metadata = fs::symlink_metadata(&self.socket_path)
            .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
        if self.socket_identity != FileIdentity::from_metadata(&metadata)
            || !metadata.file_type().is_socket()
        {
            return Err(error(InstallFilesystemErrorCode::IdentityChanged));
        }
        Ok(())
    }
}

impl Drop for InstallProcessGuard {
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

pub fn inspect_install_startup_gate(
    data_root: impl AsRef<Path>,
    expected_owner_id: u32,
    running: &RunningProgramIdentity,
) -> InstallStartupGateResult {
    let data_root = data_root.as_ref();
    if !data_root.is_absolute()
        || data_root
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return failed(InstallStartupGateErrorCode::UnsafeDataRoot);
    }
    match fs::symlink_metadata(data_root) {
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => {
            return allowed(InstallStartupGateDecision::AllowedFirstLaunch, None);
        }
        Err(_) => return failed(InstallStartupGateErrorCode::Io),
        Ok(_) => {}
    }
    let root = match VerifiedInstallRoot::verify(data_root, expected_owner_id) {
        Ok(root) => root,
        Err(_) => return failed(InstallStartupGateErrorCode::UnsafeDataRoot),
    };
    let guard_path = match build_guard_socket_path(root.identity(), expected_owner_id) {
        Ok(path) => path,
        Err(_) => return failed(InstallStartupGateErrorCode::UnsafeStateDirectory),
    };
    match fs::symlink_metadata(&guard_path) {
        Ok(metadata) => {
            if verify_private_socket(&metadata, expected_owner_id).is_err() {
                return failed(InstallStartupGateErrorCode::RootIdentityChanged);
            }
            return InstallStartupGateResult {
                decision: InstallStartupGateDecision::BlockedInstallInProgress,
                error_code: InstallStartupGateErrorCode::ActiveGuard,
                receipt_state: None,
            };
        }
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => {}
        Err(_) => return failed(InstallStartupGateErrorCode::Io),
    }

    let state_directory = data_root.join(STATE_DIRECTORY_NAME);
    let state_metadata = match fs::symlink_metadata(&state_directory) {
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => {
            return allowed(InstallStartupGateDecision::AllowedNoInstallState, None);
        }
        Err(_) => return failed(InstallStartupGateErrorCode::Io),
        Ok(metadata) => metadata,
    };
    if verify_private_directory(&state_metadata, expected_owner_id, 0o700).is_err() {
        return failed(InstallStartupGateErrorCode::UnsafeStateDirectory);
    }
    let store = InstallReceiptStore {
        guard_socket_path: guard_path,
        root,
        state_directory,
        state_directory_identity: DirectoryIdentity::from_metadata(&state_metadata),
    };
    if let Err(error) = store
        .revalidate()
        .and_then(|_| store.validate_known_entries())
    {
        return failed(map_startup_error(error.code()));
    }
    match path_exists(&store.staged_receipt_path()) {
        Ok(true) => return failed(InstallStartupGateErrorCode::InterruptedReceipt),
        Ok(false) => {}
        Err(_) => return failed(InstallStartupGateErrorCode::Io),
    }
    match store.load_current_internal() {
        Ok(None) => allowed(InstallStartupGateDecision::AllowedNoInstallState, None),
        Ok(Some((receipt, _, _))) => evaluate_install_startup_receipt(&receipt, running),
        Err(error) => failed(map_startup_error(error.code())),
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
    receipt: &InstallReceipt,
    root: &InstallRootIdentity,
) -> Result<(), InstallFilesystemError> {
    if receipt.root_identity() == root {
        Ok(())
    } else {
        Err(error(InstallFilesystemErrorCode::IdentityChanged))
    }
}

fn root_identity(metadata: &Metadata) -> Result<InstallRootIdentity, InstallFilesystemError> {
    InstallRootIdentity::new(
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.mode() & 0o7777,
    )
    .map_err(|_| error(InstallFilesystemErrorCode::UnsafeDataRoot))
}

fn verify_private_directory(
    metadata: &Metadata,
    expected_owner_id: u32,
    expected_mode: u32,
) -> Result<(), InstallFilesystemError> {
    if !metadata.file_type().is_dir()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o7777 != expected_mode
        || metadata.nlink() == 0
    {
        return Err(error(InstallFilesystemErrorCode::UnsafeStateDirectory));
    }
    Ok(())
}

fn verify_private_file(
    metadata: &Metadata,
    expected_owner_id: u32,
) -> Result<(), InstallFilesystemError> {
    if !metadata.file_type().is_file()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() > MAX_INSTALL_RECEIPT_BYTES as u64
    {
        return Err(error(InstallFilesystemErrorCode::InvalidReceipt));
    }
    Ok(())
}

fn verify_private_socket(
    metadata: &Metadata,
    expected_owner_id: u32,
) -> Result<(), InstallFilesystemError> {
    if !metadata.file_type().is_socket()
        || metadata.uid() != expected_owner_id
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err(error(InstallFilesystemErrorCode::IdentityChanged));
    }
    Ok(())
}

fn build_guard_socket_path(
    root: &InstallRootIdentity,
    expected_owner_id: u32,
) -> Result<PathBuf, InstallFilesystemError> {
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
        .ok_or_else(|| error(InstallFilesystemErrorCode::UnsafeStateDirectory))?;
    Ok(guard_root.join(format!(
        "rlx-install-{expected_owner_id}-{:x}-{:x}.sock",
        root.device_id(),
        root.inode()
    )))
}

fn read_private_file(
    path: &Path,
    expected_owner_id: u32,
) -> Result<Option<(Vec<u8>, FileIdentity)>, InstallFilesystemError> {
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(error(InstallFilesystemErrorCode::Io)),
    };
    verify_private_file(&before, expected_owner_id)?;
    let before_identity = FileIdentity::from_metadata(&before);
    let mut file = File::open(path).map_err(|_| error(InstallFilesystemErrorCode::Io))?;
    if FileIdentity::from_metadata(
        &file
            .metadata()
            .map_err(|_| error(InstallFilesystemErrorCode::Io))?,
    ) != before_identity
    {
        return Err(error(InstallFilesystemErrorCode::IdentityChanged));
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_INSTALL_RECEIPT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error(InstallFilesystemErrorCode::Io))?;
    if bytes.len() > MAX_INSTALL_RECEIPT_BYTES {
        return Err(error(InstallFilesystemErrorCode::InvalidReceipt));
    }
    let after = fs::symlink_metadata(path)
        .map_err(|_| error(InstallFilesystemErrorCode::IdentityChanged))?;
    if FileIdentity::from_metadata(&after) != before_identity {
        return Err(error(InstallFilesystemErrorCode::IdentityChanged));
    }
    Ok(Some((bytes, before_identity)))
}

fn allowed(
    decision: InstallStartupGateDecision,
    state: Option<InstallState>,
) -> InstallStartupGateResult {
    InstallStartupGateResult {
        decision,
        error_code: InstallStartupGateErrorCode::None,
        receipt_state: state,
    }
}

fn failed(error_code: InstallStartupGateErrorCode) -> InstallStartupGateResult {
    InstallStartupGateResult {
        decision: InstallStartupGateDecision::FailedClosed,
        error_code,
        receipt_state: None,
    }
}

fn map_startup_error(code: InstallFilesystemErrorCode) -> InstallStartupGateErrorCode {
    match code {
        InstallFilesystemErrorCode::UnsafeDataRoot => InstallStartupGateErrorCode::UnsafeDataRoot,
        InstallFilesystemErrorCode::UnsafeStateDirectory => {
            InstallStartupGateErrorCode::UnsafeStateDirectory
        }
        InstallFilesystemErrorCode::UnexpectedStateObject => {
            InstallStartupGateErrorCode::UnexpectedStateObject
        }
        InstallFilesystemErrorCode::InvalidReceipt
        | InstallFilesystemErrorCode::InvalidReceiptReplacement => {
            InstallStartupGateErrorCode::InvalidReceipt
        }
        InstallFilesystemErrorCode::InterruptedReceiptWrite => {
            InstallStartupGateErrorCode::InterruptedReceipt
        }
        InstallFilesystemErrorCode::OperationAlreadyActive => {
            InstallStartupGateErrorCode::ActiveGuard
        }
        InstallFilesystemErrorCode::IdentityChanged => {
            InstallStartupGateErrorCode::RootIdentityChanged
        }
        InstallFilesystemErrorCode::Io => InstallStartupGateErrorCode::Io,
    }
}

fn remove_socket_if_identity(path: &Path, expected: FileIdentity) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_socket() && FileIdentity::from_metadata(&metadata) == expected {
        let _ = fs::remove_file(path);
    }
}

fn sync_directory(path: &Path) -> Result<(), InstallFilesystemError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| error(InstallFilesystemErrorCode::Io))
}

fn path_exists(path: &Path) -> Result<bool, InstallFilesystemError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(error(InstallFilesystemErrorCode::Io)),
    }
}

const fn error(code: InstallFilesystemErrorCode) -> InstallFilesystemError {
    InstallFilesystemError::new(code)
}

#[cfg(test)]
mod tests;
