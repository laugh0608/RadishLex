use super::*;

#[test]
fn file_database_applies_sqlite_runtime_policy_and_private_permissions() {
    let path = temp_db_path("sqlite-policy");
    let mut first = UserDb::open(&path).expect("first connection opens");
    let second = UserDb::open(&path).expect("second connection opens");
    first
        .record_selection(SelectionEventDraft::new(
            "policy-session",
            "policy",
            "合成策略词",
            0,
            1,
        ))
        .expect("write creates WAL sidecars");

    for db in [&first, &second] {
        let journal_mode: String = db
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");
        let busy_timeout: i64 = db
            .connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("busy timeout");
        let foreign_keys: i64 = db
            .connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign keys");
        let synchronous: i64 = db
            .connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .expect("synchronous");

        assert_eq!(journal_mode, "wal");
        assert_eq!(busy_timeout, 5_000);
        assert_eq!(foreign_keys, 1);
        assert_eq!(synchronous, 1);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(&path)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        for sidecar in [format!("{path}-wal"), format!("{path}-shm")] {
            let sidecar_mode = std::fs::metadata(sidecar)
                .expect("sidecar metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(sidecar_mode, 0o600);
        }
    }

    drop(second);
    drop(first);
    remove_temp_db(&path);
}

#[test]
fn two_connections_wait_for_short_writer_instead_of_returning_busy() {
    let path = temp_db_path("two-writers");
    let mut first = UserDb::open(&path).expect("first connection opens");
    let transaction = first
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("first writer starts");
    transaction
        .execute(
            "INSERT INTO import_batches (
                source_name, term_count, total_count, inserted_count, updated_count,
                skipped_deleted_count, skipped_duplicate_count, created_at_ms, notes
             ) VALUES ('lock-holder', 0, 0, 0, 0, 0, 0, 1, '')",
            [],
        )
        .expect("first writer holds lock");

    let worker_path = path.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut db = UserDb::open(worker_path).expect("second connection opens");
        ready_tx.send(()).expect("worker ready");
        db.record_selection(SelectionEventDraft::new(
            "writer-session",
            "writer",
            "合成并发词",
            0,
            1,
        ))
    });
    ready_rx.recv().expect("worker started write");
    std::thread::sleep(Duration::from_millis(50));
    transaction.commit().expect("first writer commits");

    assert!(worker
        .join()
        .expect("worker joins")
        .expect("write succeeds")
        .is_some());
    assert_eq!(first.selection_event_count().expect("selection count"), 1);
    drop(first);
    remove_temp_db(&path);
}

#[test]
fn selection_transaction_rolls_back_event_term_and_weight_on_fault() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.connection
        .execute_batch(
            "CREATE TRIGGER fail_ranker_selection
             BEFORE INSERT ON ranker_weights
             BEGIN SELECT RAISE(ABORT, 'selection fault'); END;",
        )
        .expect("fault trigger");

    let error = db
        .record_selection(SelectionEventDraft::new(
            "fault-session",
            "fault",
            "合成回滚词",
            0,
            1,
        ))
        .expect_err("selection fails");

    assert!(error.to_string().contains("selection fault"));
    assert_eq!(db.selection_event_count().expect("selection count"), 0);
    assert!(db.list_active_terms().expect("terms").is_empty());
    assert!(db
        .ranker_weight("fault", "合成回滚词", None, "general")
        .expect("ranker weight")
        .is_none());
}

#[test]
fn negative_feedback_transaction_rolls_back_event_state_and_penalty_on_fault() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("fault", "合成反馈词", None, TermSource::ManualAdd)
        .expect("term added");
    db.connection
        .execute_batch(
            "CREATE TRIGGER fail_ranker_penalty
             BEFORE INSERT ON ranker_weights
             BEGIN SELECT RAISE(ABORT, 'feedback fault'); END;",
        )
        .expect("fault trigger");

    let error = db
        .record_negative_feedback(NegativeFeedbackDraft::new(
            "fault",
            "合成反馈词",
            NegativeFeedbackReason::ManualSuppress,
        ))
        .expect_err("feedback fails");

    assert!(error.to_string().contains("feedback fault"));
    assert_eq!(db.negative_feedback_count().expect("feedback count"), 0);
    assert_eq!(
        db.fetch_term("fault", "合成反馈词", "")
            .expect("term lookup")
            .expect("term")
            .status,
        TermStatus::Active
    );
}

#[test]
fn delete_transaction_rolls_back_term_tombstone_weight_and_feedback_on_fault() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.record_selection(SelectionEventDraft::new(
        "delete-session",
        "fault",
        "合成删除词",
        0,
        1,
    ))
    .expect("selection recorded");
    db.connection
        .execute_batch(
            "CREATE TRIGGER fail_tombstone
             BEFORE INSERT ON deleted_terms
             BEGIN SELECT RAISE(ABORT, 'delete fault'); END;",
        )
        .expect("fault trigger");

    let error = db
        .delete_term("fault", "合成删除词", None)
        .expect_err("delete fails");

    assert!(error.to_string().contains("delete fault"));
    assert_eq!(db.deleted_term_count().expect("tombstone count"), 0);
    assert_eq!(db.negative_feedback_count().expect("feedback count"), 0);
    assert_eq!(
        db.fetch_term("fault", "合成删除词", "")
            .expect("term lookup")
            .expect("term")
            .status,
        TermStatus::Active
    );
    assert!(db
        .ranker_weight("fault", "合成删除词", None, "general")
        .expect("ranker weight")
        .is_some());
}

#[test]
fn explicit_restore_is_versioned_and_rolls_back_when_tombstone_clear_fails() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("restore", "合成恢复词", None, TermSource::ManualAdd)
        .expect("term added");
    db.delete_term("restore", "合成恢复词", None)
        .expect("term deleted");
    let deleted_at_ms = db
        .fetch_term("restore", "合成恢复词", "")
        .expect("term lookup")
        .expect("deleted term")
        .updated_at_ms;

    let error = db
        .restore_term_at("restore", "合成恢复词", None, deleted_at_ms)
        .expect_err("stale restore is rejected");
    assert!(error.to_string().contains("must be newer"));

    db.connection
        .execute_batch(
            "CREATE TRIGGER fail_restore
             BEFORE DELETE ON deleted_terms
             BEGIN SELECT RAISE(ABORT, 'restore fault'); END;",
        )
        .expect("fault trigger");
    let error = db
        .restore_term_at("restore", "合成恢复词", None, deleted_at_ms + 1)
        .expect_err("restore fails");
    assert!(error.to_string().contains("restore fault"));
    assert_eq!(db.deleted_term_count().expect("tombstone count"), 1);
    assert_eq!(
        db.fetch_term("restore", "合成恢复词", "")
            .expect("term lookup")
            .expect("term")
            .status,
        TermStatus::Deleted
    );
}

#[test]
fn suppress_dominates_selection_import_and_add_until_explicit_restore() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term("priority", "合成优先级词", None, TermSource::ManualAdd)
        .expect("term added");
    db.record_negative_feedback(NegativeFeedbackDraft::new(
        "priority",
        "合成优先级词",
        NegativeFeedbackReason::ManualSuppress,
    ))
    .expect("term suppressed");
    db.record_selection(SelectionEventDraft::new(
        "priority-session",
        "priority",
        "合成优先级词",
        0,
        1,
    ))
    .expect("selection remains P1 local event");
    db.import_dictionary_records(
        &[DictionaryTermRecord::new(
            "priority",
            "合成优先级词",
            None::<String>,
            TermSource::ManualImport,
            9.0,
            TermStatus::Active,
        )],
        "priority-test",
    )
    .expect("import is accepted without unsuppressing");
    db.add_term("priority", "合成优先级词", None, TermSource::ManualAdd)
        .expect("add updates without unsuppressing");

    assert_eq!(
        db.fetch_term("priority", "合成优先级词", "")
            .expect("term lookup")
            .expect("term")
            .status,
        TermStatus::Suppressed
    );
    let restored = db
        .restore_term("priority", "合成优先级词", None)
        .expect("explicit restore clears suppress");
    assert_eq!(restored.status, TermStatus::Active);
    assert!(restored.restored_at_ms.is_some());
    let weight = db
        .ranker_weight("priority", "合成优先级词", None, "general")
        .expect("ranker lookup")
        .expect("ranker weight");
    assert_eq!(weight.frequency, 0);
    assert_eq!(weight.negative_score, 0.0);
}

#[test]
fn canonical_identity_and_delete_priority_cover_whitespace_and_late_feedback() {
    let mut db = UserDb::open_in_memory().expect("userdb opens");
    db.add_term(
        "  identity  ",
        "  合成身份词  ",
        Some("  synthetic reading  "),
        TermSource::ManualAdd,
    )
    .expect("normalized term added");
    db.delete_term("identity", "合成身份词", Some("synthetic reading"))
        .expect("normalized identity deleted");
    db.record_negative_feedback(
        NegativeFeedbackDraft::new(
            " identity ",
            " 合成身份词 ",
            NegativeFeedbackReason::ManualSuppress,
        )
        .with_reading(" synthetic reading "),
    )
    .expect("late feedback remains a P1 event");

    assert_eq!(db.deleted_term_count().expect("tombstone count"), 1);
    assert_eq!(
        db.fetch_term("identity", "合成身份词", "synthetic reading")
            .expect("term lookup")
            .expect("term")
            .status,
        TermStatus::Deleted
    );
    assert!(db
        .ranker_weight(
            "identity",
            "合成身份词",
            Some("synthetic reading"),
            "general",
        )
        .expect("ranker lookup")
        .is_none());
}

#[test]
fn v2_migration_replaces_hash_identity_and_converts_recency_timestamp() {
    let path = temp_db_path("migration-v2");
    {
        let connection = rusqlite::Connection::open(&path).expect("sqlite opens");
        connection
            .execute_batch(
                "
                CREATE TABLE user_terms (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    text TEXT NOT NULL, reading TEXT NOT NULL DEFAULT '', input_code TEXT NOT NULL,
                    source TEXT NOT NULL, weight REAL NOT NULL, status TEXT NOT NULL,
                    created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL,
                    last_used_at_ms INTEGER
                );
                CREATE UNIQUE INDEX idx_user_terms_identity ON user_terms(input_code, text, reading);
                CREATE TABLE deleted_terms (
                    id INTEGER PRIMARY KEY AUTOINCREMENT, term_id INTEGER,
                    text_hash TEXT NOT NULL, reading_hash TEXT NOT NULL,
                    input_code_hash TEXT NOT NULL, deleted_at_ms INTEGER NOT NULL, reason TEXT NOT NULL
                );
                CREATE TABLE ranker_weights (
                    id INTEGER PRIMARY KEY AUTOINCREMENT, input_code TEXT NOT NULL, text TEXT NOT NULL,
                    reading TEXT NOT NULL DEFAULT '', frequency INTEGER NOT NULL,
                    recency_score REAL NOT NULL, negative_score REAL NOT NULL,
                    context_kind TEXT NOT NULL, updated_at_ms INTEGER NOT NULL
                );
                CREATE UNIQUE INDEX idx_ranker_weights_identity
                    ON ranker_weights(input_code, text, reading, context_kind);
                CREATE TABLE import_batches (
                    id INTEGER PRIMARY KEY AUTOINCREMENT, source_name TEXT NOT NULL,
                    term_count INTEGER NOT NULL, total_count INTEGER NOT NULL DEFAULT 0,
                    inserted_count INTEGER NOT NULL DEFAULT 0, updated_count INTEGER NOT NULL DEFAULT 0,
                    skipped_deleted_count INTEGER NOT NULL DEFAULT 0,
                    skipped_duplicate_count INTEGER NOT NULL DEFAULT 0,
                    created_at_ms INTEGER NOT NULL, notes TEXT NOT NULL DEFAULT ''
                );
                INSERT INTO user_terms (
                    text, reading, input_code, source, weight, status,
                    created_at_ms, updated_at_ms, last_used_at_ms
                ) VALUES ('合成迁移词', '', 'migration', 'manual_add', 0, 'deleted', 10, 20, NULL);
                INSERT INTO ranker_weights (
                    input_code, text, reading, frequency, recency_score,
                    negative_score, context_kind, updated_at_ms
                ) VALUES ('rank', '合成排序词', '', 7, 1234.0, 2.0, 'general', 1300);
                PRAGMA user_version = 2;
                ",
            )
            .expect("v2 base schema");
        let term_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO deleted_terms (
                    term_id, text_hash, reading_hash, input_code_hash, deleted_at_ms, reason
                 ) VALUES (?1, ?2, ?3, ?4, 20, 'manual_delete')",
                params![
                    term_id,
                    legacy_stable_hash_hex("合成迁移词"),
                    legacy_stable_hash_hex(""),
                    legacy_stable_hash_hex("migration")
                ],
            )
            .expect("legacy tombstone");
    }

    let db = UserDb::open(&path).expect("v2 migrates");
    assert_eq!(db.schema_version().expect("schema version"), 8);
    assert_eq!(db.deleted_term_count().expect("tombstone count"), 1);
    let deleted_columns = db
        .connection
        .prepare("PRAGMA table_info(deleted_terms)")
        .expect("columns prepare")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("columns query")
        .collect::<Result<Vec<_>, _>>()
        .expect("columns");
    assert!(deleted_columns.contains(&"input_code".to_owned()));
    assert!(!deleted_columns.contains(&"text_hash".to_owned()));
    let ranker = db
        .ranker_weight("rank", "合成排序词", None, "general")
        .expect("ranker query")
        .expect("ranker weight");
    assert_eq!(ranker.frequency, 7);
    assert_eq!(ranker.last_used_at_ms, Some(1234));
    drop(db);
    remove_temp_db(&path);
}

#[test]
fn future_schema_is_rejected_before_migration_writes() {
    let path = temp_db_path("future-schema");
    {
        let connection = rusqlite::Connection::open(&path).expect("sqlite opens");
        connection
            .execute_batch(
                "CREATE TABLE future_sentinel (value TEXT NOT NULL);
                 INSERT INTO future_sentinel VALUES ('preserve-me');
                 PRAGMA user_version = 99;",
            )
            .expect("future schema");
    }

    let error = UserDb::open(&path).expect_err("future schema is rejected");
    assert!(error.to_string().contains("newer than supported"));
    let connection = rusqlite::Connection::open(&path).expect("sqlite reopens");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    let sentinel: String = connection
        .query_row("SELECT value FROM future_sentinel", [], |row| row.get(0))
        .expect("sentinel");
    let user_terms: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'user_terms'",
            [],
            |row| row.get(0),
        )
        .expect("user terms table count");
    assert_eq!(version, 99);
    assert_eq!(sentinel, "preserve-me");
    assert_eq!(user_terms, 0);
    drop(connection);
    remove_temp_db(&path);
}

#[test]
fn corrupt_database_is_preserved_and_reported_explicitly() {
    let path = temp_db_path("corrupt");
    let original = b"not-a-sqlite-database\0synthetic";
    std::fs::write(&path, original).expect("corrupt fixture written");

    let error = UserDb::open(&path).expect_err("corrupt database is rejected");
    let message = error.to_string();
    assert!(message.contains(&path));
    assert!(message.contains("preserved"));
    assert_eq!(
        std::fs::read(&path).expect("corrupt file preserved"),
        original
    );
    remove_temp_db(&path);
}

#[test]
fn ambiguous_identity_migration_rolls_back_atomically() {
    let path = temp_db_path("migration-rollback");
    {
        let connection = rusqlite::Connection::open(&path).expect("sqlite opens");
        connection
            .execute_batch(
                "
                CREATE TABLE deleted_terms (
                    id INTEGER PRIMARY KEY AUTOINCREMENT, term_id INTEGER,
                    text_hash TEXT NOT NULL, reading_hash TEXT NOT NULL,
                    input_code_hash TEXT NOT NULL, deleted_at_ms INTEGER NOT NULL, reason TEXT NOT NULL
                );
                INSERT INTO deleted_terms (
                    term_id, text_hash, reading_hash, input_code_hash, deleted_at_ms, reason
                ) VALUES (NULL, 'missing', 'missing', 'missing', 10, 'manual_delete');
                PRAGMA user_version = 2;
                ",
            )
            .expect("broken v2 schema");
    }

    let error = UserDb::open(&path).expect_err("ambiguous migration fails");
    assert!(error.to_string().contains("cannot be matched"));
    let connection = rusqlite::Connection::open(&path).expect("legacy db reopens");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    let columns = connection
        .prepare("PRAGMA table_info(deleted_terms)")
        .expect("columns prepare")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("columns query")
        .collect::<Result<Vec<_>, _>>()
        .expect("columns");
    let staged_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'deleted_terms_v3'",
            [],
            |row| row.get(0),
        )
        .expect("staged table count");
    assert_eq!(version, 2);
    assert!(columns.contains(&"text_hash".to_owned()));
    assert!(!columns.contains(&"input_code".to_owned()));
    assert_eq!(staged_table, 0);
    drop(connection);
    remove_temp_db(&path);
}
