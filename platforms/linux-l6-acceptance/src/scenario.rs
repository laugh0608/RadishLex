use std::fmt;

use radishlex_linux_product_install::{LinuxL6Checkpoint, LinuxOperationKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L6CrashScenario {
    InstallPrepared,
    InstallArtifactsStaged,
    UpgradeQuiesced,
    UpgradeBeforeDpkg,
    UpgradeAfterDpkg,
    UpgradeRollbackRequired,
    UpgradeBeforeSourceRestore,
    UpgradeAfterSourceRestore,
}

impl L6CrashScenario {
    pub const ALL: [Self; 8] = [
        Self::InstallPrepared,
        Self::InstallArtifactsStaged,
        Self::UpgradeQuiesced,
        Self::UpgradeBeforeDpkg,
        Self::UpgradeAfterDpkg,
        Self::UpgradeRollbackRequired,
        Self::UpgradeBeforeSourceRestore,
        Self::UpgradeAfterSourceRestore,
    ];

    pub fn parse(value: &str) -> Result<Self, L6CrashScenarioError> {
        Self::ALL
            .into_iter()
            .find(|scenario| scenario.as_str() == value)
            .ok_or(L6CrashScenarioError)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstallPrepared => "install_prepared",
            Self::InstallArtifactsStaged => "install_artifacts_staged",
            Self::UpgradeQuiesced => "upgrade_quiesced",
            Self::UpgradeBeforeDpkg => "upgrade_before_dpkg",
            Self::UpgradeAfterDpkg => "upgrade_after_dpkg",
            Self::UpgradeRollbackRequired => "upgrade_rollback_required",
            Self::UpgradeBeforeSourceRestore => "upgrade_before_source_restore",
            Self::UpgradeAfterSourceRestore => "upgrade_after_source_restore",
        }
    }

    pub const fn checkpoint(self) -> LinuxL6Checkpoint {
        match self {
            Self::InstallPrepared => LinuxL6Checkpoint::Prepared,
            Self::InstallArtifactsStaged => LinuxL6Checkpoint::ArtifactsStaged,
            Self::UpgradeQuiesced => LinuxL6Checkpoint::Quiesced,
            Self::UpgradeBeforeDpkg => LinuxL6Checkpoint::PackageMutatingBeforeDpkg,
            Self::UpgradeAfterDpkg => LinuxL6Checkpoint::TargetAppliedBeforeProof,
            Self::UpgradeRollbackRequired => LinuxL6Checkpoint::RollbackRequired,
            Self::UpgradeBeforeSourceRestore => LinuxL6Checkpoint::SourceRestoringBeforeDpkg,
            Self::UpgradeAfterSourceRestore => LinuxL6Checkpoint::SourceAppliedBeforeProof,
        }
    }

    pub const fn operation_id(self) -> &'static str {
        match self {
            Self::InstallPrepared | Self::InstallArtifactsStaged => "install_source",
            _ => "upgrade_target",
        }
    }

    pub const fn operation_kind(self) -> LinuxOperationKind {
        match self {
            Self::InstallPrepared | Self::InstallArtifactsStaged => LinuxOperationKind::Install,
            _ => LinuxOperationKind::Upgrade,
        }
    }

    pub const fn reject_target_validation(self) -> bool {
        matches!(
            self,
            Self::UpgradeRollbackRequired
                | Self::UpgradeBeforeSourceRestore
                | Self::UpgradeAfterSourceRestore
        )
    }

    pub const fn fault(self) -> &'static str {
        if self.reject_target_validation() {
            "target_validation_rejected_then_process_group_terminated"
        } else {
            "process_group_terminated"
        }
    }

    pub const fn expected_terminal(self) -> &'static str {
        if self.reject_target_validation() {
            "rolled_back"
        } else {
            "completed"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L6CrashScenarioError;

impl fmt::Display for L6CrashScenarioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("L6 crash scenario is invalid")
    }
}

impl std::error::Error for L6CrashScenarioError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_matrix_scenarios_map_to_unique_checkpoints() {
        let mut checkpoints = std::collections::BTreeSet::new();
        for scenario in L6CrashScenario::ALL {
            assert_eq!(L6CrashScenario::parse(scenario.as_str()), Ok(scenario));
            assert!(checkpoints.insert(scenario.checkpoint().as_str()));
        }
        assert_eq!(checkpoints.len(), 8);
    }
}
