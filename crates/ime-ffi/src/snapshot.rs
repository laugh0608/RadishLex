use std::ptr;

use radishlex_ime_core::{Candidate, CandidateSource, SessionState};

use crate::error::FfiError;

pub const RADISHLEX_CANDIDATE_SOURCE_ENGINE: u32 = 1;
pub const RADISHLEX_CANDIDATE_SOURCE_USER_DICTIONARY: u32 = 2;
pub const RADISHLEX_CANDIDATE_SOURCE_PERSONALIZED: u32 = 3;
pub const RADISHLEX_CANDIDATE_SOURCE_SYSTEM: u32 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexStringView {
    pub data: *const u8,
    pub len: usize,
}

impl RadishLexStringView {
    pub const fn empty() -> Self {
        Self {
            data: ptr::null(),
            len: 0,
        }
    }

    fn from_str(value: &str) -> Self {
        Self {
            data: value.as_ptr(),
            len: value.len(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexCandidateView {
    pub index: usize,
    pub text: RadishLexStringView,
    pub reading: RadishLexStringView,
    pub reading_present: u8,
    pub annotation: RadishLexStringView,
    pub annotation_present: u8,
    pub source: u32,
}

impl RadishLexCandidateView {
    pub const fn empty() -> Self {
        Self {
            index: 0,
            text: RadishLexStringView::empty(),
            reading: RadishLexStringView::empty(),
            reading_present: 0,
            annotation: RadishLexStringView::empty(),
            annotation_present: 0,
            source: 0,
        }
    }
}

pub struct RadishLexSnapshot {
    schema: String,
    preedit: String,
    cursor: usize,
    candidates: Vec<RadishLexCandidateSnapshot>,
}

impl RadishLexSnapshot {
    pub fn from_state(state: SessionState) -> Self {
        Self {
            schema: state.schema().as_str().to_owned(),
            preedit: state.composition().preedit().to_owned(),
            cursor: state.composition().cursor(),
            candidates: state
                .candidates()
                .iter()
                .map(RadishLexCandidateSnapshot::from_candidate)
                .collect(),
        }
    }

    pub fn schema(&self) -> RadishLexStringView {
        RadishLexStringView::from_str(&self.schema)
    }

    pub fn preedit(&self) -> RadishLexStringView {
        RadishLexStringView::from_str(&self.preedit)
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    pub fn candidate_view(&self, index: usize) -> Result<RadishLexCandidateView, FfiError> {
        let Some(candidate) = self.candidates.get(index) else {
            return Err(FfiError::invalid_argument(format!(
                "candidate index {index} is out of range for {} candidates",
                self.candidates.len()
            )));
        };

        Ok(candidate.view(index))
    }

    pub unsafe fn free(snapshot: *mut Self) {
        if snapshot.is_null() {
            return;
        }

        let _ = Box::from_raw(snapshot);
    }
}

struct RadishLexCandidateSnapshot {
    text: String,
    reading: Option<String>,
    annotation: Option<String>,
    source: u32,
}

impl RadishLexCandidateSnapshot {
    fn from_candidate(candidate: &Candidate) -> Self {
        Self {
            text: candidate.text().to_owned(),
            reading: candidate.reading().map(ToOwned::to_owned),
            annotation: candidate.annotation().map(ToOwned::to_owned),
            source: candidate_source_code(candidate.source()),
        }
    }

    fn view(&self, index: usize) -> RadishLexCandidateView {
        RadishLexCandidateView {
            index,
            text: RadishLexStringView::from_str(&self.text),
            reading: optional_view(self.reading.as_deref()),
            reading_present: presence_flag(self.reading.as_deref()),
            annotation: optional_view(self.annotation.as_deref()),
            annotation_present: presence_flag(self.annotation.as_deref()),
            source: self.source,
        }
    }
}

fn optional_view(value: Option<&str>) -> RadishLexStringView {
    value.map_or_else(RadishLexStringView::empty, RadishLexStringView::from_str)
}

fn presence_flag(value: Option<&str>) -> u8 {
    u8::from(value.is_some())
}

fn candidate_source_code(source: CandidateSource) -> u32 {
    match source {
        CandidateSource::Engine => RADISHLEX_CANDIDATE_SOURCE_ENGINE,
        CandidateSource::UserDictionary => RADISHLEX_CANDIDATE_SOURCE_USER_DICTIONARY,
        CandidateSource::Personalized => RADISHLEX_CANDIDATE_SOURCE_PERSONALIZED,
        CandidateSource::System => RADISHLEX_CANDIDATE_SOURCE_SYSTEM,
    }
}
