use std::fs::{self, DirBuilder, File, Metadata};
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use crate::{
    validate_operation_id, InstallArtifactEvidence, InstallArtifactSlot, InstallOperationKind,
    InstallProcessGuard, InstallReceipt, InstallReceiptStore, InstallState, ProgramComponent,
    ProgramFilesystemIdentity,
};

mod model;
pub use model::{
    NoProgramSwitchFaults, ProgramSwitchAction, ProgramSwitchBoundary, ProgramSwitchError,
    ProgramSwitchErrorCode, ProgramSwitchFaultInjector, ProgramSwitchFaultPoint,
};

pub const MANAGER_BUNDLE_NAME: &str = "RadishLex Manager.app";
pub const INPUT_METHOD_BUNDLE_NAME: &str = "RadishLexInputMethod.app";

const STAGED_BUNDLE_NAME: &str = "staged.app";
const SOURCE_BACKUP_BUNDLE_NAME: &str = "source-backup.app";

#[derive(Debug)]
pub struct VerifiedProgramTarget {
    component: ProgramComponent,
    parent: PathBuf,
    target: PathBuf,
    expected_owner_id: u32,
    parent_identity: DirectoryIdentity,
}

impl VerifiedProgramTarget {
    pub fn verify(
        parent: impl AsRef<Path>,
        component: ProgramComponent,
        expected_owner_id: u32,
    ) -> Result<Self, ProgramSwitchError> {
        let parent = parent.as_ref();
        if !parent.is_absolute()
            || parent
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        {
            return Err(error(ProgramSwitchErrorCode::UnsafeTargetParent));
        }
        let canonical = fs::canonicalize(parent)
            .map_err(|_| error(ProgramSwitchErrorCode::UnsafeTargetParent))?;
        if canonical != parent {
            return Err(error(ProgramSwitchErrorCode::UnsafeTargetParent));
        }
        let metadata = fs::symlink_metadata(parent)
            .map_err(|_| error(ProgramSwitchErrorCode::UnsafeTargetParent))?;
        verify_target_parent(&metadata, expected_owner_id)?;
        let target = parent.join(bundle_name(component));
        Ok(Self {
            component,
            parent: parent.to_path_buf(),
            target,
            expected_owner_id,
            parent_identity: DirectoryIdentity::from_metadata(&metadata),
        })
    }

    pub const fn component(&self) -> ProgramComponent {
        self.component
    }

    pub fn target_path(&self) -> &Path {
        &self.target
    }

    fn revalidate(&self) -> Result<(), ProgramSwitchError> {
        let canonical = fs::canonicalize(&self.parent)
            .map_err(|_| error(ProgramSwitchErrorCode::UnsafeTargetParent))?;
        if canonical != self.parent {
            return Err(error(ProgramSwitchErrorCode::UnsafeTargetParent));
        }
        let metadata = fs::symlink_metadata(&self.parent)
            .map_err(|_| error(ProgramSwitchErrorCode::UnsafeTargetParent))?;
        verify_target_parent(&metadata, self.expected_owner_id)?;
        if DirectoryIdentity::from_metadata(&metadata) != self.parent_identity {
            return Err(error(ProgramSwitchErrorCode::ArtifactIdentityChanged));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProgramSwitchStore {
    target: VerifiedProgramTarget,
    operation_id: String,
    transaction_directory: PathBuf,
    transaction_identity: DirectoryIdentity,
    staged_bundle: PathBuf,
    source_backup_bundle: PathBuf,
}

impl ProgramSwitchStore {
    pub fn open(
        target: VerifiedProgramTarget,
        operation_id: &str,
    ) -> Result<Self, ProgramSwitchError> {
        validate_operation_id(operation_id, "operation_id")
            .map_err(|_| error(ProgramSwitchErrorCode::InvalidOperationState))?;
        target.revalidate()?;
        let transaction_directory = target
            .parent
            .join(format!(".radishlex-install-{operation_id}"));
        match DirBuilder::new().mode(0o700).create(&transaction_directory) {
            Ok(()) => {
                fs::set_permissions(&transaction_directory, fs::Permissions::from_mode(0o700))
                    .map_err(|_| error(ProgramSwitchErrorCode::Io))?;
                sync_directory(&target.parent)?;
            }
            Err(io_error) if io_error.kind() == ErrorKind::AlreadyExists => {}
            Err(_) => return Err(error(ProgramSwitchErrorCode::Io)),
        }
        let metadata = fs::symlink_metadata(&transaction_directory)
            .map_err(|_| error(ProgramSwitchErrorCode::UnsafeTransactionDirectory))?;
        verify_private_transaction_directory(
            &metadata,
            target.expected_owner_id,
            target.parent_identity.device_id,
        )?;
        let store = Self {
            staged_bundle: transaction_directory.join(STAGED_BUNDLE_NAME),
            source_backup_bundle: transaction_directory.join(SOURCE_BACKUP_BUNDLE_NAME),
            target,
            operation_id: operation_id.to_owned(),
            transaction_directory,
            transaction_identity: DirectoryIdentity::from_metadata(&metadata),
        };
        store.revalidate()?;
        Ok(store)
    }

    pub const fn component(&self) -> ProgramComponent {
        self.target.component
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn target_path(&self) -> &Path {
        &self.target.target
    }

    pub fn staged_bundle_path(&self) -> &Path {
        &self.staged_bundle
    }

    pub fn source_backup_bundle_path(&self) -> &Path {
        &self.source_backup_bundle
    }

    fn revalidate(&self) -> Result<(), ProgramSwitchError> {
        self.target.revalidate()?;
        let metadata = fs::symlink_metadata(&self.transaction_directory)
            .map_err(|_| error(ProgramSwitchErrorCode::UnsafeTransactionDirectory))?;
        verify_private_transaction_directory(
            &metadata,
            self.target.expected_owner_id,
            self.target.parent_identity.device_id,
        )?;
        if DirectoryIdentity::from_metadata(&metadata) != self.transaction_identity {
            return Err(error(ProgramSwitchErrorCode::ArtifactIdentityChanged));
        }
        for entry in fs::read_dir(&self.transaction_directory)
            .map_err(|_| error(ProgramSwitchErrorCode::Io))?
        {
            let name = entry
                .map_err(|_| error(ProgramSwitchErrorCode::Io))?
                .file_name();
            if name != STAGED_BUNDLE_NAME && name != SOURCE_BACKUP_BUNDLE_NAME {
                return Err(error(ProgramSwitchErrorCode::UnexpectedTransactionObject));
            }
        }
        Ok(())
    }

    fn verify_binding(
        &self,
        receipt_store: &InstallReceiptStore,
        guard: &InstallProcessGuard,
        receipt: &InstallReceipt,
    ) -> Result<(), ProgramSwitchError> {
        receipt_store
            .verify_guard(guard)
            .map_err(|_| error(ProgramSwitchErrorCode::ArtifactIdentityChanged))?;
        self.revalidate()?;
        if receipt.operation_id() != self.operation_id
            || receipt.root_identity().owner_id() != self.target.expected_owner_id
        {
            return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
        }
        Ok(())
    }

    fn inspect_required(
        &self,
        path: &Path,
    ) -> Result<ProgramFilesystemIdentity, ProgramSwitchError> {
        inspect_optional_bundle(
            path,
            self.target.expected_owner_id,
            self.target.parent_identity.device_id,
        )?
        .ok_or_else(|| error(ProgramSwitchErrorCode::ArtifactMissing))
    }

    fn inspect_optional(
        &self,
        path: &Path,
    ) -> Result<Option<ProgramFilesystemIdentity>, ProgramSwitchError> {
        inspect_optional_bundle(
            path,
            self.target.expected_owner_id,
            self.target.parent_identity.device_id,
        )
    }
}

pub fn record_program_source(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    program_store.verify_binding(receipt_store, guard, receipt)?;
    if receipt.state() != InstallState::Quiesced
        || receipt.operation_kind() == InstallOperationKind::FirstInstall
    {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    let slot = source_slot(program_store.component());
    let expected = receipt
        .source_product()
        .ok_or_else(|| error(ProgramSwitchErrorCode::InvalidOperationState))?
        .program(program_store.component())
        .clone();
    let filesystem_identity = program_store.inspect_required(program_store.target_path())?;
    record_or_verify_evidence(
        receipt,
        InstallArtifactEvidence::new(slot, expected, filesystem_identity)
            .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?,
    )?;
    persist(receipt_store, guard, receipt)
}

pub fn record_staged_program(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    program_store.verify_binding(receipt_store, guard, receipt)?;
    if receipt.state() != InstallState::Quiesced
        || receipt.operation_kind() == InstallOperationKind::RemovePrograms
    {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    let slot = staged_slot(program_store.component());
    let expected = receipt
        .target_product()
        .ok_or_else(|| error(ProgramSwitchErrorCode::InvalidOperationState))?
        .program(program_store.component())
        .clone();
    let filesystem_identity = program_store.inspect_required(program_store.staged_bundle_path())?;
    record_or_verify_evidence(
        receipt,
        InstallArtifactEvidence::new(slot, expected, filesystem_identity)
            .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?,
    )?;
    persist(receipt_store, guard, receipt)
}

pub fn finish_target_staging(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    verify_store_pair(receipt_store, guard, manager, input_method, receipt)?;
    if receipt.state() == InstallState::TargetStaged {
        return persist(receipt_store, guard, receipt);
    }
    if receipt.state() != InstallState::Quiesced
        || receipt.operation_kind() == InstallOperationKind::RemovePrograms
    {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    for store in [manager, input_method] {
        verify_evidence_at_path(
            receipt,
            staged_slot(store.component()),
            store,
            &store.staged_bundle,
        )?;
        match receipt.operation_kind() {
            InstallOperationKind::FirstInstall => {
                verify_absent(store, store.target_path())?;
            }
            InstallOperationKind::Upgrade | InstallOperationKind::Repair => {
                verify_evidence_at_path(
                    receipt,
                    source_slot(store.component()),
                    store,
                    store.target_path(),
                )?;
            }
            InstallOperationKind::RemovePrograms => unreachable!(),
        }
    }
    receipt
        .advance(InstallState::TargetStaged)
        .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?;
    persist(receipt_store, guard, receipt)
}

pub fn preserve_program_source(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    preserve_program_source_with_faults(
        receipt_store,
        guard,
        program_store,
        receipt,
        &mut NoProgramSwitchFaults,
    )
}

pub fn preserve_program_source_with_faults(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
    faults: &mut dyn ProgramSwitchFaultInjector,
) -> Result<(), ProgramSwitchError> {
    program_store.verify_binding(receipt_store, guard, receipt)?;
    let allowed_state = match receipt.operation_kind() {
        InstallOperationKind::Upgrade | InstallOperationKind::Repair => {
            receipt.state() == InstallState::TargetStaged
        }
        InstallOperationKind::RemovePrograms => receipt.state() == InstallState::Quiesced,
        InstallOperationKind::FirstInstall => false,
    };
    if !allowed_state {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    let source = evidence(receipt, source_slot(program_store.component()))?.clone();
    ensure_moved(
        program_store,
        program_store.target_path(),
        program_store.source_backup_bundle_path(),
        source.filesystem_identity(),
        ProgramSwitchAction::PreserveSource,
        RenameDirectories {
            target: &program_store.transaction_directory,
            source: &program_store.target.parent,
        },
        faults,
    )?;
    let backup = InstallArtifactEvidence::new(
        backup_slot(program_store.component()),
        source.identity().clone(),
        source.filesystem_identity().clone(),
    )
    .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?;
    record_or_verify_evidence(receipt, backup)?;
    persist(receipt_store, guard, receipt)
}

pub fn finish_source_preservation(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    verify_store_pair(receipt_store, guard, manager, input_method, receipt)?;
    if receipt.state() == InstallState::SourcePreserved {
        return persist(receipt_store, guard, receipt);
    }
    let expected_state = if receipt.operation_kind() == InstallOperationKind::RemovePrograms {
        InstallState::Quiesced
    } else {
        InstallState::TargetStaged
    };
    if receipt.state() != expected_state
        || receipt.operation_kind() == InstallOperationKind::FirstInstall
    {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    for store in [manager, input_method] {
        verify_absent(store, store.target_path())?;
        verify_evidence_at_path(
            receipt,
            backup_slot(store.component()),
            store,
            store.source_backup_bundle_path(),
        )?;
    }
    receipt
        .advance(InstallState::SourcePreserved)
        .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?;
    persist(receipt_store, guard, receipt)
}

pub fn commit_program_target(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    commit_program_target_with_faults(
        receipt_store,
        guard,
        program_store,
        receipt,
        &mut NoProgramSwitchFaults,
    )
}

pub fn commit_program_target_with_faults(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
    faults: &mut dyn ProgramSwitchFaultInjector,
) -> Result<(), ProgramSwitchError> {
    program_store.verify_binding(receipt_store, guard, receipt)?;
    let (expected_state, committed_state) = commit_states(program_store.component());
    if receipt.state() == committed_state {
        verify_evidence_at_path(
            receipt,
            installed_slot(program_store.component()),
            program_store,
            program_store.target_path(),
        )?;
        return persist(receipt_store, guard, receipt);
    }
    let first_install_manager = receipt.operation_kind() == InstallOperationKind::FirstInstall
        && program_store.component() == ProgramComponent::Manager
        && receipt.state() == InstallState::TargetStaged;
    if receipt.operation_kind() == InstallOperationKind::RemovePrograms
        || (receipt.state() != expected_state && !first_install_manager)
    {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    let staged = evidence(receipt, staged_slot(program_store.component()))?.clone();
    ensure_moved(
        program_store,
        program_store.staged_bundle_path(),
        program_store.target_path(),
        staged.filesystem_identity(),
        ProgramSwitchAction::CommitTarget,
        RenameDirectories {
            target: &program_store.target.parent,
            source: &program_store.transaction_directory,
        },
        faults,
    )?;
    let installed = InstallArtifactEvidence::new(
        installed_slot(program_store.component()),
        staged.identity().clone(),
        staged.filesystem_identity().clone(),
    )
    .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?;
    record_or_verify_evidence(receipt, installed)?;
    persist(receipt_store, guard, receipt)?;
    receipt
        .advance(committed_state)
        .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?;
    persist(receipt_store, guard, receipt)
}

pub fn commit_program_removal(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    program_store.verify_binding(receipt_store, guard, receipt)?;
    let (expected_state, committed_state) = commit_states(program_store.component());
    if receipt.state() == committed_state {
        verify_absent(program_store, program_store.target_path())?;
        return persist(receipt_store, guard, receipt);
    }
    if receipt.operation_kind() != InstallOperationKind::RemovePrograms
        || receipt.state() != expected_state
    {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    verify_absent(program_store, program_store.target_path())?;
    verify_evidence_at_path(
        receipt,
        backup_slot(program_store.component()),
        program_store,
        program_store.source_backup_bundle_path(),
    )?;
    receipt
        .advance(committed_state)
        .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?;
    persist(receipt_store, guard, receipt)
}

pub fn restore_program_source(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    restore_program_source_with_faults(
        receipt_store,
        guard,
        program_store,
        receipt,
        &mut NoProgramSwitchFaults,
    )
}

pub fn restore_program_source_with_faults(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    program_store: &ProgramSwitchStore,
    receipt: &InstallReceipt,
    faults: &mut dyn ProgramSwitchFaultInjector,
) -> Result<(), ProgramSwitchError> {
    program_store.verify_binding(receipt_store, guard, receipt)?;
    if !matches!(
        receipt.state(),
        InstallState::RollbackRequired | InstallState::ProgramsRestored
    ) {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    if let Some(installed) = receipt.artifact(installed_slot(program_store.component())) {
        let already_returned = program_store
            .inspect_optional(program_store.staged_bundle_path())?
            .as_ref()
            == Some(installed.filesystem_identity());
        if !already_returned {
            ensure_moved(
                program_store,
                program_store.target_path(),
                program_store.staged_bundle_path(),
                installed.filesystem_identity(),
                ProgramSwitchAction::ReturnTargetToStage,
                RenameDirectories {
                    target: &program_store.transaction_directory,
                    source: &program_store.target.parent,
                },
                faults,
            )?;
        }
    } else if let Some(staged) = receipt.artifact(staged_slot(program_store.component())) {
        verify_identity_at_path(
            program_store,
            program_store.staged_bundle_path(),
            staged.filesystem_identity(),
        )?;
        verify_absent(program_store, program_store.target_path())?;
    }

    if let Some(source) = receipt.artifact(source_slot(program_store.component())) {
        ensure_moved(
            program_store,
            program_store.source_backup_bundle_path(),
            program_store.target_path(),
            source.filesystem_identity(),
            ProgramSwitchAction::RestoreSource,
            RenameDirectories {
                target: &program_store.target.parent,
                source: &program_store.transaction_directory,
            },
            faults,
        )?;
    } else {
        verify_absent(program_store, program_store.target_path())?;
    }
    Ok(())
}

pub fn finish_program_restore(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    receipt: &mut InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    verify_store_pair(receipt_store, guard, manager, input_method, receipt)?;
    if receipt.state() == InstallState::ProgramsRestored {
        return persist(receipt_store, guard, receipt);
    }
    if receipt.state() != InstallState::RollbackRequired {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    for store in [manager, input_method] {
        if let Some(source) = receipt.artifact(source_slot(store.component())) {
            verify_identity_at_path(store, store.target_path(), source.filesystem_identity())?;
            verify_absent(store, store.source_backup_bundle_path())?;
        } else {
            verify_absent(store, store.target_path())?;
        }
        if let Some(staged) = receipt.artifact(staged_slot(store.component())) {
            verify_identity_at_path(
                store,
                store.staged_bundle_path(),
                staged.filesystem_identity(),
            )?;
        }
    }
    receipt
        .mark_programs_restored()
        .map_err(|_| error(ProgramSwitchErrorCode::Receipt))?;
    persist(receipt_store, guard, receipt)
}

fn verify_store_pair(
    receipt_store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    manager: &ProgramSwitchStore,
    input_method: &ProgramSwitchStore,
    receipt: &InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    if manager.component() != ProgramComponent::Manager
        || input_method.component() != ProgramComponent::InputMethod
    {
        return Err(error(ProgramSwitchErrorCode::InvalidOperationState));
    }
    manager.verify_binding(receipt_store, guard, receipt)?;
    input_method.verify_binding(receipt_store, guard, receipt)
}

fn record_or_verify_evidence(
    receipt: &mut InstallReceipt,
    evidence: InstallArtifactEvidence,
) -> Result<(), ProgramSwitchError> {
    if let Some(existing) = receipt.artifact(evidence.slot()) {
        if existing == &evidence {
            return Ok(());
        }
        return Err(error(ProgramSwitchErrorCode::ArtifactIdentityChanged));
    }
    receipt
        .record_artifact(evidence)
        .map_err(|_| error(ProgramSwitchErrorCode::Receipt))
}

fn evidence(
    receipt: &InstallReceipt,
    slot: InstallArtifactSlot,
) -> Result<&InstallArtifactEvidence, ProgramSwitchError> {
    receipt
        .artifact(slot)
        .ok_or_else(|| error(ProgramSwitchErrorCode::InvalidOperationState))
}

fn verify_evidence_at_path(
    receipt: &InstallReceipt,
    slot: InstallArtifactSlot,
    store: &ProgramSwitchStore,
    path: &Path,
) -> Result<(), ProgramSwitchError> {
    let expected = evidence(receipt, slot)?;
    verify_identity_at_path(store, path, expected.filesystem_identity())
}

fn verify_identity_at_path(
    store: &ProgramSwitchStore,
    path: &Path,
    expected: &ProgramFilesystemIdentity,
) -> Result<(), ProgramSwitchError> {
    if store.inspect_required(path)? != *expected {
        return Err(error(ProgramSwitchErrorCode::ArtifactIdentityChanged));
    }
    Ok(())
}

fn verify_absent(store: &ProgramSwitchStore, path: &Path) -> Result<(), ProgramSwitchError> {
    if store.inspect_optional(path)?.is_some() {
        return Err(error(ProgramSwitchErrorCode::ArtifactIdentityChanged));
    }
    Ok(())
}

fn ensure_moved(
    store: &ProgramSwitchStore,
    source: &Path,
    target: &Path,
    expected: &ProgramFilesystemIdentity,
    action: ProgramSwitchAction,
    directories: RenameDirectories<'_>,
    faults: &mut dyn ProgramSwitchFaultInjector,
) -> Result<(), ProgramSwitchError> {
    let source_identity = store.inspect_optional(source)?;
    let target_identity = store.inspect_optional(target)?;
    match (source_identity.as_ref(), target_identity.as_ref()) {
        (Some(actual), None) if actual == expected => {
            rename_durable(
                source,
                target,
                action,
                directories.target,
                directories.source,
                faults,
            )?;
            verify_identity_at_path(store, target, expected)
        }
        (None, Some(actual)) if actual == expected => Ok(()),
        _ => Err(error(ProgramSwitchErrorCode::ArtifactIdentityChanged)),
    }
}

#[derive(Clone, Copy)]
struct RenameDirectories<'a> {
    target: &'a Path,
    source: &'a Path,
}

fn rename_durable(
    source: &Path,
    target: &Path,
    action: ProgramSwitchAction,
    target_directory: &Path,
    source_directory: &Path,
    faults: &mut dyn ProgramSwitchFaultInjector,
) -> Result<(), ProgramSwitchError> {
    check_fault(faults, action, ProgramSwitchBoundary::BeforeRename)?;
    fs::rename(source, target).map_err(|_| error(ProgramSwitchErrorCode::Io))?;
    check_fault(faults, action, ProgramSwitchBoundary::AfterRename)?;
    sync_directory(target_directory)?;
    check_fault(
        faults,
        action,
        ProgramSwitchBoundary::AfterTargetDirectorySync,
    )?;
    sync_directory(source_directory)?;
    check_fault(
        faults,
        action,
        ProgramSwitchBoundary::AfterSourceDirectorySync,
    )
}

fn check_fault(
    faults: &mut dyn ProgramSwitchFaultInjector,
    action: ProgramSwitchAction,
    boundary: ProgramSwitchBoundary,
) -> Result<(), ProgramSwitchError> {
    if faults.should_fail(ProgramSwitchFaultPoint::new(action, boundary)) {
        Err(error(ProgramSwitchErrorCode::FaultInjected))
    } else {
        Ok(())
    }
}

fn inspect_optional_bundle(
    path: &Path,
    expected_owner_id: u32,
    expected_device_id: u64,
) -> Result<Option<ProgramFilesystemIdentity>, ProgramSwitchError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(io_error) if io_error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(error(ProgramSwitchErrorCode::Io)),
    };
    if !metadata.file_type().is_dir()
        || metadata.uid() != expected_owner_id
        || !matches!(metadata.permissions().mode() & 0o7777, 0o700 | 0o755)
        || metadata.ino() == 0
    {
        return Err(error(ProgramSwitchErrorCode::ArtifactIdentityChanged));
    }
    if metadata.dev() != expected_device_id {
        return Err(error(ProgramSwitchErrorCode::CrossDevice));
    }
    ProgramFilesystemIdentity::new(
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.permissions().mode() & 0o7777,
    )
    .map(Some)
    .map_err(|_| error(ProgramSwitchErrorCode::ArtifactIdentityChanged))
}

fn verify_target_parent(
    metadata: &Metadata,
    expected_owner_id: u32,
) -> Result<(), ProgramSwitchError> {
    if !metadata.file_type().is_dir()
        || metadata.uid() != expected_owner_id
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.ino() == 0
    {
        return Err(error(ProgramSwitchErrorCode::UnsafeTargetParent));
    }
    Ok(())
}

fn verify_private_transaction_directory(
    metadata: &Metadata,
    expected_owner_id: u32,
    expected_device_id: u64,
) -> Result<(), ProgramSwitchError> {
    if !metadata.file_type().is_dir()
        || metadata.uid() != expected_owner_id
        || metadata.permissions().mode() & 0o7777 != 0o700
        || metadata.dev() != expected_device_id
        || metadata.ino() == 0
    {
        return Err(error(ProgramSwitchErrorCode::UnsafeTransactionDirectory));
    }
    Ok(())
}

fn persist(
    store: &InstallReceiptStore,
    guard: &InstallProcessGuard,
    receipt: &InstallReceipt,
) -> Result<(), ProgramSwitchError> {
    store
        .persist(guard, receipt)
        .map_err(|_| error(ProgramSwitchErrorCode::Receipt))
}

fn source_slot(component: ProgramComponent) -> InstallArtifactSlot {
    match component {
        ProgramComponent::Manager => InstallArtifactSlot::SourceManager,
        ProgramComponent::InputMethod => InstallArtifactSlot::SourceInputMethod,
    }
}

fn staged_slot(component: ProgramComponent) -> InstallArtifactSlot {
    match component {
        ProgramComponent::Manager => InstallArtifactSlot::StagedManager,
        ProgramComponent::InputMethod => InstallArtifactSlot::StagedInputMethod,
    }
}

fn backup_slot(component: ProgramComponent) -> InstallArtifactSlot {
    match component {
        ProgramComponent::Manager => InstallArtifactSlot::BackupManager,
        ProgramComponent::InputMethod => InstallArtifactSlot::BackupInputMethod,
    }
}

fn installed_slot(component: ProgramComponent) -> InstallArtifactSlot {
    match component {
        ProgramComponent::Manager => InstallArtifactSlot::InstalledManager,
        ProgramComponent::InputMethod => InstallArtifactSlot::InstalledInputMethod,
    }
}

fn commit_states(component: ProgramComponent) -> (InstallState, InstallState) {
    match component {
        ProgramComponent::Manager => (
            InstallState::SourcePreserved,
            InstallState::ManagerCommitted,
        ),
        ProgramComponent::InputMethod => (
            InstallState::ManagerCommitted,
            InstallState::ProgramsCommitted,
        ),
    }
}

fn bundle_name(component: ProgramComponent) -> &'static str {
    match component {
        ProgramComponent::Manager => MANAGER_BUNDLE_NAME,
        ProgramComponent::InputMethod => INPUT_METHOD_BUNDLE_NAME,
    }
}

fn sync_directory(path: &Path) -> Result<(), ProgramSwitchError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| error(ProgramSwitchErrorCode::Io))
}

const fn error(code: ProgramSwitchErrorCode) -> ProgramSwitchError {
    ProgramSwitchError::new(code)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectoryIdentity {
    device_id: u64,
    inode: u64,
}

impl DirectoryIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device_id: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

#[cfg(test)]
mod tests;
