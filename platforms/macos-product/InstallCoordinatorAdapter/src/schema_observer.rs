//! Synthetic integration only. No production entry point, source path parameter,
//! reader policy change, qualification authority, or final marker operation.

use super::schema_observer_budget::{BudgetError, ObservationControl, ObserverBudget, Phase};
use super::schema_observer_fs::{FsError, SyntheticBatch, SyntheticCase};
use super::schema_observer_proof::{ProofBinding, ProofError, SyntheticProofSession};
use radishlex_ime_userdb::schema_observer::{
    create_synthetic_fixture, FixtureKind, ObserverError, SyntheticFixture, SyntheticObservation,
};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Budget(BudgetError),
    Filesystem(FsError),
    Proof(ProofError),
    Sqlite(ObserverError),
    ScratchRetained {
        original: Box<Failure>,
        cleanup: FsError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    None,
    CancelAfterCapture,
    CancelAfterSql,
    CancelBeforeReturn,
    FileCloseUnconfirmed,
    CloseUnconfirmed,
    CleanupUnlink,
    CleanupRmdir,
}

impl From<BudgetError> for Failure {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}
impl From<FsError> for Failure {
    fn from(error: FsError) -> Self {
        Self::Filesystem(error)
    }
}
impl From<ProofError> for Failure {
    fn from(error: ProofError) -> Self {
        Self::Proof(error)
    }
}
impl From<ObserverError> for Failure {
    fn from(error: ObserverError) -> Self {
        Self::Sqlite(error)
    }
}

/// This value is never serialized and cannot be constructed from a schema or
/// receipt. It still cannot qualify source programs or release a marker.
struct SourceSchemaObservation {
    binding: ProofBinding,
    sqlite: SyntheticObservation,
    call_id: u64,
}

impl SourceSchemaObservation {
    fn consume(
        self,
        session: &SyntheticProofSession,
        budget: &mut ObserverBudget,
    ) -> Result<i64, Failure> {
        budget.checkpoint(Phase::Return)?;
        if budget.control().call_id() != self.call_id {
            return Err(ProofError::Replay.into());
        }
        validate_session(session, budget)?;
        session.consume_binding(&self.binding, self.call_id)?;
        budget.checkpoint(Phase::Return)?;
        Ok(self.sqlite.schema_version())
    }
}

fn sqlite_budget(error: BudgetError) -> ObserverError {
    match error {
        BudgetError::Cancelled => ObserverError::Cancelled,
        BudgetError::Deadline | BudgetError::EntryTooShort => ObserverError::Deadline,
        BudgetError::LimitExceeded | BudgetError::Overflow => ObserverError::LimitExceeded,
    }
}

fn validate_session(
    session: &SyntheticProofSession,
    budget: &mut ObserverBudget,
) -> Result<(), Failure> {
    let mut budget_failure = None;
    let result = session.validate_current_with(&mut |bytes| match budget.charge_read(bytes) {
        Ok(()) => true,
        Err(error) => {
            budget_failure = Some(error);
            false
        }
    });
    if let Some(error) = budget_failure {
        return Err(error.into());
    }
    result.map_err(Failure::from)
}

fn observe(
    case: &mut SyntheticCase,
    fixture: &SyntheticFixture,
    session: &SyntheticProofSession,
    budget: &mut ObserverBudget,
    fault: Fault,
) -> Result<SourceSchemaObservation, Failure> {
    budget.checkpoint(Phase::Capture)?;
    // No path is supplied to SQLite: the capability came from the fresh
    // generator, and must describe this same private source directory.
    if fixture.source_dir() != case.source_path() {
        return Err(ProofError::EvidenceChanged.into());
    }
    validate_session(session, budget)?;
    let call_id = budget.control().call_id();
    let binding = session.bind(call_id)?;
    let mut copy = case.capture(session.family()?, budget)?;
    if fault == Fault::FileCloseUnconfirmed {
        super::schema_observer_fs::inject_fault(super::schema_observer_fs::FsOperation::Close, 0);
    }
    let work = (|| -> Result<SyntheticObservation, Failure> {
        copy.verify_copy(budget)?;
        copy.revalidate_source(budget)?;
        if fault == Fault::CancelAfterCapture {
            budget.control().revoke();
        }
        budget.checkpoint(Phase::Sqlite)?;
        copy.mark_sqlite_started()?;
        let control = budget.control();
        let mut verification_failure = None;
        let sqlite = {
            let shared = RefCell::new(&mut *budget);
            fixture.inspect_copy(
                Arc::new(move || control.check_work().map_err(sqlite_budget)),
                &mut |bytes| {
                    shared
                        .borrow_mut()
                        .charge_read(bytes)
                        .map_err(sqlite_budget)
                },
                &mut || {
                    copy.verify_copy(&mut shared.borrow_mut()).map_err(|error| {
                        verification_failure = Some(error);
                        ObserverError::EvidenceChanged
                    })
                },
            )
        };
        // A preceding verification error must never hide an unconfirmed close:
        // that uncertainty forbids disposing the scratch files below.
        if matches!(sqlite, Err(ObserverError::CloseUnconfirmed)) {
            return Err(ObserverError::CloseUnconfirmed.into());
        }
        if let Some(error) = verification_failure {
            return Err(error.into());
        }
        let sqlite = sqlite?;
        if fault == Fault::CloseUnconfirmed {
            // This planned fault is injected only after a real successful
            // close. Register its final SHM identity for fixture-host teardown;
            // an actual CloseUnconfirmed above must never take this path.
            copy.verify_copy(budget)?;
            return Err(ObserverError::CloseUnconfirmed.into());
        }
        if fault == Fault::CancelAfterSql {
            budget.control().revoke();
        }
        budget.checkpoint(Phase::Verify)?;
        // Disposition below fully revalidates the held source; the final core
        // probe checks the complete proof again after disposition. Avoid doing
        // two extra full-family reads between these same controlled stages.
        session.validate_binding(&binding, call_id)?;
        Ok(sqlite)
    })();
    if matches!(
        work,
        Err(Failure::Sqlite(ObserverError::CloseUnconfirmed))
            | Err(Failure::Filesystem(FsError::Close))
    ) {
        // An unconfirmed SQLite or file-descriptor close forbids deletion.
        // Keep the original reason and stop this batch rather than attempting
        // another cleanup path that could hide descriptor uncertainty.
        let original = work.expect_err("matched an unconfirmed close error");
        let cleanup = copy.retain().err().unwrap_or(FsError::Close);
        return Err(Failure::ScratchRetained {
            original: Box::new(original),
            cleanup,
        });
    }
    match fault {
        Fault::CleanupUnlink => super::schema_observer_fs::inject_fault(
            super::schema_observer_fs::FsOperation::Unlink,
            0,
        ),
        Fault::CleanupRmdir => super::schema_observer_fs::inject_fault(
            super::schema_observer_fs::FsOperation::Rmdir,
            0,
        ),
        _ => {}
    }
    let disposition = copy.cleanup(budget);
    match (work, disposition) {
        (Err(original), Err(cleanup)) => Err(Failure::ScratchRetained {
            original: Box::new(original),
            cleanup,
        }),
        (Ok(_), Err(cleanup)) => Err(Failure::ScratchRetained {
            original: Box::new(Failure::Filesystem(FsError::ScratchRetained)),
            cleanup,
        }),
        (Err(error), Ok(())) => Err(error),
        (Ok(sqlite), Ok(())) => {
            if fault == Fault::CancelBeforeReturn {
                budget.control().revoke();
            }
            budget.checkpoint(Phase::Return)?;
            validate_session(session, budget)?;
            session.validate_binding(&binding, call_id)?;
            Ok(SourceSchemaObservation {
                binding,
                sqlite,
                call_id,
            })
        }
    }
}

fn fresh_budget() -> ObserverBudget {
    ObserverBudget::new(
        ObservationControl::enter(Instant::now() + Duration::from_secs(120)).unwrap(),
    )
}

#[test]
fn synthetic_observer_real_wal_and_proof_matrix() {
    use super::schema_observer_proof::{check_binding_negatives, SyntheticPredecessor};
    let mut batch = SyntheticBatch::create().expect("new exclusive synthetic batch");
    assert!(matches!(
        SyntheticBatch::create(),
        Err(FsError::ScratchOccupied)
    ));
    let source = SyntheticProofSession::synthetic_source().unwrap();
    let variants = [
        (FixtureKind::MainOnly, None),
        (FixtureKind::EmptyWal, None),
        (FixtureKind::HeaderOnlyWal, None),
        (FixtureKind::CommittedWal, None),
        (FixtureKind::MultipleCommits, None),
        (FixtureKind::OldMainNewWal, None),
        (FixtureKind::UncommittedTail, None),
        (FixtureKind::PartialCheckpoint, None),
        (FixtureKind::WithoutShm, None),
        (FixtureKind::WithShm, None),
        (
            FixtureKind::MissingWalForWalMode,
            Some(ObserverError::WalUnsupported),
        ),
        (
            FixtureKind::SaltMismatch,
            Some(ObserverError::WalUnsupported),
        ),
        (
            FixtureKind::ResetSaltTail,
            Some(ObserverError::WalUnsupported),
        ),
        (
            FixtureKind::ChecksumMismatch,
            Some(ObserverError::WalChecksum),
        ),
        (FixtureKind::TruncatedWal, Some(ObserverError::InputFormat)),
        (FixtureKind::InvalidLayout, Some(ObserverError::Layout)),
        (
            FixtureKind::UnsupportedVersion,
            Some(ObserverError::UnsupportedSchema),
        ),
        (
            FixtureKind::BadHeaderMagic,
            Some(ObserverError::WalUnsupported),
        ),
        (
            FixtureKind::BadHeaderChecksum,
            Some(ObserverError::WalChecksum),
        ),
        (
            FixtureKind::PageSizeMismatch,
            Some(ObserverError::WalUnsupported),
        ),
        (FixtureKind::ZeroFramePage, Some(ObserverError::InputFormat)),
        (
            FixtureKind::UnsupportedNegativeVersion,
            Some(ObserverError::UnsupportedSchema),
        ),
    ];
    for (index, (kind, expected)) in variants.into_iter().enumerate() {
        let mut case = batch.begin_case().unwrap();
        if index == 0 {
            super::schema_observer_fs::check_owned_negative_objects(&mut case).unwrap();
        }
        let fixture =
            create_synthetic_fixture(case.source_path(), kind).expect("synthetic generator");
        let reference = fixture.reference_schema_version();
        case.seal_source(&mut fresh_budget()).unwrap();
        let session = SyntheticProofSession::new(case.source_path(), &source)
            .expect("strict synthetic proof");
        case.register_fixture_paths(&session.created_paths())
            .unwrap();
        check_binding_negatives(&session);
        let mut budget = fresh_budget();
        let observed = observe(&mut case, &fixture, &session, &mut budget, Fault::None);
        let outcome = match observed {
            Ok(value) => {
                assert_eq!(value.sqlite.profile(), "synthetic-fresh-schema9-exact-v1");
                assert!(!value.sqlite.sqlite_source_id().is_empty());
                assert!(!value.sqlite.compile_options().is_empty());
                value
                    .consume(&session, &mut budget)
                    .map(|version| (version, None))
            }
            Err(Failure::Sqlite(error)) => Ok((0, Some(error))),
            Err(error) => Err(error),
        };
        let marker_remains = session.marker().exists();
        assert_production_reader_stays_closed(&session, case.source_path());
        drop(session);
        case.cleanup_source_and_case(&mut fresh_budget())
            .expect("exact fixture teardown");
        let (version, error) = outcome.unwrap_or_else(|error| panic!("{kind:?}: {error:?}"));
        assert!(marker_remains);
        assert_eq!(error, expected, "{kind:?}");
        if expected.is_none() {
            assert_eq!(version, 9, "{kind:?}");
        }
        if kind == FixtureKind::OldMainNewWal {
            assert_eq!(Some(version), reference);
        }
    }
    for predecessor in [
        SyntheticPredecessor::LegacyData,
        SyntheticPredecessor::TerminalV1,
        SyntheticPredecessor::TerminalV2,
        SyntheticPredecessor::CancellationV2,
        SyntheticPredecessor::RepeatedCancellationV2,
    ] {
        let mut case = batch.begin_case().unwrap();
        let fixture = create_synthetic_fixture(case.source_path(), FixtureKind::MainOnly).unwrap();
        case.seal_source(&mut fresh_budget()).unwrap();
        let session =
            SyntheticProofSession::with_predecessor(case.source_path(), &source, predecessor)
                .unwrap_or_else(|error| panic!("{predecessor:?}: {error:?}"));
        case.register_fixture_paths(&session.created_paths())
            .unwrap();
        check_binding_negatives(&session);
        let mut budget = fresh_budget();
        let result = observe(&mut case, &fixture, &session, &mut budget, Fault::None)
            .and_then(|value| value.consume(&session, &mut budget));
        let marker_remains = session.marker().exists();
        drop(session);
        case.cleanup_source_and_case(&mut fresh_budget()).unwrap();
        assert_eq!(result, Ok(9), "{predecessor:?}");
        assert!(marker_remains);
    }
    batch.finish(&mut fresh_budget()).unwrap();
}

#[test]
fn revocation_and_disposition_failures_never_return_an_observation() {
    for fault in [
        Fault::CancelAfterCapture,
        Fault::CancelAfterSql,
        Fault::CancelBeforeReturn,
        Fault::FileCloseUnconfirmed,
        Fault::CloseUnconfirmed,
        Fault::CleanupUnlink,
        Fault::CleanupRmdir,
    ] {
        let mut batch = SyntheticBatch::create().unwrap();
        let mut case = batch.begin_case().unwrap();
        let fixture =
            create_synthetic_fixture(case.source_path(), FixtureKind::CommittedWal).unwrap();
        case.seal_source(&mut fresh_budget()).unwrap();
        let source = SyntheticProofSession::synthetic_source().unwrap();
        let session = SyntheticProofSession::new(case.source_path(), &source).unwrap();
        case.register_fixture_paths(&session.created_paths())
            .unwrap();
        let outcome = observe(&mut case, &fixture, &session, &mut fresh_budget(), fault);
        super::schema_observer_fs::clear_fault();
        let rejected = match outcome {
            Err(Failure::Budget(BudgetError::Cancelled)) => matches!(
                fault,
                Fault::CancelAfterCapture | Fault::CancelAfterSql | Fault::CancelBeforeReturn
            ),
            Err(Failure::ScratchRetained { .. }) => matches!(
                fault,
                Fault::FileCloseUnconfirmed
                    | Fault::CloseUnconfirmed
                    | Fault::CleanupUnlink
                    | Fault::CleanupRmdir
            ),
            _ => false,
        };
        if matches!(
            fault,
            Fault::FileCloseUnconfirmed
                | Fault::CloseUnconfirmed
                | Fault::CleanupUnlink
                | Fault::CleanupRmdir
        ) {
            assert!(matches!(batch.begin_case(), Err(FsError::BatchStopped)));
            assert!(matches!(
                SyntheticBatch::create(),
                Err(FsError::ScratchOccupied)
            ));
            // This branch is only the predeclared fixture fault plan. Ordinary
            // failures in observe have no recovery or residual adoption path.
            case.teardown_planned_retained_scratch(&mut fresh_budget())
                .unwrap();
        }
        let marker_remains = session.marker().exists();
        drop(session);
        case.cleanup_source_and_case(&mut fresh_budget()).unwrap();
        batch.finish(&mut fresh_budget()).unwrap();
        assert!(rejected, "{fault:?}");
        assert!(marker_remains);
    }
}

#[test]
fn completed_observations_cannot_cross_call_or_authorization_lifetimes() {
    for revoke in [false, true] {
        let mut batch = SyntheticBatch::create().unwrap();
        let mut case = batch.begin_case().unwrap();
        let fixture = create_synthetic_fixture(case.source_path(), FixtureKind::MainOnly).unwrap();
        case.seal_source(&mut fresh_budget()).unwrap();
        let source = SyntheticProofSession::synthetic_source().unwrap();
        let session = SyntheticProofSession::new(case.source_path(), &source).unwrap();
        case.register_fixture_paths(&session.created_paths())
            .unwrap();
        let mut budget = fresh_budget();
        let value = observe(&mut case, &fixture, &session, &mut budget, Fault::None).unwrap();
        let rejected = if revoke {
            budget.control().revoke();
            value.consume(&session, &mut budget) == Err(Failure::Budget(BudgetError::Cancelled))
        } else {
            value.consume(&session, &mut fresh_budget()) == Err(Failure::Proof(ProofError::Replay))
        };
        drop(session);
        case.cleanup_source_and_case(&mut fresh_budget()).unwrap();
        batch.finish(&mut fresh_budget()).unwrap();
        assert!(rejected);
    }
}

// Run each capacity case in its own already-built test process under time -l.
// Compilation is excluded from measured RSS, fixture generation is included.
#[test]
#[ignore = "dedicated measured process; run once per changed capacity implementation"]
fn synthetic_observer_capacity_256_mib() {
    capacity_case(FixtureKind::LargeDatabase);
}

#[test]
#[ignore = "dedicated measured process; run once per changed capacity implementation"]
fn synthetic_observer_capacity_max_wal() {
    capacity_case(FixtureKind::MaxWal);
}

#[test]
#[ignore = "dedicated measured process; run once per changed capacity implementation"]
fn synthetic_observer_capacity_max_wal_small_pages() {
    capacity_case(FixtureKind::MaxWalSmallPages);
}

fn capacity_case(kind: FixtureKind) {
    let started = Instant::now();
    let mut batch = SyntheticBatch::create().unwrap();
    let mut case = batch.begin_case().unwrap();
    let fixture = create_synthetic_fixture(case.source_path(), kind).unwrap();
    let family = case.seal_source(&mut fresh_budget()).unwrap();
    let source = SyntheticProofSession::synthetic_source().unwrap();
    let session = SyntheticProofSession::new(case.source_path(), &source).unwrap();
    case.register_fixture_paths(&session.created_paths())
        .unwrap();
    let mut budget = fresh_budget();
    let result = observe(&mut case, &fixture, &session, &mut budget, Fault::None)
        .and_then(|value| value.consume(&session, &mut budget));
    let (read, written) = budget.counters();
    drop(session);
    case.cleanup_source_and_case(&mut fresh_budget()).unwrap();
    batch.finish(&mut fresh_budget()).unwrap();
    assert_eq!(result, Ok(9));
    match kind {
        FixtureKind::LargeDatabase => assert_eq!(family.database.byte_len, 256 * 1024 * 1024),
        FixtureKind::MaxWal | FixtureKind::MaxWalSmallPages => {
            let wal = family.wal.as_ref().unwrap().byte_len;
            let frame_size = if kind == FixtureKind::MaxWalSmallPages {
                512 + 24
            } else {
                4096 + 24
            };
            assert_eq!(
                wal,
                32 + ((128 * 1024 * 1024 - 32) / frame_size) * frame_size
            );
        }
        _ => unreachable!(),
    }
    println!("synthetic capacity: kind={kind:?} db_bytes={} wal_bytes={} reserved_read={read} written={written} elapsed_ms={}",
        family.database.byte_len, family.wal.as_ref().map_or(0, |wal| wal.byte_len), started.elapsed().as_millis());
}

fn assert_production_reader_stays_closed(session: &SyntheticProofSession, path: &std::path::Path) {
    use crate::qualification_reader::{
        review_evidence, Architecture, CancellationEvidenceRequest, CaseSemantics,
        EvidenceEnvironment, ExecutionMode, IsolationMethod, ReadControl, ReadError, ReviewStatus,
    };
    let request =
        CancellationEvidenceRequest::new(session.proof(), session.source(), session.target())
            .unwrap();
    let environment = EvidenceEnvironment {
        os_build: "synthetic".into(),
        host_architecture: Architecture::Arm64,
        process_architecture: Architecture::Arm64,
        execution_mode: ExecutionMode::Native,
        filesystem: "synthetic".into(),
        case_semantics: CaseSemantics::Sensitive,
        isolation: IsolationMethod::DedicatedGuest,
        isolation_record: "synthetic".into(),
        admission_record: "synthetic".into(),
    };
    let (control, _) = ReadControl::new(Duration::from_secs(15)).unwrap();
    let review = review_evidence(
        "synthetic-observer-is-not-qualification",
        path,
        session.source(),
        &environment,
        Some(&request),
        &control,
    );
    assert_eq!(review.status(), ReviewStatus::Rejected);
    assert_eq!(review.rejection(), Some(ReadError::UntrustedSource));
}

#[test]
fn source_identity_and_membership_changes_never_produce_observations() {
    use super::schema_observer_fs::{check_planned_drift, PlannedDrift};
    for (fault, kind) in [
        (
            PlannedDrift::SameBytesDatabaseReplacement,
            FixtureKind::CommittedWal,
        ),
        (PlannedDrift::WalAppend, FixtureKind::CommittedWal),
        (PlannedDrift::WalTruncate, FixtureKind::CommittedWal),
        (PlannedDrift::AddShm, FixtureKind::WithoutShm),
        (PlannedDrift::RemoveShm, FixtureKind::WithShm),
        (PlannedDrift::SourceDetach, FixtureKind::CommittedWal),
        (PlannedDrift::ScratchDetach, FixtureKind::CommittedWal),
    ] {
        let mut batch = SyntheticBatch::create().unwrap();
        let mut case = batch.begin_case().unwrap();
        let _fixture = create_synthetic_fixture(case.source_path(), kind).unwrap();
        case.seal_source(&mut fresh_budget()).unwrap();
        check_planned_drift(case, &mut fresh_budget(), fault).unwrap();
        assert!(matches!(batch.begin_case(), Err(FsError::BatchStopped)));
        batch.finish(&mut fresh_budget()).unwrap();
    }
}

#[test]
fn missing_and_empty_source_are_distinct_rejections() {
    let mut batch = SyntheticBatch::create().unwrap();
    for kind in [
        FixtureKind::MissingDb,
        FixtureKind::EmptyDb,
        FixtureKind::HotJournal,
    ] {
        let mut case = batch.begin_case().unwrap();
        let _fixture = create_synthetic_fixture(case.source_path(), kind).unwrap();
        case.register_generated_source(&mut fresh_budget()).unwrap();
        if matches!(kind, FixtureKind::MissingDb | FixtureKind::HotJournal) {
            assert!(case.seal_source(&mut fresh_budget()).is_err());
            case.dispose_empty_scratch(&mut fresh_budget()).unwrap();
        } else {
            let family = case.seal_source(&mut fresh_budget()).unwrap();
            let mut copy = case.capture(&family, &mut fresh_budget()).unwrap();
            copy.verify_copy(&mut fresh_budget()).unwrap();
            copy.mark_sqlite_started().unwrap();
            let outcome =
                _fixture.inspect_copy(Arc::new(|| Ok(())), &mut |_| Ok(()), &mut || Ok(()));
            copy.cleanup(&mut fresh_budget()).unwrap();
            assert!(matches!(outcome, Err(ObserverError::InputFormat)));
        }
        case.cleanup_source_and_case(&mut fresh_budget()).unwrap();
    }
    batch.finish(&mut fresh_budget()).unwrap();
}
