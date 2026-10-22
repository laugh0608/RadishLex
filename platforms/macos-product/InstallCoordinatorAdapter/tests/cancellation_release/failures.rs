use super::*;

#[test]
fn release_bridge_rejects_wrong_root_guard_and_product_before_callbacks() {
    let fixture = Preserved::new(false, true);
    let proof = fixture.prepare();
    let foreign = Preserved::new(false, false);
    let wrong_source = product(39, 'c');
    let wrong_target = product(41, 'c');
    for (install, guard, source, target) in [
        (
            &foreign.install,
            &foreign.guard,
            &fixture.source,
            &fixture.target,
        ),
        (
            &fixture.install,
            &foreign.guard,
            &fixture.source,
            &fixture.target,
        ),
        (
            &fixture.install,
            &fixture.guard,
            &wrong_source,
            &fixture.target,
        ),
        (
            &fixture.install,
            &fixture.guard,
            &fixture.source,
            &wrong_target,
        ),
    ] {
        let mut product = ReleaseProducts::default();
        let mut qualifier = Qualifier::default();
        let mut authority =
            Authority::new(install, guard, source, target, &mut product, &mut qualifier);
        assert!(!authority.confirm_authority_and_quiescence(
            &proof,
            CP::Begin,
            Requirement::QualifiedSource
        ));
        assert_eq!(authority.failure(), Some(Failure::OuterBinding));
        assert_eq!(product.calls, 0);
        assert_eq!(qualifier.calls, 0);
    }
    assert!(fixture.marker().exists());
}

#[test]
fn release_bridge_rejects_foreign_proof_or_program_qualification() {
    let fixture = Preserved::new(true, true);
    let proof = fixture.prepare();
    let foreign = Preserved::new(true, true);
    let foreign_proof = foreign.prepare();
    let source = ProductArtifactIdentity::new(
        "radishlex-macos",
        fixture.source.release().clone(),
        fixture.source.product_manifest_sha256(),
        product(39, 'c').program(ProgramComponent::Manager).clone(),
        fixture
            .source
            .program(ProgramComponent::InputMethod)
            .clone(),
    )
    .unwrap();
    for evidence in [
        Qualification::from_verified_source(fixture.source.clone(), 9, foreign_proof).unwrap(),
        Qualification::from_verified_source(source, 9, proof.clone()).unwrap(),
    ] {
        let (store, inner) = fixture.store();
        let mut product = ReleaseProducts::default();
        let mut qualifier = Qualifier {
            evidence: Some(evidence),
            ..Default::default()
        };
        let mut authority = Authority::new(
            &fixture.install,
            &fixture.guard,
            &fixture.source,
            &fixture.target,
            &mut product,
            &mut qualifier,
        );
        assert!(store
            .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
            .is_err());
        assert_eq!(
            authority.failure(),
            Some(Failure::SourceQualification(QE::EvidenceChanged))
        );
        assert!(fixture.marker().exists());
    }
    for schema in [-1, 8, 10] {
        assert_eq!(
            Qualification::from_verified_source(fixture.source.clone(), schema, proof.clone()),
            Err(QE::UnsupportedSchema)
        );
    }
}

#[test]
fn release_bridge_rechecks_qualification_before_every_marker_mutation() {
    for checkpoint in [
        CP::Begin,
        CP::SourceVerified,
        CP::BeforeMarkerMove,
        CP::MarkerFileSynced,
    ] {
        let fixture = Preserved::new(false, true);
        fixture.prepare();
        let original = fs::read(fixture.marker()).unwrap();
        let (store, inner) = fixture.store();
        let mut product = ReleaseProducts::default();
        let mut qualifier = Qualifier {
            reject: Some(checkpoint),
            ..Default::default()
        };
        let mut authority = Authority::new(
            &fixture.install,
            &fixture.guard,
            &fixture.source,
            &fixture.target,
            &mut product,
            &mut qualifier,
        );
        assert!(store
            .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
            .is_err());
        assert_eq!(
            authority.failure(),
            Some(Failure::SourceQualification(QE::Unavailable))
        );
        assert_eq!(fs::read(fixture.marker()).unwrap(), original);
    }
}

#[test]
fn release_bridge_rejects_outer_drift_during_either_callback() {
    for in_qualification in [false, true] {
        let fixture = Preserved::new(false, true);
        fixture.prepare();
        let drift = Some((fixture.outer(), b"{}\n".to_vec()));
        let mut product = ReleaseProducts::default();
        let mut qualifier = Qualifier::default();
        if in_qualification {
            qualifier.drift = drift;
        } else {
            product.drift = drift;
        }
        let (store, inner) = fixture.store();
        let mut authority = Authority::new(
            &fixture.install,
            &fixture.guard,
            &fixture.source,
            &fixture.target,
            &mut product,
            &mut qualifier,
        );
        assert!(store
            .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
            .is_err());
        assert_eq!(authority.failure(), Some(Failure::OuterBinding));
        assert!(fixture.marker().exists());
        assert_eq!(fs::read(fixture.outer()).unwrap(), b"{}\n");
    }
}

#[test]
fn release_bridge_reobserves_product_after_qualification_and_rejects_wrong_inner_guard() {
    let fixture = Preserved::new(false, true);
    fixture.prepare();
    let (store, inner) = fixture.store();
    let mut product = ReleaseProducts {
        reject_at: Some(2),
        ..Default::default()
    };
    let mut qualifier = Qualifier::default();
    let mut authority = Authority::new(
        &fixture.install,
        &fixture.guard,
        &fixture.source,
        &fixture.target,
        &mut product,
        &mut qualifier,
    );
    assert!(store
        .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
        .is_err());
    assert_eq!(authority.failure(), Some(Failure::ProductObservation));
    assert_eq!(qualifier.calls, 1);
    assert!(fixture.marker().exists());
    drop(inner);
    let foreign = Preserved::new(false, true);
    let (_, foreign_inner) = foreign.store();
    let (store, inner) = fixture.store();
    let mut product = ReleaseProducts::default();
    let mut qualifier = Qualifier::default();
    let mut authority = Authority::new(
        &fixture.install,
        &fixture.guard,
        &fixture.source,
        &fixture.target,
        &mut product,
        &mut qualifier,
    );
    assert!(store
        .finish_cancellation_release(&foreign_inner, NEW, &mut authority, &Hasher)
        .is_err());
    assert_eq!(product.calls, 0);
    assert_eq!(qualifier.calls, 0);
    assert!(fixture.marker().exists());
    drop(inner);
}

#[test]
fn qualification_cannot_bypass_live_source_family_revalidation() {
    let fixture = Preserved::new(false, true);
    fixture.prepare();
    let (store, inner) = fixture.store();
    let mut product = ReleaseProducts::default();
    let source = fixture.fixture.0.join("userdb.sqlite3");
    let mut changed = fs::read(&source).unwrap();
    changed.push(0);
    let mut qualifier = Qualifier {
        drift: Some((source, changed)),
        ..Default::default()
    };
    let mut authority = Authority::new(
        &fixture.install,
        &fixture.guard,
        &fixture.source,
        &fixture.target,
        &mut product,
        &mut qualifier,
    );
    assert!(store
        .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
        .is_err());
    // Provider approval cannot override the core's before/after physical proof.
    assert_eq!(authority.failure(), None);
    assert_eq!(qualifier.calls, 1);
    assert!(fixture.marker().exists());
}

#[test]
fn marker_move_interruption_requires_fresh_qualification_on_resume() {
    for checkpoint in [
        CP::MarkerMoved,
        CP::MarkerTargetSynced,
        CP::MarkerSourceSynced,
        CP::Released,
    ] {
        let fixture = Preserved::new(false, true);
        let proof = fixture.prepare();
        let original = fs::read(fixture.marker()).unwrap();
        let inode = fs::metadata(fixture.marker()).unwrap().ino();
        let (store, inner) = fixture.store();
        let mut product = ReleaseProducts::default();
        let mut qualifier = Qualifier {
            reject: Some(checkpoint),
            ..Default::default()
        };
        let mut authority = Authority::new(
            &fixture.install,
            &fixture.guard,
            &fixture.source,
            &fixture.target,
            &mut product,
            &mut qualifier,
        );
        assert!(store
            .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
            .is_err());
        assert_eq!(
            authority.failure(),
            Some(Failure::SourceQualification(QE::Unavailable))
        );
        assert!(!fixture.marker().exists());
        let archived = fixture
            .fixture
            .0
            .join(".radishlex-upgrade-history-v1")
            .join(NEW)
            .join("preparation-cancellation.json");
        assert_eq!(fs::read(&archived).unwrap(), original);
        assert_eq!(fs::metadata(&archived).unwrap().ino(), inode);
        drop(inner);

        // A moved marker alone never authorizes a successful resume report.
        let (store, inner) = fixture.store();
        let mut product = ReleaseProducts::default();
        let mut qualifier = Qualifier {
            unavailable: true,
            ..Default::default()
        };
        let mut authority = Authority::new(
            &fixture.install,
            &fixture.guard,
            &fixture.source,
            &fixture.target,
            &mut product,
            &mut qualifier,
        );
        assert!(store
            .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
            .is_err());
        assert_eq!(
            authority.failure(),
            Some(Failure::SourceQualification(QE::Unavailable))
        );
        drop(inner);

        let (store, inner) = fixture.store();
        let mut product = ReleaseProducts::default();
        let mut qualifier = Qualifier::default();
        let mut authority = Authority::new(
            &fixture.install,
            &fixture.guard,
            &fixture.source,
            &fixture.target,
            &mut product,
            &mut qualifier,
        );
        let (_, actual) = store
            .finish_cancellation_release(&inner, NEW, &mut authority, &Hasher)
            .unwrap();
        assert_eq!(actual, proof);
        assert_eq!(fs::read(&archived).unwrap(), original);
        assert_eq!(fs::metadata(&archived).unwrap().ino(), inode);
    }
}
