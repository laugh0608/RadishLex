//! Every path in these tests is a fresh synthetic creation under /private/tmp.
use super::*;
use crate::schema_observer_budget::ObservationControl;
use radishlex_ime_userdb::schema_observer::{
    create_synthetic_fixture, inspect_private_copy, FixtureKind, ObserverError,
};
use std::cell::RefCell;
use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    kind: FixtureKind,
    root: PathBuf,
    source: PathBuf,
    caches: PathBuf,
    identity: PreparationDirectoryIdentity,
    family: PreparationFamily,
    source_files: BTreeMap<String, (u64, u64)>,
}
impl Fixture {
    fn new(kind: FixtureKind) -> Self {
        let root = PathBuf::from(format!(
            "/private/tmp/radishlex-production-schema-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        let source = root.join("source");
        let caches = root.join("caches");
        for path in [&source, &caches] {
            DirBuilder::new().mode(0o700).create(path).unwrap();
        }
        create_synthetic_fixture(&source, kind).unwrap();
        let meta = fs::symlink_metadata(&source).unwrap();
        let identity = PreparationDirectoryIdentity {
            device_id: meta.dev(),
            inode: meta.ino(),
            owner_id: meta.uid(),
            mode: meta.mode() & 0o7777,
        };
        let mut source_files = BTreeMap::new();
        let mut member = |name: &str| {
            let path = source.join(name);
            let meta = match fs::symlink_metadata(&path) {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
                Err(error) => panic!("synthetic fixture metadata: {error}"),
            };
            source_files.insert(name.into(), (meta.dev(), meta.ino()));
            let digest = Sha256::digest(fs::read(path).unwrap());
            Some(PreparationFileIdentity {
                device_id: meta.dev(),
                inode: meta.ino(),
                owner_id: meta.uid(),
                mode: meta.mode() & 0o7777,
                link_count: meta.nlink(),
                byte_len: meta.len(),
                sha256: digest.iter().map(|v| format!("{v:02x}")).collect(),
            })
        };
        let family = PreparationFamily {
            database: member(FAMILY[0]).unwrap(),
            wal: member(FAMILY[1]),
            shm: member(FAMILY[2]),
            journal: member(FAMILY[3]),
        };
        Self {
            kind,
            root,
            source,
            caches,
            identity,
            family,
            source_files,
        }
    }
    fn scratch(&self, budget: &mut ObserverBudget) -> Scratch {
        Scratch::create_at(
            &self.caches,
            &self.source,
            &self.identity,
            &self.family,
            budget,
        )
        .unwrap()
    }
    fn active(&self) -> PathBuf {
        self.caches.join("RadishLex/SchemaObservation-v1/active")
    }
    fn capture_failed(&self, scratch: &Scratch, error: FsError) -> ! {
        // Only this host's synthetic paths are diagnosed. These are observations
        // after rejection, not evidence of metadata at the failing syscall.
        // Do not open SQLite, retry capture or dispose an unexplained failure.
        let mut paths = vec![self.source.clone(), self.active()];
        paths.extend(self.caches.ancestors().map(Path::to_path_buf));
        for directory in [&self.source, &self.active()] {
            paths.extend(FAMILY.map(|name| directory.join(name)));
        }
        let observed: Vec<_> = paths
            .iter()
            .map(|path| (path, fs::symlink_metadata(path)))
            .collect();
        let held: Vec<_> = scratch
            .held
            .iter()
            .map(|held| (held.name, &held.id, held.file.as_ref().map(posix::identity)))
            .collect();
        let copied: Vec<_> = scratch
            .copied
            .iter()
            .map(|(name, copy)| (name, &copy.id, &copy.digest))
            .collect();
        panic!(
            "synthetic capture rejected: {error:?}; kind={:?}; expected_root={:?}; \
             expected_family={:?}; held={held:?}; copied={copied:?}; \
             post_failure_metadata={observed:?}; retained_root={:?}",
            self.kind, self.identity, self.family, self.root
        );
    }
    fn finish(self) {
        assert!(
            !self.active().exists(),
            "unresolved active must remain for inspection"
        );
        for (name, expected) in self.source_files {
            let path = self.source.join(name);
            let meta = fs::symlink_metadata(&path).unwrap();
            assert_eq!((meta.dev(), meta.ino()), expected);
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(&self.source).unwrap();
        for path in [
            self.caches.join("RadishLex/SchemaObservation-v1"),
            self.caches.join("RadishLex"),
        ] {
            match fs::remove_dir(path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => panic!("synthetic directory cleanup: {error}"),
            }
        }
        fs::remove_dir(&self.caches).unwrap();
        fs::remove_dir(self.root).unwrap();
    }
}
fn budget() -> ObserverBudget {
    ObserverBudget::new(
        ObservationControl::enter(Instant::now() + Duration::from_secs(120)).unwrap(),
    )
}
fn sql_error(error: BudgetError) -> ObserverError {
    match error {
        BudgetError::Cancelled => ObserverError::Cancelled,
        BudgetError::Deadline | BudgetError::EntryTooShort => ObserverError::Deadline,
        _ => ObserverError::LimitExceeded,
    }
}

fn acl_sibling_change(
    point: posix::diagnostics::Checkpoint,
    read: fn(&File) -> Result<(), FsError>,
) {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let directory = File::open(&fixture.caches).unwrap();
    let sibling = fixture.caches.join("planned-sibling");
    let created = sibling.clone();
    let hook = posix::diagnostics::install(point, &directory, move || {
        DirBuilder::new().mode(0o700).create(created).unwrap();
    });
    let result = read(&directory);
    assert!(hook.fired());
    assert_eq!(result, Ok(()), "directory entry churn is not an ACL change");
    let meta = fs::symlink_metadata(&sibling).unwrap();
    assert!(meta.is_dir() && meta.mode() & 0o7777 == 0o700);
    fs::remove_dir(sibling).unwrap();
    close(directory).unwrap();
    fixture.finish();
}

#[test]
fn private_directory_acl_accepts_unrelated_entry_churn() {
    acl_sibling_change(
        posix::diagnostics::Checkpoint::PrivateAclRead,
        posix::reject_acl,
    );
}

#[test]
fn ancestor_acl_accepts_unrelated_entry_churn() {
    acl_sibling_change(
        posix::diagnostics::Checkpoint::AncestorAclRead,
        posix::reject_grant_acl,
    );
}

#[test]
fn ancestor_open_reports_changes_without_adopting_the_path() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let directory = File::open(&fixture.root).unwrap();
    let sibling = fixture.root.join("planned-sibling");
    let created = sibling.clone();
    let hook = posix::diagnostics::install(
        posix::diagnostics::Checkpoint::AncestorOpened,
        &directory,
        move || {
            DirBuilder::new().mode(0o700).create(created).unwrap();
        },
    );
    let chain = Chain::open(&fixture.caches, fixture.identity.owner_id, &budget());
    assert!(hook.fired());
    assert!(matches!(chain, Err(FsError::Changed)));
    fs::remove_dir(sibling).unwrap();
    close(directory).unwrap();
    fixture.finish();
}

fn install_synthetic_acl(file: &File, tag: i32) {
    use std::ffi::c_void;
    use std::os::fd::AsRawFd;
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
    let subject = [0x17u8; 16]; // Fictional UUID, never a user/contact lookup.
    let mut entry = std::ptr::null_mut();
    let mut permissions = std::ptr::null_mut();
    // SAFETY: new synthetic fd only; live owned ACL/output pointers, Darwin
    // ALLOW=1, DENY=2 and READ_DATA=1<<1. Only the new fixture is changed.
    unsafe {
        let mut acl = acl_init(1);
        assert!(!acl.is_null());
        assert_eq!(acl_create_entry(&mut acl, &mut entry), 0);
        assert_eq!(acl_set_tag_type(entry, tag), 0);
        assert_eq!(acl_set_qualifier(entry, subject.as_ptr().cast()), 0);
        assert_eq!(acl_get_permset(entry, &mut permissions), 0);
        assert_eq!(acl_add_perm(permissions, 1 << 1), 0);
        assert_eq!(acl_set_fd(file.as_raw_fd(), acl), 0);
        assert_eq!(acl_free(acl), 0);
    }
}

#[test]
fn directory_acl_grants_between_observations_are_rejected() {
    for (point, read) in [
        (
            posix::diagnostics::Checkpoint::PrivateAclRead,
            posix::reject_acl as fn(&File) -> Result<(), FsError>,
        ),
        (
            posix::diagnostics::Checkpoint::AncestorAclRead,
            posix::reject_grant_acl,
        ),
    ] {
        let fixture = Fixture::new(FixtureKind::MainOnly);
        let directory = File::open(&fixture.caches).unwrap();
        let changed = directory.try_clone().unwrap();
        let hook = posix::diagnostics::install(point, &directory, move || {
            install_synthetic_acl(&changed, 1)
        });
        assert!(read(&directory).is_err());
        assert!(hook.fired());
        close(directory).unwrap();
        fixture.finish();
    }
}

#[test]
fn directory_mode_drift_during_acl_query_is_rejected() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let directory = File::open(&fixture.caches).unwrap();
    let changed = directory.try_clone().unwrap();
    let hook = posix::diagnostics::install(
        posix::diagnostics::Checkpoint::PrivateAclRead,
        &directory,
        move || {
            changed
                .set_permissions(fs::Permissions::from_mode(0o750))
                .unwrap();
        },
    );
    assert!(posix::reject_acl(&directory).is_err());
    assert!(hook.fired());
    directory
        .set_permissions(fs::Permissions::from_mode(0o700))
        .unwrap();
    close(directory).unwrap();
    fixture.finish();
}

#[test]
fn deny_only_acl_is_allowed_only_for_ancestors() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let directory = File::open(&fixture.caches).unwrap();
    install_synthetic_acl(&directory, 2);
    assert_eq!(posix::reject_grant_acl(&directory), Ok(()));
    assert!(posix::reject_acl(&directory).is_err());
    close(directory).unwrap();
    fixture.finish();
}

#[test]
fn source_file_content_drift_during_acl_query_is_rejected() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let database = File::open(fixture.source.join(FAMILY[0])).unwrap();
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .open(fixture.source.join(FAMILY[0]))
        .unwrap();
    let hook = posix::diagnostics::install(
        posix::diagnostics::Checkpoint::PrivateAclRead,
        &database,
        move || {
            writer.write_all(b"x").unwrap();
        },
    );
    assert!(posix::reject_acl(&database).is_err());
    assert!(hook.fired());
    close(database).unwrap();
    fixture.finish();
}

#[test]
fn evidence_source_and_cache_overlap_fail_before_copy_creation() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    for evidence in [&fixture.source, &fixture.root, &fixture.caches] {
        assert!(matches!(
            Scratch::create_checked(
                &fixture.caches,
                &fixture.source,
                &fixture.identity,
                &fixture.family,
                Some(evidence),
                &mut budget(),
            ),
            Err(FsError::Identity)
        ));
        assert!(!fixture.active().exists());
    }
    assert!(matches!(
        Scratch::create_at(
            &fixture.source,
            &fixture.source,
            &fixture.identity,
            &fixture.family,
            &mut budget(),
        ),
        Err(FsError::Identity)
    ));
    assert!(!fixture.source.join("RadishLex").exists());
    fixture.finish();
}

#[test]
fn production_copy_uses_compiled_layout_and_disposes_main_and_wal() {
    for kind in [
        FixtureKind::MainOnly,
        FixtureKind::OldMainNewWal,
        FixtureKind::WithShm,
    ] {
        let fixture = Fixture::new(kind);
        let mut budget = budget();
        let mut scratch = fixture.scratch(&mut budget);
        scratch
            .capture(&mut budget)
            .unwrap_or_else(|error| fixture.capture_failed(&scratch, error));
        scratch.mark_sqlite_started(&budget).unwrap();
        let path = scratch.database_path();
        let control = budget.control();
        let shared = RefCell::new(&mut budget);
        let observation = inspect_private_copy(
            &path,
            Arc::new(move || control.check_work().map_err(sql_error)),
            &mut |bytes| shared.borrow_mut().charge_read(bytes).map_err(sql_error),
            &mut || {
                scratch
                    .verify_copy(&mut shared.borrow_mut())
                    .map_err(|_| ObserverError::EvidenceChanged)
            },
        )
        .unwrap();
        assert_eq!(observation.schema_version(), 9);
        assert_eq!(observation.profile(), "compiled-fresh-schema9-exact-v1");
        scratch.dispose(&mut budget).unwrap();
        assert!(!fixture.active().exists());
        fixture.finish();
    }
}

#[test]
fn task_revocation_stops_work_but_preserves_disposal_budget() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let active = Arc::new(AtomicBool::new(true));
    let check = active.clone();
    let control = ObservationControl::enter_task(
        Instant::now() + Duration::from_secs(120),
        Arc::new(move || check.load(Ordering::Acquire)),
    )
    .unwrap();
    let mut budget = ObserverBudget::new(control.clone());
    let mut scratch = fixture.scratch(&mut budget);
    scratch.capture(&mut budget).unwrap();
    active.store(false, Ordering::Release);
    assert_eq!(control.check_work(), Err(BudgetError::Cancelled));
    scratch.dispose(&mut budget).unwrap();
    assert_eq!(control.check_work(), Err(BudgetError::Cancelled));
    fixture.finish();
}

#[test]
fn existing_active_is_never_adopted_and_unknown_objects_are_retained() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let mut budget = budget();
    let scratch = fixture.scratch(&mut budget);
    assert!(matches!(
        Scratch::create_at(
            &fixture.caches,
            &fixture.source,
            &fixture.identity,
            &fixture.family,
            &mut budget
        ),
        Err(FsError::ScratchOccupied)
    ));
    let path = fixture.active().join("unknown");
    let file = posix::create_file(scratch.active().unwrap(), "unknown").unwrap();
    let owned = identity(&file).unwrap();
    close(file).unwrap();
    assert_eq!(scratch.dispose(&mut budget), Err(FsError::UnknownEntry));
    assert!(path.exists());
    // Fixture-host cleanup of its exact planned unknown creation, never a
    // product recovery capability or an adoption of an externally found file.
    let meta = fs::symlink_metadata(&path).unwrap();
    assert_eq!((meta.dev(), meta.ino()), (owned.device, owned.inode));
    fs::remove_file(path).unwrap();
    fs::remove_dir(fixture.active()).unwrap();
    fixture.finish();
}

#[test]
fn external_removal_cannot_succeed() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let mut budget = budget();
    let scratch = fixture.scratch(&mut budget);
    // Removing the empty owned leaf models cache eviction, with no SQLite open.
    fs::remove_dir(fixture.active()).unwrap();
    assert_eq!(scratch.dispose(&mut budget), Err(FsError::Changed));
    fixture.finish();
}

fn remove_planned_scratch(
    fixture: &Fixture,
    active: &Identity,
    files: &BTreeMap<String, Identity>,
) {
    let directory = fs::symlink_metadata(fixture.active()).unwrap();
    assert_eq!(
        (directory.dev(), directory.ino()),
        (active.device, active.inode)
    );
    let actual: BTreeSet<_> = fs::read_dir(fixture.active())
        .unwrap()
        .map(|v| v.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(actual, files.keys().cloned().collect());
    for (name, expected) in files {
        let path = fixture.active().join(name);
        let meta = fs::symlink_metadata(&path).unwrap();
        assert_eq!(
            (meta.dev(), meta.ino(), meta.uid(), meta.mode(), meta.len()),
            (
                expected.device,
                expected.inode,
                expected.owner,
                expected.mode,
                expected.size
            )
        );
        fs::remove_file(path).unwrap();
    }
    fs::remove_dir(fixture.active()).unwrap();
}

#[test]
fn backup_property_failure_stops_before_capture_and_keeps_owned_leaf() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let mut budget = budget();
    assert_eq!(
        native::exclude_from_backup(
            &fixture.root.join("missing"),
            &File::open(&fixture.root).unwrap()
        ),
        Err(FsError::BackupExclusion)
    );
    let mut scratch = fixture.scratch(&mut budget);
    let active = scratch.active_id.clone();
    native::clear_backup_exclusion_for_test(&fixture.caches.join("RadishLex/SchemaObservation-v1"))
        .unwrap();
    native::clear_backup_exclusion_for_test(&fixture.active()).unwrap();
    assert_eq!(scratch.capture(&mut budget), Err(FsError::BackupExclusion));
    assert!(names(scratch.active().unwrap()).unwrap().is_empty());
    assert_eq!(scratch.dispose(&mut budget), Err(FsError::BackupExclusion));
    remove_planned_scratch(&fixture, &active, &BTreeMap::new());
    fixture.finish();
}

#[test]
fn close_unlink_and_rmdir_failures_never_claim_disposal_success() {
    for operation in [
        posix::FsOperation::Close,
        posix::FsOperation::Unlink,
        posix::FsOperation::Rmdir,
    ] {
        let fixture = Fixture::new(FixtureKind::MainOnly);
        let mut budget = budget();
        let mut scratch = fixture.scratch(&mut budget);
        scratch.capture(&mut budget).unwrap();
        let active = scratch.active_id.clone();
        let files = scratch
            .copied
            .iter()
            .map(|(name, file)| (name.clone(), file.id.clone()))
            .collect();
        posix::inject_fault(operation, 0);
        assert!(scratch.dispose(&mut budget).is_err());
        posix::clear_fault();
        // The injected close executes the actual close successfully before
        // reporting the planned error. Only this fixture host may tear it down.
        let retained = if operation == posix::FsOperation::Rmdir {
            BTreeMap::new()
        } else {
            files
        };
        remove_planned_scratch(&fixture, &active, &retained);
        fixture.finish();
    }
}

#[test]
fn sqlite_close_uncertainty_retains_the_actual_owned_copy() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let mut budget = budget();
    let mut scratch = fixture.scratch(&mut budget);
    scratch.capture(&mut budget).unwrap();
    let active = scratch.active_id.clone();
    let files = scratch
        .copied
        .iter()
        .map(|(name, file)| (name.clone(), file.id.clone()))
        .collect();
    let result = super::super::dispose_observation(
        Err(ObserverError::CloseUnconfirmed.into()),
        scratch,
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(super::super::ObservationError::ScratchRetained { .. })
    ));
    remove_planned_scratch(&fixture, &active, &files);
    fixture.finish();
}

#[test]
fn capture_permission_rejection_keeps_evidence_and_reports_synthetic_context() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let mut budget = budget();
    let mut scratch = fixture.scratch(&mut budget);
    let database = fixture.source.join(FAMILY[0]);
    fs::set_permissions(&database, fs::Permissions::from_mode(0o400)).unwrap();
    let error = scratch.capture(&mut budget).unwrap_err();
    assert_eq!(error, FsError::Identity);
    assert!(scratch.held.is_empty());
    assert!(scratch.copied.is_empty());
    assert!(names(scratch.active().unwrap()).unwrap().is_empty());
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fixture.capture_failed(&scratch, error);
    }))
    .unwrap_err();
    let message = failure.downcast_ref::<String>().unwrap();
    for expected in [
        "synthetic capture rejected: Identity",
        "kind=MainOnly",
        "expected_family=",
        "post_failure_metadata=",
        fixture.root.to_str().unwrap(),
    ] {
        assert!(message.contains(expected), "missing diagnostic {expected}");
    }
    assert!(fixture.active().is_dir());
    let meta = fs::symlink_metadata(&database).unwrap();
    assert_eq!((meta.dev(), meta.ino()), fixture.source_files[FAMILY[0]]);
    assert_eq!(meta.mode() & 0o7777, 0o400);
    // Restore only this fixture's planned permission fault, after verifying its
    // original inode. Neither capture nor diagnostics performs this repair.
    fs::set_permissions(&database, fs::Permissions::from_mode(0o600)).unwrap();
    scratch.dispose(&mut budget).unwrap();
    fixture.finish();
}

#[test]
fn compiled_layout_rejects_same_version_unknown_schema_and_unsupported_version() {
    for (kind, expected) in [
        (FixtureKind::InvalidLayout, ObserverError::Layout),
        (
            FixtureKind::UnsupportedVersion,
            ObserverError::UnsupportedSchema,
        ),
    ] {
        let fixture = Fixture::new(kind);
        let mut budget = budget();
        let mut scratch = fixture.scratch(&mut budget);
        scratch
            .capture(&mut budget)
            .unwrap_or_else(|error| fixture.capture_failed(&scratch, error));
        scratch.mark_sqlite_started(&budget).unwrap();
        let path = scratch.database_path();
        let control = budget.control();
        let result = inspect_private_copy(
            &path,
            Arc::new(move || control.check_work().map_err(sql_error)),
            &mut |bytes| budget.charge_read(bytes).map_err(sql_error),
            &mut || Ok(()),
        );
        assert_eq!(result.unwrap_err(), expected);
        scratch.dispose(&mut budget).unwrap();
        fixture.finish();
    }
}

#[test]
fn concurrent_calls_have_one_exclusive_active_owner() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let mut initial = budget();
    fixture.scratch(&mut initial).dispose(&mut initial).unwrap();
    let start = Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let source = fixture.source.clone();
            let caches = fixture.caches.clone();
            let root = fixture.identity.clone();
            let family = fixture.family.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                let mut budget = budget();
                start.wait();
                Scratch::create_at(&caches, &source, &root, &family, &mut budget)
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|v| v.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|v| v.is_ok()).count(), 1);
    for result in results {
        match result {
            Ok(scratch) => scratch.dispose(&mut budget()).unwrap(),
            Err(error) => assert_eq!(error, FsError::ScratchOccupied),
        }
    }
    fixture.finish();
}

#[test]
fn crashed_copy_is_retained_and_a_new_process_cannot_adopt_it() {
    let fixture = Fixture::new(FixtureKind::MainOnly);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "source_schema::filesystem::tests::production_crash_worker",
            "--ignored",
            "--nocapture",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let input = serde_json::to_vec(&(
        fixture.root.clone(),
        fixture.identity.clone(),
        fixture.family.clone(),
    ))
    .unwrap();
    child.stdin.take().unwrap().write_all(&input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(73),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line = stdout
        .lines()
        .find_map(|line| line.strip_prefix("CREATED_SCHEMA_COPY="))
        .unwrap();
    let (active_node, database_node): ((u64, u64), (u64, u64)) =
        serde_json::from_str(line).unwrap();
    let mut budget = budget();
    assert!(matches!(
        Scratch::create_at(
            &fixture.caches,
            &fixture.source,
            &fixture.identity,
            &fixture.family,
            &mut budget
        ),
        Err(FsError::ScratchOccupied)
    ));
    assert!(fixture.active().join(FAMILY[0]).exists());
    // This test owns the child and all its exact reported creations. Product
    // reentry above performs no recovery; host cleanup is a planned test action.
    let directory = Chain::open(&fixture.active(), fixture.identity.owner_id, &budget).unwrap();
    let active = identity(directory.file().unwrap()).unwrap();
    let file = open_read(directory.file().unwrap(), FAMILY[0]).unwrap();
    let database = identity(&file).unwrap();
    close(file).unwrap();
    directory.close().unwrap();
    assert_eq!((active.device, active.inode), active_node);
    assert_eq!((database.device, database.inode), database_node);
    remove_planned_scratch(
        &fixture,
        &active,
        &BTreeMap::from([(FAMILY[0].into(), database)]),
    );
    fixture.finish();
}

#[test]
#[ignore = "owned child of crashed_copy_is_retained_and_a_new_process_cannot_adopt_it"]
fn production_crash_worker() {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(64 * 1024)
        .read_to_end(&mut bytes)
        .unwrap();
    let (root, identity, family): (PathBuf, PreparationDirectoryIdentity, PreparationFamily) =
        serde_json::from_slice(&bytes).unwrap();
    assert_eq!(root.parent(), Some(Path::new("/private/tmp")));
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("radishlex-production-schema-"));
    let mut budget = budget();
    let mut scratch = Scratch::create_at(
        &root.join("caches"),
        &root.join("source"),
        &identity,
        &family,
        &mut budget,
    )
    .unwrap();
    scratch.capture(&mut budget).unwrap();
    let database = &scratch.copied[FAMILY[0]].id;
    let nodes = (
        (scratch.active_id.device, scratch.active_id.inode),
        (database.device, database.inode),
    );
    println!(
        "\nCREATED_SCHEMA_COPY={}",
        serde_json::to_string(&nodes).unwrap()
    );
    std::io::stdout().flush().unwrap();
    // SAFETY: this deliberately terminates only the newly spawned test worker;
    // no destructors/cleanup run, so the next call must reject its residual copy.
    unsafe {
        libc::_exit(73);
    }
}
