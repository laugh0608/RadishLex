use std::fs;

use radishlex_ime_userdb::{
    decode_dictionary_terms_tsv_document, encode_dictionary_terms_tsv, DictionaryImportBatch,
    DictionaryImportSummary, DictionaryTermsFormat, TermSource, TermStatus, UserDb, UserTerm,
};

use crate::error::FfiError;
use crate::snapshot::RadishLexStringView;

pub const RADISHLEX_TERM_SOURCE_ENGINE_SELECTION: u32 = 1;
pub const RADISHLEX_TERM_SOURCE_MANUAL_IMPORT: u32 = 2;
pub const RADISHLEX_TERM_SOURCE_MANUAL_ADD: u32 = 3;
pub const RADISHLEX_TERM_SOURCE_PHRASE_LEARNING: u32 = 4;

pub const RADISHLEX_TERM_STATUS_ACTIVE: u32 = 1;
pub const RADISHLEX_TERM_STATUS_SUPPRESSED: u32 = 2;
pub const RADISHLEX_TERM_STATUS_DELETED: u32 = 3;

pub const RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1: u32 = 1;
pub const RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC: u32 = 2;

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

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexDictionaryInspectSummary {
    pub format_version: u32,
    pub record_count: usize,
    pub sync_class: u32,
}

impl RadishLexDictionaryInspectSummary {
    pub const fn empty() -> Self {
        Self {
            format_version: 0,
            record_count: 0,
            sync_class: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexDictionaryExportSummary {
    pub format_version: u32,
    pub exported_terms: usize,
    pub sync_class: u32,
}

impl RadishLexDictionaryExportSummary {
    pub const fn empty() -> Self {
        Self {
            format_version: 0,
            exported_terms: 0,
            sync_class: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexDictionaryImportSummary {
    pub import_batch_id: i64,
    pub import_batch_id_present: u8,
    pub total_records: usize,
    pub imported_terms: usize,
    pub inserted_terms: usize,
    pub updated_terms: usize,
    pub skipped_deleted_terms: usize,
    pub skipped_duplicate_terms: usize,
    pub dry_run: u8,
}

impl RadishLexDictionaryImportSummary {
    pub const fn empty() -> Self {
        Self {
            import_batch_id: 0,
            import_batch_id_present: 0,
            total_records: 0,
            imported_terms: 0,
            inserted_terms: 0,
            updated_terms: 0,
            skipped_deleted_terms: 0,
            skipped_duplicate_terms: 0,
            dry_run: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadishLexImportBatchView {
    pub id: i64,
    pub source_name: RadishLexStringView,
    pub total_records: usize,
    pub imported_terms: usize,
    pub inserted_terms: usize,
    pub updated_terms: usize,
    pub skipped_deleted_terms: usize,
    pub skipped_duplicate_terms: usize,
    pub created_at_ms: i64,
    pub notes: RadishLexStringView,
    pub notes_present: u8,
}

impl RadishLexImportBatchView {
    pub const fn empty() -> Self {
        Self {
            id: 0,
            source_name: RadishLexStringView::empty(),
            total_records: 0,
            imported_terms: 0,
            inserted_terms: 0,
            updated_terms: 0,
            skipped_deleted_terms: 0,
            skipped_duplicate_terms: 0,
            created_at_ms: 0,
            notes: RadishLexStringView::empty(),
            notes_present: 0,
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

pub struct RadishLexImportBatchList {
    batches: Vec<DictionaryImportBatch>,
}

impl RadishLexImportBatchList {
    pub fn new(batches: Vec<DictionaryImportBatch>) -> Self {
        Self { batches }
    }

    pub fn len(&self) -> usize {
        self.batches.len()
    }

    pub fn batch_view(&self, index: usize) -> Result<RadishLexImportBatchView, FfiError> {
        let Some(batch) = self.batches.get(index) else {
            return Err(FfiError::invalid_argument(format!(
                "import batch index {index} is out of range for {} batches",
                self.batches.len()
            )));
        };

        Ok(import_batch_view(batch))
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

pub fn inspect_dictionary_file(
    file_path: &str,
) -> Result<RadishLexDictionaryInspectSummary, FfiError> {
    let encoded = read_dictionary_file(file_path)?;
    let document = decode_dictionary_terms_tsv_document(&encoded)?;

    Ok(RadishLexDictionaryInspectSummary {
        format_version: dictionary_format_code(document.format),
        record_count: document.records.len(),
        sync_class: RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC,
    })
}

pub fn export_dictionary_file(
    db_path: &str,
    file_path: &str,
) -> Result<RadishLexDictionaryExportSummary, FfiError> {
    let db = UserDb::open(db_path)?;
    let records = db.export_dictionary_records()?;
    fs::write(file_path, encode_dictionary_terms_tsv(&records)).map_err(|error| {
        FfiError::new(
            crate::error::RadishLexStatusCode::UserDbError,
            format!("failed to write dictionary export file {file_path}: {error}"),
        )
    })?;

    Ok(RadishLexDictionaryExportSummary {
        format_version: RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1,
        exported_terms: records.len(),
        sync_class: RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC,
    })
}

pub fn import_dictionary_file(
    db_path: &str,
    file_path: &str,
    source_name: Option<&str>,
    dry_run: bool,
) -> Result<RadishLexDictionaryImportSummary, FfiError> {
    let source_name = source_name.unwrap_or("ffi");
    let encoded = read_dictionary_file(file_path)?;
    let document = decode_dictionary_terms_tsv_document(&encoded)?;

    let mut db = UserDb::open(db_path)?;
    let summary = if dry_run {
        db.preview_dictionary_import(&document.records, source_name)?
    } else {
        db.import_dictionary_records(&document.records, source_name)?
    };

    Ok(import_summary(summary, dry_run))
}

pub fn list_import_batches(db_path: &str) -> Result<RadishLexImportBatchList, FfiError> {
    let db = UserDb::open(db_path)?;
    Ok(RadishLexImportBatchList::new(db.list_import_batches()?))
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

fn import_batch_view(batch: &DictionaryImportBatch) -> RadishLexImportBatchView {
    RadishLexImportBatchView {
        id: batch.id,
        source_name: RadishLexStringView::from_str(&batch.source_name),
        total_records: batch.total_records,
        imported_terms: batch.imported_terms,
        inserted_terms: batch.inserted_terms,
        updated_terms: batch.updated_terms,
        skipped_deleted_terms: batch.skipped_deleted_terms,
        skipped_duplicate_terms: batch.skipped_duplicate_terms,
        created_at_ms: batch.created_at_ms,
        notes: RadishLexStringView::from_str(&batch.notes),
        notes_present: u8::from(!batch.notes.is_empty()),
    }
}

fn import_summary(
    summary: DictionaryImportSummary,
    dry_run: bool,
) -> RadishLexDictionaryImportSummary {
    RadishLexDictionaryImportSummary {
        import_batch_id: summary.import_batch_id.unwrap_or_default(),
        import_batch_id_present: u8::from(summary.import_batch_id.is_some()),
        total_records: summary.total_records,
        imported_terms: summary.imported_terms,
        inserted_terms: summary.inserted_terms,
        updated_terms: summary.updated_terms,
        skipped_deleted_terms: summary.skipped_deleted_terms,
        skipped_duplicate_terms: summary.skipped_duplicate_terms,
        dry_run: u8::from(dry_run),
    }
}

fn dictionary_format_code(format: DictionaryTermsFormat) -> u32 {
    match format {
        DictionaryTermsFormat::V1 => RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1,
    }
}

fn read_dictionary_file(file_path: &str) -> Result<String, FfiError> {
    fs::read_to_string(file_path).map_err(|error| {
        FfiError::new(
            crate::error::RadishLexStatusCode::UserDbError,
            format!("failed to read dictionary file {file_path}: {error}"),
        )
    })
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
