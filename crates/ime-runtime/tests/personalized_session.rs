use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use radishlex_ime_core::{
    Candidate, CandidateSource, Commit, CommitSource, Composition, CoreError, CoreResult, Engine,
    Key, KeyEvent, KeyOutcome, KeyPhase, NamedKey, SchemaId,
};
use radishlex_ime_runtime::{
    LearningContext, LearningDisposition, PersonalizationStatus, PersonalizedInputSession,
};
use radishlex_ime_userdb::{SelectionEventDraft, UserDb};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TempUserDb {
    directory: PathBuf,
    path: PathBuf,
}

impl TempUserDb {
    fn new(label: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "radishlex-ime-runtime-{label}-{}-{timestamp}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temp directory creates");
        let path = directory.join("userdb.sqlite3");
        Self { directory, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempUserDb {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct TestEngine {
    schema: SchemaId,
    buffer: String,
    segmented_selection: bool,
    pending_commit: Option<String>,
}

impl TestEngine {
    fn immediate() -> Self {
        Self::new(false)
    }

    fn segmented() -> Self {
        Self::new(true)
    }

    fn new(segmented_selection: bool) -> Self {
        Self {
            schema: SchemaId::new("test.pinyin").expect("schema"),
            buffer: String::new(),
            segmented_selection,
            pending_commit: None,
        }
    }

    fn current_candidates(&self) -> Vec<Candidate> {
        match self.buffer.as_str() {
            "luobo" => vec![
                Candidate::new("落泊")
                    .with_reading("luobo")
                    .with_source(CandidateSource::Engine),
                Candidate::new("萝卜")
                    .with_reading("luobo")
                    .with_source(CandidateSource::Engine),
            ],
            "bulk" => (0..50)
                .map(|index| {
                    Candidate::new(format!("合成候选{index:02}"))
                        .with_reading("bulk")
                        .with_source(CandidateSource::Engine)
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl Engine for TestEngine {
    fn reset(&mut self) -> CoreResult<()> {
        self.buffer.clear();
        self.pending_commit = None;
        Ok(())
    }

    fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome> {
        if key.phase() != KeyPhase::Press {
            return Ok(KeyOutcome::ignored());
        }
        match key.key() {
            Key::Char(ch) if ch.is_ascii_alphanumeric() => {
                self.buffer.push(ch.to_ascii_lowercase());
                Ok(KeyOutcome::consumed())
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(text) = self.pending_commit.take() {
                    self.buffer.clear();
                    Ok(KeyOutcome::committed(Commit::new(
                        text,
                        CommitSource::Engine,
                    )))
                } else {
                    Ok(KeyOutcome::ignored())
                }
            }
            Key::Named(NamedKey::Backspace) => {
                self.buffer.pop();
                Ok(KeyOutcome::consumed())
            }
            _ => Ok(KeyOutcome::ignored()),
        }
    }

    fn composition(&self) -> CoreResult<Composition> {
        Composition::new(self.buffer.clone(), self.buffer.len())
    }

    fn candidates(&self) -> CoreResult<Vec<Candidate>> {
        Ok(self.current_candidates())
    }

    fn input_code(&self) -> CoreResult<String> {
        Ok(self.buffer.clone())
    }

    fn select_candidate(&mut self, index: usize) -> CoreResult<KeyOutcome> {
        let candidates = self.current_candidates();
        let candidate = candidates
            .get(index)
            .ok_or(CoreError::InvalidCandidateIndex {
                index,
                len: candidates.len(),
            })?;
        if self.segmented_selection {
            self.pending_commit = Some(candidate.text().to_owned());
            self.buffer = "segment".to_owned();
            Ok(KeyOutcome::consumed())
        } else {
            self.buffer.clear();
            Ok(KeyOutcome::committed(Commit::new(
                candidate.text(),
                CommitSource::Candidate { index },
            )))
        }
    }

    fn set_schema(&mut self, schema: SchemaId) -> CoreResult<()> {
        self.schema = schema;
        self.reset()
    }

    fn schema(&self) -> CoreResult<SchemaId> {
        Ok(self.schema.clone())
    }
}

fn type_input(session: &mut PersonalizedInputSession<TestEngine>, input: &str) {
    for ch in input.chars() {
        let result = session
            .handle_key(KeyEvent::press_char(ch))
            .expect("key handles");
        assert!(result.outcome().is_consumed());
    }
}

fn candidate_texts(session: &mut PersonalizedInputSession<TestEngine>) -> Vec<String> {
    session
        .snapshot()
        .expect("snapshot")
        .candidates()
        .iter()
        .map(|candidate| candidate.candidate().text().to_owned())
        .collect()
}

#[test]
fn selection_changes_display_order_maps_back_to_engine_and_persists_restart() {
    let temp = TempUserDb::new("persistence");
    {
        let mut session =
            PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "session-a")
                .expect("runtime opens");
        type_input(&mut session, "luobo");
        assert_eq!(candidate_texts(&mut session), ["落泊", "萝卜"]);
        let selected = session.select_candidate(1).expect("candidate selects");
        assert_eq!(selected.outcome().commit().expect("commit").text(), "萝卜");
        assert_eq!(
            selected.learning_disposition(),
            LearningDisposition::Recorded
        );
    }

    let mut restarted =
        PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "session-b")
            .expect("runtime reopens");
    type_input(&mut restarted, "luobo");
    let snapshot = restarted.snapshot().expect("ranked snapshot");
    assert_eq!(snapshot.candidates()[0].candidate().text(), "萝卜");
    assert_eq!(snapshot.candidates()[0].display_index(), 0);
    assert_eq!(snapshot.candidates()[0].engine_index(), 1);
    let selected = restarted
        .select_candidate(0)
        .expect("display candidate maps");
    assert_eq!(
        selected.outcome().commit().expect("commit").source(),
        &CommitSource::Candidate { index: 1 }
    );

    drop(restarted);
    let db = UserDb::open(temp.path()).expect("userdb opens");
    assert_eq!(db.selection_event_count().expect("count"), 2);
}

#[test]
fn secure_unknown_and_privacy_contexts_do_not_write() {
    let temp = TempUserDb::new("privacy");
    let mut seed = UserDb::open(temp.path()).expect("userdb opens");
    seed.record_selection(
        SelectionEventDraft::new("seed", "luobo", "萝卜", 1, 2).with_reading("luobo"),
    )
    .expect("seed records");
    drop(seed);

    let mut privacy =
        PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "privacy-session")
            .expect("runtime opens");
    privacy.set_learning_context(
        LearningContext::new("editor")
            .expect("context")
            .with_privacy_mode(true),
    );
    type_input(&mut privacy, "luobo");
    assert_eq!(candidate_texts(&mut privacy)[0], "萝卜");
    let selected = privacy.select_candidate(0).expect("candidate selects");
    assert_eq!(
        selected.learning_disposition(),
        LearningDisposition::SkippedByPolicy
    );
    drop(privacy);

    for context in [
        LearningContext::new("general")
            .expect("context")
            .with_secure_input(true),
        LearningContext::new("general")
            .expect("context")
            .with_context_known(false),
        LearningContext::new("general")
            .expect("context")
            .with_sensitive_application(true),
    ] {
        let mut session =
            PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "blocked-session")
                .expect("runtime opens");
        session.set_learning_context(context);
        type_input(&mut session, "luobo");
        let snapshot = session.snapshot().expect("snapshot");
        assert_eq!(
            snapshot.personalization_status(),
            PersonalizationStatus::PolicyBlocked
        );
        assert_eq!(snapshot.candidates()[0].candidate().text(), "落泊");
        assert_eq!(
            session
                .select_candidate(1)
                .expect("candidate selects")
                .learning_disposition(),
            LearningDisposition::SkippedByPolicy
        );
    }

    let db = UserDb::open(temp.path()).expect("userdb opens");
    assert_eq!(db.selection_event_count().expect("count"), 1);
}

#[test]
fn context_transition_invalidates_personalized_order_before_selection() {
    let temp = TempUserDb::new("policy-transition");
    let mut seed = UserDb::open(temp.path()).expect("userdb opens");
    seed.record_selection(
        SelectionEventDraft::new("seed", "luobo", "萝卜", 1, 2).with_reading("luobo"),
    )
    .expect("seed records");
    drop(seed);

    let mut session =
        PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "transition-session")
            .expect("runtime opens");
    type_input(&mut session, "luobo");
    assert_eq!(candidate_texts(&mut session)[0], "萝卜");

    session.set_learning_context(
        LearningContext::new("general")
            .expect("context")
            .with_secure_input(true),
    );
    let snapshot = session.snapshot().expect("engine-only snapshot");
    assert_eq!(
        snapshot.personalization_status(),
        PersonalizationStatus::PolicyBlocked
    );
    assert_eq!(snapshot.candidates()[0].candidate().text(), "落泊");
    assert_eq!(snapshot.candidates()[0].engine_index(), 0);
}

#[test]
fn segmented_selection_records_only_after_matching_commit() {
    let temp = TempUserDb::new("segmented");
    let mut session =
        PersonalizedInputSession::open(TestEngine::segmented(), temp.path(), "segmented-session")
            .expect("runtime opens");
    type_input(&mut session, "luobo");
    let selected = session.select_candidate(1).expect("segment selects");
    assert!(selected.outcome().commit().is_none());
    assert_eq!(
        selected.learning_disposition(),
        LearningDisposition::Deferred
    );
    {
        let db = UserDb::open(temp.path()).expect("userdb opens");
        assert_eq!(db.selection_event_count().expect("count"), 0);
    }
    let committed = session
        .handle_key(KeyEvent::press(Key::Named(NamedKey::Enter)))
        .expect("segment commits");
    assert_eq!(committed.outcome().commit().expect("commit").text(), "萝卜");
    assert_eq!(
        committed.learning_disposition(),
        LearningDisposition::Recorded
    );
    drop(session);
    let db = UserDb::open(temp.path()).expect("userdb opens");
    assert_eq!(db.selection_event_count().expect("count"), 1);
}

#[test]
fn context_change_clears_deferred_selection() {
    let temp = TempUserDb::new("context-change");
    let mut session =
        PersonalizedInputSession::open(TestEngine::segmented(), temp.path(), "segmented-session")
            .expect("runtime opens");
    type_input(&mut session, "luobo");
    session.select_candidate(1).expect("segment selects");
    session.set_learning_context(
        LearningContext::new("editor")
            .expect("context")
            .with_privacy_mode(true),
    );
    let committed = session
        .handle_key(KeyEvent::press(Key::Named(NamedKey::Enter)))
        .expect("segment commits");
    assert!(committed.outcome().commit().is_some());
    assert_eq!(
        committed.learning_disposition(),
        LearningDisposition::NotApplicable
    );
    drop(session);
    let db = UserDb::open(temp.path()).expect("userdb opens");
    assert_eq!(db.selection_event_count().expect("count"), 0);
}

#[test]
fn learning_write_failure_keeps_commit_and_rolls_back_selection() {
    let temp = TempUserDb::new("write-failure");
    let mut session =
        PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "failure-session")
            .expect("runtime opens");
    let connection = rusqlite::Connection::open(temp.path()).expect("second connection opens");
    connection
        .execute_batch(
            "CREATE TRIGGER fail_runtime_selection
             BEFORE INSERT ON selection_events
             BEGIN
               SELECT RAISE(FAIL, 'synthetic runtime write failure');
             END;",
        )
        .expect("failure trigger creates");
    drop(connection);

    type_input(&mut session, "luobo");
    let selected = session
        .select_candidate(1)
        .expect("engine selection succeeds");
    assert_eq!(
        selected.outcome().commit().expect("commit survives").text(),
        "萝卜"
    );
    assert_eq!(selected.learning_disposition(), LearningDisposition::Failed);
    drop(session);

    let db = UserDb::open(temp.path()).expect("userdb opens");
    assert_eq!(db.selection_event_count().expect("count"), 0);
    assert!(db
        .fetch_term("luobo", "萝卜", "luobo")
        .expect("term query")
        .is_none());
}

#[test]
fn ranking_read_failure_falls_back_but_blocked_policy_does_not_read() {
    let temp = TempUserDb::new("read-failure");
    let mut session =
        PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "read-session")
            .expect("runtime opens");
    let connection = rusqlite::Connection::open(temp.path()).expect("second connection opens");
    connection
        .execute_batch("DROP TABLE ranker_weights;")
        .expect("ranking table drops");
    drop(connection);

    type_input(&mut session, "luobo");
    let snapshot = session.snapshot().expect("fallback snapshot");
    assert_eq!(
        snapshot.personalization_status(),
        PersonalizationStatus::ReadFailed
    );
    assert_eq!(snapshot.candidates()[0].candidate().text(), "落泊");

    session.reset().expect("session resets");
    session.set_learning_context(
        LearningContext::new("general")
            .expect("context")
            .with_secure_input(true),
    );
    type_input(&mut session, "luobo");
    let snapshot = session.snapshot().expect("policy snapshot");
    assert_eq!(
        snapshot.personalization_status(),
        PersonalizationStatus::PolicyBlocked
    );
}

#[test]
fn delete_blocks_old_learning_until_explicit_restore() {
    let temp = TempUserDb::new("restore");
    let mut db = UserDb::open(temp.path()).expect("userdb opens");
    db.record_selection(
        SelectionEventDraft::new("seed", "luobo", "萝卜", 1, 2).with_reading("luobo"),
    )
    .expect("selection seeds");
    db.delete_term("luobo", "萝卜", Some("luobo"))
        .expect("term deletes");
    drop(db);

    let mut deleted =
        PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "deleted-session")
            .expect("runtime opens");
    type_input(&mut deleted, "luobo");
    assert_eq!(candidate_texts(&mut deleted)[0], "落泊");
    let selected = deleted.select_candidate(1).expect("selection commits");
    assert_eq!(
        selected.learning_disposition(),
        LearningDisposition::Recorded
    );
    drop(deleted);

    let mut db = UserDb::open(temp.path()).expect("userdb opens");
    db.restore_term("luobo", "萝卜", Some("luobo"))
        .expect("term restores explicitly");
    drop(db);
    let mut restored =
        PersonalizedInputSession::open(TestEngine::immediate(), temp.path(), "restored-session")
            .expect("runtime opens");
    type_input(&mut restored, "luobo");
    assert_eq!(candidate_texts(&mut restored)[0], "萝卜");
}

#[test]
fn integrated_fifty_candidate_latency_is_observational_not_a_ci_threshold() {
    const WARM_UP_ITERATIONS: usize = 20;
    const MEASURED_ITERATIONS: usize = 200;

    let db = UserDb::open_in_memory().expect("userdb opens");
    let mut session = PersonalizedInputSession::with_userdb(TestEngine::immediate(), db, "latency")
        .expect("runtime opens");
    type_input(&mut session, "bulk");
    assert_eq!(session.snapshot().expect("snapshot").candidates().len(), 50);
    for _ in 0..WARM_UP_ITERATIONS {
        assert_eq!(session.snapshot().expect("warmup").candidates().len(), 50);
    }

    let mut samples = Vec::with_capacity(MEASURED_ITERATIONS);
    for _ in 0..MEASURED_ITERATIONS {
        let started = Instant::now();
        assert_eq!(session.snapshot().expect("measured").candidates().len(), 50);
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let p95 = samples[samples.len() * 95 / 100];
    assert!(p95 >= median);
    eprintln!(
        "R01B runtime latency baseline: candidates=50 warmup={WARM_UP_ITERATIONS} measured={MEASURED_ITERATIONS} median_us={} p95_us={}",
        duration_micros(median),
        duration_micros(p95)
    );
}

fn duration_micros(duration: Duration) -> u128 {
    duration.as_micros()
}
