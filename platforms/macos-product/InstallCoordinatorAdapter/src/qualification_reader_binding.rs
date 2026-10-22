//! Structural request binding only. No filesystem observation or source opening.

use super::model::{
    BlockingObject, DataPredecessor, Lifecycle, Maintenance, OuterState, RejectionFault, Scenario,
    SourceFamily,
};
use super::{digest, ReadError};
use radishlex_ime_product_install::{
    validate_preparation_cancellation_outer, InstallReceipt, InstallRootIdentity, InstallState,
    ProductArtifactIdentity,
};
use radishlex_ime_product_upgrade::{
    CancellationPredecessorKind, CancellationReleaseReceipt, PreparationPhase,
};

/// A validated description of a request, never proof of current on-disk state.
/// Its complete canonical proof is bound; callers cannot supply a coverage flag.
pub struct CancellationEvidenceRequest {
    pub(super) source: ProductArtifactIdentity,
    pub(super) target: ProductArtifactIdentity,
    pub(super) proof_sha256: String,
    pub(super) family_sha256: String,
    pub(super) scenario: Scenario,
    pub(super) schema: Option<i64>,
    pub(super) root: InstallRootIdentity,
}

impl CancellationEvidenceRequest {
    pub fn new(
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
    ) -> Result<Self, ReadError> {
        let bytes = proof.encode().map_err(|_| ReadError::IdentityMismatch)?;
        let preparation = proof.preparation();
        let binding = preparation.binding();
        let root = InstallRootIdentity::new(
            binding.data_root.device_id,
            binding.data_root.inode,
            binding.data_root.owner_id,
            binding.data_root.mode,
        )
        .map_err(|_| ReadError::IdentityMismatch)?;
        let outer = proof.archive().outer();
        validate_preparation_cancellation_outer(
            outer.previous_bytes(),
            outer.new_bytes(),
            proof.operation_id(),
            proof.legacy_outer_operation_id(),
            &root,
            source,
            target,
        )
        .map_err(|_| ReadError::IdentityMismatch)?;
        if source.product_manifest_sha256() != binding.source_product_sha256
            || source.release().product_version() != binding.source_release.product_version()
            || source.release().build_number() != binding.source_release.build_number()
            || target.product_manifest_sha256() != binding.target_product_sha256
            || target.release().product_version() != binding.target_release.product_version()
            || target.release().build_number() != binding.target_release.build_number()
        {
            return Err(ReadError::IdentityMismatch);
        }
        let previous = InstallReceipt::decode(outer.previous_bytes())
            .map_err(|_| ReadError::IdentityMismatch)?;
        let outer_state = match previous.state() {
            InstallState::Completed => OuterState::Completed,
            InstallState::AbortedPreserved => OuterState::AbortedPreserved,
            InstallState::RolledBack => OuterState::RolledBack,
            _ => return Err(ReadError::IdentityMismatch),
        };
        let data_predecessor = match binding.previous_data_operation_id.as_deref() {
            None => DataPredecessor::Absent,
            Some(id) if id == proof.legacy_outer_operation_id() => DataPredecessor::SameOperation,
            Some(_) => DataPredecessor::DistinctOperation,
        };
        let lifecycle = match proof.predecessor_kind() {
            CancellationPredecessorKind::NoIndex => Lifecycle::NoIndex,
            CancellationPredecessorKind::TerminalV1 => Lifecycle::V1,
            CancellationPredecessorKind::TerminalV2
            | CancellationPredecessorKind::CancellationV2 => Lifecycle::V2,
        };
        let maintained = preparation.phase() >= PreparationPhase::MaintenanceIntent;
        let family = proof
            .archive()
            .source()
            .source_ready()
            .ok_or(ReadError::IdentityMismatch)?;
        // The full proof hash above additionally binds both initial and maintained
        // families, all receipts, original identities and the predecessor locator.
        let family_bytes = serde_json::to_vec(family).map_err(|_| ReadError::Malformed)?;
        Ok(Self {
            source: source.clone(),
            target: target.clone(),
            proof_sha256: digest(&bytes),
            family_sha256: digest(&family_bytes),
            scenario: Scenario {
                maintenance: if maintained {
                    Maintenance::AfterEquivalent
                } else {
                    Maintenance::Before
                },
                outer_state,
                data_predecessor,
                lifecycle,
                family: if maintained {
                    SourceFamily::MaintenanceEquivalent
                } else {
                    SourceFamily::Original
                },
                repeated_cancellation: proof.predecessor_kind()
                    == CancellationPredecessorKind::CancellationV2,
                blocking_object: BlockingObject::None,
                rejection_fault: RejectionFault::None,
            },
            schema: preparation.snapshot_schema_version(),
            root,
        })
    }
}
