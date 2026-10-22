//! Phase-specific program material for a guarded prepared-upgrade continuation.
use super::*;
use radishlex_ime_product_install::InstallArtifactSlot as Slot;

impl MacOsProductInstallAdapter {
    /// Read-only view check. Initial prepared upgrades have no transaction
    /// folders yet; progressed upgrades require their exact existing stores.
    pub fn verify_prepared_upgrade_material_readonly(
        &self,
        receipt: &InstallReceipt,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.verify_data_root(receipt.root_identity())?;
        if self.revalidate_target_product()?
            != *receipt
                .target_product()
                .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidOperation))?
        {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        if receipt.state() == InstallState::Prepared && receipt.artifacts().is_empty() {
            let source = receipt
                .source_product()
                .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidOperation))?;
            if self.payload.upgrade_source_product(source.release()) != Some(source) {
                return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
            }
            self.verify_installed_product(Some(source))?;
            // A crash before Quiesced may have created empty directories only.
            for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
                let parent = self
                    .layout
                    .target_path(component)
                    .parent()
                    .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidOperation))?
                    .to_path_buf();
                let transaction =
                    parent.join(format!(".radishlex-install-{}", receipt.operation_id()));
                if !path_is_absent(&transaction)? {
                    let store = ProgramSwitchStore::open_existing(
                        self.layout.program_target(component)?,
                        receipt.operation_id(),
                    )
                    .map_err(|_| error(MacOsInstallAdapterErrorCode::CoreRejected))?;
                    if !path_is_absent(store.staged_bundle_path())?
                        || !path_is_absent(store.source_backup_bundle_path())?
                    {
                        return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
                    }
                }
            }
        } else {
            for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
                let store = ProgramSwitchStore::open_existing(
                    self.layout.program_target(component)?,
                    receipt.operation_id(),
                )
                .map_err(|_| error(MacOsInstallAdapterErrorCode::CoreRejected))?;
                self.verify_program_continuation_material(&store, receipt)?;
            }
        }
        Ok(())
    }
    /// Verify exact source/target bundles in their recorded or single pending
    /// rename locations. Unbound staging is never adopted by a restarted task.
    pub fn verify_program_continuation_material(
        &self,
        store: &ProgramSwitchStore,
        receipt: &InstallReceipt,
    ) -> Result<(), MacOsInstallAdapterError> {
        self.verify_store_binding(store, receipt)?;
        let source = receipt
            .source_product()
            .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidOperation))?;
        let target = receipt
            .target_product()
            .ok_or_else(|| error(MacOsInstallAdapterErrorCode::InvalidOperation))?;
        if receipt.operation_kind() != InstallOperationKind::Upgrade
            || self.payload.upgrade_source_product(source.release()) != Some(source)
            || self.revalidate_target_product()? != *target
        {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        let component = store.component();
        let (source_slot, staged_slot) = match component {
            ProgramComponent::Manager => (Slot::SourceManager, Slot::StagedManager),
            ProgramComponent::InputMethod => (Slot::SourceInputMethod, Slot::StagedInputMethod),
        };
        let backup = !path_is_absent(store.source_backup_bundle_path())?;
        let staged = !path_is_absent(store.staged_bundle_path())?;
        let installed = !path_is_absent(store.target_path())?;
        let initial = matches!(
            receipt.state(),
            InstallState::Prepared | InstallState::Quiesced
        );
        let restoring = matches!(
            receipt.state(),
            InstallState::RollbackRequired
                | InstallState::ProgramsRestored
                | InstallState::RolledBack
        );
        let committing = (receipt.state() == InstallState::SourcePreserved
            && component == ProgramComponent::Manager)
            || (receipt.state() == InstallState::ManagerCommitted
                && component == ProgramComponent::InputMethod);
        let committed = matches!(
            receipt.state(),
            InstallState::ProgramsCommitted
                | InstallState::DataCoordinating
                | InstallState::DataSettled
                | InstallState::FinalVerified
                | InstallState::Completed
        ) || (receipt.state() == InstallState::ManagerCommitted
            && component == ProgramComponent::Manager);
        let preserving = receipt.state() == InstallState::TargetStaged;
        let preserved = receipt.state() == InstallState::SourcePreserved;
        if (receipt.manual_recovery_required()
            && !matches!(
                receipt.state(),
                InstallState::RollbackRequired | InstallState::ProgramsRestored
            ))
            || !(initial || restoring || committing || committed || preserving || preserved)
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
        }
        let source_path = if backup {
            store.source_backup_bundle_path()
        } else {
            store.target_path()
        };
        if (initial && (backup || !installed))
            || ((committed || committing || preserved) && !backup)
            || (restoring
                && matches!(
                    receipt.state(),
                    InstallState::ProgramsRestored | InstallState::RolledBack
                )
                && backup)
        {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
        }
        self.verify_program(source_path, source.program(component))?;
        self.verify_continuation_inode(source_path, receipt.artifact(source_slot), initial)?;
        if let Some(evidence) = receipt.artifact(staged_slot) {
            let target_path = if staged {
                store.staged_bundle_path()
            } else {
                store.target_path()
            };
            if (!backup && !staged)
                || (backup && staged && installed)
                || ((preserving || (preserved && !committing)) && !staged)
                || (committed && (staged || !installed))
                || (initial && !staged)
            {
                return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
            }
            self.verify_continuation_inode(target_path, Some(evidence), false)?;
            self.verify_program(target_path, target.program(component))?;
        } else if !initial || staged || backup {
            return Err(error(MacOsInstallAdapterErrorCode::InvalidOperation));
        }
        Ok(())
    }

    fn verify_continuation_inode(
        &self,
        path: &Path,
        evidence: Option<&radishlex_ime_product_install::InstallArtifactEvidence>,
        optional: bool,
    ) -> Result<(), MacOsInstallAdapterError> {
        let Some(evidence) = evidence else {
            return if optional {
                Ok(())
            } else {
                Err(error(MacOsInstallAdapterErrorCode::InvalidOperation))
            };
        };
        let expected = evidence.filesystem_identity();
        let m = fs::symlink_metadata(path)
            .map_err(|_| error(MacOsInstallAdapterErrorCode::ProductChanged))?;
        if !m.is_dir()
            || (m.dev(), m.ino(), m.uid(), m.mode() & 0o7777)
                != (
                    expected.device_id(),
                    expected.inode(),
                    expected.owner_id(),
                    expected.mode(),
                )
        {
            return Err(error(MacOsInstallAdapterErrorCode::ProductChanged));
        }
        Ok(())
    }
}
