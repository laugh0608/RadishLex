use super::*;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{symlink, DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    holder: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let temporary = PathBuf::from("/private/tmp");
        #[cfg(target_os = "linux")]
        let temporary = PathBuf::from("/tmp");
        let holder = temporary.join(format!(
            "radishlex-reader-fs-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&holder).unwrap();
        let root = holder.join("pack");
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self { holder, root }
    }

    fn file(&self, relative: &str, bytes: &[u8]) {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.root.join(relative))
            .unwrap();
        file.write_all(bytes).unwrap();
    }

    fn directory(&self, relative: &str) {
        DirBuilder::new()
            .mode(0o700)
            .create(self.root.join(relative))
            .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.holder).unwrap();
    }
}

fn control() -> ReadControl {
    ReadControl::new(Duration::from_secs(30)).unwrap().0
}

#[test]
fn reads_and_hashes_descriptor_bound_files_without_changing_identity() {
    let fixture = Fixture::new();
    fixture.directory("observations");
    fixture.file("manifest.json", b"{}\n");
    fixture.file("observations/synthetic.bin", b"synthetic evidence only");
    let control = control();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    assert_eq!(root.inventory().len(), 3);
    let identity = root.inventory()["observations/synthetic.bin"].clone();
    let retained = root
        .read_file("observations/synthetic.bin", 1024, true, &control)
        .unwrap();
    let streamed = root
        .read_file("observations/synthetic.bin", 1024, false, &control)
        .unwrap();
    assert_eq!(retained.identity, identity);
    assert_eq!(
        retained.bytes.as_deref(),
        Some(b"synthetic evidence only".as_slice())
    );
    assert!(streamed.bytes.is_none());
    assert_eq!(
        streamed.sha256,
        <[u8; 32]>::from(Sha256::digest(b"synthetic evidence only"))
    );
    assert_eq!(streamed.sha256, retained.sha256);
    root.verify_unchanged(&control).unwrap();
    root.verify_unchanged(&control).unwrap();
}

#[test]
fn rejects_same_bytes_inode_replacement_after_inventory() {
    let fixture = Fixture::new();
    fixture.file("sample.bin", b"same synthetic bytes");
    let control = control();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    fixture.file("replacement.bin", b"same synthetic bytes");
    fs::rename(
        fixture.root.join("replacement.bin"),
        fixture.root.join("sample.bin"),
    )
    .unwrap();
    assert_eq!(
        root.read_file("sample.bin", 1024, false, &control).err(),
        Some(ReadError::EvidenceChanged)
    );
    assert_eq!(
        root.verify_unchanged(&control),
        Err(ReadError::EvidenceChanged)
    );
}

#[test]
fn rejects_root_and_ancestor_rebinding() {
    let fixture = Fixture::new();
    fixture.directory("ancestor");
    fixture.directory("ancestor/evidence");
    let path = fixture.root.join("ancestor/evidence");
    let control = control();
    let root = EvidenceRoot::open(&path, &control).unwrap();
    fs::rename(fixture.root.join("ancestor"), fixture.root.join("previous")).unwrap();
    fixture.directory("ancestor");
    fixture.directory("ancestor/evidence");
    assert_eq!(
        root.verify_unchanged(&control),
        Err(ReadError::EvidenceChanged)
    );

    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    fs::rename(&fixture.root, fixture.holder.join("previous-pack")).unwrap();
    DirBuilder::new().mode(0o700).create(&fixture.root).unwrap();
    assert_eq!(
        root.verify_unchanged(&control),
        Err(ReadError::EvidenceChanged)
    );
}

#[test]
fn rejects_symlink_hardlink_socket_and_fifo_without_following_or_blocking() {
    let fixture = Fixture::new();
    fixture.file("synthetic.bin", b"no real user data");
    let control = control();
    symlink("synthetic.bin", fixture.root.join("alias.bin")).unwrap();
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    fs::remove_file(fixture.root.join("alias.bin")).unwrap();
    fs::hard_link(
        fixture.root.join("synthetic.bin"),
        fixture.root.join("alias.bin"),
    )
    .unwrap();
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    fs::remove_file(fixture.root.join("alias.bin")).unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(fixture.root.join("socket")).unwrap();
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    fs::remove_file(fixture.root.join("socket")).unwrap();
    let fifo = CString::new(fixture.root.join("fifo").as_os_str().as_bytes()).unwrap();
    // SAFETY: creates only this test's synthetic private FIFO, no external path.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::IdentityMismatch)
    );
}

#[test]
fn rejects_symlink_root_and_ancestor_and_raw_aliases() {
    let fixture = Fixture::new();
    symlink(&fixture.root, fixture.holder.join("alias")).unwrap();
    let control = control();
    assert_eq!(
        EvidenceRoot::open(&fixture.holder.join("alias"), &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    fixture.directory("nested");
    assert_eq!(
        EvidenceRoot::open(&fixture.holder.join("alias/nested"), &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    for path in [
        format!("{}/.", fixture.root.display()),
        format!("{}//nested", fixture.root.display()),
        format!("{}/nested/..", fixture.root.display()),
    ] {
        assert_eq!(
            EvidenceRoot::open(Path::new(&path), &control).err(),
            Some(ReadError::Malformed)
        );
    }
}

#[test]
fn rejects_permission_drift_and_unknown_entries() {
    let fixture = Fixture::new();
    fixture.file("sample.bin", b"synthetic");
    let control = control();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    fs::set_permissions(
        fixture.root.join("sample.bin"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(
        root.read_file("sample.bin", 1024, false, &control).err(),
        Some(ReadError::EvidenceChanged)
    );
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    fs::set_permissions(
        fixture.root.join("sample.bin"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    fixture.file("unknown.bin", b"unknown");
    assert_eq!(
        root.verify_unchanged(&control),
        Err(ReadError::EvidenceChanged)
    );
}

#[test]
fn rejects_case_and_unicode_aliases_and_component_escape() {
    for value in [
        "../outside",
        "/outside",
        "a/../b",
        "a//b",
        "a/./b",
        ".hidden",
        "trailing.",
        "a\\b",
        "caf\u{e9}",
        "a\0b",
    ] {
        assert!(!valid_relative_path(value));
    }
    assert!(!valid_relative_path(&"a".repeat(181)));
    assert!(valid_relative_path(&"a".repeat(180)));
    let fixture = Fixture::new();
    fixture.file("Case.bin", b"one");
    // Case-sensitive filesystems can physically hold both aliases. On macOS a
    // case-insensitive volume refuses create_new; assert the raw case mismatch
    // is not accepted as an inventory lookup in either environment.
    if let Ok(mut file) = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(fixture.root.join("case.bin"))
    {
        file.write_all(b"two").unwrap();
        assert_eq!(
            EvidenceRoot::open(&fixture.root, &control()).err(),
            Some(ReadError::IdentityMismatch)
        );
    } else {
        let control = control();
        let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
        assert_eq!(
            root.read_file("case.bin", 1024, false, &control).err(),
            Some(ReadError::MissingEvidence)
        );
    }
}

#[test]
fn enforces_read_budget_and_cancellation_and_expired_deadline() {
    let fixture = Fixture::new();
    fixture.file("sample.bin", b"12345");
    let control = control();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    assert_eq!(
        root.read_file("sample.bin", 4, false, &control).err(),
        Some(ReadError::ResourceLimit)
    );
    let (cancelled, signal) = ReadControl::new(Duration::from_secs(30)).unwrap();
    signal.cancel();
    assert_eq!(
        root.read_file("sample.bin", 5, true, &cancelled).err(),
        Some(ReadError::Cancelled)
    );
    assert_eq!(root.verify_unchanged(&cancelled), Err(ReadError::Cancelled));
    let expired = ReadControl {
        deadline: std::time::Instant::now() - Duration::from_secs(1),
        cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &expired).err(),
        Some(ReadError::DeadlineExceeded)
    );
}

#[cfg(target_os = "macos")]
#[test]
fn rejects_a_real_descriptor_acl_even_when_mode_is_private() {
    use std::ffi::c_void;
    extern "C" {
        fn acl_init(count: libc::c_int) -> *mut c_void;
        fn acl_create_entry(acl: *mut *mut c_void, entry: *mut *mut c_void) -> libc::c_int;
        fn acl_set_tag_type(entry: *mut c_void, tag: libc::c_int) -> libc::c_int;
        fn acl_set_qualifier(entry: *mut c_void, qualifier: *const c_void) -> libc::c_int;
        fn acl_get_permset(entry: *mut c_void, permissions: *mut *mut c_void) -> libc::c_int;
        fn acl_add_perm(permissions: *mut c_void, permission: libc::c_int) -> libc::c_int;
        fn acl_set_fd(fd: libc::c_int, acl: *mut c_void) -> libc::c_int;
        fn acl_free(object: *mut c_void) -> libc::c_int;
    }
    let fixture = Fixture::new();
    fixture.file("sample.bin", b"synthetic ACL fixture");
    let file = File::open(fixture.root.join("sample.bin")).unwrap();
    let mut entry = std::ptr::null_mut();
    let mut permissions = std::ptr::null_mut();
    // A fictional UUID deliberately names no actual user or contact.
    let synthetic_subject = [0x17u8; 16];
    // SAFETY: all ACL objects and output pointers are live, and only the new
    // private synthetic fixture's descriptor is changed. Darwin header values:
    // ACL_EXTENDED_ALLOW = 1; ACL_READ_DATA = 1 << 1.
    unsafe {
        let mut acl = acl_init(1);
        assert!(!acl.is_null());
        assert_eq!(acl_create_entry(&mut acl, &mut entry), 0);
        assert_eq!(acl_set_tag_type(entry, 1), 0);
        assert_eq!(
            acl_set_qualifier(entry, synthetic_subject.as_ptr().cast()),
            0
        );
        assert_eq!(acl_get_permset(entry, &mut permissions), 0);
        assert_eq!(acl_add_perm(permissions, 1 << 1), 0);
        assert_eq!(acl_set_fd(file.as_raw_fd(), acl), 0);
        assert_eq!(acl_free(acl), 0);
    }
    assert_eq!(identity(&file).unwrap().mode & 0o7777, 0o600);
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control()).err(),
        Some(ReadError::IdentityMismatch)
    );
}

#[test]
fn detects_same_bytes_replacement_during_streaming_read() {
    let fixture = Fixture::new();
    let synthetic = vec![b'x'; CHUNK + 9];
    fixture.file("sample.bin", &synthetic);
    let control = control();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    let mut replaced = false;
    let result = root.read_file_observed("sample.bin", synthetic.len(), false, &control, || {
        if !replaced {
            fixture.file("replacement.bin", &synthetic);
            fs::rename(
                fixture.root.join("replacement.bin"),
                fixture.root.join("sample.bin"),
            )
            .unwrap();
            replaced = true;
        }
        Ok(())
    });
    assert!(replaced);
    assert_eq!(result.err(), Some(ReadError::EvidenceChanged));
}

#[test]
fn honours_cancellation_and_failure_between_actual_read_chunks() {
    let fixture = Fixture::new();
    let synthetic = vec![b'x'; CHUNK + 9];
    fixture.file("sample.bin", &synthetic);
    let (control, signal) = ReadControl::new(Duration::from_secs(30)).unwrap();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    let cancelled = root.read_file_observed("sample.bin", synthetic.len(), false, &control, || {
        signal.cancel();
        Ok(())
    });
    assert_eq!(cancelled.err(), Some(ReadError::Cancelled));
    let control = self::control();
    let failed = root.read_file_observed("sample.bin", synthetic.len(), false, &control, || {
        Err(ReadError::DeadlineExceeded)
    });
    assert_eq!(failed.err(), Some(ReadError::DeadlineExceeded));
}

#[test]
fn bounds_file_inventory_without_skipping_entries() {
    let fixture = Fixture::new();
    fixture.file("manifest.json", b"{}");
    for index in 0..4096 {
        fixture.file(&format!("sample-{index}.bin"), b"");
    }
    let control = control();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    assert_eq!(root.inventory().len(), 4097);
    root.verify_unchanged(&control).unwrap();
    fixture.file("overflow.bin", b"");
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::ResourceLimit)
    );
}

#[test]
fn detects_truncation_during_streaming_read() {
    let fixture = Fixture::new();
    let synthetic = vec![b'x'; CHUNK + 9];
    fixture.file("sample.bin", &synthetic);
    let control = control();
    let root = EvidenceRoot::open(&fixture.root, &control).unwrap();
    let mut truncated = false;
    let result = root.read_file_observed("sample.bin", synthetic.len(), false, &control, || {
        if !truncated {
            OpenOptions::new()
                .write(true)
                .open(fixture.root.join("sample.bin"))
                .unwrap()
                .set_len(CHUNK as u64 - 1)
                .unwrap();
            truncated = true;
        }
        Ok(())
    });
    assert!(truncated);
    assert_eq!(result.err(), Some(ReadError::EvidenceChanged));
}

#[test]
fn rejects_nonprivate_root_descendants_and_writable_ancestors() {
    let fixture = Fixture::new();
    let control = control();
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o700)).unwrap();
    fixture.directory("nested");
    fs::set_permissions(
        fixture.root.join("nested"),
        fs::Permissions::from_mode(0o750),
    )
    .unwrap();
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    fs::set_permissions(
        fixture.root.join("nested"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    // A sticky bit on an arbitrary private ancestor is not the exception for a
    // known system temporary parent.
    fs::set_permissions(&fixture.holder, fs::Permissions::from_mode(0o1777)).unwrap();
    assert_eq!(
        EvidenceRoot::open(&fixture.root, &control).err(),
        Some(ReadError::IdentityMismatch)
    );
    fs::set_permissions(&fixture.holder, fs::Permissions::from_mode(0o700)).unwrap();
    EvidenceRoot::open(&fixture.root, &control).unwrap();
}
