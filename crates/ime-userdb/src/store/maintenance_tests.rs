use super::*;
use crate::SelectionEventDraft;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    snapshot: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-maintenance-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.sqlite3");
        let snapshot = root.join("snapshot.sqlite3");
        Self {
            root,
            source,
            snapshot,
        }
    }
    fn populate(&self) {
        let mut db = UserDb::open(&self.source).unwrap();
        db.record_selection(SelectionEventDraft::new(
            "synthetic-maintenance",
            "shi",
            "时",
            0,
            2,
        ))
        .unwrap();
        db.record_selection(SelectionEventDraft::new(
            "synthetic-maintenance",
            "shan",
            "删",
            0,
            2,
        ))
        .unwrap();
        db.delete_term("shan", "删", None).unwrap();
        db.connection.execute("INSERT INTO sync_domain_state(domain_id,state_version,cursor,last_success_at_ms) VALUES ('synthetic-domain',1,17,23)", []).unwrap();
    }
    fn snapshot(&self) {
        fs::File::create(&self.snapshot).unwrap();
        UserDb::create_consistent_snapshot(&self.source, &self.snapshot).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn normal_close_wal_prepares_without_migration_and_keeps_deletion() {
    let fixture = Fixture::new();
    fixture.populate();
    fixture.snapshot();
    let protected = fs::read(&fixture.snapshot).unwrap();
    let summary = UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).unwrap();
    assert_eq!(summary.schema_version, UserDb::supported_schema_version());
    assert!(summary.checkpointed_wal);
    require_no_sidecars(&fixture.source).unwrap();
    assert_eq!(fs::read(&fixture.snapshot).unwrap(), protected);
    assert!(
        !UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot)
            .unwrap()
            .checkpointed_wal
    );
    let mut db = UserDb::open(&fixture.source).unwrap();
    assert_eq!(db.list_active_terms().unwrap().len(), 1);
    assert_eq!(db.list_deleted_term_tombstones().unwrap().len(), 1);
    db.record_selection(SelectionEventDraft::new(
        "synthetic-maintenance",
        "shan",
        "删",
        0,
        2,
    ))
    .unwrap();
    assert_eq!(db.list_active_terms().unwrap().len(), 1);
    assert_eq!(journal_mode(&db.connection).unwrap(), "wal");
}

#[test]
fn comparison_detects_equal_count_changes_storage_classes_rowids_and_sequence() {
    let fixture = Fixture::new();
    fixture.populate();
    fixture.snapshot();
    UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).unwrap();
    let original = fs::read(&fixture.source).unwrap();
    for sql in [
        "UPDATE deleted_terms SET reason='synthetic-changed'",
        "UPDATE sync_domain_state SET cursor=18",
        "UPDATE sqlite_sequence SET seq=seq+10",
        "UPDATE user_terms SET text=CAST(text AS BLOB)",
        "UPDATE selection_events SET id=id+100",
        "UPDATE sync_domain_state SET cursor=cursor",
    ] {
        fs::write(&fixture.source, &original).unwrap();
        let c = Connection::open(&fixture.source).unwrap();
        c.execute_batch(sql).unwrap();
        close(c).unwrap();
        let result = UserDb::verify_maintenance_snapshot(&fixture.source, &fixture.snapshot);
        if sql == "UPDATE sync_domain_state SET cursor=cursor" {
            assert!(result.is_ok());
        } else {
            assert_eq!(result, Err(Error::ContentChanged), "{sql}");
        }
    }
}

#[test]
fn schema_zero_to_nine_are_compared_without_migration() {
    for version in 0..=9 {
        let fixture = Fixture::new();
        if version == 9 {
            fixture.populate();
        } else {
            let c = Connection::open(&fixture.source).unwrap();
            if version > 0 {
                c.execute_batch("CREATE TABLE import_batches(id INTEGER PRIMARY KEY AUTOINCREMENT, source_name TEXT, term_count INTEGER, created_at_ms INTEGER, notes TEXT); INSERT INTO import_batches(source_name,term_count,created_at_ms,notes) VALUES ('synthetic',2,3,NULL)").unwrap();
            }
            c.pragma_update(None, "user_version", version).unwrap();
            close(c).unwrap();
        }
        fixture.snapshot();
        assert_eq!(
            UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot)
                .unwrap()
                .schema_version,
            version
        );
        assert_eq!(
            UserDb::inspect_file(&fixture.source)
                .unwrap()
                .schema_version,
            version
        );
    }
}

#[test]
fn unknown_schema_and_content_changes_are_rejected_before_write_connection() {
    for sql in [
        "CREATE TABLE unknown_data(value TEXT)",
        "CREATE TRIGGER unknown_trigger AFTER INSERT ON user_terms BEGIN SELECT 1; END",
        "PRAGMA user_version=10",
        "UPDATE sync_domain_state SET cursor=999",
    ] {
        let fixture = Fixture::new();
        fixture.populate();
        fixture.snapshot();
        let c = Connection::open(&fixture.source).unwrap();
        c.execute_batch(sql).unwrap();
        close(c).unwrap();
        let before = fs::read(&fixture.source).unwrap();
        let result = UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot);
        assert!(matches!(
            result,
            Err(Error::UnsupportedSchema | Error::ContentChanged)
        ));
        assert_eq!(fs::read(&fixture.source).unwrap(), before);
    }
}

#[test]
fn existing_writer_and_pinned_reader_do_not_report_success() {
    let fixture = Fixture::new();
    fixture.populate();
    fixture.snapshot();
    let writer = Connection::open(&fixture.source).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(
        UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot),
        Err(Error::DatabaseBusy)
    );
    writer.execute_batch("ROLLBACK").unwrap();
    close(writer).unwrap();
    UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).unwrap();

    let reader = Connection::open(&fixture.source).unwrap();
    reader
        .execute_batch("PRAGMA journal_mode=WAL; BEGIN; SELECT count(*) FROM user_terms;")
        .unwrap();
    let mut writer = UserDb::open(&fixture.source).unwrap();
    writer
        .record_selection(SelectionEventDraft::new(
            "synthetic-reader",
            "shi",
            "时",
            0,
            2,
        ))
        .unwrap();
    fs::remove_file(&fixture.snapshot).unwrap();
    fixture.snapshot();
    assert_eq!(
        UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot),
        Err(Error::DatabaseBusy)
    );
    reader.execute_batch("ROLLBACK").unwrap();
    close(reader).unwrap();
    close(writer.connection).unwrap();
    UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).unwrap();
}

#[test]
fn faults_at_each_maintenance_boundary_leave_equivalent_replayable_data() {
    for point in [
        MaintenancePoint::BeforeCheckpoint,
        MaintenancePoint::AfterCheckpoint,
        MaintenancePoint::AfterJournalMode,
        MaintenancePoint::BeforeClose,
        MaintenancePoint::AfterClose,
    ] {
        let fixture = Fixture::new();
        fixture.populate();
        fixture.snapshot();
        let protected = fs::read(&fixture.snapshot).unwrap();
        assert_eq!(
            prepare(&fixture.source, &fixture.snapshot, &mut |actual| {
                if actual == point {
                    Err(Error::Io)
                } else {
                    Ok(())
                }
            }),
            Err(Error::Io)
        );
        assert_eq!(fs::read(&fixture.snapshot).unwrap(), protected);
        UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).unwrap();
    }
}

#[test]
fn missing_alias_and_unsafe_sidecars_are_rejected_without_creation() {
    let fixture = Fixture::new();
    fixture.populate();
    fixture.snapshot();
    assert_eq!(
        UserDb::prepare_source_for_upgrade(fixture.root.join("missing"), &fixture.snapshot),
        Err(Error::UnsafeFile)
    );
    assert!(!fixture.root.join("missing").exists());
    assert_eq!(
        UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.source),
        Err(Error::UnsafeFile)
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&fixture.source, fixture.root.join("alias")).unwrap();
        assert_eq!(
            UserDb::prepare_source_for_upgrade(fixture.root.join("alias"), &fixture.snapshot),
            Err(Error::UnsafeFile)
        );
        fs::hard_link(&fixture.source, fixture.root.join("linked")).unwrap();
        assert_eq!(
            UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot),
            Err(Error::UnsafeFile)
        );
        fs::remove_file(fixture.root.join("linked")).unwrap();
    }
    fs::write(sidecar(&fixture.source, "-journal"), b"synthetic-unowned").unwrap();
    assert_eq!(
        UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot),
        Err(Error::SidecarRemaining)
    );
}

#[test]
fn actual_unclosed_wal_and_process_interruptions_preserve_snapshot_data() {
    for point in [
        "seed",
        "AfterCheckpoint",
        "AfterJournalMode",
        "BeforeClose",
        "AfterClose",
    ] {
        let fixture = Fixture::new();
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "store::maintenance::tests::maintenance_process_child",
                "--ignored",
            ])
            .env("RADISHLEX_MAINTENANCE_TEST_ROOT", &fixture.root)
            .env("RADISHLEX_MAINTENANCE_TEST_POINT", point)
            .output()
            .unwrap();
        assert_eq!(
            child.status.code(),
            Some(73),
            "{point}: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).unwrap();
        let db = UserDb::open(&fixture.source).unwrap();
        assert_eq!(db.list_active_terms().unwrap().len(), 1);
        assert_eq!(db.list_deleted_term_tombstones().unwrap().len(), 1);
    }
}

#[test]
fn protected_snapshot_compares_all_sync_tables_and_binary_material() {
    let fixture = Fixture::new();
    fixture.populate();
    let c = Connection::open(&fixture.source).unwrap();
    c.execute_batch("INSERT INTO sync_remote_objects VALUES ('synthetic', 'object', 'user_terms', 1, 'hash', 'device', 1, 1);
        INSERT INTO sync_local_objects VALUES ('synthetic', 'object', 'user_terms', 'hash', 1, 0, 1);
        INSERT INTO sync_prepared_outbox VALUES ('synthetic','object','user_terms',1,1,NULL,'device','key',1,'synthetic',X'0001',X'0203','hash',1,1,'synthetic','key','device',X'0405',1,1,0,NULL);
        INSERT INTO sync_cycle_journal VALUES ('synthetic','prepared',1,2,0);
        INSERT INTO sync_trusted_domains VALUES ('synthetic',1,'key',1,2,'cursor',1);
        INSERT INTO sync_trusted_devices VALUES ('synthetic','device','synthetic','key',X'0001',1,'agreement',X'0203','active',1,NULL,NULL,NULL);
        INSERT INTO sync_trusted_lifecycle_events VALUES ('synthetic',1,'initial_device','record',1,NULL,1,'{\"synthetic\":true}');
        INSERT INTO sync_wrapped_epoch_materials VALUES ('synthetic','device','agreement','key',1,1,'synthetic',X'0001',X'0203','hash',1);").unwrap();
    close(c).unwrap();
    fixture.snapshot();
    UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).unwrap();
    let original = fs::read(&fixture.source).unwrap();
    for sql in [
        "UPDATE sync_remote_objects SET ciphertext_hash='changed'",
        "UPDATE sync_local_objects SET acknowledged_revision=1",
        "UPDATE sync_prepared_outbox SET encrypted_payload=X'0204'",
        "UPDATE sync_cycle_journal SET cancel_requested=1",
        "UPDATE sync_trusted_domains SET active_key_id='changed'",
        "UPDATE sync_trusted_devices SET signing_public_key=X'0002'",
        "UPDATE sync_trusted_lifecycle_events SET record_json='{\"synthetic\":false}'",
        "UPDATE sync_wrapped_epoch_materials SET wrapped_key=X'0204'",
    ] {
        fs::write(&fixture.source, &original).unwrap();
        let c = Connection::open(&fixture.source).unwrap();
        c.execute_batch(sql).unwrap();
        close(c).unwrap();
        assert_eq!(
            UserDb::verify_maintenance_snapshot(&fixture.source, &fixture.snapshot),
            Err(Error::ContentChanged),
            "{sql}"
        );
    }
}

#[test]
fn maintenance_accepts_old_sqlite_file_and_detects_real_close_failure() {
    let fixture = Fixture::new();
    fs::write(
        &fixture.source,
        include_bytes!("../../tests/fixtures/sqlite-3.46.0-schema9.sqlite3"),
    )
    .unwrap();
    fixture.snapshot();
    UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "store::maintenance::tests::maintenance_process_child",
            "--ignored",
        ])
        .env("RADISHLEX_MAINTENANCE_TEST_POINT", "close_failure")
        .output()
        .unwrap();
    assert_eq!(
        child.status.code(),
        Some(74),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
#[ignore = "only launched by the bounded synthetic parent test"]
fn maintenance_process_child() {
    let point = std::env::var("RADISHLEX_MAINTENANCE_TEST_POINT").unwrap();
    if point == "close_failure" {
        let c = Connection::open_in_memory().unwrap();
        // An intentionally unfinalized statement causes a real SQLITE_BUSY
        // close. Keep this fault wholly inside an exiting child process.
        let statement = c.prepare("SELECT 1").unwrap();
        std::mem::forget(statement);
        assert_eq!(close(c), Err(Error::CloseFailed));
        std::process::exit(74);
    }
    let root = PathBuf::from(std::env::var_os("RADISHLEX_MAINTENANCE_TEST_ROOT").unwrap());
    let fixture = Fixture {
        source: root.join("source.sqlite3"),
        snapshot: root.join("snapshot.sqlite3"),
        root,
    };
    fixture.populate();
    let mut db = UserDb::open(&fixture.source).unwrap();
    db.connection
        .execute_batch("PRAGMA wal_autocheckpoint=0")
        .unwrap();
    db.record_selection(SelectionEventDraft::new(
        "synthetic-crash",
        "shi",
        "时",
        0,
        2,
    ))
    .unwrap();
    fixture.snapshot();
    // Deliberately leave a real writer unclosed. The process ends without
    // connection destructors, leaving committed frames for a new process.
    if point == "seed" {
        std::process::exit(73);
    }
    close(db.connection).unwrap();
    prepare(&fixture.source, &fixture.snapshot, &mut |actual| {
        if format!("{actual:?}") == point {
            std::process::exit(73);
        }
        Ok(())
    })
    .unwrap();
    panic!("requested checkpoint was not reached");
}
