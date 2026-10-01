//! Typed validation for the dedicated preparation-cancellation coordinator.
//! Does not relax persist/can_replace or grant filesystem/product authority.
use crate::*;

/// Validate the immutable original receipts, even while the active outer slot
/// is temporarily absent. The caller separately holds both guards, verifies
/// sealed file identities and freshly proves installed source/quiescence.
pub fn validate_preparation_cancellation_outer(
    previous: &[u8],
    new: Option<&[u8]>,
    operation_id: &str,
    previous_operation_id: &str,
    root: &InstallRootIdentity,
    source: &ProductArtifactIdentity,
    target: &ProductArtifactIdentity,
) -> Result<(), InstallReceiptError> {
    let invalid = || {
        InstallReceiptError::invalid("cancellation", "unbound preparation cancellation receipts")
    };
    let old = InstallReceipt::decode(previous)?;
    if old.encode()? != previous
        || operation_id == previous_operation_id
        || old.operation_id() != previous_operation_id
        || old.root_identity() != root
        || !old.state().is_terminal()
        || old.manual_recovery_required()
        || old.installed_product() != Some(source)
    {
        return Err(invalid());
    }
    // Constructing the expected Prepared value applies the same normal v1
    // source/target/version and operation contract without inventing a terminal.
    let expected = InstallReceipt::new(
        operation_id,
        Some(previous_operation_id.to_owned()),
        InstallOperationKind::Upgrade,
        root.clone(),
        Some(source.clone()),
        Some(target.clone()),
    )?;
    if !expected.can_replace(&old) {
        return Err(invalid());
    }
    if let Some(bytes) = new {
        let value = InstallReceipt::decode(bytes)?;
        if value != expected || value.encode()? != bytes || !value.artifacts().is_empty() {
            return Err(invalid());
        }
    }
    Ok(())
}
