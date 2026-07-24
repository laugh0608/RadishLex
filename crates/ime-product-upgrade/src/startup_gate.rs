use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

use crate::UpgradeState;

use super::{
    build_guard_socket_path, path_exists, verify_private_directory, DirectoryIdentity,
    UpgradeFilesystemErrorCode, UpgradeReceiptStore, VerifiedDataRoot, STATE_DIRECTORY_NAME,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupGateDecision {
    AllowedFirstLaunch,
    AllowedNoUpgradeState,
    AllowedTerminalReceipt,
    BlockedUpgradeInProgress,
    FailedClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupGateErrorCode {
    None,
    UpgradeInProgress,
    ActiveGuard,
    UnsafeDataRoot,
    UnsafeStateDirectory,
    InterruptedArtifact,
    InvalidReceipt,
    UnexpectedStateObject,
    IdentityChanged,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupGateResult {
    decision: StartupGateDecision,
    error_code: StartupGateErrorCode,
    receipt_state: Option<UpgradeState>,
}

impl StartupGateResult {
    pub const fn decision(self) -> StartupGateDecision {
        self.decision
    }

    pub const fn error_code(self) -> StartupGateErrorCode {
        self.error_code
    }

    pub const fn receipt_state(self) -> Option<UpgradeState> {
        self.receipt_state
    }

    pub const fn is_allowed(self) -> bool {
        matches!(
            self.decision,
            StartupGateDecision::AllowedFirstLaunch
                | StartupGateDecision::AllowedNoUpgradeState
                | StartupGateDecision::AllowedTerminalReceipt
        )
    }
}

/// Inspects the fixed product data root without creating, chmodding, deleting,
/// connecting to, or otherwise changing any filesystem object.
pub fn inspect_startup_gate(
    data_root: impl AsRef<Path>,
    expected_owner_id: u32,
) -> StartupGateResult {
    let data_root = data_root.as_ref();
    if !data_root.is_absolute()
        || data_root
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return failed(StartupGateErrorCode::UnsafeDataRoot);
    }
    match fs::symlink_metadata(data_root) {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return allowed(StartupGateDecision::AllowedFirstLaunch, None);
        }
        Err(_) => return failed(StartupGateErrorCode::Io),
        Ok(_) => {}
    }

    let root = match VerifiedDataRoot::verify(data_root, expected_owner_id) {
        Ok(root) => root,
        Err(_) => return failed(StartupGateErrorCode::UnsafeDataRoot),
    };
    let guard_path = match build_guard_socket_path(root.identity(), expected_owner_id) {
        Ok(path) => path,
        Err(_) => return failed(StartupGateErrorCode::UnsafeStateDirectory),
    };
    match path_exists(&guard_path) {
        Ok(true) => return failed(StartupGateErrorCode::ActiveGuard),
        Ok(false) => {}
        Err(_) => return failed(StartupGateErrorCode::Io),
    }

    let state_directory = data_root.join(STATE_DIRECTORY_NAME);
    let metadata = match fs::symlink_metadata(&state_directory) {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return allowed(StartupGateDecision::AllowedNoUpgradeState, None);
        }
        Err(_) => return failed(StartupGateErrorCode::Io),
        Ok(metadata) => metadata,
    };
    if verify_private_directory(&metadata, expected_owner_id, 0o700).is_err() {
        return failed(StartupGateErrorCode::UnsafeStateDirectory);
    }
    let store = UpgradeReceiptStore {
        root,
        state_directory,
        state_directory_identity: DirectoryIdentity::from_metadata(&metadata),
        guard_socket_path: guard_path,
    };
    match store.load() {
        Ok(None) => allowed(StartupGateDecision::AllowedNoUpgradeState, None),
        Ok(Some(receipt)) if receipt.state().is_terminal() => allowed(
            StartupGateDecision::AllowedTerminalReceipt,
            Some(receipt.state()),
        ),
        Ok(Some(receipt)) => StartupGateResult {
            decision: StartupGateDecision::BlockedUpgradeInProgress,
            error_code: StartupGateErrorCode::UpgradeInProgress,
            receipt_state: Some(receipt.state()),
        },
        Err(error) => failed(map_filesystem_error(error.code())),
    }
}

const fn allowed(
    decision: StartupGateDecision,
    receipt_state: Option<UpgradeState>,
) -> StartupGateResult {
    StartupGateResult {
        decision,
        error_code: StartupGateErrorCode::None,
        receipt_state,
    }
}

const fn failed(error_code: StartupGateErrorCode) -> StartupGateResult {
    StartupGateResult {
        decision: StartupGateDecision::FailedClosed,
        error_code,
        receipt_state: None,
    }
}

const fn map_filesystem_error(code: UpgradeFilesystemErrorCode) -> StartupGateErrorCode {
    match code {
        UpgradeFilesystemErrorCode::OperationAlreadyActive => StartupGateErrorCode::ActiveGuard,
        UpgradeFilesystemErrorCode::InterruptedReceiptWrite
        | UpgradeFilesystemErrorCode::InterruptedSnapshot
        | UpgradeFilesystemErrorCode::InterruptedCandidate
        | UpgradeFilesystemErrorCode::InterruptedSwitch
        | UpgradeFilesystemErrorCode::InterruptedSettingsBackup => {
            StartupGateErrorCode::InterruptedArtifact
        }
        UpgradeFilesystemErrorCode::InvalidReceipt
        | UpgradeFilesystemErrorCode::InvalidReceiptReplacement => {
            StartupGateErrorCode::InvalidReceipt
        }
        UpgradeFilesystemErrorCode::UnexpectedStateObject => {
            StartupGateErrorCode::UnexpectedStateObject
        }
        UpgradeFilesystemErrorCode::UnsafeDataRoot => StartupGateErrorCode::UnsafeDataRoot,
        UpgradeFilesystemErrorCode::UnsafeStateDirectory => {
            StartupGateErrorCode::UnsafeStateDirectory
        }
        UpgradeFilesystemErrorCode::IdentityChanged => StartupGateErrorCode::IdentityChanged,
        _ => StartupGateErrorCode::Io,
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, DirBuilder};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::{ProductRelease, UpgradeFailureCode, UpgradeReceipt};

    use super::*;

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct Fixture {
        container: std::path::PathBuf,
        data_root: std::path::PathBuf,
        owner_id: u32,
    }

    impl Fixture {
        fn new(create_root: bool) -> Self {
            let container = fs::canonicalize(std::env::temp_dir())
                .expect("temp root")
                .join(format!(
                    "radishlex-startup-gate-{}-{}",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                ));
            DirBuilder::new()
                .mode(0o700)
                .create(&container)
                .expect("container");
            let data_root = container.join("RadishLex");
            if create_root {
                DirBuilder::new()
                    .mode(0o700)
                    .create(&data_root)
                    .expect("data root");
            }
            let owner_id = fs::metadata(&container).expect("metadata").uid();
            Self {
                container,
                data_root,
                owner_id,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.container);
        }
    }

    #[test]
    fn absent_root_and_state_are_allowed_without_creation() {
        let absent = Fixture::new(false);
        assert_eq!(
            inspect_startup_gate(&absent.data_root, absent.owner_id).decision(),
            StartupGateDecision::AllowedFirstLaunch
        );
        assert!(!absent.data_root.exists());

        let empty = Fixture::new(true);
        assert_eq!(
            inspect_startup_gate(&empty.data_root, empty.owner_id).decision(),
            StartupGateDecision::AllowedNoUpgradeState
        );
        assert!(!empty.data_root.join(STATE_DIRECTORY_NAME).exists());

        let interrupted = Fixture::new(true);
        let state = interrupted.data_root.join(STATE_DIRECTORY_NAME);
        DirBuilder::new().mode(0o700).create(&state).expect("state");
        let staged = state.join("receipt.json.tmp");
        fs::write(&staged, b"interrupted\n").expect("staged receipt");
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o600)).expect("staged mode");
        assert_eq!(
            inspect_startup_gate(&interrupted.data_root, interrupted.owner_id).error_code(),
            StartupGateErrorCode::InterruptedArtifact
        );
    }

    #[test]
    fn active_nonterminal_terminal_and_corrupt_receipts_fail_or_allow_exactly() {
        let fixture = Fixture::new(true);
        let store = UpgradeReceiptStore::open(
            VerifiedDataRoot::verify(&fixture.data_root, fixture.owner_id).expect("verified"),
        )
        .expect("store");
        let mut receipt = UpgradeReceipt::new(
            "00112233445566778899aabbccddeeff",
            None,
            ProductRelease::new("0.0.9", 34).expect("source"),
            ProductRelease::new("0.1.0", 35).expect("target"),
            None,
            9,
            vec![store.data_root_identity().clone()],
        )
        .expect("receipt");
        let guard = store.acquire_guard().expect("guard");
        store.persist(&guard, &receipt).expect("persist");
        assert_eq!(
            inspect_startup_gate(&fixture.data_root, fixture.owner_id).error_code(),
            StartupGateErrorCode::ActiveGuard
        );
        drop(guard);
        assert_eq!(
            inspect_startup_gate(&fixture.data_root, fixture.owner_id).decision(),
            StartupGateDecision::BlockedUpgradeInProgress
        );

        let guard = store.acquire_guard().expect("guard");
        receipt
            .abort_preserved(UpgradeFailureCode::SnapshotFailed, false)
            .expect("abort");
        store.persist(&guard, &receipt).expect("persist terminal");
        drop(guard);
        assert!(inspect_startup_gate(&fixture.data_root, fixture.owner_id).is_allowed());

        fs::write(store.receipt_path(), b"broken\n").expect("corrupt receipt");
        fs::set_permissions(store.receipt_path(), fs::Permissions::from_mode(0o600)).expect("mode");
        assert_eq!(
            inspect_startup_gate(&fixture.data_root, fixture.owner_id).error_code(),
            StartupGateErrorCode::InvalidReceipt
        );
    }
}
