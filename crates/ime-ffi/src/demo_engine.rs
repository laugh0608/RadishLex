use radishlex_ime_core::{
    Candidate, CandidateSource, Commit, CommitSource, Composition, CoreError, CoreResult, Engine,
    Key, KeyEvent, KeyOutcome, KeyPhase, NamedKey, SchemaId,
};

pub struct FfiDemoEngine {
    schema: SchemaId,
    buffer: String,
}

impl FfiDemoEngine {
    pub fn new() -> Self {
        Self {
            schema: SchemaId::new("ffi.demo").expect("FFI demo schema id is valid"),
            buffer: String::new(),
        }
    }

    fn candidates_for_buffer(&self) -> Vec<Candidate> {
        match self.buffer.as_str() {
            "luobo" => vec![
                Candidate::new("萝卜")
                    .with_reading("luobo")
                    .with_annotation("ffi host smoke")
                    .with_source(CandidateSource::Engine),
                Candidate::new("萝卜词核")
                    .with_reading("luobo")
                    .with_annotation("project term")
                    .with_source(CandidateSource::Engine),
            ],
            "cihe" => vec![Candidate::new("词核")
                .with_reading("cihe")
                .with_annotation("project term")
                .with_source(CandidateSource::Engine)],
            _ => Vec::new(),
        }
    }
}

impl Default for FfiDemoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for FfiDemoEngine {
    fn reset(&mut self) -> CoreResult<()> {
        self.buffer.clear();
        Ok(())
    }

    fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome> {
        if key.phase() != KeyPhase::Press {
            return Ok(KeyOutcome::ignored());
        }

        match key.key() {
            Key::Char(ch) if ch.is_ascii_alphanumeric() || ch == '\'' => {
                self.buffer.push(ch.to_ascii_lowercase());
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
        Ok(self.candidates_for_buffer())
    }

    fn commit_candidate(&mut self, index: usize) -> CoreResult<Commit> {
        let candidates = self.candidates_for_buffer();
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
