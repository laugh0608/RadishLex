use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;

use super::{stable_hash_hex, UserDb};
use crate::{
    decode_dictionary_terms_tsv, decode_dictionary_terms_tsv_document, encode_dictionary_terms_tsv,
    DictionaryTermRecord, DictionaryTermsFormat, NegativeFeedbackDraft, NegativeFeedbackReason,
    PrivacyLevel, SelectionEventDraft, TermSource, TermStatus, UserDbSyncPayloadObjectType,
};

fn temp_db_path(test_name: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!(
        "radishlex-userdb-{test_name}-{}-{timestamp}.sqlite",
        std::process::id()
    ));
    path.to_string_lossy().into_owned()
}

#[test]
fn migration_initializes_empty_database() {
    let db = UserDb::open_in_memory().expect("userdb opens");

    assert_eq!(db.schema_version().expect("schema version"), 2);
    assert!(db.list_active_terms().expect("terms").is_empty());
    assert!(db.list_import_batches().expect("batches").is_empty());
}

#[test]
fn migration_upgrades_v1_import_batches() {
    let path = temp_db_path("migration-v1");
    {
        let connection = rusqlite::Connection::open(&path).expect("sqlite opens");
        connection
            .execute_batch(
                "
                CREATE TABLE import_batches (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    source_name TEXT NOT NULL,
                    term_count INTEGER NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    notes TEXT NOT NULL DEFAULT ''
                );
                INSERT INTO import_batches (source_name, term_count, created_at_ms, notes)
                VALUES ('legacy', 3, 42, '');
                PRAGMA user_version = 1;
                ",
            )
            .expect("legacy schema is created");
    }

    let db = UserDb::open(&path).expect("userdb migrates");
    assert_eq!(db.schema_version().expect("schema version"), 2);

    let batches = db.list_import_batches().expect("batches");
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].source_name, "legacy");
    assert_eq!(batches[0].total_records, 3);
    assert_eq!(batches[0].imported_terms, 3);
    assert_eq!(batches[0].inserted_terms, 3);

    let _ = std::fs::remove_file(path);
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
fn p2_plaintext_payloads_export_stable_user_and_deleted_term_schema() {
    let db = UserDb::open_in_memory().expect("userdb opens");
    db.connection
        .execute(
            "INSERT INTO user_terms (
                text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "词核",
                "",
                "cihe",
                "manual_add",
                2.5,
                "active",
                10,
                20,
                Option::<i64>::None
            ],
        )
        .expect("insert active term");
    db.connection
        .execute(
            "INSERT INTO user_terms (
                text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "萝卜",
                "luo bo",
                "luobo",
                "engine_selection",
                4.0,
                "suppressed",
                30,
                40,
                Some(35_i64)
            ],
        )
        .expect("insert suppressed term");
    db.connection
        .execute(
            "INSERT INTO user_terms (
                text, reading, input_code, source, weight, status, created_at_ms, updated_at_ms, last_used_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
            params!["删除", "shan chu", "shanchu", "manual_add", 0.0, "deleted", 50, 60],
        )
        .expect("insert deleted term");
    db.connection
        .execute(
            "INSERT INTO deleted_terms (
                term_id, text_hash, reading_hash, input_code_hash, deleted_at_ms, reason
             )
             VALUES (NULL, ?1, ?2, ?3, ?4, ?5)",
            params![
                stable_hash_hex("删除"),
                stable_hash_hex("shan chu"),
                stable_hash_hex("shanchu"),
                55,
                "manual_delete"
            ],
        )
        .expect("insert tombstone");

    let payloads: Vec<_> = db
        .p2_plaintext_payloads()
        .expect("payload iterator")
        .collect();

    assert_eq!(payloads.len(), 2);
    assert_eq!(
        payloads[0].object_type,
        UserDbSyncPayloadObjectType::DictionaryUserTerms
    );
    assert_eq!(payloads[0].record_count, 2);
    assert_eq!(
        payloads[0].as_str().expect("utf-8 payload"),
        r#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input_code":"cihe","text":"词核","reading":"","source":"manual_add","weight":2.5,"status":"active","created_at_ms":10,"updated_at_ms":20,"last_used_at_ms":null},{"input_code":"luobo","text":"萝卜","reading":"luo bo","source":"engine_selection","weight":4,"status":"suppressed","created_at_ms":30,"updated_at_ms":40,"last_used_at_ms":35}]}"#
    );

    assert_eq!(
        payloads[1].object_type,
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms
    );
    assert_eq!(payloads[1].record_count, 1);
    assert_eq!(
        payloads[1].as_str().expect("utf-8 payload"),
        r#"{"payload_schema_version":1,"object_type":"dictionary.deleted_terms","tombstones":[{"input_code":"shanchu","text":"删除","reading":"shan chu","deleted_at_ms":55,"reason":"manual_delete"}]}"#
    );
}

#[test]
fn p2_plaintext_payloads_export_stable_ranker_weight_schema() {
    let db = UserDb::open_in_memory().expect("userdb opens");
    db.connection
        .execute(
            "INSERT INTO ranker_weights (
                input_code, text, reading, frequency, recency_score, negative_score, context_kind, updated_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params!["luo\\bo", "萝\"卜\\词", "luo\tbo\nline", 1, 20.0, 0.0, "general", 10],
        )
        .expect("insert escaped ranker weight");
    db.connection
        .execute(
            "INSERT INTO ranker_weights (
                input_code, text, reading, frequency, recency_score, negative_score, context_kind, updated_at_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params!["cihe", "词核", "", 2, 1000.5, 1.25, "chat", 30],
        )
        .expect("insert ranker weight");

    let payloads: Vec<_> = db
        .p2_plaintext_payloads()
        .expect("payload iterator")
        .collect();

    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0].object_type,
        UserDbSyncPayloadObjectType::RankerWeights
    );
    assert_eq!(payloads[0].record_count, 2);
    assert_eq!(
        payloads[0].as_str().expect("utf-8 payload"),
        r#"{"payload_schema_version":1,"object_type":"ranker.weights","weights":[{"input_code":"cihe","text":"词核","reading":"","frequency":2,"recency_score":1000.5,"negative_score":1.25,"context_kind":"chat","updated_at_ms":30},{"input_code":"luo\\bo","text":"萝\"卜\\词","reading":"luo\tbo\nline","frequency":1,"recency_score":20,"negative_score":0,"context_kind":"general","updated_at_ms":10}]}"#
    );
}

#[test]
fn p2_plaintext_payloads_exclude_p1_and_local_audit_sources() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");
    db.record_selection(
        SelectionEventDraft::new("session-private", "cihe", "词核", 0, 1).with_context_kind("chat"),
    )
    .expect("selection is recorded");
    db.record_negative_feedback(
        NegativeFeedbackDraft::new("cihe", "词核", NegativeFeedbackReason::ManualSuppress)
            .with_context_kind("chat"),
    )
    .expect("feedback is recorded");
    db.import_dictionary_records(
        &[DictionaryTermRecord::new(
            "daoru",
            "导入",
            None::<String>,
            TermSource::ManualImport,
            1.0,
            TermStatus::Active,
        )],
        "source-secret",
    )
    .expect("import records local audit batch");
    db.delete_term("luobo", "萝卜", Some("luo bo"))
        .expect("term is deleted");

    let payloads: Vec<_> = db
        .p2_plaintext_payloads()
        .expect("payload iterator")
        .collect();
    let payload_debug = format!("{payloads:?}");
    let payload_text = payloads
        .iter()
        .map(|payload| payload.as_str().expect("utf-8 payload").to_owned())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(payload_debug.contains("[redacted]"));
    assert!(!payload_debug.contains("词核"));
    assert!(!payload_debug.contains("萝卜"));
    assert!(payload_text.contains("dictionary.user_terms"));
    assert!(payload_text.contains("dictionary.deleted_terms"));
    assert!(payload_text.contains("ranker.weights"));
    assert!(payload_text.contains(r#""weights":"#));
    assert!(payload_text.contains(r#""context_kind":"chat""#));
    assert!(payload_text.contains(r#""frequency":"#));
    assert!(payload_text.contains(r#""negative_score":"#));
    assert!(payload_text.contains("词核"));
    assert!(payload_text.contains("导入"));
    assert!(payload_text.contains("萝卜"));
    assert!(!payload_text.contains("session-private"));
    assert!(!payload_text.contains("manual_suppress"));
    assert!(!payload_text.contains("source-secret"));
    assert!(!payload_text.contains("candidate_index"));
    assert!(!payload_text.contains("candidate_count"));
    assert!(!payload_text.contains("selection_events"));
    assert!(!payload_text.contains("negative_feedback"));
    assert!(!payload_text.contains("import_batches"));
}

#[test]
fn p2_plaintext_payloads_escape_json_strings() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term(
        "luo\\bo",
        "萝\"卜\\词",
        Some("luo\tbo\nline"),
        TermSource::ManualAdd,
    )
    .expect("term is added");

    let payloads: Vec<_> = db
        .p2_plaintext_payloads()
        .expect("payload iterator")
        .collect();
    let payload = payloads[0].as_str().expect("utf-8 payload");

    assert!(payload.contains(r#""input_code":"luo\\bo""#));
    assert!(payload.contains(r#""text":"萝\"卜\\词""#));
    assert!(payload.contains(r#""reading":"luo\tbo\nline""#));
}

#[test]
fn p2_plaintext_payloads_empty_database_exports_no_payloads() {
    let db = UserDb::open_in_memory().expect("userdb opens");

    let payloads: Vec<_> = db
        .p2_plaintext_payloads()
        .expect("payload iterator")
        .collect();

    assert!(payloads.is_empty());
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
    assert_eq!(summary.inserted_terms, 1);
    assert_eq!(summary.updated_terms, 0);
    assert_eq!(summary.skipped_deleted_terms, 1);
    assert_eq!(summary.skipped_duplicate_terms, 0);
    assert!(summary.import_batch_id.is_some());
    assert_eq!(db.import_batch_count().expect("batch count"), 1);

    let batches = db.list_import_batches().expect("batches");
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].source_name, "unit-test");
    assert_eq!(batches[0].total_records, 2);
    assert_eq!(batches[0].imported_terms, 1);
    assert_eq!(batches[0].inserted_terms, 1);
    assert_eq!(batches[0].skipped_deleted_terms, 1);

    let terms = db.list_active_terms().expect("terms");
    assert_eq!(terms.len(), 1);
    assert_eq!(terms[0].input_code, "cihe");
    assert_eq!(terms[0].text, "词核");
    assert_eq!(terms[0].weight, 3.5);
    assert_eq!(terms[0].status, TermStatus::Suppressed);
}

#[test]
fn dictionary_import_preview_does_not_write_batch_or_terms() {
    let db = UserDb::open_in_memory().expect("userdb opens");
    let records = vec![DictionaryTermRecord::new(
        "cihe",
        "词核",
        None::<String>,
        TermSource::ManualImport,
        1.0,
        TermStatus::Active,
    )];

    let summary = db
        .preview_dictionary_import(&records, "preview")
        .expect("preview succeeds");

    assert_eq!(summary.import_batch_id, None);
    assert_eq!(summary.total_records, 1);
    assert_eq!(summary.imported_terms, 1);
    assert_eq!(summary.inserted_terms, 1);
    assert_eq!(db.import_batch_count().expect("batch count"), 0);
    assert!(db.list_active_terms().expect("terms").is_empty());
}

#[test]
fn dictionary_import_reports_updates_duplicates_and_distinct_readings() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");

    let records = vec![
        DictionaryTermRecord::new(
            "luobo",
            "萝卜",
            Some("luo bo"),
            TermSource::ManualImport,
            4.0,
            TermStatus::Suppressed,
        ),
        DictionaryTermRecord::new(
            "luobo",
            "萝卜",
            Some("luo bo"),
            TermSource::ManualImport,
            8.0,
            TermStatus::Active,
        ),
        DictionaryTermRecord::new(
            "luobo",
            "萝卜",
            Some("luo bu"),
            TermSource::ManualImport,
            2.0,
            TermStatus::Active,
        ),
    ];

    let preview = db
        .preview_dictionary_import(&records, "batch-1")
        .expect("preview succeeds");
    assert_eq!(preview.total_records, 3);
    assert_eq!(preview.imported_terms, 2);
    assert_eq!(preview.inserted_terms, 1);
    assert_eq!(preview.updated_terms, 1);
    assert_eq!(preview.skipped_duplicate_terms, 1);

    let summary = db
        .import_dictionary_records(&records, "batch-1")
        .expect("import succeeds");
    assert_eq!(summary.imported_terms, 2);
    assert_eq!(summary.inserted_terms, 1);
    assert_eq!(summary.updated_terms, 1);
    assert_eq!(summary.skipped_duplicate_terms, 1);

    let updated = db
        .fetch_term("luobo", "萝卜", "luo bo")
        .expect("term lookup")
        .expect("term exists");
    assert_eq!(updated.status, TermStatus::Suppressed);
    assert_eq!(updated.weight, 4.0);
    assert!(db
        .fetch_term("luobo", "萝卜", "luo bu")
        .expect("term lookup")
        .is_some());
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

    let document = decode_dictionary_terms_tsv_document(&encoded).expect("document decodes");
    assert_eq!(document.format, DictionaryTermsFormat::V1);
    assert_eq!(document.records, records);
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

    let bad_source = "\
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜\t\tunknown\t1.0\tactive
";
    let error = decode_dictionary_terms_tsv(bad_source).expect_err("source fails");
    assert!(error.to_string().contains("unknown term source"));

    let future_version = "\
# radishlex-user-terms-v2
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜\t\tmanual_import\t1.0\tactive
";
    let error = decode_dictionary_terms_tsv(future_version).expect_err("future version fails");
    assert!(error.to_string().contains("unsupported dictionary format"));
    assert!(error.to_string().contains("radishlex-user-terms-v1"));
}

#[test]
fn dictionary_import_rejects_invalid_batch_source_name() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    let records = vec![DictionaryTermRecord::new(
        "cihe",
        "词核",
        None::<String>,
        TermSource::ManualImport,
        1.0,
        TermStatus::Active,
    )];

    let error = db
        .import_dictionary_records(&records, "bad source")
        .expect_err("bad source fails");
    assert!(error.to_string().contains("source_name"));

    let error = db
        .preview_dictionary_import(&records, "bad source")
        .expect_err("bad preview source fails");
    assert!(error.to_string().contains("source_name"));
}

#[test]
fn sync_preflight_separates_syncable_and_local_only_counts() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");
    db.record_selection(
        SelectionEventDraft::new("session-local", "cihe", "词核", 0, 1).with_context_kind("chat"),
    )
    .expect("selection is recorded");
    db.record_negative_feedback(
        NegativeFeedbackDraft::new("cihe", "词核", NegativeFeedbackReason::ManualSuppress)
            .with_context_kind("chat"),
    )
    .expect("feedback is recorded");
    db.delete_term("luobo", "萝卜", Some("luo bo"))
        .expect("term is deleted");

    let summary = db.sync_preflight_summary().expect("summary");

    assert_eq!(summary.schema_version, 2);
    assert_eq!(summary.syncable_user_terms, 1);
    assert_eq!(summary.syncable_ranker_weights, 1);
    assert_eq!(summary.syncable_deleted_terms, 1);
    assert_eq!(summary.local_selection_events, 1);
    assert_eq!(summary.local_negative_feedback, 1);
    assert_eq!(summary.local_import_batches, 0);
}

#[test]
fn learning_status_reports_only_aggregate_counts_and_timestamps() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");

    let empty = db.learning_status_summary().expect("empty summary");
    assert_eq!(empty.schema_version, 2);
    assert_eq!(empty.active_user_terms, 0);
    assert_eq!(empty.suppressed_user_terms, 0);
    assert_eq!(empty.selection_events, 0);
    assert_eq!(empty.negative_feedback, 0);
    assert_eq!(empty.latest_activity_at_ms, None);

    db.add_term("luobo", "萝卜", Some("luo bo"), TermSource::ManualAdd)
        .expect("term is added");
    db.record_selection(
        SelectionEventDraft::new("session-local", "cihe", "词核", 0, 1).with_context_kind("chat"),
    )
    .expect("selection is recorded");
    db.record_negative_feedback(
        NegativeFeedbackDraft::new("cihe", "词核", NegativeFeedbackReason::ManualSuppress)
            .with_context_kind("chat"),
    )
    .expect("feedback is recorded");
    db.delete_term("luobo", "萝卜", Some("luo bo"))
        .expect("term is deleted");

    let summary = db.learning_status_summary().expect("summary");

    assert_eq!(summary.schema_version, 2);
    assert_eq!(summary.active_user_terms, 0);
    assert_eq!(summary.suppressed_user_terms, 1);
    assert_eq!(summary.ranker_weights, 1);
    assert_eq!(summary.deleted_term_tombstones, 1);
    assert_eq!(summary.selection_events, 1);
    assert_eq!(summary.negative_feedback, 1);
    assert_eq!(summary.import_batches, 0);
    assert!(summary.latest_user_term_updated_at_ms.is_some());
    assert!(summary.latest_selection_event_at_ms.is_some());
    assert!(summary.latest_negative_feedback_at_ms.is_some());
    assert!(summary.latest_deleted_term_at_ms.is_some());
    assert_eq!(summary.latest_import_batch_at_ms, None);
    assert_eq!(
        summary.latest_activity_at_ms,
        [
            summary.latest_user_term_updated_at_ms,
            summary.latest_selection_event_at_ms,
            summary.latest_negative_feedback_at_ms,
            summary.latest_deleted_term_at_ms,
            summary.latest_import_batch_at_ms,
        ]
        .into_iter()
        .flatten()
        .max()
    );

    let debug = format!("{summary:?}");
    assert!(!debug.contains("session-local"));
    assert!(!debug.contains("chat"));
    assert!(!debug.contains("manual_suppress"));
    assert!(!debug.contains("萝卜"));
    assert!(!debug.contains("词核"));
}
