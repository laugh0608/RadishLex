use super::{
    ArtifactVersionRelation, DpkgPackageState, LinuxArtifactIdentity, LinuxInstallReceipt,
    LinuxInstallReceiptError, LinuxInstallState, LinuxOperationKind,
};

impl LinuxInstallReceipt {
    /// Projects an already decoded receipt for source seed collection; never observes the system.
    pub fn source_seed_facts(
        &self,
        expected: &LinuxArtifactIdentity,
    ) -> Result<serde_json::Value, LinuxInstallReceiptError> {
        self.validate()?;
        if self.operation_kind != LinuxOperationKind::Install
            || self.version_relation != ArtifactVersionRelation::NotApplicable
            || self.state != LinuxInstallState::Completed
            || self.operation_chain.len() != 1
            || !self.initial_package.matches_absent()
            || self.initial_package.state != DpkgPackageState::NotInstalled
            || self.source_artifact.is_some()
            || self.source_proof.is_some()
            || self.target_artifact.as_ref() != Some(expected)
            || !self
                .target_proof
                .as_ref()
                .is_some_and(|proof| proof.matches_installed(expected))
            || self.failure_code.is_some()
            || self.failure_after_state.is_some()
            || self.manual_recovery_required
        {
            return Err(LinuxInstallReceiptError::invalid(
                "source_seed",
                "receipt is not one fresh completed install of the exact source",
            ));
        }
        Ok(serde_json::json!({
            "operation_kind": self.operation_kind,
            "version_relation": self.version_relation,
            "state": self.state,
            "chain_length": self.operation_chain.len(),
            "initial_package": self.initial_package,
            "source_artifact": self.source_artifact,
            "source_proof": self.source_proof,
            "target_artifact": self.target_artifact,
            "target_proof": self.target_proof,
            "failure_code": self.failure_code,
            "failure_after_state": self.failure_after_state,
            "manual_recovery_required": self.manual_recovery_required
        }))
    }
}
