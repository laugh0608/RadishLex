//! Runtime identity of this test artifact's bundled SQLite, not a frozen product.

#[test]
#[ignore = "opt-in linked SQLite identity evidence"]
fn sqlite_library_identity() {
    let connection = rusqlite::Connection::open_in_memory().expect("isolated SQLite");
    let (version, source_id): (String, String) = connection
        .query_row("SELECT sqlite_version(), sqlite_source_id()", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("SQLite runtime identity");
    assert_eq!(version, rusqlite::version());
    println!("SQLite version: {version}\nSQLite source id: {source_id}");
}
