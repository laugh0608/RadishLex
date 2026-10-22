use super::*;
use std::io::Cursor;

#[test]
fn synthetic_product_identity_is_complete_and_repeatable() {
    let source = SyntheticProofSession::synthetic_source().unwrap();
    assert_eq!(source, SyntheticProofSession::synthetic_source().unwrap());
    assert_eq!(source.release().build_number(), 39);
    assert_ne!(
        source.program(ProgramComponent::Manager),
        source.program(ProgramComponent::InputMethod)
    );
}

#[test]
fn core_hasher_reserves_before_reads_and_stops_on_control_loss() {
    let mut charged = 0u64;
    {
        let mut allow = |bytes| {
            charged += bytes;
            true
        };
        let hasher = CoreHasher {
            check: RefCell::new(&mut allow),
        };
        let actual = hasher
            .sha256(&mut Cursor::new(b"synthetic control bytes"))
            .unwrap();
        assert_eq!(
            actual,
            <[u8; 32]>::from(Sha256::digest(b"synthetic control bytes"))
        );
    }
    assert_eq!(charged, 2 * 64 * 1024);
    let mut source = Cursor::new(b"unread");
    let mut deny = |_| false;
    let denied = CoreHasher {
        check: RefCell::new(&mut deny),
    };
    assert_eq!(
        denied.sha256(&mut source).unwrap_err().kind(),
        std::io::ErrorKind::Interrupted
    );
    assert_eq!(source.position(), 0);
}

/// The one batch test invokes this against each real current proof. No second
/// scratch root or unregistered source object is created by binding negatives.
pub(crate) fn check_binding_negatives(session: &SyntheticProofSession) {
    let call_id = u64::MAX - 1;
    let mut binding = session.bind(call_id).unwrap();
    assert_eq!(session.bind(call_id).unwrap_err(), ProofError::Replay);
    assert_eq!(
        session.validate_binding(&binding, call_id + 1),
        Err(ProofError::Replay)
    );
    let canonical = binding.canonical_proof.clone();
    binding.canonical_proof.push(b' ');
    assert_eq!(
        session.validate_binding(&binding, call_id),
        Err(ProofError::EvidenceChanged)
    );
    binding.canonical_proof = canonical;
    let source = binding.source.clone();
    binding.source = with_manifest(&source, b"different source manifest same version");
    assert_eq!(
        session.validate_binding(&binding, call_id),
        Err(ProofError::EvidenceChanged)
    );
    binding.source = source;
    let target = binding.target.clone();
    binding.target = with_manifest(&target, b"different target manifest same version");
    assert_eq!(
        session.validate_binding(&binding, call_id),
        Err(ProofError::EvidenceChanged)
    );
    binding.target = target;
    binding.family.database.inode += 1;
    assert_eq!(
        session.validate_binding(&binding, call_id),
        Err(ProofError::EvidenceChanged)
    );
    binding.family.database.inode -= 1;
    assert_eq!(session.validate_binding(&binding, call_id), Ok(()));
    assert_eq!(session.consume_binding(&binding, call_id), Ok(()));
    assert_eq!(
        session.consume_binding(&binding, call_id),
        Err(ProofError::Replay)
    );
    assert!(session.marker().exists());
}

fn with_manifest(identity: &ProductArtifactIdentity, bytes: &[u8]) -> ProductArtifactIdentity {
    ProductArtifactIdentity::new(
        identity.product_id(),
        identity.release().clone(),
        digest(bytes),
        identity.program(ProgramComponent::Manager).clone(),
        identity.program(ProgramComponent::InputMethod).clone(),
    )
    .unwrap()
}

#[test]
fn exact_creation_plans_include_history_and_never_claim_family_files() {
    for kind in [
        SyntheticPredecessor::NoIndex,
        SyntheticPredecessor::LegacyData,
        SyntheticPredecessor::TerminalV1,
        SyntheticPredecessor::TerminalV2,
        SyntheticPredecessor::CancellationV2,
        SyntheticPredecessor::RepeatedCancellationV2,
    ] {
        let paths = expected_paths(kind);
        assert!(paths.len() < 128);
        assert!(paths.iter().all(|path| path.components().count() <= 8));
        assert!(!paths
            .iter()
            .any(|path| path.file_name().unwrap() == "userdb.sqlite3"));
        assert!(paths.contains(Path::new(
            ".radishlex-upgrade-v1/preparation-cancellation.json"
        )));
    }
}

#[test]
fn historical_receipt_requires_declared_schema_with_database_artifact() {
    let root =
        UpgradeArtifactIdentity::private_directory(UpgradeArtifactSlot::DataRoot, 1, 2, 501, 2)
            .unwrap();
    let database =
        UpgradeArtifactIdentity::private_file(UpgradeArtifactSlot::SourceDatabase, 1, 3, 501, 4096)
            .unwrap();
    let source = DataRelease::new("0.1.0", 39).unwrap();
    let target = DataRelease::new("0.1.0", 41).unwrap();
    assert!(UpgradeReceipt::new(
        DATA,
        None,
        source.clone(),
        target.clone(),
        None,
        9,
        vec![root.clone(), database.clone()],
    )
    .is_err());
    let mut historical = UpgradeReceipt::new(
        DATA,
        None,
        source,
        target,
        Some(SYNTHETIC_HISTORICAL_SCHEMA),
        9,
        vec![root, database],
    )
    .unwrap();
    historical
        .abort_preserved(UpgradeFailureCode::ProcessNotQuiescent, false)
        .unwrap();
    assert_eq!(
        historical.source_schema_version(),
        Some(SYNTHETIC_HISTORICAL_SCHEMA)
    );
}
