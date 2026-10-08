//! Exclusive canonical outer preservation before reservation. Unknown partial
//! directories/files are retained and never adopted by a later new task.
use super::*;
use radishlex_ime_product_upgrade::{PreparationFileIdentity, PreparationHasher};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::Path;

const HISTORY: &str = ".radishlex-install-history-v1";
const STATE: &str = ".radishlex-install-v1";

pub(super) fn present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(Error::Io),
    }
}
fn directory(
    path: &Path,
    root: &PreparationDirectoryIdentity,
) -> Result<PreparationDirectoryIdentity> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::InvalidBinding)?;
    if !metadata.is_dir()
        || fs::canonicalize(path).map_err(|_| Error::InvalidBinding)? != path
        || metadata.dev() != root.device_id
        || metadata.uid() != root.owner_id
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(Error::InvalidBinding);
    }
    Ok(PreparationDirectoryIdentity {
        device_id: metadata.dev(),
        inode: metadata.ino(),
        owner_id: metadata.uid(),
        mode: metadata.mode() & 0o7777,
    })
}
fn file(
    path: &Path,
    root: &PreparationDirectoryIdentity,
) -> Result<(PreparationFileIdentity, Vec<u8>)> {
    bounded_file(
        path,
        root,
        radishlex_ime_product_install::MAX_INSTALL_RECEIPT_BYTES,
    )
}

fn bounded_file(
    path: &Path,
    root: &PreparationDirectoryIdentity,
    limit: usize,
) -> Result<(PreparationFileIdentity, Vec<u8>)> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::InvalidBinding)?;
    if !metadata.is_file()
        || metadata.dev() != root.device_id
        || metadata.uid() != root.owner_id
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() > limit as u64
        || fs::canonicalize(path).map_err(|_| Error::InvalidBinding)? != path
    {
        return Err(Error::InvalidBinding);
    }
    let opened = File::open(path).map_err(|_| Error::Io)?;
    let opened_metadata = opened.metadata().map_err(|_| Error::Io)?;
    if metadata.dev() != opened_metadata.dev() || metadata.ino() != opened_metadata.ino() {
        return Err(Error::InvalidBinding);
    }
    let mut bytes = vec![];
    opened
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Io)?;
    let after = fs::symlink_metadata(path).map_err(|_| Error::Io)?;
    let stamp = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.uid(),
            m.mode(),
            m.nlink(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if stamp(&metadata) != stamp(&after) || bytes.len() as u64 != metadata.len() {
        return Err(Error::InvalidBinding);
    }
    let sha256 = MacOsPreparationHasher
        .sha256(&mut bytes.as_slice())
        .map_err(|_| Error::Io)?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((
        PreparationFileIdentity {
            device_id: metadata.dev(),
            inode: metadata.ino(),
            owner_id: metadata.uid(),
            mode: metadata.mode() & 0o7777,
            link_count: metadata.nlink(),
            byte_len: metadata.len(),
            sha256,
        },
        bytes,
    ))
}
fn sync(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Io)
}
fn only(path: &Path, allowed: &[&str]) -> Result<()> {
    for entry in fs::read_dir(path).map_err(|_| Error::Io)? {
        if !allowed
            .iter()
            .any(|name| entry.as_ref().is_ok_and(|e| e.file_name() == *name))
        {
            return Err(Error::InvalidBinding);
        }
    }
    Ok(())
}

pub(super) fn verify(
    install: &InstallReceiptStore,
    previous: &str,
    evidence: &PreparationInstallEvidence,
    bytes: &[u8],
) -> Result<()> {
    let root = directory_identity(install);
    if directory(install.data_root_path(), &root)? != root {
        return Err(Error::InvalidBinding);
    }
    let history = install.data_root_path().join(HISTORY);
    let operation = history.join(previous);
    for (path, expected) in [
        (
            &install.data_root_path().join(STATE),
            &evidence.state_directory,
        ),
        (&history, &evidence.history_root),
        (&operation, &evidence.previous_directory),
    ] {
        if directory(path, &root)? != *expected {
            return Err(Error::InvalidBinding);
        }
    }
    only(&operation, &["receipt.json"])?;
    let (identity, actual) = file(&operation.join("receipt.json"), &root)?;
    if identity != evidence.previous_receipt || actual != bytes {
        return Err(Error::InvalidBinding);
    }
    Ok(())
}

pub(crate) fn verify_preparation(
    install: &InstallReceiptStore,
    prep: &PreparationReceipt,
) -> Result<InstallReceipt> {
    let b = prep.binding();
    if b.data_root != directory_identity(install) {
        return Err(Error::InvalidBinding);
    }
    let previous = b
        .previous_install_operation_id
        .as_ref()
        .ok_or(Error::InvalidBinding)?;
    let evidence = prep
        .previous_install_evidence()
        .ok_or(Error::InvalidBinding)?;
    let (_, bytes) = file(
        &install
            .data_root_path()
            .join(HISTORY)
            .join(previous)
            .join("receipt.json"),
        &b.data_root,
    )?;
    verify(install, previous, evidence, &bytes)?;
    let value = InstallReceipt::decode(&bytes).map_err(|_| Error::InvalidBinding)?;
    if value.encode().map_err(|_| Error::InvalidBinding)? != bytes
        || value.operation_id() != previous
        || !value.state().is_terminal()
        || value.manual_recovery_required()
        || value.root_identity() != install.root_identity()
        || !value.installed_product().is_some_and(|p| {
            same_release(&b.source_release, p)
                && b.source_product_sha256 == p.product_manifest_sha256()
        })
    {
        return Err(Error::InvalidBinding);
    }
    Ok(value)
}

pub(super) fn seal(
    install: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    previous: &InstallReceipt,
    reuse: Option<&PreparationInstallEvidence>,
    mut checkpoint: impl FnMut(ExecutionPoint) -> Result<()>,
) -> Result<PreparationInstallEvidence> {
    let root = directory_identity(install);
    let root_path = install.data_root_path();
    let history = root_path.join(HISTORY);
    let operation = history.join(previous.operation_id());
    let active = root_path.join(STATE).join("receipt.json");
    let bytes = previous.encode().map_err(|_| Error::InvalidBinding)?;
    let original = file(&active, &root)?;
    if original.1 != bytes {
        return Err(Error::InvalidBinding);
    }
    let verify_active = || -> Result<()> {
        check_current(install, guard, previous)?;
        if file(&active, &root)? != original {
            return Err(Error::InvalidBinding);
        }
        Ok(())
    };
    if let Some(evidence) = reuse {
        // Only the independently resolved cancellation lifecycle can select
        // this branch. Bare existing bytes cannot make an orphan reusable.
        verify(install, previous.operation_id(), evidence, &bytes)?;
        checkpoint(ExecutionPoint::OriginalDirectorySynced)?;
        verify_active()?;
        verify(install, previous.operation_id(), evidence, &bytes)?;
        return Ok(evidence.clone());
    }
    if present(&operation)? {
        return Err(Error::InvalidBinding);
    }
    let state_directory = directory(&root_path.join(STATE), &root)?;
    if !present(&history)? {
        DirBuilder::new()
            .mode(0o700)
            .create(&history)
            .map_err(|_| Error::Io)?;
    }
    let history_root = directory(&history, &root)?;
    let checked = |point, checkpoint: &mut dyn FnMut(ExecutionPoint) -> Result<()>| -> Result<()> {
        verify_active()?;
        if directory(&history, &root)? != history_root {
            return Err(Error::InvalidBinding);
        }
        checkpoint(point)?;
        verify_active()?;
        if directory(&history, &root)? != history_root {
            return Err(Error::InvalidBinding);
        }
        Ok(())
    };
    checked(ExecutionPoint::HistoryRootCreated, &mut checkpoint)?;
    sync(&history)?;
    checked(ExecutionPoint::HistoryRootSynced, &mut checkpoint)?;
    sync(root_path)?;
    checked(ExecutionPoint::HistoryRootParentSynced, &mut checkpoint)?;
    DirBuilder::new()
        .mode(0o700)
        .create(&operation)
        .map_err(|_| Error::Io)?;
    let previous_directory = directory(&operation, &root)?;
    let checked_operation =
        |point, checkpoint: &mut dyn FnMut(ExecutionPoint) -> Result<()>| -> Result<()> {
            if directory(&operation, &root)? != previous_directory {
                return Err(Error::InvalidBinding);
            }
            checked(point, checkpoint)?;
            if directory(&operation, &root)? != previous_directory {
                return Err(Error::InvalidBinding);
            }
            Ok(())
        };
    checked_operation(ExecutionPoint::HistoryDirectoryCreated, &mut checkpoint)?;
    sync(&operation)?;
    checked_operation(ExecutionPoint::HistoryDirectorySynced, &mut checkpoint)?;
    sync(&history)?;
    checked_operation(
        ExecutionPoint::HistoryDirectoryParentSynced,
        &mut checkpoint,
    )?;
    only(&operation, &[])?;
    let staged = operation.join("receipt.json.tmp");
    let final_path = operation.join("receipt.json");
    let mut output = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staged)
        .map_err(|_| Error::Io)?;
    let initial = file(&staged, &root)?.0;
    let opened = output.metadata().map_err(|_| Error::Io)?;
    if opened.ino() != initial.inode || opened.dev() != initial.device_id {
        return Err(Error::InvalidBinding);
    }
    checked_operation(ExecutionPoint::OriginalCreated, &mut checkpoint)?;
    if file(&staged, &root)?.0 != initial {
        return Err(Error::InvalidBinding);
    }
    output.write_all(&bytes).map_err(|_| Error::Io)?;
    let (identity, actual) = file(&staged, &root)?;
    if actual != bytes || identity.inode != initial.inode {
        return Err(Error::InvalidBinding);
    }
    checked_operation(ExecutionPoint::OriginalWritten, &mut checkpoint)?;
    if file(&staged, &root)?.0 != identity {
        return Err(Error::InvalidBinding);
    }
    output.sync_all().map_err(|_| Error::Io)?;
    checked_operation(ExecutionPoint::OriginalSynced, &mut checkpoint)?;
    only(&operation, &["receipt.json.tmp"])?;
    if file(&staged, &root)?.0 != identity || present(&final_path)? {
        return Err(Error::InvalidBinding);
    }
    fs::rename(&staged, &final_path).map_err(|_| Error::Io)?;
    let evidence = PreparationInstallEvidence {
        state_directory,
        history_root: history_root.clone(),
        previous_directory: previous_directory.clone(),
        previous_receipt: identity,
    };
    checked_operation(ExecutionPoint::OriginalRenamed, &mut checkpoint)?;
    verify(install, previous.operation_id(), &evidence, &bytes)?;
    sync(&operation)?;
    checked_operation(ExecutionPoint::OriginalDirectorySynced, &mut checkpoint)?;
    verify(install, previous.operation_id(), &evidence, &bytes)?;
    Ok(evidence)
}

pub(crate) fn sync_active(install: &InstallReceiptStore, expected: &InstallReceipt) -> Result<()> {
    let root = directory_identity(install);
    let state = install.data_root_path().join(STATE);
    let path = state.join("receipt.json");
    let observed = file(&path, &root)?;
    if observed.1 != expected.encode().map_err(|_| Error::InvalidBinding)? {
        return Err(Error::InvalidBinding);
    }
    let directory_before = directory(&state, &root)?;
    sync(&path)?;
    sync(&state)?;
    if file(&path, &root)? != observed || directory(&state, &root)? != directory_before {
        return Err(Error::InvalidBinding);
    }
    Ok(())
}

pub(crate) fn active_identity(install: &InstallReceiptStore) -> Result<PreparationFileIdentity> {
    file(
        &install.data_root_path().join(STATE).join("receipt.json"),
        &directory_identity(install),
    )
    .map(|value| value.0)
}

pub(crate) fn preparation_proof_identity(
    install: &InstallReceiptStore,
    preparation: &PreparationReceipt,
) -> Result<PreparationFileIdentity> {
    let binding = preparation.binding();
    let path = install
        .data_root_path()
        .join(".radishlex-upgrade-history-v1")
        .join(&binding.operation_id)
        .join("preparation.json");
    bounded_file(
        &path,
        &binding.data_root,
        radishlex_ime_product_upgrade::MAX_PREPARATION_RECEIPT_BYTES,
    )
    .map(|value| value.0)
}

/// Reaffirm the retained predecessor's durability without replacing its inode.
pub(crate) fn sync_preparation_history(
    install: &InstallReceiptStore,
    preparation: &PreparationReceipt,
    mut checkpoint: impl FnMut() -> Result<()>,
) -> Result<()> {
    let previous = verify_preparation(install, preparation)?;
    let history = install.data_root_path().join(HISTORY);
    let operation = history.join(previous.operation_id());
    for path in [
        operation.join("receipt.json"),
        operation,
        history,
        install.data_root_path().to_path_buf(),
    ] {
        checkpoint()?;
        if verify_preparation(install, preparation)? != previous {
            return Err(Error::InvalidBinding);
        }
        sync(&path)?;
        if verify_preparation(install, preparation)? != previous {
            return Err(Error::InvalidBinding);
        }
        checkpoint()?;
    }
    Ok(())
}
