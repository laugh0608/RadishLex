use serde_json::{json, Value};

use crate::{ArtifactVersionRelation, LinuxOperationKind};

use super::super::*;
use super::helper::*;

#[test]
fn artifact_relationship_is_derived_from_all_immutable_inputs() {
    let fixture = ArtifactFixture::new("26.7.1+38-1");
    let verified = fixture.verify();
    assert_eq!(verified.package_name(), "radishlex");
    assert_eq!(verified.package_version(), "26.7.1+38-1");
    assert_eq!(verified.architecture(), "arm64");
    assert_eq!(verified.package_size(), fixture.package.len() as u64);
    assert_eq!(verified.package_sha256(), sha256_bytes(&fixture.package));
    assert_eq!(verified.evidence_size(), fixture.evidence.len() as u64);
    assert_eq!(verified.evidence_sha256(), sha256_bytes(&fixture.evidence));
    assert_eq!(verified.dependencies().len(), DEPENDENCIES.len());
}

#[test]
fn artifact_relationship_accepts_adjacent_positive_debian_revisions() {
    let source = ArtifactFixture::new("26.7.1+38-1").verify();
    let target = ArtifactFixture::new("26.7.1+38-2").verify();
    assert_eq!(source.data_contract(), target.data_contract());
    assert_eq!(
        validate_operation_relation(LinuxOperationKind::Upgrade, Some(&source), Some(&target))
            .expect("adjacent target revision must be upgrade-compatible"),
        ArtifactVersionRelation::TargetNewer
    );
}

#[test]
fn verified_relationship_bridges_only_the_exact_linux_artifact_identity() {
    let verified = ArtifactFixture::new("26.7.1+38-1").verify();
    let exact = verified
        .to_linux_artifact_identity()
        .expect("bridge Linux artifact identity");
    assert!(verified
        .matches_linux_artifact_identity(&exact)
        .expect("compare exact identity"));

    let version_drift = LinuxArtifactIdentity::new(
        "26.7.1+39-1",
        exact.package_size(),
        exact.evidence_size(),
        exact.package_sha256(),
        exact.evidence_sha256(),
        exact.product_manifest_sha256(),
        exact.data_contract().clone(),
    )
    .expect("build version drift identity");
    assert!(!verified
        .matches_linux_artifact_identity(&version_drift)
        .expect("compare version drift"));

    for (package_sha256, evidence_sha256, manifest_sha256) in [
        (
            "55".repeat(32),
            exact.evidence_sha256().to_owned(),
            exact.product_manifest_sha256().to_owned(),
        ),
        (
            exact.package_sha256().to_owned(),
            "66".repeat(32),
            exact.product_manifest_sha256().to_owned(),
        ),
        (
            exact.package_sha256().to_owned(),
            exact.evidence_sha256().to_owned(),
            "77".repeat(32),
        ),
    ] {
        let hash_drift = LinuxArtifactIdentity::new(
            exact.package_version(),
            exact.package_size(),
            exact.evidence_size(),
            package_sha256,
            evidence_sha256,
            manifest_sha256,
            exact.data_contract().clone(),
        )
        .expect("build hash drift identity");
        assert!(!verified
            .matches_linux_artifact_identity(&hash_drift)
            .expect("compare hash drift"));
    }

    let contract = exact.data_contract();
    let contract_drift = DataContractIdentity::new(
        contract.ffi_abi_version() + 1,
        contract.userdb_schema_version(),
        contract.runtime_layout(),
        contract.data_layout(),
        contract.settings_format_version(),
        contract.privacy_format_version(),
        contract.rime_schema_id(),
        contract.rime_data_lock_sha256(),
    )
    .expect("build data contract drift");
    let contract_drift = LinuxArtifactIdentity::new(
        exact.package_version(),
        exact.package_size(),
        exact.evidence_size(),
        exact.package_sha256(),
        exact.evidence_sha256(),
        exact.product_manifest_sha256(),
        contract_drift,
    )
    .expect("build contract drift identity");
    assert!(!verified
        .matches_linux_artifact_identity(&contract_drift)
        .expect("compare data contract drift"));
}

#[test]
fn artifact_evidence_rejects_unknown_fields_and_noncanonical_json() {
    let fixture = ArtifactFixture::new("26.7.1+38-1");
    let mut unknown: Value = serde_json::from_slice(&fixture.evidence).expect("parse evidence");
    unknown["unexpected"] = json!(true);
    let mut unknown_bytes = serde_json::to_vec_pretty(&unknown).expect("encode unknown evidence");
    unknown_bytes.push(b'\n');
    let mut input = fixture.input();
    input.evidence_bytes = &unknown_bytes;
    assert_eq!(
        VerifiedArtifactRelationship::verify(input)
            .expect_err("unknown evidence field must fail")
            .code(),
        DebianRelationshipErrorCode::ArtifactEvidenceInvalid
    );

    let compact = serde_json::to_vec(&serde_json::from_slice::<Value>(&fixture.evidence).unwrap())
        .expect("compact evidence");
    input.evidence_bytes = &compact;
    assert_eq!(
        VerifiedArtifactRelationship::verify(input)
            .expect_err("noncanonical evidence must fail")
            .code(),
        DebianRelationshipErrorCode::ArtifactEvidenceInvalid
    );
}

#[test]
fn product_manifest_rejects_unknown_fields_and_control_identity_drift() {
    let fixture = ArtifactFixture::new("26.7.1+38-1");
    let mut unknown: Value = serde_json::from_slice(&fixture.manifest).expect("parse manifest");
    unknown["unexpected"] = json!(true);
    let (unknown_manifest, unknown_evidence) = changed_manifest_inputs(&fixture, &unknown);
    let mut input = fixture.input();
    input.product_manifest_bytes = &unknown_manifest;
    input.evidence_bytes = &unknown_evidence;
    assert_eq!(
        VerifiedArtifactRelationship::verify(input)
            .expect_err("unknown product manifest field must fail")
            .code(),
        DebianRelationshipErrorCode::ProductManifestInvalid
    );

    let mut drift: Value = serde_json::from_slice(&fixture.manifest).expect("parse manifest");
    drift["product"]["package_version"] = json!("26.7.1+39-1");
    let (drift_manifest, drift_evidence) = changed_manifest_inputs(&fixture, &drift);
    input = fixture.input();
    input.product_manifest_bytes = &drift_manifest;
    input.evidence_bytes = &drift_evidence;
    assert_eq!(
        VerifiedArtifactRelationship::verify(input)
            .expect_err("manifest and binary control identity drift must fail")
            .code(),
        DebianRelationshipErrorCode::ProductManifestInvalid
    );

    for (field, value) in [("product_version", "26.7.2"), ("build_number", "39")] {
        let mut invalid: Value = serde_json::from_slice(&fixture.manifest).expect("parse manifest");
        invalid["product"][field] = json!(value);
        assert_manifest_error(&fixture, &invalid);
    }

    for ffi_abi_version in [8, 10] {
        let invalid = ArtifactFixture::with_ffi_abi("26.7.1+38-1", ffi_abi_version);
        assert_eq!(
            VerifiedArtifactRelationship::verify(invalid.input())
                .expect_err("Debian 13 profile fixes FFI ABI 9")
                .code(),
            DebianRelationshipErrorCode::ProductManifestInvalid
        );
    }
    for value in [8, 10] {
        let mut invalid: Value = serde_json::from_slice(&fixture.manifest).expect("parse manifest");
        invalid["product"]["userdb_schema_version"] = json!(value);
        assert_manifest_error(&fixture, &invalid);
    }
}

#[test]
fn product_manifest_rejects_invalid_inventory_relationships() {
    let fixture = ArtifactFixture::new("26.7.1+38-1");
    let base: Value = serde_json::from_slice(&fixture.manifest).expect("parse manifest");

    let mut unknown_component = base.clone();
    unknown_component["files"][0]["component_id"] = json!("unknown-component");
    assert_manifest_error(&fixture, &unknown_component);

    let mut unsafe_owner = base.clone();
    unsafe_owner["files"][0]["uid"] = json!(1);
    assert_manifest_error(&fixture, &unsafe_owner);

    let mut extra_directory = base.clone();
    extra_directory["directories"]
        .as_array_mut()
        .expect("directory array")
        .insert(
            0,
            json!({"gid": 0, "mode": "0755", "path": "/etc", "uid": 0}),
        );
    assert_manifest_error(&fixture, &extra_directory);

    let mut unsorted_files = base.clone();
    unsorted_files["files"]
        .as_array_mut()
        .expect("file array")
        .swap(0, 1);
    assert_manifest_error(&fixture, &unsorted_files);

    let mut missing_fixed = base.clone();
    let fixed_directories = {
        let files = missing_fixed["files"].as_array_mut().expect("file array");
        files.retain(|record| record["component_id"] != "manager-desktop-entry");
        manifest_directories(files)
    };
    missing_fixed["directories"] = json!(fixed_directories);
    assert_manifest_error(&fixture, &missing_fixed);

    let mut missing_rime = base.clone();
    let rime_directories = {
        let files = missing_rime["files"].as_array_mut().expect("file array");
        files.retain(|record| record["component_id"] != "rime-data");
        manifest_directories(files)
    };
    missing_rime["directories"] = json!(rime_directories);
    assert_manifest_error(&fixture, &missing_rime);

    let mut extra_font = base.clone();
    let font_directories = {
        let files = extra_font["files"].as_array_mut().expect("file array");
        files.insert(
            3,
            manifest_file(
                "manager-bundle",
                "0644",
                "/usr/lib/aarch64-linux-gnu/radishlex/manager/data/flutter_assets/fonts/Unexpected.ttf",
                &"bb".repeat(32),
            ),
        );
        manifest_directories(files)
    };
    extra_font["directories"] = json!(font_directories);
    assert_manifest_error(&fixture, &extra_font);

    let mut ffi_drift = base;
    ffi_drift["ffi_equivalence"]["sha256"] = json!("cc".repeat(32));
    assert_manifest_error(&fixture, &ffi_drift);
}

#[test]
fn artifact_relationship_rejects_control_manifest_and_filename_drift() {
    let fixture = ArtifactFixture::new("26.7.1+38-1");
    let control_drift = fixture
        .control
        .replace(b"fcitx5 (>= 5.1.9)", b"fcitx5 (>= 5.1.8)");
    let mut input = fixture.input();
    input.binary_control_bytes = &control_drift;
    assert_eq!(
        VerifiedArtifactRelationship::verify(input)
            .expect_err("control bytes must bind evidence")
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let manifest_drift = b"{\"package_version\":\"26.7.1+39-1\"}\n";
    input = fixture.input();
    input.product_manifest_bytes = manifest_drift;
    assert_eq!(
        VerifiedArtifactRelationship::verify(input)
            .expect_err("manifest bytes must bind evidence")
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    input = fixture.input();
    input.package_filename = "radishlex_latest_arm64.deb";
    assert_eq!(
        VerifiedArtifactRelationship::verify(input)
            .expect_err("package filename must bind control")
            .code(),
        DebianRelationshipErrorCode::PackageIdentityMismatch
    );
}

#[test]
fn artifact_relationship_rejects_softened_or_unsupported_dependencies() {
    let without_font =
        ArtifactFixture::with_dependencies("26.7.1+38-1", &DEPENDENCIES[..DEPENDENCIES.len() - 1]);
    assert_eq!(
        VerifiedArtifactRelationship::verify(without_font.input())
            .expect_err("missing font dependency must fail")
            .code(),
        DebianRelationshipErrorCode::DependencyDeclarationInvalid
    );

    assert_eq!(
        DirectDependency::parse("libc6 (>= 2.34) | libc6.1")
            .expect_err("dependency alternatives are outside profile")
            .code(),
        DebianRelationshipErrorCode::DependencyDeclarationInvalid
    );
    assert_eq!(
        DirectDependency::parse("libc6:any (>= 2.34)")
            .expect_err("ambiguous architecture qualifiers are outside profile")
            .code(),
        DebianRelationshipErrorCode::DependencyDeclarationInvalid
    );
}

#[test]
fn operation_relation_is_computed_instead_of_trusting_a_label() {
    let old = ArtifactFixture::new("26.7.1+38-1").verify();
    let new = ArtifactFixture::new("26.7.1+39-1").verify();
    assert_eq!(
        validate_operation_relation(LinuxOperationKind::Upgrade, Some(&old), Some(&new)).unwrap(),
        ArtifactVersionRelation::TargetNewer
    );
    assert_eq!(
        validate_operation_relation(LinuxOperationKind::Rollback, Some(&new), Some(&old)).unwrap(),
        ArtifactVersionRelation::TargetOlder
    );
    assert_eq!(
        validate_operation_relation(LinuxOperationKind::Repair, Some(&old), Some(&old)).unwrap(),
        ArtifactVersionRelation::SameRelease
    );
    assert_eq!(
        validate_operation_relation(LinuxOperationKind::Upgrade, Some(&new), Some(&old))
            .expect_err("old target cannot be called an upgrade")
            .code(),
        DebianRelationshipErrorCode::VersionRelationInvalid
    );

    let same_version_different_bytes = ArtifactFixture::new("26.7.1+38-1");
    let mut changed_package = same_version_different_bytes.package.clone();
    changed_package.extend_from_slice(b"different\n");
    let mut evidence: Value =
        serde_json::from_slice(&same_version_different_bytes.evidence).unwrap();
    evidence["package"]["size"] = json!(changed_package.len());
    evidence["package"]["sha256"] = json!(sha256_bytes(&changed_package));
    let mut changed_evidence = serde_json::to_vec_pretty(&evidence).unwrap();
    changed_evidence.push(b'\n');
    let changed_content = PackageContentIdentity::from_stable_digest(
        changed_package.len() as u64,
        sha256_bytes(&changed_package),
    )
    .expect("build changed package content identity");
    let changed = VerifiedArtifactRelationship::verify(ArtifactRelationshipInput {
        package_filename: &same_version_different_bytes.package_filename,
        evidence_filename: &same_version_different_bytes.evidence_filename,
        package_content: &changed_content,
        binary_control_bytes: &same_version_different_bytes.control,
        product_manifest_bytes: &same_version_different_bytes.manifest,
        evidence_bytes: &changed_evidence,
    })
    .expect("verify changed same-version package");
    assert_eq!(
        validate_operation_relation(LinuxOperationKind::Repair, Some(&old), Some(&changed))
            .expect_err("same version with different bytes is not repair")
            .code(),
        DebianRelationshipErrorCode::VersionRelationInvalid
    );
}

#[test]
fn operation_relation_rejects_product_data_contract_drift() {
    let source = ArtifactFixture::with_rime_lock("26.7.1+38-1", &"44".repeat(32)).verify();
    let target = ArtifactFixture::with_rime_lock("26.7.1+39-1", &"45".repeat(32)).verify();
    assert_eq!(
        validate_operation_relation(LinuxOperationKind::Upgrade, Some(&source), Some(&target),)
            .expect_err("upgrade cannot cross an unplanned data contract change")
            .code(),
        DebianRelationshipErrorCode::VersionRelationInvalid
    );
}
