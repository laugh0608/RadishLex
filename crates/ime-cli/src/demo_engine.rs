use radishlex_ime_core::{
    Candidate, CandidateSource, Commit, CommitSource, Composition, CoreError, CoreResult, Engine,
    Key, KeyEvent, KeyOutcome, KeyPhase, NamedKey, SchemaId,
};

/// Deterministic demo adapter used by the CLI before a real engine exists.
pub struct DemoEngine {
    schema: SchemaId,
    buffer: String,
}

impl DemoEngine {
    pub fn new() -> Self {
        Self {
            schema: SchemaId::new("demo.pinyin").expect("demo schema id is valid"),
            buffer: String::new(),
        }
    }

    fn demo_candidates(&self) -> Vec<Candidate> {
        match self.buffer.as_str() {
            "luobo" => vec![
                Candidate::new("萝卜")
                    .with_reading("luobo")
                    .with_annotation("demo candidate")
                    .with_source(CandidateSource::Engine),
                Candidate::new("萝卜词核")
                    .with_reading("luobo")
                    .with_annotation("project term")
                    .with_source(CandidateSource::Engine),
            ],
            "radishlex" => vec![Candidate::new("萝卜词核")
                .with_reading("radishlex")
                .with_annotation("project name")
                .with_source(CandidateSource::Engine)],
            "zhong" => vec![
                Candidate::new("中")
                    .with_reading("zhong")
                    .with_annotation("demo candidate")
                    .with_source(CandidateSource::Engine),
                Candidate::new("种")
                    .with_reading("zhong")
                    .with_annotation("demo candidate")
                    .with_source(CandidateSource::Engine),
            ],
            _ => Vec::new(),
        }
    }
}

impl Default for DemoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for DemoEngine {
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
        Ok(self.demo_candidates())
    }

    fn commit_candidate(&mut self, index: usize) -> CoreResult<Commit> {
        let candidates = self.demo_candidates();
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

#[cfg(test)]
mod tests {
    use radishlex_ime_core::{Engine, KeyEvent};

    use super::DemoEngine;

    #[test]
    fn demo_engine_returns_synthetic_candidates() {
        let mut engine = DemoEngine::new();
        for ch in "luobo".chars() {
            engine
                .push_key(KeyEvent::press_char(ch))
                .expect("key press succeeds");
        }

        let candidates = engine.candidates().expect("candidates are readable");
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].text(), "萝卜");
        assert_eq!(candidates[1].text(), "萝卜词核");
    }
}
