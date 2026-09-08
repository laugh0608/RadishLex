use super::*;

fn checkpoint(connection: &rusqlite::Connection, sql: &str) -> (i64, i64, i64) {
    connection
        .query_row(sql, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("checkpoint returns its busy and frame counts")
}

#[test]
fn concurrent_writers_and_checkpoints_preserve_every_learning_transaction() {
    let path = temp_db_path("concurrent-wal-checkpoint");
    let observer = UserDb::open(&path).expect("observer opens");
    const WRITERS: usize = 3;
    const ROUNDS: usize = 64;
    let barrier = Arc::new(Barrier::new(WRITERS + 1));
    // Open every connection before spawning so initialization errors cannot strand a barrier.
    let writers = (0..WRITERS)
        .map(|_| {
            let db = UserDb::open(&path).expect("writer opens");
            db.connection
                .pragma_update(None, "wal_autocheckpoint", 0)
                .expect("explicit checkpoints only");
            db
        })
        .collect::<Vec<_>>();
    let workers = writers
        .into_iter()
        .enumerate()
        .map(|(writer, mut db)| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut results = Vec::new();
                for _ in 0..ROUNDS {
                    barrier.wait();
                    results.push(db.record_selection(SelectionEventDraft::new(
                        format!("synthetic-writer-{writer}"),
                        "bingfa",
                        "合成并发词",
                        0,
                        1,
                    )));
                    barrier.wait();
                }
                results
            })
        })
        .collect::<Vec<_>>();
    let mut checkpoint_results = Vec::new();
    for round in 0..ROUNDS {
        barrier.wait();
        let mode = match round % 3 {
            0 => "PRAGMA wal_checkpoint(PASSIVE)",
            1 => "PRAGMA wal_checkpoint(RESTART)",
            _ => "PRAGMA wal_checkpoint(TRUNCATE)",
        };
        // Defer assertions until all workers leave the barriers, including on errors.
        checkpoint_results.push(observer.connection.query_row(mode, [], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        }));
        barrier.wait();
    }
    for worker in workers {
        for result in worker.join().expect("writer joins") {
            assert!(result.expect("each learning transaction commits").is_some());
        }
    }
    for result in checkpoint_results {
        let (busy, total, copied) = result.expect("checkpoint returns status");
        assert!((0..=1).contains(&busy));
        assert!(total >= 0 && copied >= 0 && copied <= total);
    }
    assert_eq!(
        checkpoint(&observer.connection, "PRAGMA wal_checkpoint(TRUNCATE)"),
        (0, 0, 0)
    );
    drop(observer);

    let reopened = UserDb::open(&path).expect("database reopens after WAL reset");
    let counts: (i64, i64, i64) = reopened
        .connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM selection_events),
                    (SELECT COUNT(*) FROM user_terms),
                    (SELECT frequency FROM ranker_weights WHERE input_code = 'bingfa')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("learning tables agree");
    assert_eq!(
        counts,
        ((WRITERS * ROUNDS) as i64, 1, (WRITERS * ROUNDS) as i64)
    );
    let integrity: String = reopened
        .connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("full integrity check");
    assert_eq!(integrity, "ok");
    drop(reopened);
    remove_temp_db(&path);
}

#[test]
fn pinned_reader_prevents_reset_and_retains_snapshot_until_transaction_ends() {
    let path = temp_db_path("pinned-reader-checkpoint");
    let mut writer = UserDb::open(&path).expect("writer opens");
    let reader = UserDb::open(&path).expect("reader opens");
    writer
        .connection
        .pragma_update(None, "wal_autocheckpoint", 0)
        .unwrap();
    writer.connection.busy_timeout(Duration::ZERO).unwrap();
    assert_eq!(
        checkpoint(&writer.connection, "PRAGMA wal_checkpoint(TRUNCATE)"),
        (0, 0, 0)
    );
    reader.connection.execute_batch("BEGIN").unwrap();
    let count = |connection: &rusqlite::Connection| -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM selection_events", [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    assert_eq!(count(&reader.connection), 0);
    writer
        .record_selection(SelectionEventDraft::new(
            "synthetic-reader",
            "kuaizhao",
            "合成快照词",
            0,
            1,
        ))
        .unwrap();
    let (busy, total, copied) = checkpoint(&writer.connection, "PRAGMA wal_checkpoint(RESTART)");
    assert_eq!(busy, 1, "an active old reader must block WAL reset");
    assert!(total > copied);
    assert_eq!(count(&reader.connection), 0);
    reader.connection.execute_batch("COMMIT").unwrap();
    assert_eq!(
        checkpoint(&writer.connection, "PRAGMA wal_checkpoint(TRUNCATE)"),
        (0, 0, 0)
    );
    assert_eq!(count(&reader.connection), 1);
    writer
        .record_selection(SelectionEventDraft::new(
            "synthetic-reader",
            "kuaizhao",
            "合成快照词",
            0,
            1,
        ))
        .unwrap();
    assert_eq!(count(&reader.connection), 2);
    drop(reader);
    drop(writer);
    remove_temp_db(&path);
}
