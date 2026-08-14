use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::coordinator::{
    DpkgExpectedProductState, DpkgOperationContext, DpkgPortErrorCode, DpkgPortPhase,
    DpkgProductValidationPhase, DpkgQuiescencePhase, DpkgRestoreRequest, DpkgStagedOperation,
    DpkgStagedPackage,
};
use crate::{
    ArtifactSlot, ArtifactVersionRelation, DataContractIdentity, DpkgPackageState, DpkgPortError,
    DpkgTransactionPort, LinuxArtifactIdentity, LinuxInstallState, LinuxInstallStore,
    LinuxOperationKind, LinuxOperationRequest, PackageSnapshot,
};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) struct TestEnvironment {
    pub(super) root: PathBuf,
    pub(super) state_root: PathBuf,
    pub(super) guard_path: PathBuf,
    pub(super) artifacts_root: PathBuf,
    pub(super) store: LinuxInstallStore,
}

impl TestEnvironment {
    pub(super) fn new(label: &str) -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root =
            Path::new("/tmp").join(format!("rlx-lp-{label}-{}-{sequence}", std::process::id()));
        fs::create_dir(&root).expect("create test root");
        let root = fs::canonicalize(&root).expect("canonicalize test root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("set test root mode");
        let metadata = fs::metadata(&root).expect("inspect test root");
        let state_root = root.join("install-v1");
        let guard_path = root.join("install-v1.lock");
        let artifacts_root = root.join("artifacts");
        fs::create_dir(&artifacts_root).expect("create artifacts root");
        fs::set_permissions(&artifacts_root, fs::Permissions::from_mode(0o700))
            .expect("set artifacts root mode");
        let store = LinuxInstallStore::bootstrap_at(
            &state_root,
            &guard_path,
            metadata.uid(),
            metadata.gid(),
        )
        .expect("bootstrap test store");
        Self {
            root,
            state_root,
            guard_path,
            artifacts_root,
            store,
        }
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub(super) struct ArtifactFixture {
    pub(super) identity: LinuxArtifactIdentity,
    pub(super) package_path: PathBuf,
    pub(super) evidence_path: PathBuf,
}

pub(super) fn artifact(
    environment: &TestEnvironment,
    version: &str,
    marker: &str,
) -> ArtifactFixture {
    let package = format!("synthetic deb payload: {marker}\n").into_bytes();
    let evidence = format!("{{\"fixture\":\"{marker}\"}}\n").into_bytes();
    let data_contract = DataContractIdentity::new(
        1,
        1,
        "linux-system-v1",
        "xdg-user-v1",
        1,
        1,
        "radishlex_pinyin",
        "11".repeat(32),
    )
    .expect("valid data contract");
    let identity = LinuxArtifactIdentity::new(
        version,
        package.len() as u64,
        evidence.len() as u64,
        sha256_bytes(&package),
        sha256_bytes(&evidence),
        "22".repeat(32),
        data_contract,
    )
    .expect("valid artifact identity");
    let directory = environment.artifacts_root.join(marker);
    fs::create_dir(&directory).expect("create artifact directory");
    let package_path = directory.join(identity.package_filename());
    let evidence_path = directory.join(identity.evidence_filename());
    write_fixture(&package_path, &package);
    write_fixture(&evidence_path, &evidence);
    ArtifactFixture {
        identity,
        package_path,
        evidence_path,
    }
}

pub(super) fn write_fixture(path: &Path, value: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .expect("create fixture");
    file.write_all(value).expect("write fixture");
    file.sync_all().expect("sync fixture");
}

pub(super) fn sha256_bytes(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug)]
pub(super) struct FakeDpkg {
    pub(super) snapshot: PackageSnapshot,
    pub(super) staged_validation_error_on_call: Option<(usize, DpkgPortErrorCode)>,
    pub(super) staged_validation_calls: usize,
    pub(super) staged_validation_states: Vec<LinuxInstallState>,
    pub(super) quiescence_error: Option<DpkgPortErrorCode>,
    pub(super) target_valid: bool,
    pub(super) source_valid: bool,
    pub(super) fail_apply_once: bool,
    pub(super) fail_restore_once: bool,
    pub(super) apply_calls: usize,
    pub(super) remove_calls: usize,
    pub(super) restore_calls: usize,
    pub(super) user_data_touches: usize,
    pub(super) next_permit_id: u64,
    pub(super) issued_permits: BTreeSet<u64>,
    pub(super) consumed_permits: Vec<u64>,
    pub(super) quiescence_phases: Vec<DpkgQuiescencePhase>,
    pub(super) first_install_recovery_target: Option<LinuxArtifactIdentity>,
}

impl FakeDpkg {
    pub(super) fn new(snapshot: PackageSnapshot) -> Self {
        Self {
            snapshot,
            staged_validation_error_on_call: None,
            staged_validation_calls: 0,
            staged_validation_states: Vec::new(),
            quiescence_error: None,
            target_valid: true,
            source_valid: true,
            fail_apply_once: false,
            fail_restore_once: false,
            apply_calls: 0,
            remove_calls: 0,
            restore_calls: 0,
            user_data_touches: 0,
            next_permit_id: 1,
            issued_permits: BTreeSet::new(),
            consumed_permits: Vec::new(),
            quiescence_phases: Vec::new(),
            first_install_recovery_target: None,
        }
    }

    fn port_error(
        code: DpkgPortErrorCode,
        phase: DpkgPortPhase,
        message: &'static str,
    ) -> DpkgPortError {
        DpkgPortError::new(code, phase, message)
    }

    fn consume_permit(&mut self, permit: FakeQuiescencePermit, phase: DpkgQuiescencePhase) {
        assert_eq!(permit.phase, phase);
        assert!(self.issued_permits.remove(&permit.id));
        assert!(!self.consumed_permits.contains(&permit.id));
        self.consumed_permits.push(permit.id);
    }
}

#[derive(Debug)]
pub(super) struct FakeQuiescencePermit {
    pub(super) id: u64,
    pub(super) phase: DpkgQuiescencePhase,
}

impl DpkgTransactionPort for FakeDpkg {
    type QuiescencePermit = FakeQuiescencePermit;

    fn validate_operation(
        &mut self,
        context: DpkgOperationContext<'_>,
    ) -> Result<(), DpkgPortError> {
        assert!(matches!(context, DpkgOperationContext::Preparing { .. }));
        Ok(())
    }

    fn inspect_package(
        &mut self,
        context: DpkgOperationContext<'_>,
    ) -> Result<PackageSnapshot, DpkgPortError> {
        if self
            .snapshot
            .artifact()
            .is_some_and(|artifact| !context.knows_artifact(artifact))
        {
            return Err(Self::port_error(
                DpkgPortErrorCode::PackageStateUnexpected,
                DpkgPortPhase::PackageInspection,
                "snapshot artifact is not known by the current operation",
            ));
        }
        Ok(self.snapshot.clone())
    }

    fn validate_staged_operation(
        &mut self,
        context: DpkgOperationContext<'_>,
        staged: DpkgStagedOperation<'_>,
    ) -> Result<(), DpkgPortError> {
        let receipt = context
            .receipt()
            .expect("staged validation requires receipt");
        self.staged_validation_calls += 1;
        self.staged_validation_states.push(receipt.state());

        let expected_slots = match receipt.operation_kind() {
            LinuxOperationKind::Install | LinuxOperationKind::Repair => (false, true),
            LinuxOperationKind::Upgrade | LinuxOperationKind::Rollback => (true, true),
            LinuxOperationKind::Remove => (true, false),
        };
        assert_eq!(staged.source().is_some(), expected_slots.0);
        assert_eq!(staged.target().is_some(), expected_slots.1);
        for package in [staged.source(), staged.target()].into_iter().flatten() {
            assert!(context.knows_artifact(package.artifact()));
            assert!(package.package_path().is_file());
            assert!(package.evidence_path().is_file());
        }

        if self
            .staged_validation_error_on_call
            .is_some_and(|(call, _)| call == self.staged_validation_calls)
        {
            let (_, code) = self
                .staged_validation_error_on_call
                .expect("matching staged failure exists");
            return Err(Self::port_error(
                code,
                DpkgPortPhase::StagedOperationValidation,
                "synthetic staged relationship failure",
            ));
        }
        Ok(())
    }

    fn prove_quiescent(
        &mut self,
        context: DpkgOperationContext<'_>,
        phase: DpkgQuiescencePhase,
    ) -> Result<Self::QuiescencePermit, DpkgPortError> {
        assert!(matches!(context, DpkgOperationContext::Resuming { .. }));
        self.quiescence_phases.push(phase);
        if let Some(code) = self.quiescence_error {
            return Err(Self::port_error(
                code,
                DpkgPortPhase::Quiescence,
                "synthetic quiescence failure",
            ));
        }
        let id = self.next_permit_id;
        self.next_permit_id += 1;
        assert!(self.issued_permits.insert(id));
        Ok(FakeQuiescencePermit { id, phase })
    }

    fn apply_package(
        &mut self,
        context: DpkgOperationContext<'_>,
        package: DpkgStagedPackage<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError> {
        assert!(matches!(context, DpkgOperationContext::Resuming { .. }));
        self.consume_permit(permit, DpkgQuiescencePhase::TargetMutation);
        assert!(package.package_path().is_file());
        assert!(package.evidence_path().is_file());
        self.apply_calls += 1;
        if self.fail_apply_once {
            self.fail_apply_once = false;
            self.snapshot = PackageSnapshot::new(
                DpkgPackageState::HalfConfigured,
                Some(package.artifact().clone()),
            )
            .expect("valid partial package state");
            return Err(Self::port_error(
                DpkgPortErrorCode::MutationFailed,
                DpkgPortPhase::TargetMutation,
                "synthetic dpkg interruption",
            ));
        }
        self.snapshot = PackageSnapshot::exact_installed(package.artifact().clone());
        self.target_valid = true;
        Ok(())
    }

    fn remove_package(
        &mut self,
        context: DpkgOperationContext<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError> {
        assert!(matches!(context, DpkgOperationContext::Resuming { .. }));
        self.consume_permit(permit, DpkgQuiescencePhase::TargetMutation);
        self.remove_calls += 1;
        self.snapshot = PackageSnapshot::absent();
        Ok(())
    }

    fn validate_product(
        &mut self,
        context: DpkgOperationContext<'_>,
        phase: DpkgProductValidationPhase,
        expected: DpkgExpectedProductState<'_>,
        snapshot: &PackageSnapshot,
    ) -> Result<(), DpkgPortError> {
        assert!(matches!(context, DpkgOperationContext::Resuming { .. }));
        let valid = match phase {
            DpkgProductValidationPhase::Target => self.target_valid,
            DpkgProductValidationPhase::Source => self.source_valid,
        };
        if valid && expected.matches_snapshot(snapshot) {
            Ok(())
        } else {
            Err(Self::port_error(
                DpkgPortErrorCode::ProductValidationFailed,
                match phase {
                    DpkgProductValidationPhase::Target => DpkgPortPhase::TargetValidation,
                    DpkgProductValidationPhase::Source => DpkgPortPhase::SourceValidation,
                },
                "synthetic product validation failure",
            ))
        }
    }

    fn restore_source(
        &mut self,
        context: DpkgOperationContext<'_>,
        request: DpkgRestoreRequest<'_>,
        permit: Self::QuiescencePermit,
    ) -> Result<(), DpkgPortError> {
        assert!(matches!(context, DpkgOperationContext::Resuming { .. }));
        self.consume_permit(permit, DpkgQuiescencePhase::SourceRestore);
        self.restore_calls += 1;
        if self.fail_restore_once {
            self.fail_restore_once = false;
            return Err(Self::port_error(
                DpkgPortErrorCode::MutationFailed,
                DpkgPortPhase::SourceRestore,
                "synthetic source restore interruption",
            ));
        }
        match request.desired_source() {
            Some(package) => {
                assert!(package.package_path().is_file());
                assert!(package.evidence_path().is_file());
                self.snapshot = PackageSnapshot::exact_installed(package.artifact().clone());
            }
            None => {
                let recovery = request
                    .recovery_target()
                    .expect("first install recovery requires the staged target");
                assert!(recovery.package_path().is_file());
                assert!(recovery.evidence_path().is_file());
                self.first_install_recovery_target = Some(recovery.artifact().clone());
                self.snapshot = PackageSnapshot::absent();
            }
        }
        self.source_valid = true;
        Ok(())
    }
}

pub(super) struct OperationCase<'a> {
    pub(super) kind: LinuxOperationKind,
    pub(super) relation: ArtifactVersionRelation,
    pub(super) source: Option<&'a ArtifactFixture>,
    pub(super) target: Option<&'a ArtifactFixture>,
}

pub(super) fn request(case: &OperationCase<'_>, operation_id: &str) -> LinuxOperationRequest {
    LinuxOperationRequest::new(
        operation_id,
        case.kind,
        case.relation,
        case.source.map(|item| item.identity.clone()),
        case.target.map(|item| item.identity.clone()),
    )
    .expect("valid operation request")
}

pub(super) fn stage_case(
    environment: &TestEnvironment,
    guard: &crate::LinuxInstallGuard,
    case: &OperationCase<'_>,
) {
    if matches!(
        case.kind,
        LinuxOperationKind::Upgrade | LinuxOperationKind::Remove | LinuxOperationKind::Rollback
    ) {
        let source = case.source.expect("source fixture");
        environment
            .store
            .stage_artifact(
                guard,
                ArtifactSlot::Source,
                &source.package_path,
                &source.evidence_path,
            )
            .expect("stage source artifact");
    }
    if case.kind != LinuxOperationKind::Remove {
        let target = case.target.expect("target fixture");
        environment
            .store
            .stage_artifact(
                guard,
                ArtifactSlot::Target,
                &target.package_path,
                &target.evidence_path,
            )
            .expect("stage target artifact");
    }
    environment
        .store
        .finish_staging(guard)
        .expect("finish artifact staging");
}
