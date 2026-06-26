use std::str::FromStr;

use crate::error::{CoreError, CoreResult};

/// Stable identifier for an input schema.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemaId(String);

impl SchemaId {
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CoreError::EmptySchemaId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for SchemaId {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SchemaId {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Current preedit text and cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
    preedit: String,
    cursor: usize,
}

impl Composition {
    pub fn new(preedit: impl Into<String>, cursor: usize) -> CoreResult<Self> {
        let preedit = preedit.into();
        if !preedit.is_char_boundary(cursor) {
            return Err(CoreError::InvalidCompositionCursor {
                cursor,
                byte_len: preedit.len(),
            });
        }
        Ok(Self { preedit, cursor })
    }

    pub fn empty() -> Self {
        Self {
            preedit: String::new(),
            cursor: 0,
        }
    }

    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_empty(&self) -> bool {
        self.preedit.is_empty()
    }
}

/// Candidate source after conversion into RadishLex core model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateSource {
    Engine,
    UserDictionary,
    Personalized,
    System,
}

/// Candidate displayed to a user or passed to a future ranker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    text: String,
    reading: Option<String>,
    annotation: Option<String>,
    source: CandidateSource,
}

impl Candidate {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            reading: None,
            annotation: None,
            source: CandidateSource::Engine,
        }
    }

    pub fn with_reading(mut self, reading: impl Into<String>) -> Self {
        self.reading = Some(reading.into());
        self
    }

    pub fn with_annotation(mut self, annotation: impl Into<String>) -> Self {
        self.annotation = Some(annotation.into());
        self
    }

    pub fn with_source(mut self, source: CandidateSource) -> Self {
        self.source = source;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn reading(&self) -> Option<&str> {
        self.reading.as_deref()
    }

    pub fn annotation(&self) -> Option<&str> {
        self.annotation.as_deref()
    }

    pub fn source(&self) -> CandidateSource {
        self.source
    }
}

/// Where a committed text came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitSource {
    Candidate { index: usize },
    RawText,
    Engine,
}

/// Text committed back to the host application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    text: String,
    source: CommitSource,
}

impl Commit {
    pub fn new(text: impl Into<String>, source: CommitSource) -> Self {
        Self {
            text: text.into(),
            source,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn source(&self) -> &CommitSource {
        &self.source
    }
}

/// Snapshot of input state exposed by an input session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionState {
    composition: Composition,
    candidates: Vec<Candidate>,
    schema: SchemaId,
}

impl SessionState {
    pub fn new(composition: Composition, candidates: Vec<Candidate>, schema: SchemaId) -> Self {
        Self {
            composition,
            candidates,
            schema,
        }
    }

    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    pub fn schema(&self) -> &SchemaId {
        &self.schema
    }
}
