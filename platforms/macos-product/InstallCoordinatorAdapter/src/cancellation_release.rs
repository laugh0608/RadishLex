//! Binds cancellation release to the live outer transaction and independent
//! source qualification. Neither a preserved archive nor a UI flag is authority.
use radishlex_ime_product_install::{
    validate_preparation_cancellation_outer, InstallProcessGuard, InstallReceipt,
    InstallReceiptStore, ProductArtifactIdentity,
};
use radishlex_ime_product_upgrade::{
    CancellationReleaseCheckpoint, CancellationReleasePort, CancellationReleaseReceipt,
    CancellationReleaseRequirement,
};

/// Reinspect both installed source programs, the intended target, the exact
/// data root, current task authorization and process quiescence on every call.
/// The complete proof binds the source family and the cancellation operation;
/// observers must not open the user's database to test old-binary compatibility.
pub trait CancellationReleaseProductAuthority {
    fn confirm_source_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        target: &ProductArtifactIdentity,
        checkpoint: CancellationReleaseCheckpoint,
    ) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationQualificationError {
    Unavailable,
    EvidenceChanged,
    UnsupportedSchema,
    VerificationFailed,
}

/// An attestation from the trusted qualification provider, not a saved boolean.
/// It binds BOTH program identities, source schema and this exact immutable
/// proof (including its source family, old outer and history). Its construction
/// performs structural checks only; the provider must verify independent source
/// copies' gates, open/learning/tombstone/settings/sync preservation and restart
/// coverage, and revalidate its backing evidence on every request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationSourceQualification {
    source: ProductArtifactIdentity,
    source_schema_version: i64,
    proof: CancellationReleaseReceipt,
}

impl CancellationSourceQualification {
    pub fn from_verified_source(
        source: ProductArtifactIdentity,
        source_schema_version: i64,
        proof: CancellationReleaseReceipt,
    ) -> Result<Self, CancellationQualificationError> {
        let binding = proof.preparation().binding();
        if proof.encode().is_err()
            || source.product_manifest_sha256() != binding.source_product_sha256
            || source.release().product_version() != binding.source_release.product_version()
            || source.release().build_number() != binding.source_release.build_number()
        {
            return Err(CancellationQualificationError::EvidenceChanged);
        }
        if source_schema_version < 0
            || source_schema_version > binding.target_schema_version
            || proof
                .preparation()
                .snapshot_schema_version()
                .is_some_and(|schema| schema != source_schema_version)
        {
            return Err(CancellationQualificationError::UnsupportedSchema);
        }
        Ok(Self {
            source,
            source_schema_version,
            proof,
        })
    }

    pub fn source_schema_version(&self) -> i64 {
        self.source_schema_version
    }

    fn matches(
        &self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
    ) -> bool {
        &self.proof == proof && &self.source == source
    }
}

/// No production issuer is supplied here. Missing qualification must return
/// Unavailable; synthetic providers belong only in isolated contract tests.
/// Before maintenance there may be no snapshot schema: the issuer must prove
/// the schema against the exact source family in `proof`, not assume the target
/// schema or treat a missing schema as a wildcard.
pub trait CancellationSourceQualificationAuthority {
    fn verify_source_qualification(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        checkpoint: CancellationReleaseCheckpoint,
    ) -> Result<CancellationSourceQualification, CancellationQualificationError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationReleaseAuthorityFailure {
    OuterBinding,
    ProductObservation,
    SourceQualification(CancellationQualificationError),
}

/// Construct after acquiring the outer guard, then acquire the inner guard.
/// The core checks the inner guard and physical evidence around this callback;
/// this authority checks the real outer guard, typed originals and live receipt
/// before and after BOTH external providers. It never persists a v1 receipt.
pub struct CancellationReleaseAuthority<'a, P, Q> {
    install: &'a InstallReceiptStore,
    guard: &'a InstallProcessGuard,
    source: &'a ProductArtifactIdentity,
    target: &'a ProductArtifactIdentity,
    product: &'a mut P,
    qualification: &'a mut Q,
    failure: Option<CancellationReleaseAuthorityFailure>,
}

impl<'a, P, Q> CancellationReleaseAuthority<'a, P, Q> {
    pub fn new(
        install: &'a InstallReceiptStore,
        guard: &'a InstallProcessGuard,
        source: &'a ProductArtifactIdentity,
        target: &'a ProductArtifactIdentity,
        product: &'a mut P,
        qualification: &'a mut Q,
    ) -> Self {
        Self {
            install,
            guard,
            source,
            target,
            product,
            qualification,
            failure: None,
        }
    }

    /// The most recent callback's rejection behind AuthorityNotProven. The
    /// core can reject before calling this port; always report its error first.
    pub fn failure(&self) -> Option<CancellationReleaseAuthorityFailure> {
        self.failure
    }

    fn verify_outer(
        &self,
        proof: &CancellationReleaseReceipt,
    ) -> Result<(), CancellationReleaseAuthorityFailure> {
        let rejected = CancellationReleaseAuthorityFailure::OuterBinding;
        let binding = proof.preparation().binding();
        let root = self.install.root_identity();
        self.install
            .verify_guard(self.guard)
            .map_err(|_| rejected)?;
        if proof.encode().is_err()
            || root.device_id() != binding.data_root.device_id
            || root.inode() != binding.data_root.inode
            || root.owner_id() != binding.data_root.owner_id
            || root.mode() != binding.data_root.mode
            || self.source.product_manifest_sha256() != binding.source_product_sha256
            || self.target.product_manifest_sha256() != binding.target_product_sha256
            || self.source.release().product_version() != binding.source_release.product_version()
            || self.source.release().build_number() != binding.source_release.build_number()
            || self.target.release().product_version() != binding.target_release.product_version()
            || self.target.release().build_number() != binding.target_release.build_number()
        {
            return Err(rejected);
        }
        let outer = proof.archive().outer();
        validate_preparation_cancellation_outer(
            outer.previous_bytes(),
            outer.new_bytes(),
            proof.operation_id(),
            proof.legacy_outer_operation_id(),
            root,
            self.source,
            self.target,
        )
        .map_err(|_| rejected)?;
        let previous = InstallReceipt::decode(outer.previous_bytes()).map_err(|_| rejected)?;
        // Unlike archive restoration, release requires the compatible old
        // terminal to be present now, not merely preserved in history.
        self.install
            .verify_current(self.guard, &previous)
            .map_err(|_| rejected)
    }
}

impl<P: CancellationReleaseProductAuthority, Q: CancellationSourceQualificationAuthority>
    CancellationReleaseAuthority<'_, P, Q>
{
    fn confirm(
        &mut self,
        proof: &CancellationReleaseReceipt,
        checkpoint: CancellationReleaseCheckpoint,
        requirement: CancellationReleaseRequirement,
    ) -> Result<(), CancellationReleaseAuthorityFailure> {
        self.verify_outer(proof)?;
        if !self
            .product
            .confirm_source_and_quiescence(proof, self.source, self.target, checkpoint)
        {
            return Err(CancellationReleaseAuthorityFailure::ProductObservation);
        }
        self.verify_outer(proof)?;
        if requirement == CancellationReleaseRequirement::QualifiedSource {
            let evidence = self
                .qualification
                .verify_source_qualification(proof, self.source, checkpoint)
                .map_err(CancellationReleaseAuthorityFailure::SourceQualification)?;
            if !evidence.matches(proof, self.source) {
                return Err(CancellationReleaseAuthorityFailure::SourceQualification(
                    CancellationQualificationError::EvidenceChanged,
                ));
            }
            self.verify_outer(proof)?;
            // A qualifier may take time. Reobserve installed programs and
            // quiescence after it returns, before permitting a marker mutation.
            if !self.product.confirm_source_and_quiescence(
                proof,
                self.source,
                self.target,
                checkpoint,
            ) {
                return Err(CancellationReleaseAuthorityFailure::ProductObservation);
            }
            self.verify_outer(proof)?;
        }
        Ok(())
    }
}

impl<P: CancellationReleaseProductAuthority, Q: CancellationSourceQualificationAuthority>
    CancellationReleasePort for CancellationReleaseAuthority<'_, P, Q>
{
    fn confirm_authority_and_quiescence(
        &mut self,
        proof: &CancellationReleaseReceipt,
        checkpoint: CancellationReleaseCheckpoint,
        requirement: CancellationReleaseRequirement,
    ) -> bool {
        self.failure = self.confirm(proof, checkpoint, requirement).err();
        self.failure.is_none()
    }
}
