use std::collections::BTreeMap;

use crate::coordinator::{
    DpkgExpectedProductState, DpkgOperationContext, DpkgPortError, DpkgPortErrorCode,
    DpkgPortPhase, DpkgProductValidationPhase, DpkgQuiescencePhase, DpkgRestoreRequest,
    DpkgStagedOperation, DpkgStagedPackage, DpkgTransactionPort,
};
use crate::debian::{
    validate_operation_relation, DebianCommandInvocation, DebianRelationshipError,
    DebianRelationshipErrorCode, DpkgCurrentState, DpkgStatusSnapshot, PrivateStagedDeb,
    StagedDebSlot, VerifiedArtifactRelationship, DPKG_ARCHITECTURE,
};
use crate::model::LinuxInstallState;
use crate::model::{ArtifactSlot, DpkgPackageState, LinuxArtifactIdentity, PackageSnapshot};

use super::executor::{
    DebianCommandExecutor, DebianExecutionError, DebianExecutionErrorCode,
    LinuxSystemCommandExecutor,
};
use super::observer::{
    DpkgSystemObserver, LinuxSystemObservationError, LinuxSystemObservationErrorCode,
    LinuxSystemObserver,
};

pub struct LinuxDpkgTransactionPort<O, E> {
    observer: O,
    executor: E,
    prepared_relationships: Vec<VerifiedArtifactRelationship>,
    verified_relationships: BTreeMap<String, VerifiedArtifactRelationship>,
}

impl LinuxDpkgTransactionPort<LinuxSystemObserver, LinuxSystemCommandExecutor> {
    pub fn system(prepared_relationships: Vec<VerifiedArtifactRelationship>) -> Self {
        Self::with_parts(
            LinuxSystemObserver,
            LinuxSystemCommandExecutor,
            prepared_relationships,
        )
    }
}

impl<O, E> LinuxDpkgTransactionPort<O, E> {
    pub fn with_parts(
        observer: O,
        executor: E,
        prepared_relationships: Vec<VerifiedArtifactRelationship>,
    ) -> Self {
        Self {
            observer,
            executor,
            prepared_relationships,
            verified_relationships: BTreeMap::new(),
        }
    }

    pub fn observer(&self) -> &O {
        &self.observer
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }
}

impl<O, E> DpkgTransactionPort for LinuxDpkgTransactionPort<O, E>
where
    O: DpkgSystemObserver,
    E: DebianCommandExecutor,
{
    type QuiescencePermit = O::QuiescencePermit;

    fn validate_operation(
        &mut self,
        context: DpkgOperationContext<'_>,
    ) -> Result<(), DpkgPortError> {
        self.observer
            .validate_environment()
            .map_err(|error| observation_port_error(error, DpkgPortPhase::OperationValidation))?;
        let output = self
            .executor
            .execute(&DebianCommandInvocation::print_architecture())
            .map_err(|error| execution_port_error(error, DpkgPortPhase::OperationValidation))?;
        if !output.succeeded()
            || output.stdout() != format!("{DPKG_ARCHITECTURE}\n").as_bytes()
            || !output.stderr().is_empty()
        {
            return Err(port_error(
                DpkgPortErrorCode::EnvironmentUnsupported,
                DpkgPortPhase::OperationValidation,
                "dpkg architecture preflight differs from arm64",
            ));
        }
        let source = context
            .source_artifact()
            .map(|artifact| self.prepared_relationship(artifact).cloned())
            .transpose()?;
        let target = context
            .target_artifact()
            .map(|artifact| self.prepared_relationship(artifact).cloned())
            .transpose()?;
        let relation =
            validate_operation_relation(context.operation_kind(), source.as_ref(), target.as_ref())
                .map_err(|error| {
                    relationship_port_error(error, DpkgPortPhase::OperationValidation)
                })?;
        if relation != context.version_relation() {
            return Err(port_error(
                DpkgPortErrorCode::VersionRelationInvalid,
                DpkgPortPhase::OperationValidation,
                "operation version relation differs from verified artifacts",
            ));
        }
        let status = self
            .observer
            .inspect_status()
            .map_err(|error| observation_port_error(error, DpkgPortPhase::PackageInspection))?;
        validate_relationship_dependencies(
            &status,
            source.as_ref(),
            target.as_ref(),
            DpkgPortPhase::OperationValidation,
        )
    }

    fn inspect_package(
        &mut self,
        context: DpkgOperationContext<'_>,
    ) -> Result<PackageSnapshot, DpkgPortError> {
        let status = self
            .observer
            .inspect_status()
            .map_err(|error| observation_port_error(error, DpkgPortPhase::PackageInspection))?;
        project_package_snapshot(&status, context)
    }

    fn validate_staged_operation(
        &mut self,
        context: DpkgOperationContext<'_>,
        staged: DpkgStagedOperation<'_>,
    ) -> Result<(), DpkgPortError> {
        let source = staged
            .source()
            .map(|package| self.verify_staged(package))
            .transpose()?;
        let target = staged
            .target()
            .map(|package| self.verify_staged(package))
            .transpose()?;
        let relation =
            validate_operation_relation(context.operation_kind(), source.as_ref(), target.as_ref())
                .map_err(|error| {
                    relationship_port_error(error, DpkgPortPhase::StagedOperationValidation)
                })?;
        if relation != context.version_relation() {
            return Err(port_error(
                DpkgPortErrorCode::VersionRelationInvalid,
                DpkgPortPhase::StagedOperationValidation,
                "staged artifact relation differs from the receipt",
            ));
        }
        let status = self.observer.inspect_status().map_err(|error| {
            observation_port_error(error, DpkgPortPhase::StagedOperationValidation)
        })?;
        validate_relationship_dependencies(
            &status,
            source.as_ref(),
            target.as_ref(),
            DpkgPortPhase::StagedOperationValidation,
        )?;
        for relationship in source.into_iter().chain(target) {
            self.verified_relationships
                .insert(relationship.package_sha256().to_owned(), relationship);
        }
        Ok(())
    }

    fn prove_quiescent(
        &mut self,
        _context: DpkgOperationContext<'_>,
        _phase: DpkgQuiescencePhase,
    ) -> Result<Self::QuiescencePermit, DpkgPortError> {
        self.observer
            .prove_quiescent()
            .map_err(|error| observation_port_error(error, DpkgPortPhase::Quiescence))
    }

    fn apply_package(
        &mut self,
        context: DpkgOperationContext<'_>,
        package: DpkgStagedPackage<'_>,
        _permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError> {
        require_receipt_state(
            context,
            LinuxInstallState::PackageMutating,
            DpkgPortPhase::TargetMutation,
        )?;
        require_fixed_staged_package(context, package, StagedDebSlot::Target)?;
        let staged = PrivateStagedDeb::new(context.operation_id(), StagedDebSlot::Target)
            .map_err(|_| command_contract_error(DpkgPortPhase::TargetMutation))?;
        let invocation = DebianCommandInvocation::apply_target(&staged)
            .map_err(|_| command_contract_error(DpkgPortPhase::TargetMutation))?;
        self.execute_mutation(&invocation, DpkgPortPhase::TargetMutation)
    }

    fn remove_package(
        &mut self,
        context: DpkgOperationContext<'_>,
        _permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError> {
        require_receipt_state(
            context,
            LinuxInstallState::PackageMutating,
            DpkgPortPhase::TargetMutation,
        )?;
        self.execute_mutation(
            &DebianCommandInvocation::remove_target(),
            DpkgPortPhase::TargetMutation,
        )
    }

    fn validate_product(
        &mut self,
        context: DpkgOperationContext<'_>,
        phase: DpkgProductValidationPhase,
        expected: DpkgExpectedProductState<'_>,
        snapshot: &PackageSnapshot,
    ) -> Result<(), DpkgPortError> {
        let port_phase = match phase {
            DpkgProductValidationPhase::Target => DpkgPortPhase::TargetValidation,
            DpkgProductValidationPhase::Source => DpkgPortPhase::SourceValidation,
        };
        require_validation_state(context, phase, port_phase)?;
        if !expected.matches_snapshot(snapshot) {
            return Err(port_error(
                DpkgPortErrorCode::ProductValidationFailed,
                port_phase,
                "package snapshot differs from the expected product state",
            ));
        }
        let status = self
            .observer
            .inspect_status()
            .map_err(|error| observation_port_error(error, port_phase))?;
        match expected {
            DpkgExpectedProductState::Installed(artifact) => {
                let relationship = self.relationship_for_context(context, artifact)?;
                self.observer
                    .validate_installed_product(artifact, &relationship, &status)
                    .map_err(|error| observation_port_error(error, port_phase))
            }
            DpkgExpectedProductState::Absent => {
                let relationship = context
                    .source_artifact()
                    .or_else(|| context.target_artifact())
                    .map(|artifact| self.relationship_for_context(context, artifact))
                    .transpose()?;
                self.observer
                    .validate_absent_product(relationship.as_ref(), &status)
                    .map_err(|error| observation_port_error(error, port_phase))
            }
        }
    }

    fn restore_source(
        &mut self,
        context: DpkgOperationContext<'_>,
        request: DpkgRestoreRequest<'_>,
        _permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError> {
        if let Some(source) = request.desired_source() {
            let slot = staged_slot_for_context(context, source)?;
            require_fixed_staged_package(context, source, slot)?;
            let staged = PrivateStagedDeb::new(context.operation_id(), slot)
                .map_err(|_| command_contract_error(DpkgPortPhase::SourceRestore))?;
            return self.execute_mutation(
                &DebianCommandInvocation::restore_staged(&staged),
                DpkgPortPhase::SourceRestore,
            );
        }

        let recovery = request.recovery_target().ok_or_else(|| {
            port_error(
                DpkgPortErrorCode::ArtifactInvalid,
                DpkgPortPhase::SourceRestore,
                "first-install recovery has no staged target",
            )
        })?;
        require_fixed_staged_package(context, recovery, StagedDebSlot::Target)?;
        let snapshot = self.inspect_package(context)?;
        if snapshot.state().is_absent() {
            return Ok(());
        }
        if snapshot.artifact() != Some(recovery.artifact()) {
            return Err(port_error(
                DpkgPortErrorCode::PackageStateUnexpected,
                DpkgPortPhase::SourceRestore,
                "first-install recovery package identity is unexpected",
            ));
        }
        if snapshot.state() != DpkgPackageState::Installed {
            let staged = PrivateStagedDeb::new(context.operation_id(), StagedDebSlot::Target)
                .map_err(|_| command_contract_error(DpkgPortPhase::SourceRestore))?;
            self.execute_mutation(
                &DebianCommandInvocation::restore_staged(&staged),
                DpkgPortPhase::SourceRestore,
            )?;
            let installed = self.inspect_package(context)?;
            if !installed.matches_installed(recovery.artifact()) {
                return Err(port_error(
                    DpkgPortErrorCode::ProductValidationFailed,
                    DpkgPortPhase::SourceRestore,
                    "recovery target did not become fully installed before removal",
                ));
            }
        }
        self.execute_mutation(
            &DebianCommandInvocation::remove_target(),
            DpkgPortPhase::SourceRestore,
        )
    }
}

impl<O, E> LinuxDpkgTransactionPort<O, E>
where
    O: DpkgSystemObserver,
    E: DebianCommandExecutor,
{
    fn prepared_relationship(
        &self,
        artifact: &LinuxArtifactIdentity,
    ) -> Result<&VerifiedArtifactRelationship, DpkgPortError> {
        self.prepared_relationships
            .iter()
            .find(|relationship| {
                relationship
                    .matches_linux_artifact_identity(artifact)
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                port_error(
                    DpkgPortErrorCode::ArtifactInvalid,
                    DpkgPortPhase::OperationValidation,
                    "operation artifact lacks a verified actual-package relationship",
                )
            })
    }

    fn verify_staged(
        &mut self,
        package: DpkgStagedPackage<'_>,
    ) -> Result<VerifiedArtifactRelationship, DpkgPortError> {
        self.observer
            .verify_staged_package(package)
            .map_err(|error| {
                observation_port_error(error, DpkgPortPhase::StagedOperationValidation)
            })
    }

    fn relationship_for_context(
        &mut self,
        context: DpkgOperationContext<'_>,
        artifact: &LinuxArtifactIdentity,
    ) -> Result<VerifiedArtifactRelationship, DpkgPortError> {
        if let Some(relationship) = self.verified_relationships.get(artifact.package_sha256()) {
            return Ok(relationship.clone());
        }
        if let Some(relationship) = self.prepared_relationships.iter().find(|relationship| {
            relationship
                .matches_linux_artifact_identity(artifact)
                .unwrap_or(false)
        }) {
            return Ok(relationship.clone());
        }
        let receipt = context.receipt().ok_or_else(|| {
            port_error(
                DpkgPortErrorCode::ArtifactInvalid,
                DpkgPortPhase::StagedOperationValidation,
                "resumed relationship requires a persisted receipt",
            )
        })?;
        let slot = [ArtifactSlot::Source, ArtifactSlot::Target]
            .into_iter()
            .find(|slot| {
                receipt
                    .staged_artifact(*slot)
                    .is_some_and(|evidence| evidence.artifact() == artifact)
            })
            .ok_or_else(|| {
                port_error(
                    DpkgPortErrorCode::ArtifactInvalid,
                    DpkgPortPhase::StagedOperationValidation,
                    "receipt has no staged proof for the product artifact",
                )
            })?;
        let staged_slot = match slot {
            ArtifactSlot::Source => StagedDebSlot::Source,
            ArtifactSlot::Target => StagedDebSlot::Target,
        };
        let package_path = PrivateStagedDeb::new(context.operation_id(), staged_slot)
            .map_err(|_| command_contract_error(DpkgPortPhase::StagedOperationValidation))?
            .path()
            .to_owned();
        let evidence_path = package_path.with_file_name(match slot {
            ArtifactSlot::Source => "source.evidence.json",
            ArtifactSlot::Target => "target.evidence.json",
        });
        let package = DpkgStagedPackage::from_paths(artifact, &package_path, &evidence_path);
        let relationship = self.verify_staged(package)?;
        self.verified_relationships.insert(
            relationship.package_sha256().to_owned(),
            relationship.clone(),
        );
        Ok(relationship)
    }

    fn execute_mutation(
        &mut self,
        invocation: &DebianCommandInvocation,
        phase: DpkgPortPhase,
    ) -> Result<(), DpkgPortError> {
        let output = self
            .executor
            .execute(invocation)
            .map_err(|error| execution_port_error(error, phase))?;
        if !output.succeeded() {
            return Err(port_error(
                DpkgPortErrorCode::MutationFailed,
                phase,
                "fixed dpkg mutation did not exit successfully",
            ));
        }
        Ok(())
    }
}

fn project_package_snapshot(
    status: &DpkgStatusSnapshot,
    context: DpkgOperationContext<'_>,
) -> Result<PackageSnapshot, DpkgPortError> {
    let product_records = status
        .records()
        .iter()
        .filter(|record| record.package() == "radishlex")
        .collect::<Vec<_>>();
    if product_records.len() > 1 {
        return Err(port_error(
            DpkgPortErrorCode::PackageStateUnknown,
            DpkgPortPhase::PackageInspection,
            "dpkg status contains ambiguous RadishLex package records",
        ));
    }
    let Some(record) = product_records.first() else {
        return Ok(PackageSnapshot::absent());
    };
    let state = match record.current() {
        DpkgCurrentState::NotInstalled => DpkgPackageState::NotInstalled,
        DpkgCurrentState::ConfigFiles => DpkgPackageState::ConfigFiles,
        DpkgCurrentState::Installed => DpkgPackageState::Installed,
        DpkgCurrentState::Unpacked => DpkgPackageState::Unpacked,
        DpkgCurrentState::HalfConfigured => DpkgPackageState::HalfConfigured,
        DpkgCurrentState::HalfInstalled => DpkgPackageState::HalfInstalled,
        DpkgCurrentState::TriggersAwaited => DpkgPackageState::TriggersAwaited,
        DpkgCurrentState::TriggersPending => DpkgPackageState::TriggersPending,
    };
    let artifact = if state.is_recoverable() {
        let version = record
            .version()
            .ok_or_else(|| package_state_unknown("missing version"))?;
        let architecture = record
            .architecture()
            .ok_or_else(|| package_state_unknown("missing architecture"))?;
        Some(
            [context.source_artifact(), context.target_artifact()]
                .into_iter()
                .flatten()
                .find(|artifact| {
                    artifact.package_version() == version && artifact.architecture() == architecture
                })
                .cloned()
                .ok_or_else(|| {
                    port_error(
                        DpkgPortErrorCode::PackageStateUnexpected,
                        DpkgPortPhase::PackageInspection,
                        "installed package identity is outside the operation",
                    )
                })?,
        )
    } else {
        None
    };
    PackageSnapshot::new(state, artifact).map_err(|_| package_state_unknown("invalid snapshot"))
}

fn validate_relationship_dependencies(
    status: &DpkgStatusSnapshot,
    source: Option<&VerifiedArtifactRelationship>,
    target: Option<&VerifiedArtifactRelationship>,
    phase: DpkgPortPhase,
) -> Result<(), DpkgPortError> {
    for relationship in source.into_iter().chain(target) {
        status
            .validate_dependencies(relationship)
            .map_err(|error| relationship_port_error(error, phase))?;
    }
    Ok(())
}

fn require_fixed_staged_package(
    context: DpkgOperationContext<'_>,
    package: DpkgStagedPackage<'_>,
    slot: StagedDebSlot,
) -> Result<(), DpkgPortError> {
    let expected = PrivateStagedDeb::new(context.operation_id(), slot)
        .map_err(|_| command_contract_error(DpkgPortPhase::TargetMutation))?;
    let evidence_name = match slot {
        StagedDebSlot::Source => "source.evidence.json",
        StagedDebSlot::Target => "target.evidence.json",
    };
    if package.package_path() != expected.path()
        || package.evidence_path() != expected.path().with_file_name(evidence_name)
        || !context.knows_artifact(package.artifact())
    {
        return Err(command_contract_error(DpkgPortPhase::TargetMutation));
    }
    Ok(())
}

fn staged_slot_for_context(
    context: DpkgOperationContext<'_>,
    package: DpkgStagedPackage<'_>,
) -> Result<StagedDebSlot, DpkgPortError> {
    let source = PrivateStagedDeb::new(context.operation_id(), StagedDebSlot::Source)
        .map_err(|_| command_contract_error(DpkgPortPhase::SourceRestore))?;
    let target = PrivateStagedDeb::new(context.operation_id(), StagedDebSlot::Target)
        .map_err(|_| command_contract_error(DpkgPortPhase::SourceRestore))?;
    if package.package_path() == source.path() {
        Ok(StagedDebSlot::Source)
    } else if package.package_path() == target.path() {
        Ok(StagedDebSlot::Target)
    } else {
        Err(command_contract_error(DpkgPortPhase::SourceRestore))
    }
}

fn require_receipt_state(
    context: DpkgOperationContext<'_>,
    expected: LinuxInstallState,
    phase: DpkgPortPhase,
) -> Result<(), DpkgPortError> {
    if context
        .receipt()
        .map(crate::model::LinuxInstallReceipt::state)
        != Some(expected)
    {
        return Err(port_error(
            DpkgPortErrorCode::CommandInvalid,
            phase,
            "dpkg mutation was requested outside its persisted receipt phase",
        ));
    }
    Ok(())
}

fn require_validation_state(
    context: DpkgOperationContext<'_>,
    phase: DpkgProductValidationPhase,
    port_phase: DpkgPortPhase,
) -> Result<(), DpkgPortError> {
    let state = context.receipt().map(|receipt| receipt.state());
    let allowed = match phase {
        DpkgProductValidationPhase::Target => matches!(
            state,
            Some(LinuxInstallState::PackageMutating | LinuxInstallState::PackageVerified)
        ),
        DpkgProductValidationPhase::Source => matches!(
            state,
            Some(LinuxInstallState::SourceRestoring | LinuxInstallState::SourceVerified)
        ),
    };
    if !allowed {
        return Err(port_error(
            DpkgPortErrorCode::CommandInvalid,
            port_phase,
            "product validation was requested outside its persisted receipt phase",
        ));
    }
    Ok(())
}

fn relationship_port_error(error: DebianRelationshipError, phase: DpkgPortPhase) -> DpkgPortError {
    let code = match error.code() {
        DebianRelationshipErrorCode::DependencyUnavailable
        | DebianRelationshipErrorCode::DependencyVersionUnsatisfied
        | DebianRelationshipErrorCode::DependencyArchitectureMismatch => {
            DpkgPortErrorCode::DependencyUnavailable
        }
        DebianRelationshipErrorCode::VersionInvalid
        | DebianRelationshipErrorCode::VersionRelationInvalid => {
            DpkgPortErrorCode::VersionRelationInvalid
        }
        DebianRelationshipErrorCode::DpkgStatusInvalid
        | DebianRelationshipErrorCode::PackageStateInvalid => {
            DpkgPortErrorCode::PackageStateUnknown
        }
        _ => DpkgPortErrorCode::ArtifactInvalid,
    };
    port_error(code, phase, "Debian package relationship validation failed")
}

fn observation_port_error(
    error: LinuxSystemObservationError,
    phase: DpkgPortPhase,
) -> DpkgPortError {
    let code = match error.code() {
        LinuxSystemObservationErrorCode::EnvironmentUnsupported => {
            DpkgPortErrorCode::EnvironmentUnsupported
        }
        LinuxSystemObservationErrorCode::ArtifactInvalid => DpkgPortErrorCode::ArtifactInvalid,
        LinuxSystemObservationErrorCode::DependencyUnavailable => {
            DpkgPortErrorCode::DependencyUnavailable
        }
        LinuxSystemObservationErrorCode::ProgramsRunning => DpkgPortErrorCode::ProgramsRunning,
        LinuxSystemObservationErrorCode::ProcessDisappeared
        | LinuxSystemObservationErrorCode::ProcessInspectionUnavailable => {
            DpkgPortErrorCode::ProcessInspectionUnavailable
        }
        LinuxSystemObservationErrorCode::PackageStateUnavailable => {
            DpkgPortErrorCode::PackageStateUnavailable
        }
        LinuxSystemObservationErrorCode::PackageStateUnknown => {
            DpkgPortErrorCode::PackageStateUnknown
        }
        LinuxSystemObservationErrorCode::ProductIdentityChanged => {
            DpkgPortErrorCode::ProductValidationFailed
        }
        LinuxSystemObservationErrorCode::PermissionDenied => DpkgPortErrorCode::PermissionDenied,
        LinuxSystemObservationErrorCode::Io => DpkgPortErrorCode::Io,
    };
    port_error(code, phase, "Linux system observation failed")
}

fn execution_port_error(error: DebianExecutionError, phase: DpkgPortPhase) -> DpkgPortError {
    let code = match error.code() {
        DebianExecutionErrorCode::ProgramIdentity
        | DebianExecutionErrorCode::DiagnosticsOverflow => DpkgPortErrorCode::CommandInvalid,
        DebianExecutionErrorCode::TimedOut => DpkgPortErrorCode::MutationFailed,
        DebianExecutionErrorCode::PermissionDenied => DpkgPortErrorCode::PermissionDenied,
        DebianExecutionErrorCode::Io => DpkgPortErrorCode::Io,
    };
    port_error(code, phase, "fixed dpkg command execution failed")
}

fn command_contract_error(phase: DpkgPortPhase) -> DpkgPortError {
    port_error(
        DpkgPortErrorCode::CommandInvalid,
        phase,
        "dpkg command paths differ from the fixed private staging contract",
    )
}

fn package_state_unknown(message: &'static str) -> DpkgPortError {
    port_error(
        DpkgPortErrorCode::PackageStateUnknown,
        DpkgPortPhase::PackageInspection,
        message,
    )
}

fn port_error(
    code: DpkgPortErrorCode,
    phase: DpkgPortPhase,
    message: &'static str,
) -> DpkgPortError {
    DpkgPortError::new(code, phase, message)
}
