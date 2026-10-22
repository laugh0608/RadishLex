//! Test-only synthetic directory capabilities. There is no arbitrary source-path
//! constructor and no production observer/qualification constructor.
#![allow(unsafe_code)]

#[path = "schema_observer_fs_capture.rs"]
mod capture;
#[path = "schema_observer_fs_crash_tests.rs"]
mod crash_tests;
#[path = "schema_observer_fs_posix.rs"]
mod posix;
#[path = "schema_observer_fs_tests.rs"]
mod tests;
use super::schema_observer_budget::{
    BudgetError, ObserverBudget, Phase, BLOCK_SIZE, MAX_BATCH, MAX_SCRATCH, MIB, MIN_FREE,
};
pub(crate) use capture::CapturedCopy;
pub(crate) use posix::{clear_fault, inject_fault, FsOperation};
use posix::{close, identity, identity_at, names, open_dir, reject_acl, validate, Identity};
use radishlex_ime_product_upgrade::PreparationFamily;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
pub(crate) use tests::{check_owned_negative_objects, check_planned_drift, PlannedDrift};

const FAMILY: [&str; 4] = [
    "userdb.sqlite3",
    "userdb.sqlite3-wal",
    "userdb.sqlite3-shm",
    "userdb.sqlite3-journal",
];
const CHUNK: usize = BLOCK_SIZE;
const CONTROL_RECORD_RESERVE: u64 = 2 * 16 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FsError {
    ScratchOccupied,
    Identity,
    Changed,
    Missing,
    UnknownEntry,
    Io,
    Close,
    Unlink,
    Capacity,
    BatchStopped,
    ScratchRetained,
    Retained(Box<FsError>),
    Budget(BudgetError),
}
impl From<BudgetError> for FsError {
    fn from(value: BudgetError) -> Self {
        Self::Budget(value)
    }
}

struct Directory {
    file: Option<File>,
    id: Identity,
    path: PathBuf,
    close_confirmed: bool,
}
impl Directory {
    fn file(&self) -> Result<&File, FsError> {
        self.file.as_ref().ok_or(FsError::Close)
    }
    fn new(file: File, user: u32, device: u64, path: PathBuf) -> Result<Self, FsError> {
        let id = validate(&file, user, device, true)?;
        Ok(Self {
            file: Some(file),
            id,
            path,
            close_confirmed: false,
        })
    }
    fn check(&self, parent: &File, component: &str, user: u32) -> Result<(), FsError> {
        let current = validate(self.file()?, user, self.id.device, true)?;
        if !current.same_node(&self.id)
            || identity_at(parent, component)?.as_ref() != Some(&current)
        {
            return Err(FsError::Changed);
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), FsError> {
        let result = close(self.file.take().ok_or(FsError::Close)?);
        self.close_confirmed = result.is_ok();
        result
    }
}
struct Ancestors {
    root: File,
    private: File,
    temporary: File,
    ids: [Identity; 3],
}
impl Ancestors {
    fn open() -> Result<Self, FsError> {
        let root = posix::open_slash()?;
        let private = open_dir(&root, "private")?;
        let temporary = open_dir(&private, "tmp")?;
        let ids = [identity(&root)?, identity(&private)?, identity(&temporary)?];
        for (index, (file, id)) in [&root, &private, &temporary]
            .into_iter()
            .zip(&ids)
            .enumerate()
        {
            validate_public_ancestor_identity(id, index)?;
            reject_acl(file)?;
        }
        let value = Self {
            root,
            private,
            temporary,
            ids,
        };
        value.check()?;
        Ok(value)
    }
    fn check(&self) -> Result<(), FsError> {
        for (file, expected) in [&self.root, &self.private, &self.temporary]
            .into_iter()
            .zip(&self.ids)
        {
            // Public directory timestamps legitimately change as unrelated tmp
            // entries change. Their descriptor/node/permissions remain fixed.
            if !identity(file)?.same_node(expected) {
                return Err(FsError::Changed);
            }
            reject_acl(file)?;
        }
        if !identity_at(&self.root, "private")?.is_some_and(|id| id.same_node(&self.ids[1]))
            || !identity_at(&self.private, "tmp")?.is_some_and(|id| id.same_node(&self.ids[2]))
        {
            return Err(FsError::Changed);
        }
        // Reopening the literal root binds the descriptor chain to the real /
        // rather than merely accepting a detached directory descriptor.
        let slash = posix::open_slash()?;
        let matches = identity(&slash)?.same_node(&self.ids[0]);
        close(slash)?;
        if !matches {
            return Err(FsError::Changed);
        }
        Ok(())
    }
}

fn validate_public_ancestor_identity(id: &Identity, index: usize) -> Result<(), FsError> {
    if index > 2
        || !id.directory()
        || id.owner != 0
        || id.mode & 0o7000 != if index == 2 { 0o1000 } else { 0 }
        || (index != 2 && id.mode & 0o022 != 0)
        || (index == 2 && id.mode & 0o777 != 0o777)
    {
        return Err(FsError::Identity);
    }
    Ok(())
}

pub(crate) struct SyntheticBatch {
    ancestors: Rc<Ancestors>,
    root: Directory,
    root_name: String,
    user: u32,
    record: Identity,
    run: [u8; 16],
    next: u16,
    active: Rc<Cell<bool>>,
    stopped: Rc<Cell<bool>>,
}
impl SyntheticBatch {
    pub(crate) fn create() -> Result<Self, FsError> {
        // SAFETY: UID queries have no memory or ownership preconditions.
        let (real, user) = unsafe { (libc::getuid(), libc::geteuid()) };
        if user == 0 || real != user {
            return Err(FsError::Identity);
        }
        let ancestors = Ancestors::open()?;
        posix::free_space(&ancestors.temporary)?;
        let root_name = format!("radishlex-schema-observer-synthetic-v1-{user}");
        let root_file = posix::create_dir(&ancestors.temporary, &root_name)?;
        let root = Directory::new(
            root_file,
            user,
            ancestors.ids[2].device,
            PathBuf::from("/private/tmp").join(&root_name),
        )?;
        root.check(&ancestors.temporary, &root_name, user)?;
        let mut run = [0; 16];
        let mut random = File::open("/dev/urandom").map_err(|_| FsError::Io)?;
        let read = random.read_exact(&mut run).map_err(|_| FsError::Io);
        close(random)?;
        read?;
        let record = write_record(
            root.file()?,
            "batch.json",
            user,
            root.id.device,
            &format!(
                "{{\"format\":\"schema-observer-synthetic-v1\",\"run\":\"{}\"}}\n",
                hex(&run)
            ),
        )?;
        Ok(Self {
            ancestors: Rc::new(ancestors),
            root,
            root_name,
            user,
            record,
            run,
            next: 1,
            active: Rc::new(Cell::new(false)),
            stopped: Rc::new(Cell::new(false)),
        })
    }
    pub(crate) fn begin_case(&mut self) -> Result<SyntheticCase, FsError> {
        if self.stopped.get() || self.active.get() || self.next > 64 {
            return Err(FsError::BatchStopped);
        }
        self.check()?;
        posix::free_space(self.root.file()?)?;
        let component = format!("case-{:04}", self.next);
        self.next += 1;
        // Set before the first mutation: any partial creation blocks the batch.
        self.active.set(true);
        let root = Directory::new(
            posix::create_dir(self.root.file()?, &component)?,
            self.user,
            self.root.id.device,
            self.root.path.join(&component),
        )?;
        let source = Directory::new(
            posix::create_dir(root.file()?, "source")?,
            self.user,
            self.root.id.device,
            root.path.join("source"),
        )?;
        let scratch = Directory::new(
            posix::create_dir(root.file()?, "scratch")?,
            self.user,
            self.root.id.device,
            root.path.join("scratch"),
        )?;
        let record = write_record(
            root.file()?,
            "case.json",
            self.user,
            self.root.id.device,
            &format!(
                "{{\"format\":\"schema-observer-synthetic-v1\",\"run\":\"{}\",\"case\":{}}}\n",
                hex(&self.run),
                self.next - 1
            ),
        )?;
        Ok(SyntheticCase {
            root,
            source,
            scratch: Some(scratch),
            parent: open_fixed_dot(self.root.file()?)?,
            anchors: self.ancestors.clone(),
            batch_name: self.root_name.clone(),
            parent_id: self.root.id.clone(),
            component,
            user: self.user,
            record,
            scratch_owned: RefCell::new(BTreeMap::new()),
            baseline: None,
            sealed_ids: None,
            generated_hashes: None,
            registered: BTreeMap::new(),
            active: self.active.clone(),
            stopped: self.stopped.clone(),
        })
    }
    fn check(&self) -> Result<(), FsError> {
        self.ancestors.check()?;
        self.root
            .check(&self.ancestors.temporary, &self.root_name, self.user)
    }
    pub(crate) fn finish(mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        budget.begin_cleanup()?;
        if self.active.get() {
            return Err(FsError::BatchStopped);
        }
        budget.checkpoint(Phase::Cleanup)?;
        self.check()?;
        expect_names(self.root.file()?, &["batch.json"])?;
        remove_file(self.root.file()?, "batch.json", &self.record, self.user)?;
        self.check()?;
        self.root.close()?;
        posix::unlink(&self.ancestors.temporary, &self.root_name, true)?;
        let anchors = Rc::try_unwrap(self.ancestors).map_err(|_| FsError::BatchStopped)?;
        close(anchors.temporary)?;
        close(anchors.private)?;
        close(anchors.root)?;
        Ok(())
    }
}

pub(crate) struct SyntheticCase {
    root: Directory,
    source: Directory,
    scratch: Option<Directory>,
    parent: File,
    anchors: Rc<Ancestors>,
    batch_name: String,
    parent_id: Identity,
    component: String,
    user: u32,
    record: Identity,
    scratch_owned: RefCell<BTreeMap<String, Identity>>,
    baseline: Option<PreparationFamily>,
    sealed_ids: Option<[Option<Identity>; 4]>,
    generated_hashes: Option<[Option<String>; 4]>,
    registered: BTreeMap<PathBuf, Identity>,
    active: Rc<Cell<bool>>,
    stopped: Rc<Cell<bool>>,
}
impl SyntheticCase {
    pub(crate) fn source_path(&self) -> &Path {
        &self.source.path
    }
    /// Register only the four synthetic-generator output names before any
    /// observer validation. Missing DB/journal remain invalid observer inputs,
    /// while their fixture-host teardown retains exact provenance.
    pub(crate) fn register_generated_source(
        &mut self,
        budget: &mut ObserverBudget,
    ) -> Result<(), FsError> {
        self.check()?;
        if self.sealed_ids.is_some() {
            return Err(FsError::Changed);
        }
        let (ids, hashes) = capture::generated_source(
            self.source.file()?,
            self.user,
            self.source.id.device,
            budget,
        )?;
        let expected = FAMILY
            .into_iter()
            .zip(&ids)
            .filter_map(|(name, id)| id.as_ref().map(|_| name.to_owned()))
            .collect::<BTreeSet<_>>();
        if names(self.source.file()?)? != expected {
            return Err(FsError::UnknownEntry);
        }
        self.sealed_ids = Some(ids);
        self.generated_hashes = Some(hashes);
        Ok(())
    }
    pub(crate) fn dispose_empty_scratch(
        &mut self,
        budget: &mut ObserverBudget,
    ) -> Result<(), FsError> {
        budget.begin_cleanup()?;
        self.check()?;
        let scratch = self.scratch.as_mut().ok_or(FsError::ScratchRetained)?;
        if !self.scratch_owned.borrow().is_empty() || !names(scratch.file()?)?.is_empty() {
            return Err(FsError::UnknownEntry);
        }
        scratch.check(self.root.file()?, "scratch", self.user)?;
        scratch.close()?;
        posix::unlink(self.root.file()?, "scratch", true)?;
        self.scratch = None;
        Ok(())
    }
    pub(crate) fn seal_source(
        &mut self,
        budget: &mut ObserverBudget,
    ) -> Result<PreparationFamily, FsError> {
        self.check()?;
        if self.sealed_ids.is_none() {
            self.register_generated_source(budget)?;
        }
        let initial = capture::family_identities(self.source.file()?)?;
        let family = capture::read_family(
            self.source.file()?,
            self.user,
            self.source.id.device,
            budget,
        )?;
        if self.baseline.is_some() {
            return Err(FsError::Changed);
        }
        let observed = capture::family_identities(self.source.file()?)?;
        if initial != observed {
            return Err(FsError::Changed);
        }
        if self.sealed_ids.as_ref() != Some(&observed) {
            return Err(FsError::Changed);
        }
        self.baseline = Some(family.clone());
        Ok(family)
    }
    /// Explicit fixture-host creation inventory, never an ownership sweep.
    pub(crate) fn register_fixture_paths(&mut self, paths: &[PathBuf]) -> Result<(), FsError> {
        if self
            .registered
            .len()
            .checked_add(paths.len())
            .ok_or(FsError::Identity)?
            > 128
        {
            return Err(FsError::Identity);
        }
        self.check()?;
        let mut ordered = paths.to_vec();
        ordered.sort_by_key(|path| path.components().count());
        for path in ordered {
            let components = checked_components(&path)?;
            if components.len() == 1 && FAMILY.contains(&components[0].as_str()) {
                return Err(FsError::Identity);
            }
            let (parent, leaf) = self.fixture_parent(&path)?;
            let id = identity_at(&parent, &leaf)?.ok_or(FsError::Missing)?;
            let file = if id.directory() {
                open_dir(&parent, &leaf)?
            } else {
                posix::open_read(&parent, &leaf)?
            };
            let observed = validate(&file, self.user, self.source.id.device, id.directory())?;
            close(file)?;
            close(parent)?;
            if observed != id || self.registered.insert(path, id).is_some() {
                return Err(FsError::Changed);
            }
        }
        Ok(())
    }
    fn fixture_parent(&self, path: &Path) -> Result<(File, String), FsError> {
        let mut components = checked_components(path)?;
        let leaf = components.pop().ok_or(FsError::Identity)?;
        let mut file = open_fixed_dot(self.source.file()?)?;
        let mut relative = PathBuf::new();
        for component in components {
            relative.push(&component);
            let expected = self
                .registered
                .get(&relative)
                .ok_or(FsError::UnknownEntry)?;
            let next = open_dir(&file, &component)?;
            let id = validate(&next, self.user, self.source.id.device, true)?;
            if !id.same_node(expected)
                || !identity_at(&file, &component)?.is_some_and(|entry| entry.same_node(expected))
            {
                return Err(FsError::Changed);
            }
            close(file)?;
            file = next;
        }
        Ok((file, leaf))
    }
    fn check(&self) -> Result<(), FsError> {
        self.anchors.check()?;
        if !identity_at(&self.anchors.temporary, &self.batch_name)?
            .is_some_and(|id| id.same_node(&self.parent_id))
        {
            return Err(FsError::Changed);
        }
        if !identity(&self.parent)?.same_node(&self.parent_id) {
            return Err(FsError::Changed);
        }
        self.root.check(&self.parent, &self.component, self.user)?;
        self.source.check(self.root.file()?, "source", self.user)?;
        if let Some(scratch) = &self.scratch {
            scratch.check(self.root.file()?, "scratch", self.user)?;
        }
        Ok(())
    }
    /// Logical retention accounting, independently of source-family admission.
    /// Every registered core file contributes its actual current length. Reserve
    /// both control-record maxima even though these records are much smaller.
    fn check_retained_budget(&self, projected_scratch: Option<u64>) -> Result<(), FsError> {
        let mut source = 0u64;
        for component in FAMILY {
            let Some(expected) = identity_at(self.source.file()?, component)? else {
                continue;
            };
            let file = posix::open_read(self.source.file()?, component)?;
            let observed = validate(&file, self.user, self.source.id.device, false)?;
            close(file)?;
            if observed != expected {
                return Err(FsError::Changed);
            }
            source = source.checked_add(observed.size).ok_or(FsError::Capacity)?;
        }
        let mut material = 0u64;
        for (path, expected) in &self.registered {
            if expected.directory() {
                continue;
            }
            let (parent, leaf) = self.fixture_parent(path)?;
            let file = posix::open_read(&parent, &leaf)?;
            let observed = validate(&file, self.user, self.source.id.device, false)?;
            close(file)?;
            close(parent)?;
            if observed != *expected {
                return Err(FsError::Changed);
            }
            material = material
                .checked_add(observed.size)
                .ok_or(FsError::Capacity)?;
        }
        let mut scratch_size = 0u64;
        if let Some(scratch) = &self.scratch {
            for component in names(scratch.file()?)? {
                if !FAMILY[..3].contains(&component.as_str()) {
                    return Err(FsError::UnknownEntry);
                }
                let expected = identity_at(scratch.file()?, &component)?.ok_or(FsError::Changed)?;
                let file = posix::open_read(scratch.file()?, &component)?;
                let observed = validate(&file, self.user, scratch.id.device, false)?;
                close(file)?;
                if observed != expected {
                    return Err(FsError::Changed);
                }
                if component == FAMILY[2] && observed.size > 8 * MIB {
                    return Err(FsError::Capacity);
                }
                scratch_size = scratch_size
                    .checked_add(observed.size)
                    .ok_or(FsError::Capacity)?;
            }
        }
        check_retained_totals(
            source,
            material,
            scratch_size.max(projected_scratch.unwrap_or(0)),
        )
    }
    fn check_source_inventory(&self) -> Result<(), FsError> {
        let ids = self.sealed_ids.as_ref().ok_or(FsError::Identity)?;
        let mut roots = FAMILY
            .into_iter()
            .zip(ids)
            .filter_map(|(name, id)| id.as_ref().map(|_| name.to_owned()))
            .collect::<BTreeSet<_>>();
        for path in self.registered.keys() {
            if path.components().count() == 1 {
                roots.insert(path.to_string_lossy().into_owned());
            }
        }
        if names(self.source.file()?)? != roots {
            return Err(FsError::UnknownEntry);
        }
        for (path, expected) in &self.registered {
            let (parent, leaf) = self.fixture_parent(path)?;
            let file = if expected.directory() {
                open_dir(&parent, &leaf)?
            } else {
                posix::open_read(&parent, &leaf)?
            };
            let observed = validate(
                &file,
                self.user,
                self.source.id.device,
                expected.directory(),
            )?;
            if if expected.directory() {
                !observed.same_node(expected)
            } else {
                observed != *expected
            } {
                return Err(FsError::Changed);
            }
            if expected.directory() {
                let children = self
                    .registered
                    .keys()
                    .filter(|child| child.parent() == Some(path.as_path()))
                    .map(|child| child.file_name().unwrap().to_string_lossy().into_owned())
                    .collect::<BTreeSet<_>>();
                if names(&file)? != children {
                    return Err(FsError::UnknownEntry);
                }
            }
            close(file)?;
            close(parent)?;
        }
        Ok(())
    }
    pub(crate) fn capture<'a>(
        &'a mut self,
        expected: &PreparationFamily,
        budget: &mut ObserverBudget,
    ) -> Result<CapturedCopy<'a>, FsError> {
        capture::capture(self, expected, budget)
    }
    /// Fixture-host teardown for a deliberately scheduled failure only. This
    /// never adopts entries: only identities registered during successful
    /// capture/explicit SQLite close verification can be removed. The batch
    /// remains stopped even after this fixture teardown succeeds. A simulated
    /// SQLite-close failure must first finish real close and call verify_copy;
    /// a pre-close SHM timestamp/length snapshot is not a disposal identity.
    /// This helper never refreshes stale SHM identities or treats an uncertain
    /// real close as permission to delete.
    pub(crate) fn teardown_planned_retained_scratch(
        &mut self,
        budget: &mut ObserverBudget,
    ) -> Result<(), FsError> {
        budget.begin_cleanup()?;
        // A scheduled rmdir failure can leave an explicitly closed directory.
        // Reopen only that recorded inode, and only if close returned success.
        // Unknown/failed close state is never treated as confirmed disposal.
        if self
            .scratch
            .as_ref()
            .is_some_and(|scratch| scratch.file.is_none())
        {
            self.anchors.check()?;
            if !identity_at(&self.anchors.temporary, &self.batch_name)?
                .is_some_and(|id| id.same_node(&self.parent_id))
            {
                return Err(FsError::Changed);
            }
            self.root.check(&self.parent, &self.component, self.user)?;
            self.source.check(self.root.file()?, "source", self.user)?;
            let scratch = self.scratch.as_mut().ok_or(FsError::ScratchRetained)?;
            if !scratch.close_confirmed {
                return Err(FsError::Close);
            }
            let expected = identity_at(self.root.file()?, "scratch")?.ok_or(FsError::Changed)?;
            if !expected.same_node(&scratch.id) {
                return Err(FsError::Changed);
            }
            let reopened = open_dir(self.root.file()?, "scratch")?;
            let actual = validate(&reopened, self.user, scratch.id.device, true)?;
            if actual != expected {
                close(reopened)?;
                return Err(FsError::Changed);
            }
            scratch.file = Some(reopened);
            scratch.close_confirmed = false;
        }
        self.check()?;
        self.check_retained_budget(None)?;
        let scratch = self.scratch.as_mut().ok_or(FsError::ScratchRetained)?;
        let owned = self.scratch_owned.borrow().clone();
        if names(scratch.file()?)? != owned.keys().cloned().collect() {
            return Err(FsError::UnknownEntry);
        }
        for (component, id) in owned {
            budget.checkpoint(Phase::Cleanup)?;
            remove_file(scratch.file()?, &component, &id, self.user)?;
            self.scratch_owned.borrow_mut().remove(&component);
        }
        expect_names(scratch.file()?, &[])?;
        scratch.check(self.root.file()?, "scratch", self.user)?;
        scratch.close()?;
        posix::unlink(self.root.file()?, "scratch", true)?;
        self.scratch = None;
        Ok(())
    }
    pub(crate) fn cleanup_source_and_case(
        mut self,
        budget: &mut ObserverBudget,
    ) -> Result<(), FsError> {
        budget.begin_cleanup()?;
        let result = self.cleanup_inner(budget);
        if let Err(error) = result {
            self.stopped.set(true);
            return Err(FsError::Retained(Box::new(error)));
        }
        if let Err(error) = close(self.parent) {
            self.stopped.set(true);
            return Err(FsError::Retained(Box::new(error)));
        }
        self.active.set(false);
        Ok(())
    }
    fn cleanup_inner(&mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        budget.checkpoint(Phase::Cleanup)?;
        if self.scratch.is_some() {
            return Err(FsError::ScratchRetained);
        }
        self.check()?;
        self.check_retained_budget(None)?;
        let ids = self.sealed_ids.clone().ok_or(FsError::Identity)?;
        let (current, hashes) = capture::generated_source(
            self.source.file()?,
            self.user,
            self.source.id.device,
            budget,
        )?;
        if current != ids || self.generated_hashes.as_ref() != Some(&hashes) {
            return Err(FsError::Changed);
        }
        let mut root_names = FAMILY
            .into_iter()
            .zip(&ids)
            .filter_map(|(name, id)| id.as_ref().map(|_| name.to_owned()))
            .collect::<BTreeSet<_>>();
        for path in self.registered.keys() {
            if path.components().count() == 1 {
                root_names.insert(path.to_string_lossy().into_owned());
            }
        }
        if names(self.source.file()?)? != root_names {
            return Err(FsError::UnknownEntry);
        }
        // Validate the entire closed tree before any destructive operation.
        for (path, expected) in &self.registered {
            let (parent, leaf) = self.fixture_parent(path)?;
            let file = if expected.directory() {
                open_dir(&parent, &leaf)?
            } else {
                posix::open_read(&parent, &leaf)?
            };
            let id = validate(
                &file,
                self.user,
                self.source.id.device,
                expected.directory(),
            )?;
            if if expected.directory() {
                !id.same_node(expected)
            } else {
                id != *expected
            } {
                return Err(FsError::Changed);
            }
            if expected.directory() {
                let children = self
                    .registered
                    .keys()
                    .filter(|child| child.parent() == Some(path.as_path()))
                    .map(|child| child.file_name().unwrap().to_string_lossy().into_owned())
                    .collect::<BTreeSet<_>>();
                if names(&file)? != children {
                    return Err(FsError::UnknownEntry);
                }
            }
            close(file)?;
            close(parent)?;
        }
        let mut paths = self.registered.keys().cloned().collect::<Vec<_>>();
        paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for path in paths {
            budget.checkpoint(Phase::Cleanup)?;
            let expected = self.registered.get(&path).ok_or(FsError::Identity)?;
            let (parent, leaf) = self.fixture_parent(&path)?;
            if expected.directory() {
                let dir = open_dir(&parent, &leaf)?;
                if !validate(&dir, self.user, self.source.id.device, true)?.same_node(expected)
                    || !names(&dir)?.is_empty()
                {
                    return Err(FsError::Changed);
                }
                close(dir)?;
                if !identity_at(&parent, &leaf)?.is_some_and(|id| id.same_node(expected)) {
                    return Err(FsError::Changed);
                }
                posix::unlink(&parent, &leaf, true)?;
            } else {
                remove_file(&parent, &leaf, expected, self.user)?;
            }
            close(parent)?;
        }
        for (name, member) in FAMILY.into_iter().zip(&ids) {
            if let Some(id) = member {
                remove_file(self.source.file()?, name, id, self.user)?;
            }
        }
        if !names(self.source.file()?)?.is_empty() {
            return Err(FsError::UnknownEntry);
        }
        self.source.check(self.root.file()?, "source", self.user)?;
        self.source.close()?;
        posix::unlink(self.root.file()?, "source", true)?;
        expect_names(self.root.file()?, &["case.json"])?;
        remove_file(self.root.file()?, "case.json", &self.record, self.user)?;
        self.root.check(&self.parent, &self.component, self.user)?;
        self.root.close()?;
        posix::unlink(&self.parent, &self.component, true)?;
        Ok(())
    }
}
fn check_retained_totals(source: u64, material: u64, scratch: u64) -> Result<(), FsError> {
    if scratch > MAX_SCRATCH {
        return Err(FsError::Capacity);
    }
    let retained = source
        .checked_add(material)
        .and_then(|size| size.checked_add(scratch))
        .and_then(|size| size.checked_add(CONTROL_RECORD_RESERVE))
        .ok_or(FsError::Capacity)?;
    if retained > MAX_BATCH {
        return Err(FsError::Capacity);
    }
    Ok(())
}

fn checked_components(path: &Path) -> Result<Vec<String>, FsError> {
    let raw = path.to_str().ok_or(FsError::Identity)?;
    if raw
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(FsError::Identity);
    }
    let mut result = Vec::new();
    for component in path.components() {
        let Component::Normal(text) = component else {
            return Err(FsError::Identity);
        };
        let text = text.to_str().ok_or(FsError::Identity)?;
        posix::name(text)?;
        result.push(text.to_owned());
    }
    if result.is_empty() || result.len() > 8 {
        return Err(FsError::Identity);
    }
    Ok(result)
}
fn write_record(
    parent: &File,
    name: &str,
    user: u32,
    device: u64,
    text: &str,
) -> Result<Identity, FsError> {
    if text.len() > 16 * 1024 {
        return Err(FsError::Capacity);
    }
    let mut file = posix::create_file(parent, name)?;
    validate(&file, user, device, false)?;
    let result = file.write_all(text.as_bytes()).map_err(|_| FsError::Io);
    let id = validate(&file, user, device, false)?;
    close(file)?;
    result?;
    if identity_at(parent, name)?.as_ref() != Some(&id) {
        return Err(FsError::Changed);
    }
    Ok(id)
}
fn remove_file(
    parent: &File,
    component: &str,
    expected: &Identity,
    user: u32,
) -> Result<(), FsError> {
    let file = posix::open_read(parent, component)?;
    let id = validate(&file, user, expected.device, false)?;
    if &id != expected || identity_at(parent, component)?.as_ref() != Some(expected) {
        return Err(FsError::Changed);
    }
    close(file)?;
    if identity_at(parent, component)?.as_ref() != Some(expected) {
        return Err(FsError::Changed);
    }
    posix::unlink(parent, component, false)
}
fn expect_names(file: &File, expected: &[&str]) -> Result<(), FsError> {
    if names(file)? != expected.iter().map(|text| (*text).to_owned()).collect() {
        return Err(FsError::UnknownEntry);
    }
    Ok(())
}
fn open_fixed_dot(parent: &File) -> Result<File, FsError> {
    use std::os::fd::{AsRawFd, FromRawFd};
    // SAFETY: fixed terminated dot, borrowed live directory, no creation.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            c".".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(FsError::Io);
    }
    // SAFETY: successful openat transfers one fresh descriptor into File.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|value| format!("{value:02x}")).collect()
}
