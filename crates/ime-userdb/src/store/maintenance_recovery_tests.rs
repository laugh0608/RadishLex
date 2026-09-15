use super::*;
use crate::SelectionEventDraft;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
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
                "radishlex-journal-recovery-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let source = root.join("source.sqlite3");
        let snapshot = root.join("snapshot.sqlite3");
        let mut db = UserDb::open(&source).unwrap();
        for (code, text) in [("shi", "时"), ("shan", "删")] {
            db.record_selection(SelectionEventDraft::new(
                "synthetic-recovery",
                code,
                text,
                0,
                2,
            ))
            .unwrap();
        }
        db.delete_term("shan", "删", None).unwrap();
        drop(db);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&snapshot)
            .unwrap();
        UserDb::create_consistent_snapshot(&source, &snapshot).unwrap();
        UserDb::prepare_source_for_upgrade(&source, &snapshot).unwrap();
        Self {
            root,
            source,
            snapshot,
        }
    }
    fn child(&self, point: &str, code: i32) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "store::maintenance::recovery::tests::journal_recovery_child",
                "--ignored",
            ])
            .env("RADISHLEX_JOURNAL_RECOVERY_ROOT", &self.root)
            .env("RADISHLEX_JOURNAL_RECOVERY_POINT", point)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn spill(connection: &Connection) {
    connection.execute_batch("PRAGMA cache_size=1; PRAGMA cache_spill=1; BEGIN IMMEDIATE; UPDATE user_terms SET text=text || printf('%08000d',1);").unwrap();
}

#[test]
fn sqlite_recovers_real_spilled_journal_and_preserves_learning_deletion_and_inode() {
    let fixture = Fixture::new();
    let original = fs::read(&fixture.source).unwrap();
    let protected = fs::read(&fixture.snapshot).unwrap();
    let inode = fs::metadata(&fixture.source).unwrap().ino();
    fixture.child("seed", 70);
    assert_ne!(fs::read(&fixture.source).unwrap(), original);
    let journal = sidecar(&fixture.source, "-journal");
    assert!(fs::metadata(&journal).unwrap().len() > 512);
    assert!(UserDb::prepare_source_for_upgrade(&fixture.source, &fixture.snapshot).is_err());
    assert_eq!(qualify(&fixture.source, &fixture.snapshot).unwrap(), 9);
    assert_eq!(
        UserDb::recover_maintenance_journal(&fixture.source, &fixture.snapshot).unwrap(),
        9
    );
    assert_eq!(fs::metadata(&fixture.source).unwrap().ino(), inode);
    assert_eq!(fs::read(&fixture.snapshot).unwrap(), protected);
    require_no_sidecars(&fixture.source).unwrap();
    let db = UserDb::open(&fixture.source).unwrap();
    assert_eq!(db.list_active_terms().unwrap().len(), 1);
    assert_eq!(db.list_deleted_term_tombstones().unwrap().len(), 1);
}

#[test]
fn synthetic_wal_page_one_beforeimage_finishes_as_standalone_delete() {
    let fixture = Fixture::new();
    fixture.child("seed", 70);
    let journal = sidecar(&fixture.source, "-journal");
    let mut bytes = fs::read(&journal).unwrap();
    let word = |offset: usize| u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let (records, sector, page) = (word(8) as usize, word(20) as usize, word(24) as usize);
    let offset = (0..records)
        .map(|index| sector + index * (page + 8))
        .find(|offset| word(*offset) == 1)
        .expect("first journal segment contains page one");
    // Synthetic header variant, not a real WAL-to-DELETE power-loss fixture.
    // These two header bytes are outside SQLite's sparse journal checksum.
    bytes[offset + 4 + 18..offset + 4 + 20].fill(2);
    fs::write(&journal, bytes).unwrap();
    assert_eq!(
        UserDb::recover_maintenance_journal(&fixture.source, &fixture.snapshot).unwrap(),
        9
    );
    UserDb::verify_prepared_source(&fixture.source, &fixture.snapshot).unwrap();
    assert_eq!(&fs::read(&fixture.source).unwrap()[18..20], &[1, 1]);
}

#[test]
fn every_recovery_boundary_can_be_reloaded_without_inventing_success() {
    for point in ["Opened", "Replayed", "Compared", "Closed"] {
        let fixture = Fixture::new();
        fixture.child("seed", 70);
        fixture.child(point, 71);
        if sidecar(&fixture.source, "-journal").exists() {
            UserDb::recover_maintenance_journal(&fixture.source, &fixture.snapshot).unwrap();
        } else {
            UserDb::verify_prepared_source(&fixture.source, &fixture.snapshot).unwrap();
        }
    }
}

#[test]
fn malformed_multidatabase_and_mixed_wal_journals_do_not_write_source() {
    for kind in [
        "magic",
        "zero-records",
        "page-size",
        "short",
        "super-journal",
        "wal",
        "shm",
    ] {
        let fixture = Fixture::new();
        fixture.child("seed", 70);
        let journal = sidecar(&fixture.source, "-journal");
        let mut bytes = fs::read(&journal).unwrap();
        match kind {
            "magic" => bytes[0] ^= 1,
            "zero-records" => bytes[8..12].fill(0),
            "page-size" => bytes[24..28].fill(0),
            "short" => bytes.truncate(511),
            "super-journal" => bytes.extend_from_slice(&MAGIC),
            "wal" | "shm" => {
                fs::File::create(sidecar(&fixture.source, &format!("-{kind}"))).unwrap();
            }
            _ => unreachable!(),
        }
        fs::write(&journal, &bytes).unwrap();
        let source = fs::read(&fixture.source).unwrap();
        assert!(
            UserDb::recover_maintenance_journal(&fixture.source, &fixture.snapshot).is_err(),
            "{kind}"
        );
        assert_eq!(fs::read(&fixture.source).unwrap(), source);
        assert_eq!(fs::read(&journal).unwrap(), bytes);
    }
}

#[test]
fn recovery_never_claims_equivalence_to_a_different_protected_snapshot() {
    let fixture = Fixture::new();
    fixture.child("seed", 70);
    let changed = open(&fixture.snapshot, true).unwrap();
    changed
        .execute_batch("UPDATE user_terms SET weight=weight+1")
        .unwrap();
    close(changed).unwrap();
    let protected = fs::read(&fixture.snapshot).unwrap();
    assert_eq!(
        UserDb::recover_maintenance_journal(&fixture.source, &fixture.snapshot),
        Err(Error::ContentChanged)
    );
    assert_eq!(fs::read(&fixture.snapshot).unwrap(), protected);
    // SQLite may already have replayed/deleted the journal. A failed comparison
    // is not an instruction to replace the source with the different snapshot.
    assert_ne!(fs::read(&fixture.source).unwrap(), protected);
}

#[test]
fn live_spilled_writer_is_busy_and_cannot_be_recovered() {
    let fixture = Fixture::new();
    let writer = open(&fixture.source, true).unwrap();
    spill(&writer);
    let source = fs::read(&fixture.source).unwrap();
    let journal = fs::read(sidecar(&fixture.source, "-journal")).unwrap();
    assert_eq!(
        UserDb::recover_maintenance_journal(&fixture.source, &fixture.snapshot),
        Err(Error::DatabaseBusy)
    );
    assert_eq!(fs::read(&fixture.source).unwrap(), source);
    assert_eq!(
        fs::read(sidecar(&fixture.source, "-journal")).unwrap(),
        journal
    );
    close(writer).unwrap();
}

#[test]
#[ignore = "only invoked by the synthetic recovery parent tests"]
fn journal_recovery_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_JOURNAL_RECOVERY_ROOT").unwrap());
    assert_eq!(
        root.parent().unwrap(),
        fs::canonicalize(std::env::temp_dir()).unwrap()
    );
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("radishlex-journal-recovery-"));
    let source = root.join("source.sqlite3");
    let snapshot = root.join("snapshot.sqlite3");
    let point = std::env::var("RADISHLEX_JOURNAL_RECOVERY_POINT").unwrap();
    if point == "export" {
        let fixture = Fixture::new();
        fixture.child("seed", 70);
        for (input, name) in [
            (fixture.snapshot.clone(), "journal-before.sqlite3"),
            (fixture.source.clone(), "journal-spilled.sqlite3"),
            (
                sidecar(&fixture.source, "-journal"),
                "journal-spilled.sqlite3-journal",
            ),
        ] {
            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(root.join(name))
                .unwrap();
            std::io::copy(&mut fs::File::open(input).unwrap(), &mut destination).unwrap();
            destination.sync_all().unwrap();
        }
        return;
    }
    if point == "seed" {
        let connection = open(&source, true).unwrap();
        spill(&connection);
        assert_eq!(fs::read(sidecar(&source, "-journal")).unwrap()[..8], MAGIC);
        std::process::exit(70);
    }
    recover(&source, &snapshot, &mut |actual| {
        if format!("{actual:?}") == point {
            std::process::exit(71);
        }
        Ok(())
    })
    .unwrap();
    panic!("checkpoint not reached");
}
