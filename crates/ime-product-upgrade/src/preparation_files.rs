use std::io::Seek;

use crate::{PreparationFamily, PreparationFileIdentity};

use super::*;

impl PreparationJournalStore {
    pub(super) fn evidence(
        &self,
        path: &Path,
        hasher: &impl PreparationHasher,
    ) -> Result<PreparationFileIdentity> {
        let before = self.private_metadata(path)?;
        let mut file = File::open(path).map_err(|_| Error::Io)?;
        if stamp(&file.metadata().map_err(|_| Error::Io)?) != stamp(&before) {
            return Err(Error::EvidenceChanged);
        }
        let digest = hasher.sha256(&mut file).map_err(|_| Error::Io)?;
        if file.stream_position().map_err(|_| Error::Io)? != before.len()
            || stamp(&file.metadata().map_err(|_| Error::Io)?) != stamp(&before)
            || stamp(&self.private_metadata(path)?) != stamp(&before)
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(PreparationFileIdentity {
            device_id: before.dev(),
            inode: before.ino(),
            owner_id: before.uid(),
            mode: before.mode() & 0o7777,
            link_count: before.nlink(),
            byte_len: before.len(),
            sha256: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
        })
    }

    pub(super) fn private_metadata(&self, path: &Path) -> Result<Metadata> {
        let metadata = fs::symlink_metadata(path).map_err(|_| Error::EvidenceChanged)?;
        if !metadata.file_type().is_file()
            || metadata.dev() != self.store.root.identity.device_id()
            || metadata.uid() != self.store.root.expected_owner_id
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(metadata)
    }

    pub(super) fn family(&self, hasher: &impl PreparationHasher) -> Result<PreparationFamily> {
        let family = self.read_family(hasher)?;
        if family.journal.is_some() {
            return Err(Error::Maintenance(UserDbMaintenanceError::SidecarRemaining));
        }
        Ok(family)
    }

    pub(super) fn read_family(&self, hasher: &impl PreparationHasher) -> Result<PreparationFamily> {
        let source = self.source_path();
        let optional = |suffix: &str| -> Result<Option<PreparationFileIdentity>> {
            let path = sidecar(&source, suffix);
            if path_exists(&path)? {
                Ok(Some(self.evidence(&path, hasher)?))
            } else {
                Ok(None)
            }
        };
        let family = PreparationFamily {
            database: self.evidence(&source, hasher)?,
            wal: optional("-wal")?,
            shm: optional("-shm")?,
            journal: optional("-journal")?,
        };
        let mut inodes = std::collections::BTreeSet::new();
        for file in [
            Some(&family.database),
            family.wal.as_ref(),
            family.shm.as_ref(),
            family.journal.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if !inodes.insert(file.inode) {
                return Err(Error::EvidenceChanged);
            }
        }
        Ok(family)
    }

    pub(super) fn source_path(&self) -> PathBuf {
        self.store.root.path.join("userdb.sqlite3")
    }

    pub(super) fn verify_snapshot(
        &self,
        record: &PreparationReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        let path = self.path(PREPARATION_SNAPSHOT);
        no_sidecars(&path)?;
        let identity = self.evidence(&path, hasher)?;
        if record.snapshot_identity() != Some(&identity) {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    pub(super) fn budget(
        &self,
        logical_bytes: u64,
        port: &mut impl SourcePreparationPort,
    ) -> Result<SourcePreparationSpaceBudget> {
        let settings = self.store.root.path.join("manager-settings.json");
        let settings_bytes = if path_exists(&settings)? {
            self.private_metadata(&settings)?.len()
        } else {
            0
        };
        let source_bytes = self.private_metadata(&self.source_path())?.len();
        SourcePreparationSpaceBudget::evaluate(
            logical_bytes.max(source_bytes),
            settings_bytes,
            port.available_bytes().ok_or(Error::InsufficientSpace)?,
        )
    }
}

pub(super) fn no_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        if path_exists(&sidecar(path, suffix))? {
            return Err(Error::EvidenceChanged);
        }
    }
    Ok(())
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut path = path.as_os_str().to_os_string();
    path.push(suffix);
    path.into()
}

fn stamp(metadata: &Metadata) -> (FileIdentity, i64, i64, i64, i64) {
    (
        FileIdentity::from_metadata(metadata),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

pub(super) fn same_object(a: &PreparationFileIdentity, b: &PreparationFileIdentity) -> bool {
    a.device_id == b.device_id
        && a.inode == b.inode
        && a.owner_id == b.owner_id
        && a.mode == b.mode
        && a.link_count == b.link_count
}

pub(super) fn readonly_family(before: &PreparationFamily, after: &PreparationFamily) -> Result<()> {
    if before.database != after.database
        || before.journal.is_some()
        || after.journal.is_some()
        || before
            .wal
            .as_ref()
            .is_some_and(|wal| after.wal.as_ref() != Some(wal))
        || (before.wal.is_none() && after.wal.as_ref().is_some_and(|wal| wal.byte_len != 0))
    {
        return Err(Error::EvidenceChanged);
    }
    Ok(())
}
