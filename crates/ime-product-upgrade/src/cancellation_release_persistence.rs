//! Exclusive writes retain unknown temporaries; bound renames can resume.
use super::*;

impl CancellationReleaseStore {
    pub(super) fn sync_file(
        &self,
        path: &Path,
        identity: &PreparationFileIdentity,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        if self.journal.evidence(path, hasher)? != *identity {
            return Err(Error::EvidenceChanged);
        }
        let file = File::open(path).map_err(|_| Error::Io)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.ino() != identity.inode || metadata.dev() != identity.device_id {
            return Err(Error::EvidenceChanged);
        }
        file.sync_all().map_err(|_| Error::Io)?;
        if self.journal.evidence(path, hasher)? != *identity {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }
    pub(super) fn write_proof(
        &self,
        guard: &UpgradeProcessGuard,
        proof: &CancellationReleaseReceipt,
        port: &mut impl CancellationReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<PreparationFileIdentity> {
        let directory = self.directory(proof.operation_id());
        let path = directory.join(CANCELLATION_RELEASE);
        let staged = directory.join(PROOF_TEMP);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|_| Error::Io)?;
        let created = self.journal.evidence(&staged, hasher)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.ino() != created.inode || metadata.dev() != created.device_id {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            proof,
            None,
            Some((&staged, &created)),
            port,
            hasher,
            CP::ProofCreated,
            CancellationReleaseRequirement::Preserve,
        )?;
        let bytes = proof.encode()?;
        file.write_all(&bytes).map_err(|_| Error::Io)?;
        let written = self.journal.evidence(&staged, hasher)?;
        if !same_object(&created, &written) {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            proof,
            None,
            Some((&staged, &written)),
            port,
            hasher,
            CP::ProofWritten,
            CancellationReleaseRequirement::Preserve,
        )?;
        file.sync_all().map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            proof,
            None,
            Some((&staged, &written)),
            port,
            hasher,
            CP::ProofFileSynced,
            CancellationReleaseRequirement::Preserve,
        )?;
        if path_exists(&path)?
            || self.journal.read_history_bytes(&staged)? != bytes
            || self.journal.evidence(&staged, hasher)? != written
        {
            return Err(Error::EvidenceChanged);
        }
        fs::rename(&staged, &path).map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            proof,
            Some(&written),
            None,
            port,
            hasher,
            CP::ProofPublished,
            CancellationReleaseRequirement::Preserve,
        )?;
        sync_directory(&directory)?;
        self.checkpoint(
            guard,
            proof,
            Some(&written),
            None,
            port,
            hasher,
            CP::ProofDirectorySynced,
            CancellationReleaseRequirement::Preserve,
        )?;
        Ok(written)
    }
    pub(super) fn move_material(
        &self,
        guard: &UpgradeProcessGuard,
        proof: &CancellationReleaseReceipt,
        proof_identity: &PreparationFileIdentity,
        marker: bool,
        port: &mut impl CancellationReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        let (name, identity, points, requirement) = if marker {
            (
                REQUEST,
                proof.archive.source.request_identity(),
                [
                    CP::BeforeMarkerMove,
                    CP::MarkerFileSynced,
                    CP::MarkerMoved,
                    CP::MarkerTargetSynced,
                    CP::MarkerSourceSynced,
                ],
                CancellationReleaseRequirement::QualifiedSource,
            )
        } else {
            (
                ARCHIVE,
                &proof.archive_identity,
                [
                    CP::BeforeArchiveMove,
                    CP::ArchiveFileSynced,
                    CP::ArchiveMoved,
                    CP::ArchiveTargetSynced,
                    CP::ArchiveSourceSynced,
                ],
                CancellationReleaseRequirement::Preserve,
            )
        };
        self.checkpoint(
            guard,
            proof,
            Some(proof_identity),
            None,
            port,
            hasher,
            points[0],
            requirement,
        )?;
        let source = self.active(name);
        let target = self.directory(proof.operation_id()).join(name);
        let location = self.unique(&source, &target)?;
        self.sync_file(&location, identity, hasher)?;
        self.checkpoint(
            guard,
            proof,
            Some(proof_identity),
            None,
            port,
            hasher,
            points[1],
            requirement,
        )?;
        if location == source {
            if path_exists(&target)? || self.journal.evidence(&source, hasher)? != *identity {
                return Err(Error::EvidenceChanged);
            }
            fs::rename(&source, &target).map_err(|_| Error::Io)?;
        }
        self.checkpoint(
            guard,
            proof,
            Some(proof_identity),
            None,
            port,
            hasher,
            points[2],
            requirement,
        )?;
        sync_directory(&self.directory(proof.operation_id()))?;
        self.checkpoint(
            guard,
            proof,
            Some(proof_identity),
            None,
            port,
            hasher,
            points[3],
            requirement,
        )?;
        sync_directory(&self.journal.store.state_directory)?;
        self.checkpoint(
            guard,
            proof,
            Some(proof_identity),
            None,
            port,
            hasher,
            points[4],
            requirement,
        )
    }
    pub(super) fn publish_index(
        &self,
        guard: &UpgradeProcessGuard,
        proof: &CancellationReleaseReceipt,
        identity: &PreparationFileIdentity,
        port: &mut impl CancellationReleasePort,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        self.checkpoint(
            guard,
            proof,
            Some(identity),
            None,
            port,
            hasher,
            CP::BeforeIndexWrite,
            CancellationReleaseRequirement::Preserve,
        )?;
        let root = self.journal.store.root.path.join(HISTORY);
        let path = root.join(INDEX);
        let expected = self
            .history()
            .cancellation_index(&proof.encoded()?, hasher)?;
        if let Some(current) = self.history().load_index(hasher)? {
            if current.index == expected {
                self.sync_file(&path, &current.identity, hasher)?;
                sync_directory(&root)?;
                return self.require_index(proof, hasher);
            }
        }
        let staged = root.join(INDEX_TEMP);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|_| Error::Io)?;
        let created = self.journal.evidence(&staged, hasher)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.ino() != created.inode || metadata.dev() != created.device_id {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            proof,
            Some(identity),
            Some((&staged, &created)),
            port,
            hasher,
            CP::IndexCreated,
            CancellationReleaseRequirement::Preserve,
        )?;
        let bytes = encode(&expected)?;
        file.write_all(&bytes).map_err(|_| Error::Io)?;
        let written = self.journal.evidence(&staged, hasher)?;
        if !same_object(&created, &written) {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            proof,
            Some(identity),
            Some((&staged, &written)),
            port,
            hasher,
            CP::IndexWritten,
            CancellationReleaseRequirement::Preserve,
        )?;
        file.sync_all().map_err(|_| Error::Io)?;
        self.checkpoint(
            guard,
            proof,
            Some(identity),
            Some((&staged, &written)),
            port,
            hasher,
            CP::IndexFileSynced,
            CancellationReleaseRequirement::Preserve,
        )?;
        if self
            .history()
            .load_index_with_temp(hasher, Some(&written))?
            != proof.previous_index
            || self.journal.read_history_bytes(&staged)? != bytes
            || self.journal.evidence(&staged, hasher)? != written
        {
            return Err(Error::EvidenceChanged);
        }
        fs::rename(&staged, &path).map_err(|_| Error::Io)?;
        if self.journal.evidence(&path, hasher)? != written {
            return Err(Error::EvidenceChanged);
        }
        self.checkpoint(
            guard,
            proof,
            Some(identity),
            None,
            port,
            hasher,
            CP::IndexPublished,
            CancellationReleaseRequirement::Preserve,
        )?;
        sync_directory(&root)?;
        self.checkpoint(
            guard,
            proof,
            Some(identity),
            None,
            port,
            hasher,
            CP::IndexDirectorySynced,
            CancellationReleaseRequirement::Preserve,
        )?;
        self.require_index(proof, hasher)
    }
}
