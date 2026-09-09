//! Immutable, operation-bound preservation evidence. Never opens SQLite.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use radishlex_ime_product_install::{
    InstallFailureCode, InstallOperationKind, InstallProcessGuard, InstallReceipt,
    InstallReceiptStore, InstallRootIdentity, InstallState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const DIRECTORY: &str = ".radishlex-pre-switch-recovery-v1";
const EVIDENCE: &str = "evidence.json";
const MAX_BYTES: u64 = 256 * 1024;
// Order is a format contract. Absence is evidence too; no caller-supplied paths.
const FILES: &[&str] = &[
    "userdb.sqlite3",
    "userdb.sqlite3-wal",
    "userdb.sqlite3-shm",
    "userdb.sqlite3-journal",
    "manager-settings.json",
    ".radishlex-upgrade-v1/source-snapshot.sqlite3",
    ".radishlex-upgrade-v1/source-snapshot.sqlite3-wal",
    ".radishlex-upgrade-v1/source-snapshot.sqlite3-shm",
    ".radishlex-upgrade-v1/source-snapshot.sqlite3-journal",
    ".radishlex-upgrade-v1/migration-candidate.sqlite3",
    ".radishlex-upgrade-v1/migration-candidate.sqlite3-wal",
    ".radishlex-upgrade-v1/migration-candidate.sqlite3-shm",
    ".radishlex-upgrade-v1/migration-candidate.sqlite3-journal",
    ".radishlex-upgrade-v1/source-settings.json",
    ".radishlex-upgrade-v1/source-backup.sqlite3",
    ".radishlex-upgrade-v1/source-backup.sqlite3-wal",
    ".radishlex-upgrade-v1/source-backup.sqlite3-shm",
    ".radishlex-upgrade-v1/source-backup.sqlite3-journal",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryEvidenceError {
    UnsafeObject,
    InvalidEvidence,
    BindingChanged,
    DataChanged,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileEvidence {
    device: u64,
    inode: u64,
    owner: u32,
    mode: u32,
    length: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    version: u32,
    install_receipt: InstallReceipt,
    // Opaque to this platform storage adapter; the upgrade owner validates it.
    data_receipt: String,
    files: Vec<Option<FileEvidence>>,
    rime_root: Option<InstallRootIdentity>,
}

pub struct PreSwitchRecoveryEvidence {
    root: PathBuf,
    directory: PathBuf,
    evidence: Evidence,
    bytes: Vec<u8>,
    directory_identity: InstallRootIdentity,
    parent_identity: InstallRootIdentity,
    file_identity: FileEvidence,
}

impl PreSwitchRecoveryEvidence {
    /// Read-only, including when the recovery directory has never existed.
    pub fn load(
        root: &Path,
        receipt: &InstallReceipt,
    ) -> Result<Option<Self>, RecoveryEvidenceError> {
        validate_receipt(receipt)?;
        verify_directory(root, receipt.root_identity())?;
        let parent = root.join(DIRECTORY);
        if absent(&parent)? {
            return Ok(None);
        }
        let parent_identity = directory_identity(&parent, receipt.root_identity().owner_id())?;
        let directory = parent.join(receipt.operation_id());
        if absent(&directory)? {
            return Ok(None);
        }
        let directory_identity =
            directory_identity(&directory, receipt.root_identity().owner_id())?;
        verify_entries(&directory)?;
        let path = directory.join(EVIDENCE);
        // An incomplete first write is retained and fails closed; no baseline can be rebound.
        if fs::symlink_metadata(&path)
            .map_err(|_| RecoveryEvidenceError::InvalidEvidence)?
            .len()
            > MAX_BYTES
        {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
        let file_identity = inspect_file(&path, receipt.root_identity().owner_id())?
            .ok_or(RecoveryEvidenceError::InvalidEvidence)?;
        if file_identity.length > MAX_BYTES {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(|_| RecoveryEvidenceError::Io)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| RecoveryEvidenceError::Io)?;
        if bytes.len() as u64 > MAX_BYTES || hash(&bytes) != file_identity.sha256 {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
        let evidence: Evidence =
            serde_json::from_slice(&bytes).map_err(|_| RecoveryEvidenceError::InvalidEvidence)?;
        if evidence.version != 1
            || evidence.files.len() != FILES.len()
            || evidence.data_receipt.is_empty()
            || evidence.data_receipt.len() > 64 * 1024
        {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
        validate_initial(&evidence.install_receipt)?;
        let value = Self {
            root: root.to_owned(),
            directory,
            evidence,
            bytes,
            directory_identity,
            parent_identity,
            file_identity,
        };
        value.verify_binding(receipt)?;
        value.verify_storage()?;
        Ok(Some(value))
    }

    pub fn create(
        root: &Path,
        store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        receipt: &InstallReceipt,
        data_receipt: &[u8],
    ) -> Result<Self, RecoveryEvidenceError> {
        store
            .verify_current(guard, receipt)
            .map_err(|_| RecoveryEvidenceError::BindingChanged)?;
        validate_initial(receipt)?;
        verify_directory(root, receipt.root_identity())?;
        if Self::load(root, receipt)?.is_some() {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
        let data_receipt = std::str::from_utf8(data_receipt)
            .map_err(|_| RecoveryEvidenceError::InvalidEvidence)?
            .to_owned();
        if data_receipt.is_empty() || data_receipt.len() > 64 * 1024 {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
        let owner = receipt.root_identity().owner_id();
        let files = capture_files(root, owner)?;
        // Only the active WAL/SHM may be present. Backups/candidates remain standalone.
        for (index, path) in FILES.iter().enumerate() {
            if (path.ends_with("-journal")
                || (index >= 5 && (path.ends_with("-wal") || path.ends_with("-shm")))
                || path.contains("source-backup.sqlite3"))
                && files[index].is_some()
            {
                return Err(RecoveryEvidenceError::DataChanged);
            }
        }
        if files[0].is_none() || files[5].is_none() || files[9].is_none() {
            return Err(RecoveryEvidenceError::DataChanged);
        }
        let rime_root = optional_directory(&root.join("Rime"), owner)?;
        let evidence = Evidence {
            version: 1,
            install_receipt: receipt.clone(),
            data_receipt,
            files,
            rime_root,
        };
        let bytes =
            serde_json::to_vec(&evidence).map_err(|_| RecoveryEvidenceError::InvalidEvidence)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
        let parent = root.join(DIRECTORY);
        create_directory(&parent, root, owner)?;
        let directory = parent.join(receipt.operation_id());
        // A pre-existing empty operation directory is an interrupted creation, not reusable space.
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|_| RecoveryEvidenceError::InvalidEvidence)?;
        directory_identity(&directory, owner)?;
        sync(&parent)?;
        store
            .verify_current(guard, receipt)
            .map_err(|_| RecoveryEvidenceError::BindingChanged)?;
        if capture_files(root, owner)? != evidence.files
            || optional_directory(&root.join("Rime"), owner)? != evidence.rime_root
        {
            return Err(RecoveryEvidenceError::DataChanged);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join(EVIDENCE))
            .map_err(|_| RecoveryEvidenceError::Io)?;
        file.write_all(&bytes)
            .map_err(|_| RecoveryEvidenceError::Io)?;
        file.sync_all().map_err(|_| RecoveryEvidenceError::Io)?;
        drop(file);
        sync(&directory)?;
        let value = Self::load(root, receipt)?.ok_or(RecoveryEvidenceError::InvalidEvidence)?;
        value.verify_preserved(store, guard, receipt)?;
        Ok(value)
    }

    pub fn initial_data_receipt(&self) -> &[u8] {
        self.evidence.data_receipt.as_bytes()
    }

    pub fn verify_preserved(
        &self,
        store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        receipt: &InstallReceipt,
    ) -> Result<(), RecoveryEvidenceError> {
        store
            .verify_current(guard, receipt)
            .map_err(|_| RecoveryEvidenceError::BindingChanged)?;
        self.verify_binding(receipt)?;
        self.verify_storage()?;
        let owner = receipt.root_identity().owner_id();
        if capture_files(&self.root, owner)? != self.evidence.files
            || optional_directory(&self.root.join("Rime"), owner)? != self.evidence.rime_root
        {
            return Err(RecoveryEvidenceError::DataChanged);
        }
        self.verify_storage()
    }

    fn verify_binding(&self, receipt: &InstallReceipt) -> Result<(), RecoveryEvidenceError> {
        let mut expected = self.evidence.install_receipt.clone();
        match receipt.state() {
            InstallState::DataCoordinating => {}
            InstallState::RollbackRequired
            | InstallState::ProgramsRestored
            | InstallState::RolledBack => {
                expected
                    .require_rollback(InstallFailureCode::DataCoordinationFailed)
                    .map_err(|_| RecoveryEvidenceError::BindingChanged)?;
                if receipt.state() != InstallState::RollbackRequired {
                    expected
                        .mark_programs_restored()
                        .map_err(|_| RecoveryEvidenceError::BindingChanged)?;
                    if receipt.state() == InstallState::RolledBack {
                        expected
                            .mark_rolled_back()
                            .map_err(|_| RecoveryEvidenceError::BindingChanged)?;
                    }
                }
            }
            _ => return Err(RecoveryEvidenceError::BindingChanged),
        }
        if &expected != receipt {
            return Err(RecoveryEvidenceError::BindingChanged);
        }
        verify_directory(&self.root, receipt.root_identity())
    }

    fn verify_storage(&self) -> Result<(), RecoveryEvidenceError> {
        verify_directory(&self.root, self.evidence.install_receipt.root_identity())?;
        verify_directory(&self.root.join(DIRECTORY), &self.parent_identity)?;
        verify_directory(&self.directory, &self.directory_identity)?;
        verify_entries(&self.directory)?;
        let file = inspect_file(
            &self.directory.join(EVIDENCE),
            self.parent_identity.owner_id(),
        )?
        .ok_or(RecoveryEvidenceError::InvalidEvidence)?;
        if file != self.file_identity || hash(&self.bytes) != file.sha256 {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
        Ok(())
    }
}

fn validate_receipt(receipt: &InstallReceipt) -> Result<(), RecoveryEvidenceError> {
    let bytes = receipt
        .encode()
        .map_err(|_| RecoveryEvidenceError::InvalidEvidence)?;
    InstallReceipt::decode(&bytes).map_err(|_| RecoveryEvidenceError::InvalidEvidence)?;
    if receipt.operation_kind() != InstallOperationKind::Upgrade
        || Path::new(receipt.operation_id()).components().count() != 1
    {
        return Err(RecoveryEvidenceError::BindingChanged);
    }
    Ok(())
}

fn validate_initial(receipt: &InstallReceipt) -> Result<(), RecoveryEvidenceError> {
    validate_receipt(receipt)?;
    if receipt.state() != InstallState::DataCoordinating
        || receipt.failure_code().is_some()
        || receipt.manual_recovery_required()
    {
        return Err(RecoveryEvidenceError::BindingChanged);
    }
    Ok(())
}

fn capture_files(
    root: &Path,
    owner: u32,
) -> Result<Vec<Option<FileEvidence>>, RecoveryEvidenceError> {
    // Revalidate the nested parent too; no symlink traversal through an upgrade directory.
    directory_identity(&root.join(".radishlex-upgrade-v1"), owner)?;
    FILES
        .iter()
        .map(|path| inspect_file(&root.join(path), owner))
        .collect()
}

fn inspect_file(path: &Path, owner: u32) -> Result<Option<FileEvidence>, RecoveryEvidenceError> {
    if absent(path)? {
        return Ok(None);
    }
    let before = fs::symlink_metadata(path).map_err(|_| RecoveryEvidenceError::Io)?;
    check_file(&before, owner)?;
    if fs::canonicalize(path).map_err(|_| RecoveryEvidenceError::Io)? != path {
        return Err(RecoveryEvidenceError::UnsafeObject);
    }
    let mut file = File::open(path).map_err(|_| RecoveryEvidenceError::Io)?;
    let opened = file.metadata().map_err(|_| RecoveryEvidenceError::Io)?;
    check_file(&opened, owner)?;
    if metadata_key(&before) != metadata_key(&opened) {
        return Err(RecoveryEvidenceError::DataChanged);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut length = 0u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| RecoveryEvidenceError::Io)?;
        if count == 0 {
            break;
        }
        length = length
            .checked_add(count as u64)
            .ok_or(RecoveryEvidenceError::DataChanged)?;
        if length > before.len() {
            return Err(RecoveryEvidenceError::DataChanged);
        }
        digest.update(&buffer[..count]);
    }
    let after = fs::symlink_metadata(path).map_err(|_| RecoveryEvidenceError::Io)?;
    check_file(&after, owner)?;
    if metadata_key(&before) != metadata_key(&after)
        || length != before.len()
        || metadata_key(&before)
            != metadata_key(&file.metadata().map_err(|_| RecoveryEvidenceError::Io)?)
    {
        return Err(RecoveryEvidenceError::DataChanged);
    }
    Ok(Some(FileEvidence {
        device: before.dev(),
        inode: before.ino(),
        owner,
        mode: before.mode() & 0o7777,
        length,
        sha256: format!("{:x}", digest.finalize()),
    }))
}

fn metadata_key(value: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        value.dev(),
        value.ino(),
        value.len(),
        value.mtime(),
        value.mtime_nsec(),
        value.ctime(),
        value.ctime_nsec(),
    )
}

fn check_file(value: &Metadata, owner: u32) -> Result<(), RecoveryEvidenceError> {
    if !value.is_file()
        || value.uid() != owner
        || value.mode() & 0o7777 != 0o600
        || value.nlink() != 1
    {
        return Err(RecoveryEvidenceError::UnsafeObject);
    }
    Ok(())
}

fn directory_identity(
    path: &Path,
    owner: u32,
) -> Result<InstallRootIdentity, RecoveryEvidenceError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RecoveryEvidenceError::UnsafeObject)?;
    if !metadata.is_dir()
        || metadata.uid() != owner
        || metadata.mode() & 0o7777 != 0o700
        || fs::canonicalize(path).map_err(|_| RecoveryEvidenceError::UnsafeObject)? != path
    {
        return Err(RecoveryEvidenceError::UnsafeObject);
    }
    InstallRootIdentity::new(metadata.dev(), metadata.ino(), owner, 0o700)
        .map_err(|_| RecoveryEvidenceError::UnsafeObject)
}

fn optional_directory(
    path: &Path,
    owner: u32,
) -> Result<Option<InstallRootIdentity>, RecoveryEvidenceError> {
    if absent(path)? {
        Ok(None)
    } else {
        directory_identity(path, owner).map(Some)
    }
}

fn verify_directory(
    path: &Path,
    identity: &InstallRootIdentity,
) -> Result<(), RecoveryEvidenceError> {
    if &directory_identity(path, identity.owner_id())? != identity {
        return Err(RecoveryEvidenceError::BindingChanged);
    }
    Ok(())
}

fn create_directory(path: &Path, parent: &Path, owner: u32) -> Result<(), RecoveryEvidenceError> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => sync(parent)?,
        Err(io) if io.kind() == ErrorKind::AlreadyExists => {}
        Err(_) => return Err(RecoveryEvidenceError::Io),
    }
    directory_identity(path, owner)?;
    Ok(())
}

fn verify_entries(path: &Path) -> Result<(), RecoveryEvidenceError> {
    for item in fs::read_dir(path).map_err(|_| RecoveryEvidenceError::Io)? {
        if item.map_err(|_| RecoveryEvidenceError::Io)?.file_name() != EVIDENCE {
            return Err(RecoveryEvidenceError::InvalidEvidence);
        }
    }
    Ok(())
}

fn absent(path: &Path) -> Result<bool, RecoveryEvidenceError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(io) if io.kind() == ErrorKind::NotFound => Ok(true),
        Err(_) => Err(RecoveryEvidenceError::Io),
    }
}

fn sync(path: &Path) -> Result<(), RecoveryEvidenceError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| RecoveryEvidenceError::Io)
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
