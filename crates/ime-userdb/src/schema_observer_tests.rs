use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

#[test]
fn progress_interrupts_sql_and_closes_connection() {
    let connection = Connection::open_in_memory().unwrap();
    let interrupted = Arc::new(Mutex::new(None));
    let checks = Arc::new(AtomicU64::new(0));
    let count = checks.clone();
    install_progress(
        &connection,
        Arc::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            Err(ObserverError::Deadline)
        }),
        interrupted.clone(),
    )
    .unwrap();
    let result = connection.query_row(
        "WITH RECURSIVE n(v) AS (VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<10000) SELECT sum(v) FROM n",
        [], |row| row.get::<_, i64>(0),
    );
    assert!(result.is_err());
    assert_eq!(checks.load(Ordering::SeqCst), 1);
    assert_eq!(*interrupted.lock().unwrap(), Some(ObserverError::Deadline));
    connection
        .progress_handler(0, None::<fn() -> bool>)
        .unwrap();
    connection.close().unwrap();
}

#[test]
fn manifest_material_boundary_and_overflow_fail_before_clone() {
    let mut bytes = 0;
    account(&mut bytes, MAX_MATERIAL).unwrap();
    assert_eq!(account(&mut bytes, 1), Err(ObserverError::LimitExceeded));
    let mut overflow = usize::MAX;
    assert_eq!(account(&mut overflow, 1), Err(ObserverError::LimitExceeded));
    let input = vec![b'a'; MAX_MATERIAL];
    assert_eq!(
        text(ValueRef::Text(&input), &mut 0).unwrap().len(),
        MAX_MATERIAL
    );
    assert_eq!(
        text(ValueRef::Text(&input), &mut 1),
        Err(ObserverError::LimitExceeded)
    );
}

#[test]
fn schema_object_limit_allows_exactly_sixty_four() {
    let connection = Connection::open_in_memory().unwrap();
    for index in 0..MAX_OBJECTS {
        connection
            .execute_batch(&format!("CREATE TABLE synthetic_{index}(value INTEGER)"))
            .unwrap();
    }
    let control: ObserverControl = Arc::new(|| Ok(()));
    assert_eq!(
        read_manifest(&connection, &mut 0, &control).unwrap().len(),
        MAX_OBJECTS
    );
    connection
        .execute_batch("CREATE TABLE synthetic_overflow(value INTEGER)")
        .unwrap();
    assert_eq!(
        read_manifest(&connection, &mut 0, &control),
        Err(ObserverError::LimitExceeded)
    );
    connection.close().unwrap();
}

#[test]
fn checksum_word_order_is_independent_of_host_endian() {
    let words = [1, 0, 0, 0, 2, 0, 0, 0];
    assert_eq!(wal::checksum(&words, true, [0, 0]), [1, 3]);
    assert_eq!(
        wal::checksum(&words, false, [0, 0]),
        [0x01000000, 0x03000000]
    );
    let continuation = wal::checksum(&words, true, [1, 3]);
    assert_eq!(continuation, [5, 10]);
}
