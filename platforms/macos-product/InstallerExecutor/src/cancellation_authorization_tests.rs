use super::*;
use crate::cancellation_fixture::Fixture;

fn confirmation() -> InstallerUserAuthorization {
    InstallerUserAuthorization {
        explicit_action_confirmed: true,
        data_retention_acknowledged: true,
        neutral_input_source_selected: true,
        manager_closed: true,
    }
}
fn grant(f: &Fixture) -> (CancellationTaskAuthorization, CancellationTaskControl) {
    authorize_cancellation_release(
        &f.proof,
        &f.source,
        confirmation(),
        Duration::from_secs(300),
    )
    .unwrap()
}

#[test]
fn task_requires_current_confirmation_and_bounded_duration() {
    let f = Fixture::new();
    for index in 0..4 {
        let mut flags = confirmation();
        match index {
            0 => flags.explicit_action_confirmed = false,
            1 => flags.data_retention_acknowledged = false,
            2 => flags.neutral_input_source_selected = false,
            _ => flags.manager_closed = false,
        }
        assert!(matches!(
            authorize_cancellation_release(&f.proof, &f.source, flags, Duration::from_secs(10)),
            Err(CancellationTaskAuthorizationError::UserConfirmation(_))
        ));
    }
    for duration in [Duration::ZERO, Duration::from_secs(301)] {
        assert!(matches!(
            authorize_cancellation_release(&f.proof, &f.source, confirmation(), duration),
            Err(CancellationTaskAuthorizationError::InvalidDuration)
        ));
    }
}

#[test]
fn task_rejects_foreign_proof_source_and_expiration_before_observation() {
    let f = Fixture::new();
    let foreign = Fixture::new();
    let (mut task, _control) = grant(&f);
    assert!(task.verify(&f.proof, &f.source));
    assert_eq!(
        task.observe(&foreign.proof, &f.source, || panic!(
            "foreign proof must not launch host"
        )),
        Err(CancellationQuiescenceFailure::TaskAuthorization)
    );
    // Full program identity, not just the release/manifest, is fixed by the grant.
    use radishlex_ime_product_install::{ProgramBundleIdentity, ProgramComponent};
    let changed = ProductArtifactIdentity::new(
        "radishlex-macos",
        f.source.release().clone(),
        f.source.product_manifest_sha256(),
        ProgramBundleIdentity::new(
            ProgramComponent::Manager,
            "dev.radishlex.radishlexManager",
            "e".repeat(64),
            "e".repeat(64),
        )
        .unwrap(),
        f.source.program(ProgramComponent::InputMethod).clone(),
    )
    .unwrap();
    assert!(!task.verify(&f.proof, &changed));
    task.grant.expires = Instant::now();
    assert_eq!(
        task.observe(&f.proof, &f.source, || panic!(
            "expired task must not launch host"
        )),
        Err(CancellationQuiescenceFailure::TaskAuthorization)
    );
}

#[test]
fn task_revocation_during_observation_and_control_drop_are_final() {
    let f = Fixture::new();
    let (task, control) = grant(&f);
    assert_eq!(
        task.observe(&f.proof, &f.source, || false),
        Err(CancellationQuiescenceFailure::NativeObservation)
    );
    assert_eq!(task.observe(&f.proof, &f.source, || true), Ok(()));
    assert_eq!(
        task.observe(&f.proof, &f.source, || {
            control.revoke();
            true
        }),
        Err(CancellationQuiescenceFailure::TaskAuthorization)
    );
    assert_eq!(
        task.observe(&f.proof, &f.source, || panic!(
            "revoked task must not launch host"
        )),
        Err(CancellationQuiescenceFailure::TaskAuthorization)
    );
    let (task, control) = grant(&f);
    drop(control);
    assert!(!task.verify(&f.proof, &f.source));
    let (task, _control) = authorize_cancellation_release(
        &f.proof,
        &f.source,
        confirmation(),
        Duration::from_millis(1),
    )
    .unwrap();
    assert_eq!(
        task.observe(&f.proof, &f.source, || {
            std::thread::sleep(Duration::from_millis(3));
            true
        }),
        Err(CancellationQuiescenceFailure::TaskAuthorization)
    );
}
