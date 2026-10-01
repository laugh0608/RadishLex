use super::*;

#[test]
fn sqlite_346_fixture_keeps_learning_and_delete_semantics_through_backup_and_restore() {
    let fixture = include_bytes!("../../../tests/fixtures/sqlite-3.46.0-schema9.sqlite3");
    assert_eq!(&fixture[96..100], &3_046_000_u32.to_be_bytes());
    let source = temp_db_path("sqlite-346-upgrade");
    let backup = temp_db_path("sqlite-346-backup");
    std::fs::write(&source, fixture).unwrap();
    let before = UserDb::migrate_and_validate(&source).expect("old SQLite file validates");
    assert_eq!(before.source_schema_version, 9);
    assert_eq!(before.target_schema_version, 9);
    assert!(
        !before.migrated,
        "SQLite upgrade must not change the product schema"
    );

    let mut db = UserDb::open(&source).unwrap();
    assert_eq!(db.list_active_terms().unwrap().len(), 1);
    assert_eq!(
        db.fetch_term("shi", "时", "").unwrap().unwrap().status,
        TermStatus::Active
    );
    let event = || SelectionEventDraft::new("synthetic-upgraded", "shi", "时", 0, 1);
    db.record_selection(event()).unwrap();
    let frequency: i64 = db
        .connection
        .query_row("SELECT frequency FROM ranker_weights", [], |row| row.get(0))
        .unwrap();
    assert_eq!(frequency, 2);
    db.delete_term("shi", "时", None).unwrap();
    db.record_selection(event()).unwrap();
    assert!(db.list_active_terms().unwrap().is_empty());
    let tombstones = db.list_deleted_term_tombstones().unwrap();
    assert_eq!(tombstones.len(), 1);
    std::fs::File::create(&backup).unwrap();
    UserDb::create_consistent_snapshot(&source, &backup)
        .expect("backup captures committed WAL including deletion");
    drop(db);

    let mut restored = UserDb::open(&backup).expect("isolated backup reopens");
    assert_eq!(restored.list_deleted_term_tombstones().unwrap(), tombstones);
    restored.record_selection(event()).unwrap();
    assert!(
        restored.list_active_terms().unwrap().is_empty(),
        "backup must not revive deleted learning"
    );
    let term = restored
        .restore_term("shi", "时", None)
        .expect("explicit restore is accepted");
    assert!(term.updated_at_ms > tombstones[0].deleted_at_ms);
    restored.record_selection(event()).unwrap();
    assert_eq!(restored.list_active_terms().unwrap().len(), 1);
    assert!(restored.list_deleted_term_tombstones().unwrap().is_empty());
    drop(restored);
    let reopened = UserDb::open(&backup).unwrap();
    assert_eq!(reopened.list_active_terms().unwrap().len(), 1);
    assert!(reopened.list_deleted_term_tombstones().unwrap().is_empty());
    let original = UserDb::open(&source).unwrap();
    assert!(original.list_active_terms().unwrap().is_empty());
    assert_eq!(original.list_deleted_term_tombstones().unwrap(), tombstones);
    drop(original);
    drop(reopened);
    remove_temp_db(&source);
    remove_temp_db(&backup);
}

#[test]
fn sqlite_backup_snapshot_captures_uncheckpointed_wal_content() {
    let source = temp_db_path("backup-source-wal");
    let destination = temp_db_path("backup-destination-wal");
    let mut source_db = UserDb::open(&source).expect("source userdb opens");
    source_db
        .connection
        .pragma_update(None, "wal_autocheckpoint", 0)
        .expect("automatic checkpoint is disabled");
    source_db
        .record_selection(
            SelectionEventDraft::new("synthetic-backup", "kuaizhao", "快照词", 0, 1)
                .with_reading("kuai zhao")
                .with_context_kind("editor"),
        )
        .expect("uncheckpointed selection is written");
    assert!(
        std::fs::metadata(format!("{source}-wal"))
            .expect("source WAL exists")
            .len()
            > 0
    );

    let estimate = UserDb::estimate_snapshot(&source).expect("snapshot is estimated");
    std::fs::File::create(&destination).expect("empty destination is created");
    let summary = UserDb::create_consistent_snapshot(&source, &destination)
        .expect("consistent snapshot is created");

    assert_eq!(summary.source_schema_version, 9);
    assert_eq!(summary.snapshot_schema_version, 9);
    assert_eq!(summary.page_size_bytes, estimate.page_size_bytes);
    assert_eq!(summary.page_count, estimate.page_count);
    assert_eq!(summary.logical_size_bytes, estimate.logical_size_bytes);
    assert_eq!(summary.snapshot_file_bytes, estimate.logical_size_bytes);
    assert!(!std::path::Path::new(&format!("{destination}-wal")).exists());
    assert!(!std::path::Path::new(&format!("{destination}-shm")).exists());
    let snapshot = rusqlite::Connection::open(&destination).expect("snapshot opens");
    let term_count: i64 = snapshot
        .query_row(
            "SELECT COUNT(*) FROM user_terms WHERE input_code = 'kuaizhao'",
            [],
            |row| row.get(0),
        )
        .expect("snapshotted term count");
    assert_eq!(term_count, 1);

    drop(snapshot);
    drop(source_db);
    remove_temp_db(&source);
    remove_temp_db(&destination);
}

#[test]
fn sqlite_backup_snapshot_requires_an_existing_empty_destination() {
    let source = temp_db_path("backup-source-nonempty-destination");
    let destination = temp_db_path("backup-nonempty-destination");
    drop(UserDb::open(&source).expect("source userdb opens"));
    std::fs::write(&destination, b"synthetic-nonempty\n").expect("nonempty destination is created");

    let error = UserDb::create_consistent_snapshot(&source, &destination)
        .expect_err("nonempty destination is rejected");

    assert!(error.to_string().contains("existing empty regular file"));
    assert_eq!(
        std::fs::read(&destination).expect("destination remains readable"),
        b"synthetic-nonempty\n"
    );
    remove_temp_db(&source);
    remove_temp_db(&destination);
}

#[test]
fn sqlite_backup_snapshot_rejects_corrupt_source_without_writing_destination() {
    let source = temp_db_path("backup-corrupt-source");
    let destination = temp_db_path("backup-corrupt-destination");
    std::fs::write(&source, b"synthetic-not-sqlite\n").expect("corrupt source is created");
    std::fs::File::create(&destination).expect("empty destination is created");

    UserDb::create_consistent_snapshot(&source, &destination)
        .expect_err("corrupt source is rejected");

    assert_eq!(
        std::fs::metadata(&destination)
            .expect("destination metadata")
            .len(),
        0
    );
    remove_temp_db(&source);
    remove_temp_db(&destination);
}

#[test]
fn sqlite_backup_snapshot_preserves_an_empty_schema_zero_database() {
    let source = temp_db_path("backup-empty-source");
    let destination = temp_db_path("backup-empty-destination");
    std::fs::File::create(&source).expect("empty source is created");
    std::fs::File::create(&destination).expect("empty destination is created");

    let estimate = UserDb::estimate_snapshot(&source).expect("empty source is estimated");
    let summary = UserDb::create_consistent_snapshot(&source, &destination)
        .expect("empty source is snapshotted");

    assert_eq!(estimate.schema_version, 0);
    assert_eq!(estimate.page_count, 1);
    assert_eq!(estimate.logical_size_bytes, estimate.page_size_bytes);
    assert_eq!(summary.source_schema_version, 0);
    assert_eq!(summary.snapshot_schema_version, 0);
    assert_eq!(summary.snapshot_file_bytes, estimate.logical_size_bytes);
    assert_eq!(
        UserDb::inspect_file(&destination)
            .expect("empty snapshot inspects")
            .compatibility,
        UserDbSchemaCompatibility::MigrationRequired
    );
    remove_temp_db(&source);
    remove_temp_db(&destination);
}
