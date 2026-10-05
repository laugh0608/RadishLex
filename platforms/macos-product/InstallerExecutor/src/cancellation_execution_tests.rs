use super::*;
use radishlex_ime_product_upgrade::{
    CancellationReleaseCheckpoint, CancellationSourceCheckpoint, PreparationCancellationCheckpoint,
};
use radishlex_macos_product_install_coordinator::CancellationSourceQualification;

#[path = "cancellation_execution_fixture.rs"]
mod fixture;
use fixture::*;

#[derive(Default)]
struct Observer {
    seen: Vec<ExecutionCheckpoint>,
    fail: Option<ExecutionCheckpoint>,
    mutation: Option<(ExecutionCheckpoint, Box<dyn FnOnce()>)>,
    available: Option<u64>,
    exit: bool,
}
impl ExecutionObserver for Observer {
    fn inspect(
        &mut self,
        _: &PreparationReceipt,
        _: &ProductArtifactIdentity,
        _: &ProductArtifactIdentity,
        point: ExecutionCheckpoint,
    ) -> Result<u64> {
        self.seen.push(point);
        if self.mutation.as_ref().is_some_and(|(at, _)| *at == point) {
            (self.mutation.take().unwrap().1)();
        }
        if self.fail == Some(point) {
            if self.exit {
                std::process::exit(78);
            }
            return Err(CancellationExecutionError::NativeObservation);
        }
        Ok(self.available.unwrap_or(u64::MAX))
    }
}
#[derive(Default)]
struct Qualifier {
    available: bool,
    calls: usize,
    mutation: Option<Box<dyn FnOnce()>>,
    foreign: Option<CancellationReleaseReceipt>,
}
impl CancellationSourceQualificationAuthority for Qualifier {
    fn verify_source_qualification(
        &mut self,
        proof: &CancellationReleaseReceipt,
        source: &ProductArtifactIdentity,
        _: CancellationReleaseCheckpoint,
    ) -> std::result::Result<CancellationSourceQualification, CancellationQualificationError> {
        self.calls += 1;
        if let Some(mutation) = self.mutation.take() {
            mutation();
        }
        if !self.available {
            return Err(CancellationQualificationError::Unavailable);
        }
        // Explicit synthetic issuer; never used by the public native entry.
        CancellationSourceQualification::from_verified_source(
            source.clone(),
            radishlex_ime_userdb::UserDb::supported_schema_version(),
            self.foreign.as_ref().unwrap_or(proof).clone(),
        )
    }
}

#[test]
fn cancellation_execution_preserves_before_and_after_maintenance_and_requires_separate_qualification(
) {
    for stage in 0..5 {
        for new_outer in [false, true] {
            let fixture = Fixture::new(stage, new_outer);
            let install = fixture.install();
            let (grant, _control) = fixture.grant();
            let mut observer = Observer::default();
            let proof =
                preserve_with_observer(grant, &install, &mut observer).unwrap_or_else(|error| {
                    panic!(
                        "stage {stage}, new outer {new_outer}: {error:?}; points {:?}",
                        observer.seen
                    )
                });
            assert_eq!(
                fixture.observe().phase(),
                MaintenancePhase::CancellationAwaitingQualification
            );
            assert_eq!(install.load().unwrap().as_ref(), Some(&fixture.previous));
            assert!(fixture
                .path
                .join(STATE)
                .join("preparation-cancellation.json")
                .exists());
            assert!(observer.seen.contains(&ExecutionCheckpoint::Request(
                PreparationCancellationCheckpoint::Recorded
            )));
            assert!(observer.seen.contains(&ExecutionCheckpoint::Source(
                CancellationSourceCheckpoint::SourceReady
            )));
            let before = tree(&fixture.path);
            let (grant, _control) = super::super::authorize_cancellation_release(
                &proof,
                &fixture.source,
                confirmation(),
                Duration::from_secs(300),
            )
            .unwrap();
            let mut qualifier = Qualifier {
                available: false,
                ..Qualifier::default()
            };
            assert!(matches!(
                finish_with_observer(
                    grant,
                    &install,
                    &fixture.target,
                    &mut observer,
                    &mut qualifier
                ),
                Err(CancellationExecutionError::ReleaseAuthority(
                    CancellationReleaseAuthorityFailure::SourceQualification(
                        CancellationQualificationError::Unavailable
                    )
                ))
            ));
            assert_eq!(qualifier.calls, 1);
            assert_eq!(tree(&fixture.path), before);
            let (grant, _control) = super::super::authorize_cancellation_release(
                &proof,
                &fixture.source,
                confirmation(),
                Duration::from_secs(300),
            )
            .unwrap();
            qualifier.available = true;
            let released = finish_with_observer(
                grant,
                &install,
                &fixture.target,
                &mut observer,
                &mut qualifier,
            )
            .unwrap();
            assert_eq!(released, proof);
            assert_eq!(
                fixture.observe().phase(),
                MaintenancePhase::CancellationReleased
            );
            assert_eq!(install.load().unwrap().as_ref(), Some(&fixture.previous));
        }
    }
}

#[test]
fn cancellation_execution_resumes_every_stage_with_fresh_task_and_same_outer_guard_order() {
    use radishlex_ime_product_upgrade::{
        CancellationArchiveCheckpoint as A, CancellationArchiveSlot as Slot,
    };
    use ExecutionCheckpoint as P;
    let points = [
        P::Request(PreparationCancellationCheckpoint::Recorded),
        P::Source(CancellationSourceCheckpoint::BeforeMaintenance),
        P::Source(CancellationSourceCheckpoint::SourceReady),
        P::Archive(A::Moved(Slot::NewOuter)),
        P::Archive(A::CompatibilityPublished),
        P::Release(CancellationReleaseCheckpoint::ProofPublished),
        P::Release(CancellationReleaseCheckpoint::ArchiveMoved),
        P::Release(CancellationReleaseCheckpoint::IndexPublished),
        P::Release(CancellationReleaseCheckpoint::Ready),
    ];
    for point in points {
        let fixture = Fixture::new(2, true);
        let install = fixture.install();
        let (grant, control) = fixture.grant();
        let mut observer = Observer {
            fail: Some(point),
            ..Observer::default()
        };
        assert!(
            preserve_with_observer(grant, &install, &mut observer).is_err(),
            "{point:?}"
        );
        assert!(observer.seen.contains(&point), "{point:?}");
        assert_ne!(
            fixture.observe().phase(),
            MaintenancePhase::CancellationReleased
        );
        if point == P::Archive(A::Moved(Slot::NewOuter)) {
            assert!(
                install.load().unwrap().is_none(),
                "interrupted outer absence must be recoverable"
            );
        }
        drop(control);
        let (fresh, _control) = fixture.grant();
        let proof = preserve_with_observer(fresh, &install, &mut Observer::default())
            .unwrap_or_else(|error| panic!("resume after {point:?}: {error:?}"));
        assert_eq!(proof.operation_id(), NEW);
        assert_eq!(
            fixture.observe().phase(),
            MaintenancePhase::CancellationAwaitingQualification
        );
        assert_eq!(install.load().unwrap().as_ref(), Some(&fixture.previous));
    }
}

#[test]
fn cancellation_execution_never_adopts_interrupted_unbound_temporaries() {
    use radishlex_ime_product_upgrade::{
        CancellationArchiveCheckpoint as A, CancellationSourcePhase as Phase,
    };
    use PreparationCancellationCheckpoint as C;
    for point in [
        ExecutionCheckpoint::Request(C::Created),
        ExecutionCheckpoint::Source(CancellationSourceCheckpoint::Write(
            Phase::Finishing,
            C::Created,
        )),
        ExecutionCheckpoint::Archive(A::Record(C::Created)),
        ExecutionCheckpoint::Release(CancellationReleaseCheckpoint::ProofCreated),
        ExecutionCheckpoint::Release(CancellationReleaseCheckpoint::IndexCreated),
    ] {
        let fixture = Fixture::new(2, true);
        let install = fixture.install();
        let original = fixture.observe();
        let (grant, _control) = fixture.grant();
        let mut observer = Observer {
            fail: Some(point),
            ..Observer::default()
        };
        assert!(preserve_with_observer(grant, &install, &mut observer).is_err());
        assert!(observer.seen.contains(&point));
        let before = tree(&fixture.path);
        let (stale, _control) = authorize_cancellation_preservation(
            &original,
            &fixture.source,
            &fixture.target,
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap();
        let mut observer = Observer::default();
        assert!(preserve_with_observer(stale, &install, &mut observer).is_err());
        assert!(observer.seen.is_empty());
        assert_eq!(
            tree(&fixture.path),
            before,
            "must retain temp after {point:?}"
        );
    }
}

#[test]
fn cancellation_execution_requires_confirmation_lifetime_current_root_and_both_guards() {
    let fixture = Fixture::new(0, false);
    let observation = fixture.observe();
    for index in 0..4 {
        let mut flags = confirmation();
        match index {
            0 => flags.explicit_action_confirmed = false,
            1 => flags.data_retention_acknowledged = false,
            2 => flags.neutral_input_source_selected = false,
            _ => flags.manager_closed = false,
        }
        assert!(matches!(
            authorize_cancellation_preservation(
                &observation,
                &fixture.source,
                &fixture.target,
                flags,
                Duration::from_secs(300)
            ),
            Err(CancellationTaskAuthorizationError::UserConfirmation(_))
        ));
    }
    for duration in [Duration::ZERO, Duration::from_secs(301)] {
        assert!(matches!(
            authorize_cancellation_preservation(
                &observation,
                &fixture.source,
                &fixture.target,
                confirmation(),
                duration
            ),
            Err(CancellationTaskAuthorizationError::InvalidDuration)
        ));
    }
    let foreign = Fixture::new(0, false);
    for kind in 0..5 {
        let install = fixture.install();
        let before = tree(&fixture.path);
        let (mut grant, control) = fixture.grant();
        let mut observer = Observer::default();
        let result = match kind {
            0 => {
                control.revoke();
                preserve_with_observer(grant, &install, &mut observer)
            }
            1 => {
                grant.grant.expires = std::time::Instant::now();
                preserve_with_observer(grant, &install, &mut observer)
            }
            2 => preserve_with_observer(grant, &foreign.install(), &mut observer),
            3 => {
                let _guard = install.acquire_guard().unwrap();
                preserve_with_observer(grant, &install, &mut observer)
            }
            _ => {
                let store = CancellationArchiveStore::open_existing(
                    VerifiedDataRoot::verify(&fixture.path, fixture.owner()).unwrap(),
                )
                .unwrap();
                let _guard = store.acquire_guard().unwrap();
                preserve_with_observer(grant, &install, &mut observer)
            }
        };
        assert!(result.is_err(), "{kind}");
        assert!(observer.seen.is_empty());
        assert_eq!(tree(&fixture.path), before);
    }
}

#[test]
fn cancellation_execution_rechecks_authorization_outer_identity_and_capacity_after_callbacks() {
    for kind in 0..3 {
        let fixture = Fixture::new(2, true);
        let install = fixture.install();
        let (grant, control) = fixture.grant();
        let before = tree(&fixture.path);
        let mut observer = Observer::default();
        let retained = fixture.path.join("retained-original-outer");
        match kind {
            0 => {
                observer.mutation = Some((
                    ExecutionCheckpoint::Request(PreparationCancellationCheckpoint::BeforeCreate),
                    Box::new(move || control.revoke()),
                ))
            }
            1 => {
                let path = fixture
                    .path
                    .join(".radishlex-install-history-v1")
                    .join(OLD)
                    .join("receipt.json");
                observer.mutation = Some((
                    ExecutionCheckpoint::Request(PreparationCancellationCheckpoint::Begin),
                    Box::new(move || {
                        let bytes = fs::read(&path).unwrap();
                        fs::rename(&path, &retained).unwrap();
                        put(&path, &bytes);
                    }),
                ));
                // Keep the task owner alive for the rest of this iteration.
                let _control = control;
                assert!(preserve_with_observer(grant, &install, &mut observer).is_err());
                assert!(!fixture
                    .path
                    .join(STATE)
                    .join("preparation-cancellation.json")
                    .exists());
                continue;
            }
            _ => observer.available = Some(1),
        }
        let result = preserve_with_observer(grant, &install, &mut observer);
        match kind {
            0 => {
                assert_eq!(result, Err(CancellationExecutionError::TaskAuthorization));
                assert_eq!(tree(&fixture.path), before);
            }
            _ => {
                assert_eq!(
                    result,
                    Err(CancellationExecutionError::Preparation(
                        SourcePreparationError::InsufficientSpace
                    ))
                );
                assert_eq!(
                    fixture.observe().phase(),
                    MaintenancePhase::CancellationFinishingSource
                );
                let (fresh, _control) = fixture.grant();
                preserve_with_observer(fresh, &install, &mut Observer::default()).unwrap();
            }
        }
    }
}

#[test]
fn cancellation_execution_final_release_rechecks_task_qualification_and_partial_marker_move() {
    for kind in 0..4 {
        let fixture = Fixture::new(2, true);
        let install = fixture.install();
        let (grant, _control) = fixture.grant();
        let proof = preserve_with_observer(grant, &install, &mut Observer::default()).unwrap();
        let before = tree(&fixture.path);
        let (grant, control) = super::super::authorize_cancellation_release(
            &proof,
            &fixture.source,
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap();
        let mut observer = Observer::default();
        let mut qualifier = Qualifier {
            available: true,
            ..Qualifier::default()
        };
        let foreign = Fixture::new(0, false);
        match kind {
            0 => qualifier.mutation = Some(Box::new(move || control.revoke())),
            1 => {
                let (foreign_grant, _foreign_control) = foreign.grant();
                qualifier.foreign = Some(
                    preserve_with_observer(
                        foreign_grant,
                        &foreign.install(),
                        &mut Observer::default(),
                    )
                    .unwrap(),
                );
            }
            2 => {
                observer.fail = Some(ExecutionCheckpoint::Release(
                    CancellationReleaseCheckpoint::BeforeMarkerMove,
                ))
            }
            _ => {
                observer.fail = Some(ExecutionCheckpoint::Release(
                    CancellationReleaseCheckpoint::MarkerMoved,
                ))
            }
        }
        let result = finish_with_observer(
            grant,
            &install,
            &fixture.target,
            &mut observer,
            &mut qualifier,
        );
        match kind {
            0 => assert_eq!(result, Err(CancellationExecutionError::TaskAuthorization)),
            1 => assert!(matches!(
                result,
                Err(CancellationExecutionError::ReleaseAuthority(
                    CancellationReleaseAuthorityFailure::SourceQualification(
                        CancellationQualificationError::EvidenceChanged
                    )
                ))
            )),
            _ => assert_eq!(result, Err(CancellationExecutionError::NativeObservation)),
        }
        if kind < 3 {
            assert_eq!(tree(&fixture.path), before);
            assert_eq!(
                fixture.observe().phase(),
                MaintenancePhase::CancellationAwaitingQualification
            );
        } else {
            assert_eq!(
                fixture.observe().phase(),
                MaintenancePhase::CancellationReleased
            );
        }
        let (fresh, _control) = super::super::authorize_cancellation_release(
            &proof,
            &fixture.source,
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap();
        let released = finish_with_observer(
            fresh,
            &install,
            &fixture.target,
            &mut Observer::default(),
            &mut Qualifier {
                available: true,
                ..Qualifier::default()
            },
        )
        .unwrap();
        assert_eq!(released, proof);
        assert_eq!(
            fixture.observe().phase(),
            MaintenancePhase::CancellationReleased
        );
    }
}

fn crash_points() -> [ExecutionCheckpoint; 6] {
    use radishlex_ime_product_upgrade::{
        CancellationArchiveCheckpoint as A, CancellationArchiveSlot as Slot,
    };
    [
        ExecutionCheckpoint::Request(PreparationCancellationCheckpoint::Recorded),
        ExecutionCheckpoint::Source(CancellationSourceCheckpoint::BeforeMaintenance),
        ExecutionCheckpoint::Archive(A::Moved(Slot::NewOuter)),
        ExecutionCheckpoint::Release(CancellationReleaseCheckpoint::ProofPublished),
        ExecutionCheckpoint::Release(CancellationReleaseCheckpoint::IndexPublished),
        ExecutionCheckpoint::Release(CancellationReleaseCheckpoint::MarkerMoved),
    ]
}

#[test]
#[ignore = "subprocess entry exercised by cancellation_execution_recovers_after_process_exit_with_fresh_dual_guards"]
fn cancellation_execution_crash_child() {
    let Ok(path) = std::env::var("RADISHLEX_CANCEL_EXEC_CHILD") else {
        return;
    };
    let point: usize = std::env::var("RADISHLEX_CANCEL_EXEC_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let fixture = Fixture::reopen(path.into());
    let install = fixture.install();
    let (grant, _control) = fixture.grant();
    let mut observer = Observer {
        fail: Some(crash_points()[point]),
        exit: true,
        ..Observer::default()
    };
    let proof = preserve_with_observer(grant, &install, &mut observer).unwrap();
    if point == 5 {
        let (grant, _control) = super::super::authorize_cancellation_release(
            &proof,
            &fixture.source,
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap();
        finish_with_observer(
            grant,
            &install,
            &fixture.target,
            &mut observer,
            &mut Qualifier {
                available: true,
                ..Qualifier::default()
            },
        )
        .unwrap();
    }
    panic!("execution checkpoint was not visited");
}

#[test]
fn cancellation_execution_recovers_after_process_exit_with_fresh_dual_guards() {
    // Serial children; do not rebuild this executable alongside the matrix.
    for point in 0..crash_points().len() {
        let fixture = Fixture::new(2, true);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "cancellation_execution::tests::cancellation_execution_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("RADISHLEX_CANCEL_EXEC_CHILD", &fixture.path)
            .env("RADISHLEX_CANCEL_EXEC_POINT", point.to_string())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(78),
            "{point}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let install = fixture.install();
        let proof = if point != 5 {
            let (grant, _control) = fixture.grant();
            preserve_with_observer(grant, &install, &mut Observer::default()).unwrap()
        } else {
            assert_eq!(
                fixture.observe().phase(),
                MaintenancePhase::CancellationReleased
            );
            CancellationReleaseReceipt::decode(
                &fs::read(
                    fixture
                        .path
                        .join(".radishlex-upgrade-history-v1")
                        .join(NEW)
                        .join("cancellation-release.json"),
                )
                .unwrap(),
            )
            .unwrap()
        };
        let (grant, _control) = super::super::authorize_cancellation_release(
            &proof,
            &fixture.source,
            confirmation(),
            Duration::from_secs(300),
        )
        .unwrap();
        let released = finish_with_observer(
            grant,
            &install,
            &fixture.target,
            &mut Observer::default(),
            &mut Qualifier {
                available: true,
                ..Qualifier::default()
            },
        )
        .unwrap();
        assert_eq!(released, proof);
        assert_eq!(
            fixture.observe().phase(),
            MaintenancePhase::CancellationReleased
        );
        assert_eq!(install.load().unwrap().as_ref(), Some(&fixture.previous));
    }
}

#[test]
fn cancellation_execution_rejects_preparation_changed_to_handoff_after_authorization() {
    let fixture = Fixture::new(4, true);
    let (grant, _control) = fixture.grant();
    fixture.begin_handoff();
    let before = tree(&fixture.path);
    let mut observer = Observer::default();
    assert!(preserve_with_observer(grant, &fixture.install(), &mut observer).is_err());
    assert!(observer.seen.is_empty());
    assert_eq!(tree(&fixture.path), before);
    assert!(!fixture
        .path
        .join(STATE)
        .join("preparation-cancellation.json")
        .exists());
}
