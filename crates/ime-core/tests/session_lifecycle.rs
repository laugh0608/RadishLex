use radishlex_ime_core::{
    Candidate, CandidateSource, Commit, CommitSource, Composition, CoreError, CoreResult, Engine,
    InputSession, Key, KeyEvent, KeyOutcome, KeyPhase, NamedKey, SchemaId,
};

struct StubEngine {
    schema: SchemaId,
    buffer: String,
}

impl StubEngine {
    fn new() -> Self {
        Self {
            schema: SchemaId::new("stub.default").expect("schema id is valid"),
            buffer: String::new(),
        }
    }

    fn available_candidates(&self) -> Vec<Candidate> {
        if self.buffer == "rad" {
            vec![Candidate::new("radish")
                .with_reading("rad")
                .with_annotation("stub candidate")
                .with_source(CandidateSource::Engine)]
        } else {
            Vec::new()
        }
    }
}

impl Engine for StubEngine {
    fn reset(&mut self) -> CoreResult<()> {
        self.buffer.clear();
        Ok(())
    }

    fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome> {
        if key.phase() != KeyPhase::Press {
            return Ok(KeyOutcome::ignored());
        }

        match key.key() {
            Key::Char(ch) => {
                self.buffer.push(ch);
                Ok(KeyOutcome::consumed())
            }
            Key::Named(NamedKey::Backspace) => {
                self.buffer.pop();
                Ok(KeyOutcome::consumed())
            }
            Key::Named(NamedKey::Enter) if !self.buffer.is_empty() => {
                let text = std::mem::take(&mut self.buffer);
                Ok(KeyOutcome::committed(Commit::new(
                    text,
                    CommitSource::RawText,
                )))
            }
            _ => Ok(KeyOutcome::ignored()),
        }
    }

    fn composition(&self) -> CoreResult<Composition> {
        Composition::new(self.buffer.clone(), self.buffer.len())
    }

    fn candidates(&self) -> CoreResult<Vec<Candidate>> {
        Ok(self.available_candidates())
    }

    fn commit_candidate(&mut self, index: usize) -> CoreResult<Commit> {
        let candidates = self.available_candidates();
        let Some(candidate) = candidates.get(index) else {
            return Err(CoreError::InvalidCandidateIndex {
                index,
                len: candidates.len(),
            });
        };

        self.buffer.clear();
        Ok(Commit::new(
            candidate.text().to_owned(),
            CommitSource::Candidate { index },
        ))
    }

    fn set_schema(&mut self, schema: SchemaId) -> CoreResult<()> {
        self.schema = schema;
        self.reset()
    }

    fn schema(&self) -> CoreResult<SchemaId> {
        Ok(self.schema.clone())
    }
}

#[test]
fn session_composes_lists_candidates_and_commits() {
    let mut session = InputSession::new(StubEngine::new());
    session
        .set_schema(SchemaId::new("stub.pinyin").expect("schema id is valid"))
        .expect("schema switch succeeds");

    for ch in ['r', 'a', 'd'] {
        let outcome = session
            .push_key(KeyEvent::press_char(ch))
            .expect("key press succeeds");
        assert!(outcome.is_consumed());
        assert!(outcome.commit().is_none());
    }

    let state = session.state().expect("state is readable");
    assert_eq!(state.schema().as_str(), "stub.pinyin");
    assert_eq!(state.composition().preedit(), "rad");
    assert_eq!(state.candidates().len(), 1);
    assert_eq!(state.candidates()[0].text(), "radish");
    assert_eq!(state.candidates()[0].reading(), Some("rad"));

    let commit = session
        .commit_candidate(0)
        .expect("candidate commit succeeds");
    assert_eq!(commit.text(), "radish");
    assert_eq!(commit.source(), &CommitSource::Candidate { index: 0 });

    let state = session.state().expect("state is readable after commit");
    assert!(state.composition().is_empty());
    assert!(state.candidates().is_empty());
}

#[test]
fn invalid_candidate_index_is_reported() {
    let mut session = InputSession::new(StubEngine::new());
    for ch in ['r', 'a', 'd'] {
        session
            .push_key(KeyEvent::press_char(ch))
            .expect("key press succeeds");
    }

    let err = session
        .commit_candidate(1)
        .expect_err("index must be rejected");
    assert_eq!(err, CoreError::InvalidCandidateIndex { index: 1, len: 1 });
}

#[test]
fn schema_id_rejects_empty_values() {
    let err = SchemaId::new("   ").expect_err("empty schema id must fail");
    assert_eq!(err, CoreError::EmptySchemaId);
}
