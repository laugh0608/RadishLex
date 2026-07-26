use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use radishlex_ime_product_install::{
    InstallOperationKind, InstallReceipt, InstallReceiptStore, InstallState,
    ProductArtifactIdentity, ProductRelease, ProgramBundleIdentity, ProgramComponent,
    VerifiedInstallRoot, INSTALL_PRODUCT_ID,
};

use super::*;

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    container: PathBuf,
    data_root: PathBuf,
    owner_id: u32,
}

impl Fixture {
    fn new(create_root: bool) -> Self {
        let container = fs::canonicalize(std::env::temp_dir())
            .expect("temp root")
            .join(format!(
                "radishlex-installer-driver-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new()
            .mode(0o700)
            .create(&container)
            .expect("container");
        let owner_id = fs::metadata(&container).expect("owner").uid();
        let data_root = container.join("RadishLex");
        if create_root {
            DirBuilder::new()
                .mode(0o700)
                .create(&data_root)
                .expect("data root");
        }
        Self {
            container,
            data_root,
            owner_id,
        }
    }

    fn store(&self) -> InstallReceiptStore {
        InstallReceiptStore::open(
            VerifiedInstallRoot::verify(&self.data_root, self.owner_id).expect("root"),
        )
        .expect("store")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.container);
    }
}

fn hash(marker: char) -> String {
    marker.to_string().repeat(64)
}

fn product(version: &str, build: u64) -> ProductArtifactIdentity {
    ProductArtifactIdentity::new(
        INSTALL_PRODUCT_ID,
        ProductRelease::new(version, build).expect("release"),
        hash('1'),
        ProgramBundleIdentity::new(
            ProgramComponent::Manager,
            "org.radishlex.manager",
            hash('2'),
            hash('3'),
        )
        .expect("Manager"),
        ProgramBundleIdentity::new(
            ProgramComponent::InputMethod,
            "org.radishlex.inputmethod",
            hash('4'),
            hash('5'),
        )
        .expect("InputMethod"),
    )
    .expect("product")
}

#[test]
fn no_receipt_uses_verified_product_situation_without_guessing_paths() {
    let fixture = Fixture::new(false);
    let first = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::NotInstalled,
    );
    assert_eq!(first.phase(), InstallerViewPhase::Ready);
    assert_eq!(first.primary_action(), InstallerAction::BeginFirstInstall);
    assert_eq!(first.operation_code(), "first_install");
    assert_eq!(first.manual_prompt_code(), "none");
    assert_eq!(first.data_policy_code(), "retain_application_support");
    assert!(!fixture.data_root.exists());

    let matching = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::MatchingReleaseInstalled,
    );
    assert_eq!(matching.primary_action(), InstallerAction::BeginRepair);
    assert_eq!(matching.secondary_action(), InstallerAction::RemovePrograms);

    let newer = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::NewerReleaseInstalled,
    );
    assert_eq!(newer.phase(), InstallerViewPhase::Blocked);
    assert_eq!(
        newer.stable_error(),
        InstallerStableError::InstalledReleaseIsNewer
    );
}

#[test]
fn prepared_receipt_projects_manual_prompt_and_restart_action() {
    let fixture = Fixture::new(true);
    let store = fixture.store();
    let receipt = InstallReceipt::new(
        "00112233445566778899aabbccddeeff",
        None,
        InstallOperationKind::FirstInstall,
        store.root_identity().clone(),
        None,
        Some(product("0.1.0", 35)),
    )
    .expect("receipt");
    let guard = store.acquire_guard().expect("guard");
    store.persist(&guard, &receipt).expect("persist");
    let active = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::IdentityUnavailable,
    );
    assert_eq!(active.phase(), InstallerViewPhase::InProgress);
    assert_eq!(active.primary_action(), InstallerAction::Refresh);
    assert_eq!(active.stable_error(), InstallerStableError::OperationActive);
    drop(guard);

    let restarted = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::IdentityUnavailable,
    );
    assert_eq!(restarted.phase(), InstallerViewPhase::AwaitingUserAction);
    assert_eq!(
        restarted.primary_action(),
        InstallerAction::ConfirmQuiescence
    );
    assert_eq!(
        restarted.manual_prompt(),
        InstallerManualPrompt::SelectNeutralInputSourceAndCloseManager
    );
    assert_eq!(
        restarted.operation_kind(),
        Some(InstallOperationKind::FirstInstall)
    );
    assert_eq!(restarted.operation_code(), "first_install");
    assert_eq!(
        restarted.manual_prompt_code(),
        "select_neutral_input_source_and_close_manager"
    );
    assert_eq!(restarted.receipt_state(), Some(InstallState::Prepared));
    assert_eq!(restarted.progress_step(), 1);
}

#[test]
fn completed_receipt_reprojects_actions_from_verified_installed_product() {
    let upgrade = completed_snapshot(
        InstallOperationKind::FirstInstall,
        InstallerProductSituation::OlderReleaseInstalled,
        None,
    );
    assert_eq!(upgrade.phase(), InstallerViewPhase::Ready);
    assert_eq!(upgrade.primary_action(), InstallerAction::BeginUpgrade);

    let repair = completed_snapshot(
        InstallOperationKind::Upgrade,
        InstallerProductSituation::MatchingReleaseInstalled,
        None,
    );
    assert_eq!(repair.primary_action(), InstallerAction::BeginRepair);
    assert_eq!(repair.secondary_action(), InstallerAction::RemovePrograms);

    let drift = completed_snapshot(
        InstallOperationKind::Repair,
        InstallerProductSituation::NotInstalled,
        None,
    );
    assert_eq!(drift.phase(), InstallerViewPhase::Blocked);
    assert_eq!(
        drift.stable_error(),
        InstallerStableError::ProductIdentityUnavailable
    );
}

#[test]
fn action_authorization_rejects_stale_or_unconfirmed_mutations() {
    let fixture = Fixture::new(false);
    let snapshot = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::MatchingReleaseInstalled,
    );
    assert_eq!(
        authorize_installer_action(
            snapshot,
            InstallerAction::BeginUpgrade,
            InstallerUserAuthorization::default()
        ),
        Err(InstallerAuthorizationError::ActionNotOffered)
    );
    assert_eq!(
        authorize_installer_action(
            snapshot,
            InstallerAction::BeginRepair,
            InstallerUserAuthorization::default()
        ),
        Err(InstallerAuthorizationError::ExplicitConfirmationRequired)
    );
    let repair = authorize_installer_action(
        snapshot,
        InstallerAction::BeginRepair,
        InstallerUserAuthorization {
            explicit_action_confirmed: true,
            neutral_input_source_selected: true,
            manager_closed: true,
            ..InstallerUserAuthorization::default()
        },
    )
    .expect("authorized repair");
    assert_eq!(repair.operation_kind(), Some(InstallOperationKind::Repair));
    assert!(repair.requires_platform_preflight());
}

#[test]
fn removal_requires_data_retention_and_manual_quiescence_acknowledgements() {
    let fixture = Fixture::new(false);
    let snapshot = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::MatchingReleaseInstalled,
    );
    let explicit = InstallerUserAuthorization {
        explicit_action_confirmed: true,
        ..InstallerUserAuthorization::default()
    };
    assert_eq!(
        authorize_installer_action(snapshot, InstallerAction::RemovePrograms, explicit),
        Err(InstallerAuthorizationError::DataRetentionAcknowledgementRequired)
    );
    assert_eq!(
        authorize_installer_action(
            snapshot,
            InstallerAction::RemovePrograms,
            InstallerUserAuthorization {
                data_retention_acknowledged: true,
                ..explicit
            }
        ),
        Err(InstallerAuthorizationError::ManualQuiescenceAcknowledgementRequired)
    );
    let removal = authorize_installer_action(
        snapshot,
        InstallerAction::RemovePrograms,
        InstallerUserAuthorization {
            data_retention_acknowledged: true,
            neutral_input_source_selected: true,
            manager_closed: true,
            ..explicit
        },
    )
    .expect("authorized removal");
    assert_eq!(
        removal.operation_kind(),
        Some(InstallOperationKind::RemovePrograms)
    );
    assert!(!removal.resume_existing());
}

#[test]
fn stable_summary_contains_only_codes_and_no_paths_or_operation_identifier() {
    let fixture = Fixture::new(false);
    let snapshot = inspect_installer_view(
        &fixture.data_root,
        fixture.owner_id,
        InstallerProductSituation::IdentityUnavailable,
    );
    let summary = snapshot.stable_summary();
    assert_eq!(
        summary,
        "phase=blocked action=refresh error=product_identity_unavailable state=none"
    );
    assert!(!summary.contains("RadishLex"));
    assert!(!summary.contains('/'));
}
