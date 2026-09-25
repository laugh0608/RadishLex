//! Consume a released predecessor without rewriting or moving its frozen slots.
use super::*;

impl PreparationJournalStore {
    pub(in super::super) fn verify_empty_v1_slots(&self) -> Result<()> {
        for slot in SLOTS {
            if path_exists(&self.path(slot_name(slot)))? {
                return Err(Error::EvidenceChanged);
            }
        }
        Ok(())
    }

    pub(in super::super) fn capture_released_inventory(
        &self,
        guard: &UpgradeProcessGuard,
        operation: &str,
        previous_install: Option<&str>,
        previous_data: Option<&str>,
        hasher: &impl PreparationHasher,
    ) -> Result<PreviousInventory> {
        self.verify_guard(guard)?;
        self.verify_empty_v1_slots()?;
        if path_exists(&self.store.root.path.join(HISTORY).join(operation))? {
            return Err(Error::EvidenceChanged);
        }
        let inventory =
            self.read_released_inventory(operation, previous_install, previous_data, hasher)?;
        self.verify_guard(guard)?;
        Ok(inventory)
    }

    fn read_released_inventory(
        &self,
        operation: &str,
        previous_install: Option<&str>,
        previous_data: Option<&str>,
        hasher: &impl PreparationHasher,
    ) -> Result<PreviousInventory> {
        let mut inventory = PreviousInventory::empty(self, operation, previous_install)?;
        let reader = ReleaseHistory { journal: self };
        let index = reader.load_index(hasher)?.ok_or(Error::EvidenceChanged)?;
        if Some(index.index.data_operation_id.as_str()) != previous_data
            || index.index.operation_id == operation
        {
            return Err(Error::EvidenceChanged);
        }
        let path = self
            .store
            .root
            .path
            .join(HISTORY)
            .join(&index.index.operation_id)
            .join(RELEASE);
        let identity = self.evidence(&path, hasher)?;
        let proof = reader.resolve_index(&index.index, hasher)?;
        self.verify_previous_receipt(&proof.receipt, operation)?;
        if self.evidence(&path, hasher)? != identity
            || reader.load_index(hasher)?.as_ref() != Some(&index)
        {
            return Err(Error::EvidenceChanged);
        }
        inventory.directories = Some(
            proof
                .directories
                .try_into()
                .map_err(|_| Error::EvidenceChanged)?,
        );
        inventory.receipt = Some(proof.receipt);
        inventory.files = proof.files;
        inventory.released = Some(inventory::ReleasedInventoryEvidence {
            proof: identity,
            index: index.identity,
            installed_product_sha256: proof.binding.installed_product_sha256,
        });
        Ok(inventory)
    }

    pub(in super::super) fn verify_released_inventory(
        &self,
        inventory: &PreviousInventory,
        hasher: &impl PreparationHasher,
    ) -> Result<()> {
        let next_operation = self
            .store
            .root
            .path
            .join(HISTORY)
            .join(&inventory.operation_id);
        if path_exists(&next_operation)? {
            return Err(Error::EvidenceChanged);
        }
        let current = self.read_released_inventory(
            &inventory.operation_id,
            inventory.previous_install_operation_id.as_deref(),
            inventory.receipt.as_ref().map(UpgradeReceipt::operation_id),
            hasher,
        )?;
        if current != *inventory || path_exists(&next_operation)? {
            return Err(Error::EvidenceChanged);
        }
        Ok(())
    }

    pub(in super::super) fn load_released_inventory(
        &self,
        record: &PreparationReceipt,
        hasher: &impl PreparationHasher,
    ) -> Result<PreviousInventory> {
        let binding = record.binding();
        let inventory = self.read_released_inventory(
            &binding.operation_id,
            binding.previous_install_operation_id.as_deref(),
            binding.previous_data_operation_id.as_deref(),
            hasher,
        )?;
        let released = inventory.released.as_ref().ok_or(Error::EvidenceChanged)?;
        if record.previous_inventory_identity() != Some(&released.proof)
            || record.previous_release_index_identity() != Some(&released.index)
            || binding.source_product_sha256 != released.installed_product_sha256
        {
            return Err(Error::EvidenceChanged);
        }
        inventory.verify_context(self, record)?;
        Ok(inventory)
    }
}
