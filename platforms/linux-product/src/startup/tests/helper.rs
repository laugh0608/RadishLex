use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::super::read_only_state::{OPERATIONS_DIRECTORY, RECEIPT_FILENAME};
use super::super::*;
use crate::model::{
    ArtifactFileIdentity, ArtifactSlot, ArtifactVersionRelation, DataContractIdentity,
    LinuxArtifactIdentity, LinuxFailureCode, LinuxInstallReceipt, LinuxInstallRootIdentity,
    LinuxInstallState, LinuxOperationKind, LinuxOperationRequest, PackageSnapshot,
    StagedArtifactEvidence,
};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) struct TestSite {
    pub(super) root: PathBuf,
    pub(super) paths: LinuxStartupPaths,
}

impl TestSite {
    pub(super) fn new(label: &str) -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = Path::new("/tmp").join(format!(
            "rlx-linux-startup-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create startup test root");
        let root = fs::canonicalize(root).expect("canonicalize startup test root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("set startup test root mode");
        let metadata = fs::metadata(&root).expect("inspect startup test root");
        let paths = LinuxStartupPaths::for_testing(root.clone(), metadata.uid(), metadata.gid());
        Self { root, paths }
    }

    pub(super) fn create_state_root(&self) -> LinuxInstallRootIdentity {
        fs::create_dir(&self.paths.state_root).expect("create state root");
        fs::set_permissions(&self.paths.state_root, fs::Permissions::from_mode(0o755))
            .expect("set state root mode");
        let operations = self.paths.state_root.join(OPERATIONS_DIRECTORY);
        fs::create_dir(&operations).expect("create operations root");
        fs::set_permissions(&operations, fs::Permissions::from_mode(0o755))
            .expect("set operations root mode");
        let metadata = fs::metadata(&self.paths.state_root).expect("inspect state root");
        LinuxInstallRootIdentity::new(
            metadata.dev(),
            metadata.ino(),
            metadata.uid(),
            metadata.gid(),
            metadata.mode() & 0o7777,
        )
        .expect("valid root identity")
    }

    pub(super) fn write_receipt(&self, receipt: &LinuxInstallReceipt) {
        let path = self.paths.state_root.join(RECEIPT_FILENAME);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(path)
            .expect("create receipt");
        file.write_all(&receipt.encode().expect("encode receipt"))
            .expect("write receipt");
        file.sync_all().expect("sync receipt");
    }

    pub(super) fn tree_fingerprint(&self) -> Vec<(String, u64, u64, u32, u64, Vec<u8>)> {
        let mut paths = vec![self.root.clone()];
        let mut index = 0;
        while index < paths.len() {
            if paths[index].is_dir() {
                let mut children: Vec<_> = fs::read_dir(&paths[index])
                    .expect("read fingerprint directory")
                    .map(|entry| entry.expect("read fingerprint entry").path())
                    .collect();
                children.sort();
                paths.extend(children);
            }
            index += 1;
        }
        paths
            .into_iter()
            .map(|path| {
                let metadata = fs::symlink_metadata(&path).expect("fingerprint metadata");
                let content = if metadata.file_type().is_file() {
                    fs::read(&path).expect("fingerprint file")
                } else {
                    Vec::new()
                };
                (
                    path.strip_prefix(&self.root)
                        .expect("fingerprint relative path")
                        .display()
                        .to_string(),
                    metadata.dev(),
                    metadata.ino(),
                    metadata.mode(),
                    metadata.mtime_nsec() as u64,
                    content,
                )
            })
            .collect()
    }
}

impl Drop for TestSite {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub(super) struct FakeStartupPort {
    package: LinuxPackageObservation,
    relationship_error: Option<LinuxStartupPortErrorCode>,
    component_error: Option<LinuxStartupPortErrorCode>,
    pub(super) package_calls: Cell<usize>,
    pub(super) relationship_calls: Cell<usize>,
    pub(super) component_calls: Cell<usize>,
}

impl FakeStartupPort {
    pub(super) fn new(package: LinuxPackageObservation) -> Self {
        Self {
            package,
            relationship_error: None,
            component_error: None,
            package_calls: Cell::new(0),
            relationship_calls: Cell::new(0),
            component_calls: Cell::new(0),
        }
    }

    pub(super) fn with_relationship_error(mut self, code: LinuxStartupPortErrorCode) -> Self {
        self.relationship_error = Some(code);
        self
    }

    pub(super) fn with_component_error(mut self, code: LinuxStartupPortErrorCode) -> Self {
        self.component_error = Some(code);
        self
    }
}

impl LinuxStartupPort for FakeStartupPort {
    fn inspect_package(&self) -> Result<LinuxPackageObservation, LinuxStartupPortError> {
        self.package_calls.set(self.package_calls.get() + 1);
        Ok(self.package.clone())
    }

    fn validate_package_relationship(
        &self,
        _receipt: &LinuxInstallReceipt,
        _artifact: &LinuxArtifactIdentity,
    ) -> Result<(), LinuxStartupPortError> {
        self.relationship_calls
            .set(self.relationship_calls.get() + 1);
        match self.relationship_error {
            Some(code) => Err(LinuxStartupPortError::new(
                code,
                "synthetic package relationship failure",
            )),
            None => Ok(()),
        }
    }

    fn validate_component(
        &self,
        _component: LinuxStartupComponent,
        _artifact: &LinuxArtifactIdentity,
    ) -> Result<(), LinuxStartupPortError> {
        self.component_calls.set(self.component_calls.get() + 1);
        match self.component_error {
            Some(code) => Err(LinuxStartupPortError::new(
                code,
                "synthetic component failure",
            )),
            None => Ok(()),
        }
    }
}

pub(super) fn artifact(version: &str, marker: u8) -> LinuxArtifactIdentity {
    artifact_with_manifest_hash(version, marker, format!("{marker:02x}").repeat(32))
}

pub(super) fn artifact_with_manifest_hash(
    version: &str,
    marker: u8,
    manifest_sha256: String,
) -> LinuxArtifactIdentity {
    let package = vec![marker; 23];
    let evidence = vec![marker.wrapping_add(1); 19];
    LinuxArtifactIdentity::new(
        version,
        package.len() as u64,
        evidence.len() as u64,
        sha256(&package),
        sha256(&evidence),
        manifest_sha256,
        DataContractIdentity::new(
            9,
            9,
            "debian-system-v1",
            "xdg-v1",
            1,
            1,
            "radishlex_pinyin",
            "55".repeat(32),
        )
        .expect("valid startup data contract"),
    )
    .expect("valid startup artifact")
}

pub(super) fn write_mode(path: &Path, value: &[u8], mode: u32) {
    fs::create_dir_all(path.parent().expect("file parent")).expect("create file parent");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .expect("create identity fixture");
    file.write_all(value).expect("write identity fixture");
    file.sync_all().expect("sync identity fixture");
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set identity fixture mode");
}

pub(super) fn manifest_file_record(
    path: &Path,
    component_id: &str,
    mode: u32,
    owner_id: u32,
    group_id: u32,
) -> Value {
    let value = fs::read(path).expect("read manifest fixture file");
    json!({
        "component_id": component_id,
        "gid": group_id,
        "mode": format!("{mode:04o}"),
        "path": path.to_str().expect("UTF-8 manifest path"),
        "sha256": sha256(&value),
        "size": value.len(),
        "uid": owner_id,
    })
}

pub(super) fn sha256(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn staged_evidence(
    slot: ArtifactSlot,
    artifact: &LinuxArtifactIdentity,
    package_path: &Path,
    evidence_path: &Path,
) -> StagedArtifactEvidence {
    StagedArtifactEvidence::new(
        slot,
        artifact.clone(),
        artifact_file_identity(package_path),
        artifact_file_identity(evidence_path),
    )
    .expect("valid staged evidence")
}

fn artifact_file_identity(path: &Path) -> ArtifactFileIdentity {
    let metadata = fs::symlink_metadata(path).expect("inspect staged startup fixture");
    ArtifactFileIdentity::new(
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.gid(),
        metadata.mode() & 0o7777,
        metadata.nlink(),
        metadata.len(),
        sha256(&fs::read(path).expect("read staged startup fixture")),
    )
    .expect("valid staged startup fixture identity")
}

fn fixture_bytes(size: u64, expected_sha256: &str) -> Vec<u8> {
    (0_u8..=u8::MAX)
        .map(|marker| vec![marker; size as usize])
        .find(|value| sha256(value) == expected_sha256)
        .expect("artifact fixture digest belongs to a repeated-byte payload")
}

pub(super) fn staged_paths(site: &TestSite, slot: ArtifactSlot) -> (PathBuf, PathBuf) {
    let prefix = match slot {
        ArtifactSlot::Source => "source",
        ArtifactSlot::Target => "target",
    };
    let operation = site
        .paths
        .state_root
        .join(OPERATIONS_DIRECTORY)
        .join("11111111111111111111111111111111");
    (
        operation.join(format!("{prefix}.deb")),
        operation.join(format!("{prefix}.evidence.json")),
    )
}

pub(super) fn prepared_receipt(
    root_identity: LinuxInstallRootIdentity,
    operation: LinuxOperationKind,
    source: Option<LinuxArtifactIdentity>,
    target: Option<LinuxArtifactIdentity>,
) -> LinuxInstallReceipt {
    prepared_receipt_with_chain(
        root_identity,
        operation,
        source,
        target,
        vec!["11111111111111111111111111111111".to_owned()],
    )
}

pub(super) fn prepared_receipt_with_chain(
    root_identity: LinuxInstallRootIdentity,
    operation: LinuxOperationKind,
    source: Option<LinuxArtifactIdentity>,
    target: Option<LinuxArtifactIdentity>,
    operation_chain: Vec<String>,
) -> LinuxInstallReceipt {
    let relation = match operation {
        LinuxOperationKind::Install | LinuxOperationKind::Remove => {
            ArtifactVersionRelation::NotApplicable
        }
        LinuxOperationKind::Upgrade => ArtifactVersionRelation::TargetNewer,
        LinuxOperationKind::Repair => ArtifactVersionRelation::SameRelease,
        LinuxOperationKind::Rollback => ArtifactVersionRelation::TargetOlder,
    };
    let initial = source
        .as_ref()
        .map_or_else(PackageSnapshot::absent, |artifact| {
            PackageSnapshot::exact_installed(artifact.clone())
        });
    LinuxInstallReceipt::from_request(
        LinuxOperationRequest::new(
            "11111111111111111111111111111111",
            operation,
            relation,
            source,
            target,
        )
        .expect("valid startup operation"),
        operation_chain,
        root_identity,
        initial,
    )
    .expect("valid prepared receipt")
}

pub(super) fn stage_required(site: &TestSite, receipt: &mut LinuxInstallReceipt) {
    let operation_directory = site
        .paths
        .state_root
        .join(OPERATIONS_DIRECTORY)
        .join(receipt.operation_id());
    fs::create_dir_all(&operation_directory).expect("create staged operation directory");
    fs::set_permissions(&operation_directory, fs::Permissions::from_mode(0o700))
        .expect("set staged operation directory mode");
    if matches!(
        receipt.operation_kind(),
        LinuxOperationKind::Upgrade | LinuxOperationKind::Remove | LinuxOperationKind::Rollback
    ) {
        let source = receipt.source_artifact().expect("source artifact").clone();
        let (package_path, evidence_path) = staged_paths(site, ArtifactSlot::Source);
        write_mode(
            &package_path,
            &fixture_bytes(source.package_size(), source.package_sha256()),
            0o600,
        );
        write_mode(
            &evidence_path,
            &fixture_bytes(source.evidence_size(), source.evidence_sha256()),
            0o600,
        );
        receipt
            .record_staged_artifact(staged_evidence(
                ArtifactSlot::Source,
                &source,
                &package_path,
                &evidence_path,
            ))
            .expect("stage source evidence");
    }
    if receipt.operation_kind() != LinuxOperationKind::Remove {
        let target = receipt.target_artifact().expect("target artifact").clone();
        let (package_path, evidence_path) = staged_paths(site, ArtifactSlot::Target);
        write_mode(
            &package_path,
            &fixture_bytes(target.package_size(), target.package_sha256()),
            0o600,
        );
        write_mode(
            &evidence_path,
            &fixture_bytes(target.evidence_size(), target.evidence_sha256()),
            0o600,
        );
        receipt
            .record_staged_artifact(staged_evidence(
                ArtifactSlot::Target,
                &target,
                &package_path,
                &evidence_path,
            ))
            .expect("stage target evidence");
    }
}

pub(super) fn stage_exact_bytes(
    site: &TestSite,
    receipt: &mut LinuxInstallReceipt,
    slot: ArtifactSlot,
    package: &[u8],
    evidence: &[u8],
) {
    let operation_directory = site
        .paths
        .state_root
        .join(OPERATIONS_DIRECTORY)
        .join(receipt.operation_id());
    fs::create_dir_all(&operation_directory).expect("create staged operation directory");
    fs::set_permissions(&operation_directory, fs::Permissions::from_mode(0o700))
        .expect("set staged operation directory mode");
    let artifact = receipt
        .artifact_for_slot(slot)
        .expect("artifact for exact staged slot")
        .clone();
    let (package_path, evidence_path) = staged_paths(site, slot);
    write_mode(&package_path, package, 0o600);
    write_mode(&evidence_path, evidence, 0o600);
    receipt
        .record_staged_artifact(staged_evidence(
            slot,
            &artifact,
            &package_path,
            &evidence_path,
        ))
        .expect("record exact staged artifact");
}

pub(super) fn advance_to(
    site: &TestSite,
    receipt: &mut LinuxInstallReceipt,
    target: LinuxInstallState,
) {
    stage_required(site, receipt);
    if target == LinuxInstallState::Prepared {
        return;
    }
    receipt
        .advance(LinuxInstallState::ArtifactsStaged)
        .expect("advance artifacts staged");
    if target == LinuxInstallState::ArtifactsStaged {
        return;
    }
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("advance quiesced");
    if target == LinuxInstallState::Quiesced {
        return;
    }
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("advance package mutating");
    if target == LinuxInstallState::PackageMutating {
        return;
    }
    let target_proof = receipt
        .target_artifact()
        .map_or_else(PackageSnapshot::absent, |artifact| {
            PackageSnapshot::exact_installed(artifact.clone())
        });
    receipt
        .record_target_proof(target_proof)
        .expect("record target proof");
    receipt
        .advance(LinuxInstallState::PackageVerified)
        .expect("advance package verified");
    if target == LinuxInstallState::PackageVerified {
        return;
    }
    receipt
        .advance(LinuxInstallState::Completed)
        .expect("advance completed");
}

pub(super) fn rolled_back_receipt(
    site: &TestSite,
    root_identity: LinuxInstallRootIdentity,
    source: Option<LinuxArtifactIdentity>,
    target: LinuxArtifactIdentity,
) -> LinuxInstallReceipt {
    let operation = if source.is_some() {
        LinuxOperationKind::Upgrade
    } else {
        LinuxOperationKind::Install
    };
    let mut receipt = prepared_receipt(root_identity, operation, source, Some(target));
    stage_required(site, &mut receipt);
    for state in [
        LinuxInstallState::ArtifactsStaged,
        LinuxInstallState::Quiesced,
        LinuxInstallState::PackageMutating,
    ] {
        receipt.advance(state).expect("advance failed operation");
    }
    receipt
        .require_rollback(LinuxFailureCode::PackageMutationFailed)
        .expect("require rollback");
    receipt
        .advance(LinuxInstallState::SourceRestoring)
        .expect("advance source restoring");
    let source_proof = receipt
        .source_artifact()
        .map_or_else(PackageSnapshot::absent, |artifact| {
            PackageSnapshot::exact_installed(artifact.clone())
        });
    receipt
        .record_source_proof(source_proof)
        .expect("record source proof");
    receipt
        .advance(LinuxInstallState::SourceVerified)
        .expect("advance source verified");
    receipt.finish_rollback().expect("finish rollback");
    receipt
}
