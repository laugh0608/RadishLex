use super::*;
use std::hash::{Hash, Hasher};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const NEW: &str = "11111111111111111111111111111111";
const OLD: &str = "22222222222222222222222222222222";
const OUTER: &str = "33333333333333333333333333333333";
const POINTS: [HistoryFault; 12] = [
    HistoryFault::DirectoryCreated(0),
    HistoryFault::DirectorySynced(0),
    HistoryFault::DirectoryCreated(1),
    HistoryFault::DirectorySynced(1),
    HistoryFault::DirectoryCreated(2),
    HistoryFault::DirectorySynced(2),
    HistoryFault::InventoryCreated,
    HistoryFault::InventoryWritten,
    HistoryFault::InventorySynced,
    HistoryFault::BeforeInventoryRename,
    HistoryFault::InventoryRenamed,
    HistoryFault::InventoryDirectorySynced,
];

// Only persistence tests use this deterministic stream digest. The adapter's
// integration tests separately exercise actual SHA-256 and real SQLite.
struct SyntheticHash;
impl PreparationHasher for SyntheticHash {
    fn sha256(&self, source: &mut dyn Read) -> std::io::Result<[u8; 32]> {
        let mut bytes = Vec::new();
        source.read_to_end(&mut bytes)?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hash);
        let mut result = [0; 32];
        for chunk in result.chunks_exact_mut(8) {
            chunk.copy_from_slice(&hash.finish().to_le_bytes());
        }
        Ok(result)
    }
}

#[test]
fn verified_history_read_binds_bytes_and_digest_in_one_bounded_observation() {
    let fixture = Fixture::new();
    let (store, _guard) = fixture.open();
    let path = fixture.0.join("bounded-proof.json");
    let bytes = b"synthetic canonical proof\n";
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.write_all(bytes).unwrap();
    let (identity, observed) = store.read_verified_history(&path, &SyntheticHash).unwrap();
    assert_eq!(observed, bytes);
    assert_eq!(identity, store.evidence(&path, &SyntheticHash).unwrap());
    // A later observation must not adopt the earlier result.
    fs::write(&path, b"changed proof\n").unwrap();
    assert_ne!(
        store
            .read_verified_history(&path, &SyntheticHash)
            .unwrap()
            .0,
        identity
    );
    fs::write(&path, vec![b'x'; MAX_PREPARATION_RECEIPT_BYTES]).unwrap();
    assert_eq!(
        store
            .read_verified_history(&path, &SyntheticHash)
            .unwrap()
            .1
            .len(),
        MAX_PREPARATION_RECEIPT_BYTES
    );
    fs::write(&path, vec![b'x'; MAX_PREPARATION_RECEIPT_BYTES + 1]).unwrap();
    assert_eq!(
        store.read_verified_history(&path, &SyntheticHash),
        Err(Error::EvidenceChanged)
    );
    struct FailedHash;
    impl PreparationHasher for FailedHash {
        fn sha256(&self, _: &mut dyn Read) -> std::io::Result<[u8; 32]> {
            Err(std::io::Error::other("synthetic hash failure"))
        }
    }
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        store.read_verified_history(&path, &FailedHash),
        Err(Error::Io)
    );
}

#[test]
fn verified_history_read_rejects_drift_during_hash_and_partial_consumption() {
    struct DriftHash<'a> {
        path: &'a Path,
        retained: &'a Path,
        kind: u8,
    }
    impl PreparationHasher for DriftHash<'_> {
        fn sha256(&self, source: &mut dyn Read) -> std::io::Result<[u8; 32]> {
            if self.kind == 6 {
                return Ok([0; 32]);
            }
            let digest = SyntheticHash.sha256(source)?;
            match self.kind {
                0 => {
                    // Same bytes, new inode.
                    let bytes = fs::read(self.path)?;
                    fs::rename(self.path, self.retained)?;
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(self.path)?;
                    file.write_all(&bytes)?;
                }
                1 => fs::write(self.path, b"different proof")?,
                2 => fs::set_permissions(self.path, fs::Permissions::from_mode(0o644))?,
                3 => fs::hard_link(self.path, self.retained)?,
                4 => {
                    fs::rename(self.path, self.retained)?;
                    std::os::unix::fs::symlink(self.retained, self.path)?;
                }
                5 => fs::remove_file(self.path)?,
                _ => unreachable!(),
            }
            Ok(digest)
        }
    }
    for kind in 0..7 {
        let fixture = Fixture::new();
        let (store, _guard) = fixture.open();
        let path = fixture.0.join("bounded-proof.json");
        let retained = fixture.0.join("retained-proof.json");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(b"synthetic canonical proof\n").unwrap();
        assert_eq!(
            store.read_verified_history(
                &path,
                &DriftHash {
                    path: &path,
                    retained: &retained,
                    kind
                }
            ),
            Err(Error::EvidenceChanged),
            "kind {kind}"
        );
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "radishlex-inventory-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        let fixture = Self(root);
        let store = UpgradeReceiptStore::open(fixture.verified()).unwrap();
        let mut receipt = UpgradeReceipt::new(
            OLD,
            None,
            crate::ProductRelease::new("26.7.1", 39).unwrap(),
            crate::ProductRelease::new("26.7.1", 40).unwrap(),
            None,
            9,
            vec![store.data_root_identity().clone()],
        )
        .unwrap();
        receipt
            .abort_preserved(crate::UpgradeFailureCode::ProcessNotQuiescent, false)
            .unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(fixture.0.join(STATE_DIRECTORY_NAME).join(RECEIPT_FILE_NAME))
            .unwrap();
        file.write_all(&receipt.encode().unwrap()).unwrap();
        file.sync_all().unwrap();
        fixture
    }
    fn verified(&self) -> VerifiedDataRoot {
        VerifiedDataRoot::verify(&self.0, fs::metadata(&self.0).unwrap().uid()).unwrap()
    }
    fn open(&self) -> (PreparationJournalStore, UpgradeProcessGuard) {
        let store = PreparationJournalStore::open_existing(self.verified()).unwrap();
        let guard = store.acquire_guard().unwrap();
        (store, guard)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct Fault {
    point: HistoryFault,
    exit: bool,
}
impl HistoryFaults for Fault {
    fn hit(&self, point: HistoryFault) -> Result<()> {
        if point != self.point {
            return Ok(());
        }
        if self.exit {
            std::process::exit(74);
        }
        Err(Error::Io)
    }
}

#[test]
fn inventory_initialization_io_failures_preserve_old_slots_and_orphan_targets() {
    for point in POINTS {
        let fixture = Fixture::new();
        let receipt_path = fixture.0.join(STATE_DIRECTORY_NAME).join(RECEIPT_FILE_NAME);
        let bytes = fs::read(&receipt_path).unwrap();
        let inode = fs::metadata(&receipt_path).unwrap().ino();
        let (store, guard) = fixture.open();
        assert!(store
            .capture_inventory_with_faults(
                &guard,
                NEW,
                Some(OUTER),
                Some(OLD),
                &SyntheticHash,
                &Fault { point, exit: false }
            )
            .is_err());
        assert_eq!(fs::read(&receipt_path).unwrap(), bytes);
        assert_eq!(fs::metadata(&receipt_path).unwrap().ino(), inode);
        assert!(store.load_guarded(&guard).unwrap().is_none());
        drop(guard);
        let (store, guard) = fixture.open();
        let retry =
            store.capture_previous_inventory(&guard, NEW, Some(OUTER), Some(OLD), &SyntheticHash);
        if matches!(
            point,
            HistoryFault::DirectoryCreated(0) | HistoryFault::DirectorySynced(0)
        ) {
            // Only the empty shared parent was created; no private operation is adopted.
            assert!(retry.is_ok());
        } else {
            assert!(retry.is_err(), "{point:?}");
        }
    }
}

#[test]
fn inventory_initialization_process_exits_preserve_without_adopting_partial_operations() {
    for (index, point) in POINTS.into_iter().enumerate() {
        let fixture = Fixture::new();
        let path = fixture.0.join(STATE_DIRECTORY_NAME).join(RECEIPT_FILE_NAME);
        let bytes = fs::read(&path).unwrap();
        let status = Command::new(std::env::current_exe().unwrap()).args(["--exact", "filesystem::preparation_journal::source_preparation::history::inventory::tests::inventory_crash_child", "--ignored"])
            .env("RADISHLEX_INVENTORY_ROOT", &fixture.0).env("RADISHLEX_INVENTORY_POINT", index.to_string()).status().unwrap();
        assert_eq!(status.code(), Some(74), "{point:?}");
        assert_eq!(fs::read(path).unwrap(), bytes);
        let (store, guard) = fixture.open();
        let retry =
            store.capture_previous_inventory(&guard, NEW, Some(OUTER), Some(OLD), &SyntheticHash);
        assert_eq!(retry.is_ok(), index < 2);
    }
}
#[test]
#[ignore = "parent supplies a fresh isolated synthetic root"]
fn inventory_crash_child() {
    let root = PathBuf::from(std::env::var_os("RADISHLEX_INVENTORY_ROOT").unwrap());
    let owner = fs::metadata(&root).unwrap().uid();
    let store =
        PreparationJournalStore::open_existing(VerifiedDataRoot::verify(root, owner).unwrap())
            .unwrap();
    let guard = store.acquire_guard().unwrap();
    let index: usize = std::env::var("RADISHLEX_INVENTORY_POINT")
        .unwrap()
        .parse()
        .unwrap();
    store
        .capture_inventory_with_faults(
            &guard,
            NEW,
            Some(OUTER),
            Some(OLD),
            &SyntheticHash,
            &Fault {
                point: POINTS[index],
                exit: true,
            },
        )
        .unwrap();
    panic!("fault was not reached");
}

#[test]
fn inventory_is_canonical_bounded_and_rejects_unknown_and_duplicate_fields() {
    let fixture = Fixture::new();
    let (store, guard) = fixture.open();
    let inventory = store
        .capture_previous_inventory(&guard, NEW, Some(OUTER), Some(OLD), &SyntheticHash)
        .unwrap();
    let bytes = inventory.encode().unwrap();
    assert_eq!(PreviousInventory::decode(&bytes).unwrap(), inventory);
    let text = String::from_utf8(bytes).unwrap();
    for invalid in [
        text.trim_end().to_owned(),
        text.replace(FORMAT, "unknown"),
        text.replacen('{', "{\"unknown\":true,", 1),
        text.replacen('{', &format!("{{\"format\":\"{FORMAT}\","), 1),
        " ".repeat(MAX_PREPARATION_RECEIPT_BYTES + 1),
    ] {
        assert!(PreviousInventory::decode(invalid.as_bytes()).is_err());
    }
    let path = fixture.0.join(HISTORY).join(OLD).join(INVENTORY);
    fs::write(path, text.replace(OUTER, NEW)).unwrap();
    assert!(store
        .previous_inventory_identity(&guard, &inventory, &SyntheticHash)
        .is_err());
}
