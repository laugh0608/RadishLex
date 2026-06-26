use radishlex_ime_userdb::{TermSource, TermStatus, UserDb, UserTerm};

use crate::error::FfiError;
use crate::snapshot::RadishLexStringView;

pub const RADISHLEX_TERM_SOURCE_ENGINE_SELECTION: u32 = 1;
pub const RADISHLEX_TERM_SOURCE_MANUAL_IMPORT: u32 = 2;
pub const RADISHLEX_TERM_SOURCE_MANUAL_ADD: u32 = 3;
pub const RADISHLEX_TERM_SOURCE_PHRASE_LEARNING: u32 = 4;

pub const RADISHLEX_TERM_STATUS_ACTIVE: u32 = 1;
pub const RADISHLEX_TERM_STATUS_SUPPRESSED: u32 = 2;
pub const RADISHLEX_TERM_STATUS_DELETED: u32 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadishLexUserTermView {
    pub id: i64,
    pub input_code: RadishLexStringView,
    pub text: RadishLexStringView,
    pub reading: RadishLexStringView,
    pub reading_present: u8,
    pub source: u32,
    pub status: u32,
    pub weight: f64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub last_used_at_ms: i64,
    pub last_used_at_present: u8,
}

impl RadishLexUserTermView {
    pub const fn empty() -> Self {
        Self {
            id: 0,
            input_code: RadishLexStringView::empty(),
            text: RadishLexStringView::empty(),
            reading: RadishLexStringView::empty(),
            reading_present: 0,
            source: 0,
            status: 0,
            weight: 0.0,
            created_at_ms: 0,
            updated_at_ms: 0,
            last_used_at_ms: 0,
            last_used_at_present: 0,
        }
    }
}

pub struct RadishLexUserTermList {
    terms: Vec<UserTerm>,
}

impl RadishLexUserTermList {
    pub fn new(terms: Vec<UserTerm>) -> Self {
        Self { terms }
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn term_view(&self, index: usize) -> Result<RadishLexUserTermView, FfiError> {
        let Some(term) = self.terms.get(index) else {
            return Err(FfiError::invalid_argument(format!(
                "user term index {index} is out of range for {} terms",
                self.terms.len()
            )));
        };

        Ok(term_view(term))
    }

    pub unsafe fn free(list: *mut Self) {
        if list.is_null() {
            return;
        }

        let _ = Box::from_raw(list);
    }
}

pub fn add_user_term(
    db_path: &str,
    input_code: &str,
    text: &str,
    reading: Option<&str>,
) -> Result<(), FfiError> {
    let mut db = UserDb::open(db_path)?;
    db.add_term(input_code, text, reading, TermSource::ManualAdd)?;
    Ok(())
}

pub fn delete_user_term(
    db_path: &str,
    input_code: &str,
    text: &str,
    reading: Option<&str>,
) -> Result<(), FfiError> {
    let mut db = UserDb::open(db_path)?;
    db.delete_term(input_code, text, reading)?;
    Ok(())
}

pub fn list_user_terms(db_path: &str) -> Result<RadishLexUserTermList, FfiError> {
    let db = UserDb::open(db_path)?;
    Ok(RadishLexUserTermList::new(db.list_active_terms()?))
}

fn term_view(term: &UserTerm) -> RadishLexUserTermView {
    let last_used_at_ms = term.last_used_at_ms.unwrap_or_default();
    RadishLexUserTermView {
        id: term.id,
        input_code: RadishLexStringView::from_str(&term.input_code),
        text: RadishLexStringView::from_str(&term.text),
        reading: optional_view(term.reading.as_deref()),
        reading_present: presence_flag(term.reading.as_deref()),
        source: term_source_code(term.source),
        status: term_status_code(term.status),
        weight: term.weight,
        created_at_ms: term.created_at_ms,
        updated_at_ms: term.updated_at_ms,
        last_used_at_ms,
        last_used_at_present: u8::from(term.last_used_at_ms.is_some()),
    }
}

fn optional_view(value: Option<&str>) -> RadishLexStringView {
    value.map_or_else(RadishLexStringView::empty, RadishLexStringView::from_str)
}

fn presence_flag(value: Option<&str>) -> u8 {
    u8::from(value.is_some())
}

fn term_source_code(source: TermSource) -> u32 {
    match source {
        TermSource::EngineSelection => RADISHLEX_TERM_SOURCE_ENGINE_SELECTION,
        TermSource::ManualImport => RADISHLEX_TERM_SOURCE_MANUAL_IMPORT,
        TermSource::ManualAdd => RADISHLEX_TERM_SOURCE_MANUAL_ADD,
        TermSource::PhraseLearning => RADISHLEX_TERM_SOURCE_PHRASE_LEARNING,
    }
}

fn term_status_code(status: TermStatus) -> u32 {
    match status {
        TermStatus::Active => RADISHLEX_TERM_STATUS_ACTIVE,
        TermStatus::Suppressed => RADISHLEX_TERM_STATUS_SUPPRESSED,
        TermStatus::Deleted => RADISHLEX_TERM_STATUS_DELETED,
    }
}
