//! Immutable inventory captured before reservation and before any SQLite write.
use serde::{Deserialize, Serialize};

use super::*;

pub(super) const HISTORY: &str = ".radishlex-upgrade-history-v1";
pub(super) const INVENTORY: &str = "inventory.json";
const STAGED_INVENTORY: &str = "inventory.json.tmp";
const FORMAT: &str = "radishlex-previous-upgrade-inventory-v1";
pub(super) const SLOTS: [Slot; 5] = [
    Slot::Receipt,
    Slot::Snapshot,
    Slot::Candidate,
    Slot::Backup,
    Slot::Settings,
];

/// Canonical private evidence, not an authorization or a movable runtime database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviousInventory {
    format: String,
    pub(super) operation_id: String,
    pub(super) previous_install_operation_id: Option<String>,
    pub(super) data_root: PreparationDirectoryIdentity,
    pub(super) state_directory: PreparationDirectoryIdentity,
    pub(super) directories: Option<[PreparationDirectoryIdentity; 3]>,
    pub(super) receipt: Option<UpgradeReceipt>,
    pub(super) files: [Option<crate::PreparationFileIdentity>; 5],
    // Released inventories are projections of the sealed release, never a new
    // inventory.json. This in-memory discriminator cannot be decoded from disk.
    #[serde(skip)]
    pub(super) released: Option<ReleasedInventoryEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReleasedInventoryEvidence {
    pub proof: crate::PreparationFileIdentity,
    pub index: crate::PreparationFileIdentity,
    pub installed_product_sha256: String,
}

impl PreviousInventory {
    pub fn release_index_identity(&self) -> Option<&crate::PreparationFileIdentity> {
        self.released.as_ref().map(|value| &value.index)
    }
    pub fn receipt_sha256(&self) -> Option<&str> {
        self.files[0]
            .as_ref()
            .map(|identity| identity.sha256.as_str())
    }

    pub(super) fn empty(
        store: &PreparationJournalStore,
        operation: &str,
        install: Option<&str>,
    ) -> Result<Self> {
        if !valid_id(operation) || install.is_some_and(|id| !valid_id(id) || id == operation) {
            return Err(Error::EvidenceChanged);
        }
        Ok(Self {
            format: FORMAT.to_owned(),
            operation_id: operation.to_owned(),
            previous_install_operation_id: install.map(str::to_owned),
            data_root: store.data_root_identity(),
            state_directory: store.state_directory_identity(),
            directories: None,
            receipt: None,
            files: std::array::from_fn(|_| None),
            released: None,
        })
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>> {
        if self.released.is_some() {
            return Err(Error::EvidenceChanged);
        }
        let mut bytes = serde_json::to_vec(self).map_err(|_| Error::EvidenceChanged)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
            return Err(Error::EvidenceChanged);
        }
        Ok(bytes)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
            return Err(Error::EvidenceChanged);
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Error::EvidenceChanged)?;
        if value.format != FORMAT || value.encode()? != bytes {
            return Err(Error::EvidenceChanged);
        }
        if let Some(receipt) = &value.receipt {
            receipt.encode().map_err(|_| Error::EvidenceChanged)?;
        }
        Ok(value)
    }

    pub(super) fn paths(&self, store: &PreparationJournalStore) -> Result<[PathBuf; 3]> {
        let receipt = self.receipt.as_ref().ok_or(Error::EvidenceChanged)?;
        if !valid_id(receipt.operation_id()) {
            return Err(Error::EvidenceChanged);
        }
        let root = store.store.root.path.join(HISTORY);
        let operation = root.join(receipt.operation_id());
        let data = operation.join("data");
        Ok([root, operation, data])
    }

    pub(super) fn verify_context(
        &self,
        store: &PreparationJournalStore,
        record: &PreparationReceipt,
    ) -> Result<()> {
        let binding = record.binding();
        if self.operation_id != binding.operation_id
            || self.previous_install_operation_id != binding.previous_install_operation_id
            || self.receipt.as_ref().map(UpgradeReceipt::operation_id)
                != binding.previous_data_operation_id.as_deref()
            || self.receipt_sha256() != binding.previous_data_receipt_sha256.as_deref()
            || self.data_root != store.data_root_identity()
            || self.state_directory != store.state_directory_identity()
        {
            return Err(Error::EvidenceChanged);
        }
        if let Some(receipt) = &self.receipt {
            store.verify_previous_receipt(receipt, &binding.operation_id)?;
            // A repair may change the outer predecessor, but not the installed release.
            let release = if receipt.state() == UpgradeState::Completed {
                receipt.target_release()
            } else {
                receipt.source_release()
            };
            if release != &binding.source_release {
                return Err(Error::EvidenceChanged);
            }
        }
        Ok(())
    }
}

impl PreparationJournalStore {
    /// Call under the outer guard and fresh product/quiescence authorization,
    /// before persisting a reservation. Only private history metadata is created.
    /// Existing operation targets or interrupted initialization are never adopted.
    pub fn capture_previous_inventory(
        &self,
        guard: &UpgradeProcessGuard,
        operation: &str,
        previous_install: Option<&str>,
        previous_data: Option<&str>,
        hasher: &impl PreparationHasher,
    ) -> Result<PreviousInventory> {
        self.capture_inventory_with_faults(
            guard,
            operation,
            previous_install,
            previous_data,
            hasher,
            &NoHistoryFaults,
        )
    }

    fn capture_inventory_with_faults(
        &self,
        guard: &UpgradeProcessGuard,
        operation: &str,
        previous_install: Option<&str>,
        previous_data: Option<&str>,
        hasher: &impl PreparationHasher,
        faults: &impl HistoryFaults,
    ) -> Result<PreviousInventory> {
        if self.load_guarded(guard)?.is_some() {
            return Err(Error::InvalidPhase);
        }
        let mut inventory = PreviousInventory::empty(self, operation, previous_install)?;
        if previous_data.is_some_and(|id| !valid_id(id) || id == operation) {
            return Err(Error::EvidenceChanged);
        }
        if path_exists(&self.path(RECEIPT_FILE_NAME))? {
            let identity = self.evidence(&self.path(RECEIPT_FILE_NAME), hasher)?;
            let bytes = self.read_history_bytes(&self.path(RECEIPT_FILE_NAME))?;
            let receipt = UpgradeReceipt::decode(&bytes).map_err(|_| Error::EvidenceChanged)?;
            self.verify_previous_receipt(&receipt, operation)?;
            if Some(receipt.operation_id()) != previous_data {
                return Err(Error::EvidenceChanged);
            }
            inventory.files[0] = Some(identity);
            for (index, slot) in SLOTS.iter().enumerate().skip(1) {
                let expected = expected_private(&receipt, *slot);
                let path = self.path(slot_name(*slot));
                inventory.files[index] = match (expected, path_exists(&path)?) {
                    (Some(expected), true) => {
                        let evidence = self.evidence(&path, hasher)?;
                        if !matches_artifact(&evidence, expected) {
                            return Err(Error::EvidenceChanged);
                        }
                        Some(evidence)
                    }
                    (None, false) => None,
                    _ => return Err(Error::EvidenceChanged),
                };
            }
            inventory.receipt = Some(receipt);
        } else {
            if previous_data.is_some() {
                return self.capture_released_inventory(
                    guard,
                    operation,
                    previous_install,
                    previous_data,
                    hasher,
                );
            }
            if path_exists(&self.store.root.path.join(HISTORY))? {
                // History requires an explicit predecessor, never a scan or an
                // invented first-upgrade inventory.
                return Err(Error::EvidenceChanged);
            }
            self.verify_inventory_files(&inventory, hasher, None)?;
            self.verify_guard(guard)?;
            return Ok(inventory);
        }
        self.verify_inventory_files(&inventory, hasher, None)?;
        let paths = inventory.paths(self)?;
        // Fresh history root may be created; an existing root must already be
        // safe. The old operation directory always uses exclusive creation.
        if !path_exists(&paths[0])? {
            self.create_history_directory(&paths[0])?;
            faults.hit(HistoryFault::DirectoryCreated(0))?;
            sync_directory(&self.store.root.path)?;
            faults.hit(HistoryFault::DirectorySynced(0))?;
        }
        let root_identity = self.history_directory(&paths[0])?;
        // Re-establish durability of an empty shared parent left by interruption.
        // This never adopts an existing private operation or its materials.
        sync_directory(&paths[0])?;
        sync_directory(&self.store.root.path)?;
        if path_exists(&paths[1])? || path_exists(&paths[0].join(operation))? {
            return Err(Error::EvidenceChanged);
        }
        self.verify_guard(guard)?;
        self.create_history_directory(&paths[1])?;
        faults.hit(HistoryFault::DirectoryCreated(1))?;
        sync_directory(&paths[0])?;
        faults.hit(HistoryFault::DirectorySynced(1))?;
        let operation_identity = self.history_directory(&paths[1])?;
        self.create_history_directory(&paths[2])?;
        faults.hit(HistoryFault::DirectoryCreated(2))?;
        sync_directory(&paths[1])?;
        faults.hit(HistoryFault::DirectorySynced(2))?;
        inventory.directories = Some([
            root_identity,
            operation_identity,
            self.history_directory(&paths[2])?,
        ]);
        self.verify_history_directories(&inventory, false)?;
        let bytes = inventory.encode()?;
        let staged = paths[1].join(STAGED_INVENTORY);
        let final_path = paths[1].join(INVENTORY);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|_| Error::Io)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Io)?;
        faults.hit(HistoryFault::InventoryCreated)?;
        file.write_all(&bytes).map_err(|_| Error::Io)?;
        faults.hit(HistoryFault::InventoryWritten)?;
        file.sync_all().map_err(|_| Error::Io)?;
        faults.hit(HistoryFault::InventorySynced)?;
        let staged_identity = self.evidence(&staged, hasher)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if metadata.ino() != staged_identity.inode || metadata.dev() != staged_identity.device_id {
            return Err(Error::EvidenceChanged);
        }
        faults.hit(HistoryFault::BeforeInventoryRename)?;
        self.verify_guard(guard)?;
        self.verify_history_directories(&inventory, false)?;
        self.verify_inventory_files(&inventory, hasher, None)?;
        if self.evidence(&staged, hasher)? != staged_identity || path_exists(&final_path)? {
            return Err(Error::EvidenceChanged);
        }
        fs::rename(&staged, &final_path).map_err(|_| Error::Io)?;
        faults.hit(HistoryFault::InventoryRenamed)?;
        sync_directory(&paths[1])?;
        faults.hit(HistoryFault::InventoryDirectorySynced)?;
        self.verify_guard(guard)?;
        self.verify_history_directories(&inventory, true)?;
        self.verify_inventory_files(&inventory, hasher, None)?;
        if self.evidence(&final_path, hasher)? != staged_identity
            || self.read_history_bytes(&final_path)? != bytes
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(inventory)
    }

    /// Freshly verifies the immutable inventory and its files before the caller
    /// binds this identity into a new PreparationReceipt.
    pub fn previous_inventory_identity(
        &self,
        guard: &UpgradeProcessGuard,
        inventory: &PreviousInventory,
        hasher: &impl PreparationHasher,
    ) -> Result<Option<crate::PreparationFileIdentity>> {
        self.verify_guard(guard)?;
        self.validate_entries()?;
        self.reject_staged_record()?;
        if let Some(released) = &inventory.released {
            self.verify_released_inventory(inventory, hasher)?;
            self.verify_empty_v1_slots()?;
            self.verify_guard(guard)?;
            return Ok(Some(released.proof.clone()));
        }
        if inventory.data_root != self.data_root_identity()
            || inventory.state_directory != self.state_directory_identity()
        {
            return Err(Error::EvidenceChanged);
        }
        self.verify_inventory_files(inventory, hasher, None)?;
        if inventory.receipt.is_none() {
            if path_exists(&self.store.root.path.join(HISTORY))? {
                return Err(Error::EvidenceChanged);
            }
            return Ok(None);
        }
        self.verify_history_directories(inventory, true)?;
        let path = inventory.paths(self)?[1].join(INVENTORY);
        let identity = self.evidence(&path, hasher)?;
        if self.read_history_bytes(&path)? != inventory.encode()?
            || self.evidence(&path, hasher)? != identity
        {
            return Err(Error::EvidenceChanged);
        }
        self.verify_guard(guard)?;
        Ok(Some(identity))
    }

    pub(super) fn verify_previous_receipt(
        &self,
        receipt: &UpgradeReceipt,
        operation: &str,
    ) -> Result<()> {
        receipt.encode().map_err(|_| Error::EvidenceChanged)?;
        if !receipt.state().is_terminal()
            || receipt.manual_recovery_required()
            || receipt.operation_id() == operation
        {
            return Err(Error::EvidenceChanged);
        }
        let root = receipt
            .artifacts()
            .iter()
            .find(|item| item.slot() == UpgradeArtifactSlot::DataRoot)
            .ok_or(Error::EvidenceChanged)?;
        let actual = self.data_root_identity();
        if root.device_id() != actual.device_id
            || root.inode() != actual.inode
            || root.owner_id() != actual.owner_id
            || root.mode() != actual.mode
        {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    pub(super) fn read_history_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|_| Error::Io)?
            .take(MAX_PREPARATION_RECEIPT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Io)?;
        if bytes.len() > MAX_PREPARATION_RECEIPT_BYTES {
            return Err(Error::EvidenceChanged);
        }
        Ok(bytes)
    }

    fn create_history_directory(&self, path: &Path) -> Result<()> {
        DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|_| Error::EvidenceChanged)?;
        // No chmod of an existing path, and no adoption of an unsafe umask result.
        self.history_directory(path)?;
        sync_directory(path)?;
        Ok(())
    }
}

pub(super) fn slot_name(slot: Slot) -> &'static str {
    match slot {
        Slot::Receipt => RECEIPT_FILE_NAME,
        Slot::Snapshot => SNAPSHOT_FILE_NAME,
        Slot::Candidate => CANDIDATE_FILE_NAME,
        Slot::Backup => SOURCE_BACKUP_FILE_NAME,
        Slot::Settings => SETTINGS_BACKUP_FILE_NAME,
    }
}
pub(super) fn expected_private(
    receipt: &UpgradeReceipt,
    slot: Slot,
) -> Option<&UpgradeArtifactIdentity> {
    let artifact_slot = match slot {
        Slot::Receipt => return None,
        Slot::Snapshot => UpgradeArtifactSlot::SnapshotDatabase,
        Slot::Candidate if receipt.state() != UpgradeState::Completed => {
            UpgradeArtifactSlot::CandidateDatabase
        }
        Slot::Backup if receipt.state() == UpgradeState::Completed => {
            UpgradeArtifactSlot::BackupDatabase
        }
        Slot::Settings => UpgradeArtifactSlot::BackupSettings,
        _ => return None,
    };
    receipt
        .artifacts()
        .iter()
        .find(|artifact| artifact.slot() == artifact_slot)
}
pub(super) fn matches_artifact(
    file: &crate::PreparationFileIdentity,
    artifact: &UpgradeArtifactIdentity,
) -> bool {
    file.device_id == artifact.device_id()
        && file.inode == artifact.inode()
        && file.owner_id == artifact.owner_id()
        && file.mode == artifact.mode()
        && file.link_count == artifact.link_count()
        && file.byte_len == artifact.byte_len()
}
fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoryFault {
    DirectoryCreated(usize),
    DirectorySynced(usize),
    InventoryCreated,
    InventoryWritten,
    InventorySynced,
    BeforeInventoryRename,
    InventoryRenamed,
    InventoryDirectorySynced,
}
trait HistoryFaults {
    fn hit(&self, point: HistoryFault) -> Result<()>;
}
struct NoHistoryFaults;
impl HistoryFaults for NoHistoryFaults {
    fn hit(&self, _: HistoryFault) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "preparation_inventory_tests.rs"]
mod tests;
