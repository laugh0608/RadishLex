use std::mem::ManuallyDrop;

use super::helper::{artifact, request, stage_case, FakeDpkg, OperationCase, TestEnvironment};
use crate::{
    prepare_operation, resume_operation, ArtifactVersionRelation, LinuxArtifactIdentity,
    LinuxInstallReceipt, LinuxOperationKind, PackageSnapshot,
};

fn completed_seed() -> (LinuxInstallReceipt, LinuxArtifactIdentity) {
    // These new synthetic materials deliberately survive success, failure and panic.
    let environment = ManuallyDrop::new(TestEnvironment::new("source-seed"));
    eprintln!(
        "source seed synthetic materials retained: {}",
        environment.root.display()
    );
    let source = artifact(&environment, "26.7.1+40-2", "source");
    let case = OperationCase {
        kind: LinuxOperationKind::Install,
        relation: ArtifactVersionRelation::NotApplicable,
        source: None,
        target: Some(&source),
    };
    let mut port = FakeDpkg::new(PackageSnapshot::absent());
    let guard = environment.store.acquire_guard().expect("synthetic guard");
    prepare_operation(
        &environment.store,
        &guard,
        request(&case, "10000000000000000000000000000001"),
        &mut port,
    )
    .expect("synthetic prepare");
    stage_case(&environment, &guard, &case);
    resume_operation(&environment.store, &guard, &mut port).expect("synthetic complete");
    assert_eq!(port.user_data_touches, 0);
    let receipt = environment.store.load_receipt().unwrap().unwrap();
    (receipt, source.identity)
}

#[test]
fn source_seed_fresh_receipt_projects_full_target_without_private_ids() {
    let (receipt, source) = completed_seed();
    let decoded = LinuxInstallReceipt::decode(&receipt.encode().unwrap()).unwrap();
    let facts = decoded.source_seed_facts(&source).unwrap();
    assert_eq!(facts["state"], "completed");
    assert_eq!(facts["chain_length"], 1);
    assert_eq!(facts["initial_package"]["state"], "not_installed");
    assert_eq!(
        facts["target_artifact"],
        serde_json::to_value(&source).unwrap()
    );
    assert_eq!(facts["target_proof"]["artifact"], facts["target_artifact"]);
    assert!(facts.get("operation_id").is_none());
    assert!(facts.get("root_identity").is_none());
}

#[test]
fn source_seed_wrong_artifact_and_incomplete_receipt_rejected() {
    let (receipt, source) = completed_seed();
    let mut wrong = serde_json::to_value(&source).unwrap();
    wrong["product_manifest_sha256"] = serde_json::json!("44".repeat(32));
    let wrong = serde_json::from_value(wrong).unwrap();
    assert!(receipt.source_seed_facts(&wrong).is_err());
    let mut incomplete = serde_json::to_value(&receipt).unwrap();
    incomplete["state"] = serde_json::json!("prepared");
    incomplete["target_proof"] = serde_json::Value::Null;
    // It may fail the underlying receipt invariant even before the seed-specific gate.
    let incomplete: LinuxInstallReceipt = serde_json::from_value(incomplete).unwrap();
    assert!(incomplete.source_seed_facts(&source).is_err());
}

#[test]
fn source_seed_previous_chain_cannot_be_presented_as_first_install() {
    let (receipt, source) = completed_seed();
    let mut value = serde_json::to_value(&receipt).unwrap();
    value["operation_chain"] =
        serde_json::json!(["20000000000000000000000000000002", receipt.operation_id()]);
    let previous: LinuxInstallReceipt = serde_json::from_value(value).unwrap();
    assert!(previous.source_seed_facts(&source).is_err());
}

#[test]
fn source_seed_residual_package_configuration_is_not_fresh() {
    let (receipt, source) = completed_seed();
    let mut value = serde_json::to_value(&receipt).unwrap();
    value["initial_package"]["state"] = serde_json::json!("config_files");
    let residual: LinuxInstallReceipt = serde_json::from_value(value).unwrap();
    assert!(residual.source_seed_facts(&source).is_err());
}

#[test]
fn source_seed_raw_receipt_requires_original_canonical_bytes() {
    let (receipt, _) = completed_seed();
    let raw = receipt.encode().unwrap();
    let mut altered = raw.clone();
    altered.insert(0, b' ');
    assert!(LinuxInstallReceipt::decode(&altered).is_err());
    let value: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let pretty = serde_json::to_vec_pretty(&value).unwrap();
    assert!(LinuxInstallReceipt::decode(&pretty).is_err());
}
