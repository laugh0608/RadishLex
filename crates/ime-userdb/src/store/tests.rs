use super::UserDb;
use crate::{
    decode_dictionary_terms_tsv, encode_dictionary_terms_tsv, DictionaryTermRecord,
    NegativeFeedbackDraft, NegativeFeedbackReason, PrivacyLevel, SelectionEventDraft, TermSource,
    TermStatus,
};

#[test]
fn migration_initializes_empty_database() {
    let db = UserDb::open_in_memory().expect("userdb opens");

    assert_eq!(db.schema_version().expect("schema version"), 1);
    assert!(db.list_active_terms().expect("terms").is_empty());
}

#[test]
fn add_query_and_delete_term_records_tombstone() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");

    let term = db
        .add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");
    assert_eq!(term.status, TermStatus::Active);
    assert_eq!(term.reading.as_deref(), Some("luo bo"));

    let terms = db.list_active_terms().expect("terms");
    assert_eq!(terms.len(), 1);

    db.delete_term("luobo", "萝卜", Some("luo bo"))
        .expect("term is deleted");

    assert!(db.list_active_terms().expect("terms").is_empty());
    assert_eq!(db.deleted_term_count().expect("deleted count"), 1);
}

#[test]
fn selection_event_updates_term_and_ranker_summary() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    let event = SelectionEventDraft::new("session-1", "luobo", "萝卜", 0, 5)
        .with_reading("luo bo")
        .with_context_kind("chat");

    assert!(db.record_selection(event.clone()).expect("event").is_some());
    assert!(db.record_selection(event).expect("event").is_some());

    assert_eq!(db.selection_event_count().expect("event count"), 2);
    let term = db
        .fetch_term("luobo", "萝卜", "luo bo")
        .expect("term lookup")
        .expect("term exists");
    assert_eq!(term.weight, 2.0);

    let weight = db
        .ranker_weight("luobo", "萝卜", Some("luo bo"), "chat")
        .expect("ranker weight")
        .expect("ranker weight exists");
    assert_eq!(weight.frequency, 2);
    assert_eq!(weight.negative_score, 0.0);
}

#[test]
fn p0_selection_is_not_recorded() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    let event = SelectionEventDraft::new("session-1", "secret", "敏感", 0, 1)
        .with_privacy(PrivacyLevel::P0NeverLearn);

    assert_eq!(db.record_selection(event).expect("event"), None);
    assert_eq!(db.selection_event_count().expect("event count"), 0);
    assert!(db.list_active_terms().expect("terms").is_empty());
}

#[test]
fn negative_feedback_suppresses_term_and_records_penalty() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");

    let feedback =
        NegativeFeedbackDraft::new("luobo", "萝卜", NegativeFeedbackReason::ManualSuppress)
            .with_reading("luo bo")
            .with_context_kind("general");
    assert!(db
        .record_negative_feedback(feedback)
        .expect("feedback")
        .is_some());

    assert_eq!(
        db.fetch_term("luobo", "萝卜", "luo bo")
            .expect("term lookup")
            .expect("term exists")
            .status,
        TermStatus::Suppressed
    );
    assert_eq!(db.negative_feedback_count().expect("count"), 1);

    let weight = db
        .ranker_weight("luobo", "萝卜", Some("luo bo"), "general")
        .expect("ranker weight")
        .expect("ranker weight exists");
    assert_eq!(weight.negative_score, 1.0);
}

#[test]
fn deleted_term_is_not_revived_by_later_selection_event() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");
    db.delete_term("luobo", "萝卜", Some("luo bo"))
        .expect("term is deleted");

    let event = SelectionEventDraft::new("session-2", "luobo", "萝卜", 0, 5).with_reading("luo bo");
    db.record_selection(event).expect("event is recorded");

    assert_eq!(db.selection_event_count().expect("event count"), 1);
    assert!(db.list_active_terms().expect("terms").is_empty());
    assert_eq!(
        db.fetch_term("luobo", "萝卜", "luo bo")
            .expect("term lookup")
            .expect("term exists")
            .status,
        TermStatus::Deleted
    );
    assert!(db
        .ranker_weight("luobo", "萝卜", Some("luo bo"), "general")
        .expect("ranker weight")
        .is_none());
}

#[test]
fn manual_import_does_not_revive_deleted_term() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");
    db.delete_term("luobo", "萝卜", Some("luo bo"))
        .expect("term is deleted");

    let term = db
        .add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualImport)
        .expect("import respects tombstone");

    assert_eq!(term.status, TermStatus::Deleted);
    assert!(db.list_active_terms().expect("terms").is_empty());
    assert_eq!(db.deleted_term_count().expect("deleted count"), 1);
}

#[test]
fn manual_add_can_restore_deleted_term() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");
    db.delete_term("luobo", "萝卜", Some("luo bo"))
        .expect("term is deleted");

    let term = db
        .add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("manual add restores term");

    assert_eq!(term.status, TermStatus::Active);
    assert_eq!(db.list_active_terms().expect("terms").len(), 1);
    assert_eq!(db.deleted_term_count().expect("deleted count"), 0);
}

#[test]
fn dictionary_export_contains_only_p2_terms() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.record_selection(
        SelectionEventDraft::new("session-secret", "luobo", "萝卜", 0, 1).with_context_kind("chat"),
    )
    .expect("selection is recorded");
    db.record_negative_feedback(NegativeFeedbackDraft::new(
        "luobo",
        "萝卜",
        NegativeFeedbackReason::ManualSuppress,
    ))
    .expect("feedback is recorded");

    let records = db
        .export_dictionary_records()
        .expect("dictionary records export");
    let encoded = encode_dictionary_terms_tsv(&records);

    assert!(encoded.contains("# radishlex-user-terms-v1"));
    assert!(encoded.contains("luobo\t萝卜\t"));
    assert!(encoded.contains("\tsuppressed"));
    assert!(!encoded.contains("session-secret"));
    assert!(!encoded.contains("chat"));
    assert!(!encoded.contains("manual_suppress"));
}

#[test]
fn dictionary_import_preserves_term_fields_and_skips_deleted_tombstones() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");
    db.delete_term("luobo", "萝卜", Some("luo bo"))
        .expect("term is deleted");

    let records = vec![
        DictionaryTermRecord::new(
            "luobo",
            "萝卜",
            Some("luo bo"),
            TermSource::ManualImport,
            9.0,
            TermStatus::Active,
        ),
        DictionaryTermRecord::new(
            "cihe",
            "词核",
            None::<String>,
            TermSource::ManualImport,
            3.5,
            TermStatus::Suppressed,
        ),
    ];
    let summary = db
        .import_dictionary_records(&records, "unit-test")
        .expect("dictionary import succeeds");

    assert_eq!(summary.total_records, 2);
    assert_eq!(summary.imported_terms, 1);
    assert_eq!(summary.skipped_deleted_terms, 1);
    assert_eq!(db.import_batch_count().expect("batch count"), 1);

    let terms = db.list_active_terms().expect("terms");
    assert_eq!(terms.len(), 1);
    assert_eq!(terms[0].input_code, "cihe");
    assert_eq!(terms[0].text, "词核");
    assert_eq!(terms[0].weight, 3.5);
    assert_eq!(terms[0].status, TermStatus::Suppressed);
}

#[test]
fn dictionary_tsv_round_trips_escaped_fields() {
    let records = vec![DictionaryTermRecord::new(
        "luobo",
        "萝\t卜\\词",
        Some("luo\nbo"),
        TermSource::ManualImport,
        1.25,
        TermStatus::Active,
    )];

    let encoded = encode_dictionary_terms_tsv(&records);
    assert!(encoded.contains("萝\\t卜\\\\词"));
    assert!(encoded.contains("luo\\nbo"));

    let decoded = decode_dictionary_terms_tsv(&encoded).expect("decode succeeds");
    assert_eq!(decoded, records);
}

#[test]
fn dictionary_import_rejects_malformed_files() {
    let missing_field = "\
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜\t\tmanual_import\t1.0
";
    let error = decode_dictionary_terms_tsv(missing_field).expect_err("field count fails");
    assert!(error.to_string().contains("expected 6"));

    let deleted_term = "\
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜\t\tmanual_import\t1.0\tdeleted
";
    let error = decode_dictionary_terms_tsv(deleted_term).expect_err("deleted status fails");
    assert!(error.to_string().contains("does not accept deleted"));
}
