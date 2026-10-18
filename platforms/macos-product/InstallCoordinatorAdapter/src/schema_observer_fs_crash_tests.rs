//! R04 predeclared child-exit fixture. Creation records arrive only through the
//! stdout pipe of this test's own executable, never from a disk recovery file.
//! Any unknown/changed object aborts teardown and leaves the fixed root occupied.
use super::super::schema_observer_budget::ObservationControl;
use super::*;
use radishlex_ime_userdb::schema_observer::{create_synthetic_fixture, FixtureKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CHILD_TEST: &str = "schema_observer_fs::crash_tests::planned_child_process_exit";
const CHILD_SELECTOR: &str = "RADISHLEX_SCHEMA_OBSERVER_PLANNED_CRASH_CHILD";
const SELECTOR_VALUE: &str = "synthetic-v1-r04";
const RECORD_PREFIX: &str = "RADISHLEX_PLANNED_CRASH_V1:";
const EXIT_CODE: i32 = 77;
const OUTPUT_LIMIT: usize = 16 * 1024;
const PATHS: [(&str, bool); 11] = [
    ("", true),
    ("batch.json", false),
    ("case-0001", true),
    ("case-0001/case.json", false),
    ("case-0001/source", true),
    ("case-0001/source/userdb.sqlite3", false),
    ("case-0001/source/userdb.sqlite3-wal", false),
    ("case-0001/source/userdb.sqlite3-shm", false),
    ("case-0001/scratch", true),
    ("case-0001/scratch/userdb.sqlite3", false),
    ("case-0001/scratch/userdb.sqlite3-wal", false),
];

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedIdentity {
    device: u64,
    inode: u64,
    owner: u32,
    mode: u32,
    links: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl From<&Identity> for RecordedIdentity {
    fn from(id: &Identity) -> Self {
        Self {
            device: id.device,
            inode: id.inode,
            owner: id.owner,
            mode: id.mode,
            links: id.links,
            size: id.size,
            modified: id.modified,
            changed: id.changed,
        }
    }
}
impl RecordedIdentity {
    fn identity(&self) -> Identity {
        Identity {
            device: self.device,
            inode: self.inode,
            owner: self.owner,
            mode: self.mode,
            links: self.links,
            size: self.size,
            modified: self.modified,
            changed: self.changed,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatedNode {
    relative: String,
    directory: bool,
    identity: RecordedIdentity,
    sha256: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChildCreation {
    version: u8,
    user: u32,
    run: [u8; 16],
    nodes: Vec<CreatedNode>,
}

fn budget() -> ObserverBudget {
    ObserverBudget::new(
        ObservationControl::enter(Instant::now() + Duration::from_secs(120)).unwrap(),
    )
}
fn user() -> Result<u32, FsError> {
    // SAFETY: UID queries have no memory or ownership preconditions.
    let (real, effective) = unsafe { (libc::getuid(), libc::geteuid()) };
    if effective == 0 || real != effective {
        return Err(FsError::Identity);
    }
    Ok(effective)
}
fn parent_and_leaf(relative: &str) -> (&str, &str) {
    relative.rsplit_once('/').unwrap_or(("", relative))
}
fn add_directory(
    nodes: &mut Vec<CreatedNode>,
    path: &str,
    directory: &Directory,
    user: u32,
) -> Result<(), FsError> {
    let id = validate(directory.file()?, user, directory.id.device, true)?;
    if !id.same_node(&directory.id) {
        return Err(FsError::Changed);
    }
    nodes.push(CreatedNode {
        relative: path.to_owned(),
        directory: true,
        identity: (&id).into(),
        sha256: None,
    });
    Ok(())
}
fn file_hash(
    parent: &File,
    name: &str,
    expected: &Identity,
    user: u32,
    budget: &mut ObserverBudget,
) -> Result<String, FsError> {
    let mut file = posix::open_read(parent, name)?;
    if validate(&file, user, expected.device, false)? != *expected {
        return Err(FsError::Changed);
    }
    let mut buffer = [0u8; BLOCK_SIZE];
    let mut remaining = expected.size;
    let mut hash = Sha256::new();
    while remaining != 0 {
        let wanted = remaining.min(BLOCK_SIZE as u64) as usize;
        budget.charge_read(wanted as u64)?;
        let count = file.read(&mut buffer[..wanted]).map_err(|_| FsError::Io)?;
        if count == 0 {
            return Err(FsError::Changed);
        }
        hash.update(&buffer[..count]);
        remaining -= count as u64;
    }
    budget.charge_read(1)?;
    let extra = file.read(&mut buffer[..1]).map_err(|_| FsError::Io)?;
    let after = identity(&file)?;
    close(file)?;
    if extra != 0 || after != *expected || identity_at(parent, name)?.as_ref() != Some(expected) {
        return Err(FsError::Changed);
    }
    Ok(hex(&hash.finalize()))
}
fn add_file(
    nodes: &mut Vec<CreatedNode>,
    relative: &str,
    parent: &File,
    name: &str,
    id: &Identity,
    user: u32,
    budget: &mut ObserverBudget,
) -> Result<(), FsError> {
    let digest = file_hash(parent, name, id, user, budget)?;
    nodes.push(CreatedNode {
        relative: relative.to_owned(),
        directory: false,
        identity: id.into(),
        sha256: Some(digest),
    });
    Ok(())
}

#[test]
#[ignore = "only the bounded parent crash test launches this child selector"]
fn planned_child_process_exit() {
    // A general --ignored run must not terminate the user's test process.
    if std::env::var(CHILD_SELECTOR).as_deref() != Ok(SELECTOR_VALUE) {
        return;
    }
    let mut batch = SyntheticBatch::create().expect("exclusive child synthetic root");
    let mut case = batch.begin_case().expect("new child case");
    let _fixture = create_synthetic_fixture(case.source_path(), FixtureKind::WithShm)
        .expect("child synthetic fixture");
    let mut accounting = budget();
    let family = case
        .seal_source(&mut accounting)
        .expect("seal child source");
    assert!(family.wal.is_some() && family.shm.is_some());
    let copy = case
        .capture(&family, &mut accounting)
        .expect("capture child source");
    copy.verify_copy(&mut accounting).expect("child copy bytes");
    let copied = copy
        .close_for_planned_fault()
        .expect("explicitly close held source files");
    case.check().expect("child live directory chain");
    case.check_source_inventory()
        .expect("child closed source tree");
    case.check_retained_budget(None)
        .expect("child retained size");
    expect_names(batch.root.file().unwrap(), &["batch.json", "case-0001"]).unwrap();
    expect_names(
        case.root.file().unwrap(),
        &["case.json", "source", "scratch"],
    )
    .unwrap();
    expect_names(case.source.file().unwrap(), &FAMILY[..3]).unwrap();
    let scratch = case.scratch.as_ref().unwrap();
    expect_names(scratch.file().unwrap(), &FAMILY[..2]).unwrap();
    let mut nodes = Vec::new();
    add_directory(&mut nodes, "", &batch.root, batch.user).unwrap();
    add_file(
        &mut nodes,
        "batch.json",
        batch.root.file().unwrap(),
        "batch.json",
        &batch.record,
        batch.user,
        &mut accounting,
    )
    .unwrap();
    add_directory(&mut nodes, "case-0001", &case.root, batch.user).unwrap();
    add_file(
        &mut nodes,
        "case-0001/case.json",
        case.root.file().unwrap(),
        "case.json",
        &case.record,
        batch.user,
        &mut accounting,
    )
    .unwrap();
    add_directory(&mut nodes, "case-0001/source", &case.source, batch.user).unwrap();
    for (index, component) in FAMILY[..3].iter().enumerate() {
        let id = case.sealed_ids.as_ref().unwrap()[index].as_ref().unwrap();
        add_file(
            &mut nodes,
            &format!("case-0001/source/{component}"),
            case.source.file().unwrap(),
            component,
            id,
            batch.user,
            &mut accounting,
        )
        .unwrap();
    }
    add_directory(&mut nodes, "case-0001/scratch", scratch, batch.user).unwrap();
    for component in &FAMILY[..2] {
        add_file(
            &mut nodes,
            &format!("case-0001/scratch/{component}"),
            scratch.file().unwrap(),
            component,
            copied.get(*component).unwrap(),
            batch.user,
            &mut accounting,
        )
        .unwrap();
    }
    let record = ChildCreation {
        version: 1,
        user: batch.user,
        run: batch.run,
        nodes,
    };
    let encoded = serde_json::to_vec(&record).expect("bounded child creation record");
    assert!(encoded.len() + RECORD_PREFIX.len() + 2 < OUTPUT_LIMIT);
    let mut output = std::io::stdout().lock();
    output.write_all(b"\n").unwrap();
    output.write_all(RECORD_PREFIX.as_bytes()).unwrap();
    output.write_all(&encoded).unwrap();
    output.write_all(b"\n").unwrap();
    output.flush().unwrap();
    // This is the only deliberate process termination: this test's own fresh
    // child exits after delivering its creation record. No Drop/cleanup runs.
    std::process::exit(EXIT_CODE);
}

#[test]
fn planned_child_exit_retains_fixed_root_and_exact_fixture_host_teardown() {
    let current_user = user().expect("unprivileged test host");
    let anchors = Ancestors::open().expect("literal private tmp ancestry");
    let root_name = format!("radishlex-schema-observer-synthetic-v1-{current_user}");
    assert!(
        identity_at(&anchors.temporary, &root_name)
            .unwrap()
            .is_none(),
        "crash test refuses any preexisting object"
    );
    let executable = std::env::current_exe().expect("own test executable");
    let mut child = Command::new(executable)
        .args([
            "--ignored",
            "--exact",
            CHILD_TEST,
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_SELECTOR, SELECTOR_VALUE)
        .env("RUST_BACKTRACE", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start only this test's own child");
    let pipe = child.stdout.take().expect("owned child stdout pipe");
    let mut bounded = pipe.take((OUTPUT_LIMIT + 1) as u64);
    let mut output = Vec::with_capacity(OUTPUT_LIMIT + 1);
    bounded
        .read_to_end(&mut output)
        .expect("bounded child creation record");
    drop(bounded);
    let status = child.wait().expect("wait for own planned exit");
    assert_eq!(
        status.code(),
        Some(EXIT_CODE),
        "child must reach the declared fault point"
    );
    assert!(
        output.len() <= OUTPUT_LIMIT,
        "child output must remain bounded"
    );
    let text = std::str::from_utf8(&output).expect("fixed child stdout encoding");
    let records = text
        .lines()
        .filter_map(|line| line.strip_prefix(RECORD_PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let creation: ChildCreation =
        serde_json::from_str(records[0]).expect("direct child creation record");
    assert_eq!(creation.version, 1);
    assert_eq!(creation.user, current_user);
    assert!(
        matches!(SyntheticBatch::create(), Err(FsError::ScratchOccupied)),
        "crash residual cannot be adopted"
    );
    verify_this_child_creation_and_teardown(creation, anchors, current_user)
        .expect("only exact declared child fixture is disposed");
}

/// Only called with this invocation's pipe record after checking its own child's
/// fixed exit. There is no read-from-disk, arbitrary path, or startup entry point.
fn verify_this_child_creation_and_teardown(
    creation: ChildCreation,
    anchors: Ancestors,
    user: u32,
) -> Result<(), FsError> {
    if creation.nodes.len() != PATHS.len() {
        return Err(FsError::UnknownEntry);
    }
    let mut expected = BTreeMap::new();
    for node in creation.nodes {
        if !PATHS.contains(&(node.relative.as_str(), node.directory))
            || expected.insert(node.relative.clone(), node).is_some()
        {
            return Err(FsError::UnknownEntry);
        }
    }
    if expected.len() != PATHS.len() {
        return Err(FsError::UnknownEntry);
    }
    let root_name = format!("radishlex-schema-observer-synthetic-v1-{user}");
    anchors.check()?;
    let root_file = open_dir(&anchors.temporary, &root_name)?;
    let mut directories = BTreeMap::from([(String::new(), root_file)]);
    for (path, directory) in PATHS {
        if !directory {
            continue;
        }
        if !path.is_empty() {
            let (parent, leaf) = parent_and_leaf(path);
            let file = open_dir(directories.get(parent).ok_or(FsError::Missing)?, leaf)?;
            directories.insert(path.to_owned(), file);
        }
        let node = expected.get(path).ok_or(FsError::Missing)?;
        if node.sha256.is_some() {
            return Err(FsError::Identity);
        }
        let file = directories.get(path).ok_or(FsError::Missing)?;
        if validate(file, user, anchors.ids[2].device, true)? != node.identity.identity() {
            return Err(FsError::Changed);
        }
    }
    let mut remaining = expected.keys().cloned().collect::<BTreeSet<_>>();
    check_current_creation_tree(
        &anchors,
        &root_name,
        &directories,
        &expected,
        &remaining,
        user,
    )?;
    let mut accounting = budget();
    let mut source_size = 0u64;
    let mut scratch_size = 0u64;
    for (path, node) in &expected {
        if node.directory {
            continue;
        }
        let digest = node.sha256.as_ref().ok_or(FsError::Identity)?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(FsError::Identity);
        }
        let id = node.identity.identity();
        let (parent, leaf) = parent_and_leaf(path);
        let limit = if leaf.ends_with("-shm") {
            8 * MIB
        } else if leaf.ends_with("-wal") {
            128 * MIB
        } else if leaf.ends_with(".json") {
            16 * 1024
        } else {
            256 * MIB
        };
        if id.size > limit {
            return Err(FsError::Capacity);
        }
        if file_hash(
            directories.get(parent).ok_or(FsError::Missing)?,
            leaf,
            &id,
            user,
            &mut accounting,
        )? != *digest
        {
            return Err(FsError::Changed);
        }
        if parent.ends_with("/source") {
            source_size = source_size.checked_add(id.size).ok_or(FsError::Capacity)?;
        }
        if parent.ends_with("/scratch") {
            scratch_size = scratch_size.checked_add(id.size).ok_or(FsError::Capacity)?;
        }
    }
    check_retained_totals(source_size, 0, scratch_size)?;
    if source_size > 256 * MIB {
        return Err(FsError::Capacity);
    }
    // Every full identity/hash and closed directory inventory passed before the
    // first mutation. Subsequent checks account for precisely our own removals.
    for (path, node) in &expected {
        if node.directory {
            continue;
        }
        check_current_creation_tree(
            &anchors,
            &root_name,
            &directories,
            &expected,
            &remaining,
            user,
        )?;
        let (parent, leaf) = parent_and_leaf(path);
        remove_file(
            directories.get(parent).ok_or(FsError::Missing)?,
            leaf,
            &node.identity.identity(),
            user,
        )?;
        remaining.remove(path);
    }
    let mut paths = directories
        .keys()
        .filter(|path| !path.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for path in paths {
        check_current_creation_tree(
            &anchors,
            &root_name,
            &directories,
            &expected,
            &remaining,
            user,
        )?;
        let (parent, leaf) = parent_and_leaf(&path);
        let directory = directories.remove(&path).ok_or(FsError::Missing)?;
        expect_names(&directory, &[])?;
        close(directory)?;
        posix::unlink(directories.get(parent).ok_or(FsError::Missing)?, leaf, true)?;
        remaining.remove(&path);
    }
    check_current_creation_tree(
        &anchors,
        &root_name,
        &directories,
        &expected,
        &remaining,
        user,
    )?;
    let root = directories.remove("").ok_or(FsError::Missing)?;
    expect_names(&root, &[])?;
    close(root)?;
    posix::unlink(&anchors.temporary, &root_name, true)?;
    close(anchors.temporary)?;
    close(anchors.private)?;
    close(anchors.root)?;
    Ok(())
}

fn check_current_creation_tree(
    anchors: &Ancestors,
    root_name: &str,
    directories: &BTreeMap<String, File>,
    expected: &BTreeMap<String, CreatedNode>,
    remaining: &BTreeSet<String>,
    user: u32,
) -> Result<(), FsError> {
    anchors.check()?;
    for (path, file) in directories {
        let node = expected.get(path).ok_or(FsError::Missing)?;
        let actual = validate(file, user, anchors.ids[2].device, true)?;
        if !actual.same_node(&node.identity.identity()) {
            return Err(FsError::Changed);
        }
        let (parent, leaf) = parent_and_leaf(path);
        let binding = if path.is_empty() {
            identity_at(&anchors.temporary, root_name)?
        } else {
            identity_at(directories.get(parent).ok_or(FsError::Missing)?, leaf)?
        };
        if binding.as_ref() != Some(&actual) {
            return Err(FsError::Changed);
        }
        let children = remaining
            .iter()
            .filter(|child| !child.is_empty() && parent_and_leaf(child).0 == path)
            .map(|child| parent_and_leaf(child).1.to_owned())
            .collect::<BTreeSet<_>>();
        if names(file)? != children {
            return Err(FsError::UnknownEntry);
        }
    }
    Ok(())
}
