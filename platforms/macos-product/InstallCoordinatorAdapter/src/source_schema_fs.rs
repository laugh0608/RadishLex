//! Owned private-copy lifecycle. Source access is byte-only and descriptor-relative.
#![allow(unsafe_code)]

#[path = "schema_observer_fs_posix.rs"]
pub(crate) mod posix;
use super::native;
use crate::schema_observer_budget::{
    BudgetError, ObserverBudget, Phase, BLOCK_SIZE, MAX_SCRATCH, MIB, MIN_FREE,
};
use posix::{close, identity, identity_at, names, open_dir, open_read, validate, Identity};
use radishlex_ime_product_upgrade::{
    PreparationDirectoryIdentity, PreparationFamily, PreparationFileIdentity,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub(crate) const FAMILY: [&str; 4] = [
    "userdb.sqlite3",
    "userdb.sqlite3-wal",
    "userdb.sqlite3-shm",
    "userdb.sqlite3-journal",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
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
    Platform,
    BackupExclusion,
    Retained(Box<FsError>),
    Budget(BudgetError),
}
impl From<BudgetError> for FsError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}

struct Anchor {
    file: File,
    id: Identity,
    name: Option<OsString>,
}
struct Chain {
    nodes: Vec<Anchor>,
    path: PathBuf,
    user: u32,
}
impl Chain {
    fn open(path: &Path, user: u32, budget: &ObserverBudget) -> Result<Self, FsError> {
        let raw = path.as_os_str().as_bytes();
        if raw.first() != Some(&b'/') || raw.len() > 4096 || raw.len() < 2 || raw.ends_with(b"/") {
            return Err(FsError::Identity);
        }
        let parts: Vec<_> = raw[1..].split(|byte| *byte == b'/').collect();
        if parts.len() > 64
            || parts
                .iter()
                .any(|part| part.is_empty() || *part == b"." || *part == b".." || part.contains(&0))
        {
            return Err(FsError::Identity);
        }
        let file = posix::open_slash()?;
        let id = identity(&file)?;
        let mut value = Self {
            nodes: vec![Anchor {
                file,
                id,
                name: None,
            }],
            path: path.to_owned(),
            user,
        };
        for part in parts {
            budget.check_io()?;
            let name = std::ffi::OsStr::from_bytes(part);
            let parent = value.file()?;
            let before = posix::ancestor_identity_at(parent, name)?;
            let file = posix::open_ancestor(parent, name)?;
            let id = identity(&file)?;
            if before != id {
                return Err(FsError::Changed);
            }
            value.nodes.push(Anchor {
                file,
                id,
                name: Some(name.to_owned()),
            });
        }
        value.check(budget)?;
        Ok(value)
    }
    fn file(&self) -> Result<&File, FsError> {
        self.nodes.last().map(|v| &v.file).ok_or(FsError::Close)
    }
    fn id(&self) -> Result<&Identity, FsError> {
        self.nodes.last().map(|v| &v.id).ok_or(FsError::Close)
    }
    fn contains(&self, id: &Identity) -> bool {
        self.nodes.iter().any(|v| same_inode(&v.id, id))
    }
    fn check(&self, budget: &ObserverBudget) -> Result<(), FsError> {
        let slash = posix::open_slash()?;
        let same = identity(&slash)?.same_node(&self.nodes.first().ok_or(FsError::Close)?.id);
        close(slash)?;
        if !same {
            return Err(FsError::Changed);
        }
        let mut physical = PathBuf::from("/");
        for (index, node) in self.nodes.iter().enumerate() {
            budget.check_io()?;
            if let Some(name) = &node.name {
                physical.push(name);
            }
            let current = identity(&node.file)?;
            let temporary = physical == Path::new("/private/tmp");
            if !current.same_node(&node.id)
                || !current.directory()
                || current.inode == 0
                || (current.owner != 0 && current.owner != self.user)
                || (temporary && (current.owner != 0 || current.mode & 0o7777 != 0o1777))
                || (!temporary && current.mode & 0o7022 != 0)
            {
                return Err(FsError::Identity);
            }
            posix::reject_grant_acl(&node.file)?;
            if index > 0
                && !posix::ancestor_identity_at(
                    &self.nodes[index - 1].file,
                    node.name.as_deref().ok_or(FsError::Identity)?,
                )?
                .same_node(&current)
            {
                return Err(FsError::Changed);
            }
        }
        Ok(())
    }
    fn private_child(
        &mut self,
        name: &str,
        exclusive: bool,
        budget: &ObserverBudget,
    ) -> Result<(), FsError> {
        self.check(budget)?;
        let parent = self.file()?;
        let file = match identity_at(parent, name)? {
            Some(_) if exclusive => return Err(FsError::ScratchOccupied),
            Some(_) => open_dir(parent, name)?,
            None => posix::create_dir(parent, name)?,
        };
        let id = validate(&file, self.user, self.id()?.device, true)?;
        if !identity_at(parent, name)?.is_some_and(|entry| entry.same_node(&id)) {
            return Err(FsError::Changed);
        }
        self.nodes.push(Anchor {
            file,
            id,
            name: Some(name.into()),
        });
        self.path.push(name);
        self.check(budget)
    }
    fn close(mut self) -> Result<(), FsError> {
        let mut error = None;
        while let Some(node) = self.nodes.pop() {
            if let Err(value) = close(node.file) {
                error.get_or_insert(value);
            }
        }
        error.map_or(Ok(()), Err)
    }
}

struct Held {
    name: &'static str,
    file: Option<File>,
    id: Identity,
}
struct CopyFile {
    id: Identity,
    digest: Option<String>,
}

pub(super) struct Scratch {
    source: Option<Chain>,
    base: Option<Chain>,
    active: Option<File>,
    active_id: Identity,
    source_id: Identity,
    path: PathBuf,
    user: u32,
    family: PreparationFamily,
    held: Vec<Held>,
    copied: BTreeMap<String, CopyFile>,
    sqlite_started: bool,
    close_uncertain: bool,
}

fn same_inode(left: &Identity, right: &Identity) -> bool {
    left.device == right.device && left.inode == right.inode
}
fn members(family: &PreparationFamily) -> [Option<&PreparationFileIdentity>; 4] {
    [
        Some(&family.database),
        family.wal.as_ref(),
        family.shm.as_ref(),
        family.journal.as_ref(),
    ]
}
fn member_matches(id: &Identity, member: &PreparationFileIdentity) -> bool {
    id.device == member.device_id
        && id.inode == member.inode
        && id.owner == member.owner_id
        && id.mode & 0o7777 == member.mode
        && id.links == member.link_count
        && id.size == member.byte_len
}
fn hash(file: &mut File, id: &Identity, budget: &mut ObserverBudget) -> Result<String, FsError> {
    file.seek(SeekFrom::Start(0)).map_err(|_| FsError::Io)?;
    let mut remaining = id.size;
    let mut buffer = [0; BLOCK_SIZE];
    let mut digest = Sha256::new();
    while remaining > 0 {
        let wanted = remaining.min(buffer.len() as u64) as usize;
        budget.charge_read(wanted as u64)?;
        let count = file.read(&mut buffer[..wanted]).map_err(|_| FsError::Io)?;
        if count == 0 {
            return Err(FsError::Changed);
        }
        remaining -= count as u64;
        digest.update(&buffer[..count]);
    }
    budget.charge_read(1)?;
    if file.read(&mut buffer[..1]).map_err(|_| FsError::Io)? != 0 || identity(file)? != *id {
        return Err(FsError::Changed);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

impl Scratch {
    pub(super) fn create(
        source: &Path,
        root: &PreparationDirectoryIdentity,
        family: &PreparationFamily,
        evidence: &Path,
        budget: &mut ObserverBudget,
    ) -> Result<Self, FsError> {
        let caches = native::caches_path()?;
        Self::create_checked(&caches, source, root, family, Some(evidence), budget)
    }
    #[cfg(test)]
    pub(super) fn create_at(
        caches: &Path,
        source: &Path,
        root: &PreparationDirectoryIdentity,
        family: &PreparationFamily,
        budget: &mut ObserverBudget,
    ) -> Result<Self, FsError> {
        Self::create_checked(caches, source, root, family, None, budget)
    }
    fn create_checked(
        caches: &Path,
        source: &Path,
        root: &PreparationDirectoryIdentity,
        family: &PreparationFamily,
        evidence: Option<&Path>,
        budget: &mut ObserverBudget,
    ) -> Result<Self, FsError> {
        budget.checkpoint(Phase::Capture)?;
        // SAFETY: UID queries have no memory/ownership preconditions.
        let (user, real) = unsafe { (libc::geteuid(), libc::getuid()) };
        if user == 0 || user != real {
            return Err(FsError::Identity);
        }
        budget.admit_family(members(family).map(|v| v.map(|v| v.byte_len)))?;
        let source = Chain::open(source, user, budget)?;
        let source_id = validate(source.file()?, user, root.device_id, true)?;
        if source_id.device != root.device_id
            || source_id.inode != root.inode
            || source_id.owner != root.owner_id
            || source_id.mode & 0o7777 != root.mode
        {
            return Err(FsError::Identity);
        }
        let evidence = evidence
            .map(|path| Chain::open(path, user, budget))
            .transpose()?;
        if let Some(evidence) = &evidence {
            let id = validate(evidence.file()?, user, evidence.id()?.device, true)?;
            if source.contains(&id) || evidence.contains(&source_id) {
                return Err(FsError::Identity);
            }
        }
        let mut base = Chain::open(caches, user, budget)?;
        if base.id()?.owner != user {
            return Err(FsError::Identity);
        }
        // Reject before creating any object under a source root.
        if base.contains(&source_id) {
            return Err(FsError::Identity);
        }
        if let Some(evidence) = &evidence {
            if base.contains(evidence.id()?) {
                return Err(FsError::Identity);
            }
        }
        base.private_child("RadishLex", false, budget)?;
        if source.contains(base.id()?) {
            return Err(FsError::Identity);
        }
        if let Some(evidence) = &evidence {
            if evidence.contains(base.id()?) {
                return Err(FsError::Identity);
            }
        }
        base.private_child("SchemaObservation-v1", false, budget)?;
        if source.contains(base.id()?) || base.contains(&source_id) {
            return Err(FsError::Identity);
        }
        if let Some(evidence) = evidence {
            evidence.check(budget)?;
            if evidence.contains(base.id()?) || base.contains(evidence.id()?) {
                return Err(FsError::Identity);
            }
            evidence.close()?;
        }
        if !names(base.file()?)?.is_empty() {
            return Err(FsError::ScratchOccupied);
        }
        posix::free_space(base.file()?)?;
        base.check(budget)?;
        // Exclusive mkdir is the process-independent lease. Existing active is
        // never reopened, adopted, renamed, swept or retried elsewhere.
        let active = posix::create_dir(base.file()?, "active")?;
        let active_id = validate(&active, user, base.id()?.device, true)?;
        let path = base.path.join("active");
        let value = Self {
            source: Some(source),
            base: Some(base),
            active: Some(active),
            active_id,
            source_id,
            path,
            user,
            family: family.clone(),
            held: Vec::new(),
            copied: BTreeMap::new(),
            sqlite_started: false,
            close_uncertain: false,
        };
        let ready = (|| {
            value.check(budget)?;
            // The exclusive empty leaf also serializes resource-property
            // changes on the shared dedicated root. No sensitive bytes exist.
            native::exclude_from_backup(&value.base()?.path)?;
            native::exclude_from_backup(&value.path)?;
            value.check_backup(budget)
        })();
        if let Err(error) = ready {
            // Creation has crossed the exclusive-active boundary. Preserve the
            // leaf on an unconfirmed namespace/backup state, even while empty.
            return Err(FsError::Retained(Box::new(error)));
        }
        Ok(value)
    }
    fn source(&self) -> Result<&Chain, FsError> {
        self.source.as_ref().ok_or(FsError::Close)
    }
    fn base(&self) -> Result<&Chain, FsError> {
        self.base.as_ref().ok_or(FsError::Close)
    }
    fn active(&self) -> Result<&File, FsError> {
        self.active.as_ref().ok_or(FsError::Close)
    }
    fn check(&self, budget: &ObserverBudget) -> Result<(), FsError> {
        self.source()?.check(budget)?;
        self.base()?.check(budget)?;
        let observed = validate(self.active()?, self.user, self.active_id.device, true)?;
        if !observed.same_node(&self.active_id)
            || !identity_at(self.base()?.file()?, "active")?.is_some_and(|v| v.same_node(&observed))
        {
            return Err(FsError::Changed);
        }
        if names(self.base()?.file()?)? != BTreeSet::from(["active".to_owned()]) {
            return Err(FsError::UnknownEntry);
        }
        Ok(())
    }
    fn check_backup(&self, budget: &ObserverBudget) -> Result<(), FsError> {
        self.check(budget)?;
        native::verify_backup_exclusion(&self.base()?.path)?;
        native::verify_backup_exclusion(&self.path)?;
        self.check(budget)
    }
    pub(super) fn database_path(&self) -> PathBuf {
        self.path.join(FAMILY[0])
    }
    pub(super) fn capture(&mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        self.check_backup(budget)?;
        if !self.held.is_empty() || !names(self.active()?)?.is_empty() {
            return Err(FsError::UnknownEntry);
        }
        for (name, expected) in FAMILY.into_iter().zip(members(&self.family)) {
            let observed = identity_at(self.source()?.file()?, name)?;
            let Some(expected) = expected else {
                if observed.is_some() {
                    return Err(FsError::Changed);
                }
                continue;
            };
            let file = open_read(self.source()?.file()?, name)?;
            let id = match validate(&file, self.user, self.source_id.device, false) {
                Ok(id) => id,
                Err(error) => {
                    if let Err(close_error) = close(file) {
                        self.close_uncertain = true;
                        return Err(close_error);
                    }
                    return Err(error);
                }
            };
            if observed.as_ref() != Some(&id) || !member_matches(&id, expected) {
                if let Err(error) = close(file) {
                    self.close_uncertain = true;
                    return Err(error);
                }
                return Err(FsError::Changed);
            }
            self.held.push(Held {
                name,
                file: Some(file),
                id,
            });
        }
        self.revalidate_source(budget)?;
        for index in 0..self.held.len() {
            if self.held[index].name == FAMILY[2] {
                continue;
            }
            self.copy_one(index, budget)?;
        }
        self.revalidate_source(budget)?;
        self.verify_copy(budget)
    }
    fn copy_one(&mut self, index: usize, budget: &mut ObserverBudget) -> Result<(), FsError> {
        self.check_backup(budget)?;
        posix::free_space(self.active()?)?;
        let name = self.held[index].name;
        let mut destination = posix::create_file(self.active()?, name)?;
        let initial = validate(&destination, self.user, self.active_id.device, false)?;
        if self
            .held
            .iter()
            .any(|source| same_inode(&initial, &source.id))
        {
            return Err(FsError::Identity);
        }
        self.copied.insert(
            name.into(),
            CopyFile {
                id: initial.clone(),
                digest: None,
            },
        );
        let work = (|| {
            let held = &mut self.held[index];
            let source = held.file.as_mut().ok_or(FsError::Close)?;
            source.seek(SeekFrom::Start(0)).map_err(|_| FsError::Io)?;
            let mut remaining = held.id.size;
            let mut buffer = [0; BLOCK_SIZE];
            let mut digest = Sha256::new();
            while remaining > 0 {
                let wanted = remaining.min(buffer.len() as u64) as usize;
                budget.charge_read(wanted as u64)?;
                let count = source
                    .read(&mut buffer[..wanted])
                    .map_err(|_| FsError::Io)?;
                if count == 0 {
                    return Err(FsError::Changed);
                }
                budget.charge_write(count as u64)?;
                destination
                    .write_all(&buffer[..count])
                    .map_err(|_| FsError::Io)?;
                remaining -= count as u64;
                digest.update(&buffer[..count]);
            }
            budget.charge_read(1)?;
            if source.read(&mut buffer[..1]).map_err(|_| FsError::Io)? != 0
                || identity(source)? != held.id
            {
                return Err(FsError::Changed);
            }
            let actual: String = digest
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let expected = if name == FAMILY[0] {
                &self.family.database
            } else {
                self.family.wal.as_ref().ok_or(FsError::Identity)?
            };
            if actual != expected.sha256 {
                return Err(FsError::Changed);
            }
            Ok(actual)
        })();
        // Record the exact final identity of our creation even after a partial
        // write, but never substitute an independently reopened pathname.
        let final_id = validate(&destination, self.user, self.active_id.device, false);
        if let Err(error) = close(destination) {
            self.close_uncertain = true;
            return Err(error);
        }
        let final_id = final_id?;
        if !final_id.same_node(&initial)
            || identity_at(self.active()?, name)?.as_ref() != Some(&final_id)
        {
            return Err(FsError::Changed);
        }
        self.copied.get_mut(name).ok_or(FsError::Identity)?.id = final_id;
        self.copied.get_mut(name).ok_or(FsError::Identity)?.digest = Some(work?);
        Ok(())
    }
    pub(super) fn revalidate_source(&mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        self.check(budget)?;
        if validate(
            self.source()?.file()?,
            self.user,
            self.source_id.device,
            true,
        )? != self.source_id
        {
            return Err(FsError::Changed);
        }
        for (name, expected) in FAMILY.into_iter().zip(members(&self.family)) {
            let observed = identity_at(self.source()?.file()?, name)?;
            if observed
                .as_ref()
                .map(|id| expected.is_some_and(|v| member_matches(id, v)))
                != expected.map(|_| true)
            {
                return Err(FsError::Changed);
            }
        }
        for held in &mut self.held {
            let source = held.file.as_mut().ok_or(FsError::Close)?;
            let expected = match held.name {
                "userdb.sqlite3" => &self.family.database,
                "userdb.sqlite3-wal" => self.family.wal.as_ref().ok_or(FsError::Identity)?,
                _ => self.family.shm.as_ref().ok_or(FsError::Identity)?,
            };
            if validate(source, self.user, self.source_id.device, false)? != held.id
                || hash(source, &held.id, budget)? != expected.sha256
            {
                return Err(FsError::Changed);
            }
        }
        self.check(budget)
    }
    pub(super) fn mark_sqlite_started(&mut self, budget: &ObserverBudget) -> Result<(), FsError> {
        self.check_backup(budget)?;
        if self.sqlite_started || identity_at(self.active()?, FAMILY[2])?.is_some() {
            return Err(FsError::UnknownEntry);
        }
        self.sqlite_started = true;
        Ok(())
    }
    pub(super) fn verify_copy(&mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        self.check_backup(budget)?;
        let entries = names(self.active()?)?;
        let mut allowed: BTreeSet<_> = self.copied.keys().cloned().collect();
        if self.sqlite_started && entries.contains(FAMILY[2]) {
            allowed.insert(FAMILY[2].into());
        }
        if entries != allowed {
            return Err(FsError::UnknownEntry);
        }
        let mut total = 0u64;
        for name in entries {
            let mut file = open_read(self.active()?, &name)?;
            let result = (|| {
                let id = validate(&file, self.user, self.active_id.device, false)?;
                if identity_at(self.active()?, &name)?.as_ref() != Some(&id) {
                    return Err(FsError::Changed);
                }
                total = total.checked_add(id.size).ok_or(FsError::Capacity)?;
                if name == FAMILY[2] {
                    if !self.sqlite_started
                        || id.size > 8 * MIB
                        || self.copied.get(&name).is_some_and(|v| !v.id.same_node(&id))
                    {
                        return Err(FsError::Changed);
                    }
                    return Ok((id, None));
                }
                let expected = self.copied.get(&name).ok_or(FsError::UnknownEntry)?;
                if id != expected.id {
                    return Err(FsError::Changed);
                }
                if let Some(digest) = &expected.digest {
                    if hash(&mut file, &id, budget)? != *digest {
                        return Err(FsError::Changed);
                    }
                }
                Ok((id, expected.digest.clone()))
            })();
            if let Err(error) = close(file) {
                self.close_uncertain = true;
                return Err(error);
            }
            let (id, digest) = result?;
            if identity_at(self.active()?, &name)?.as_ref() != Some(&id) {
                return Err(FsError::Changed);
            }
            self.copied.insert(name, CopyFile { id, digest });
        }
        if total > MAX_SCRATCH {
            return Err(FsError::Capacity);
        }
        self.check_backup(budget)
    }
    pub(super) fn retain(mut self) -> Result<(), FsError> {
        self.close_held()
    }
    fn close_held(&mut self) -> Result<(), FsError> {
        let mut failure = None;
        for held in &mut self.held {
            if let Some(file) = held.file.take() {
                if let Err(error) = close(file) {
                    self.close_uncertain = true;
                    failure.get_or_insert(error);
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }
    pub(super) fn dispose(mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        budget.begin_cleanup()?;
        if self.close_uncertain {
            return Err(FsError::Retained(Box::new(FsError::Close)));
        }
        self.revalidate_source(budget)?;
        self.verify_copy(budget)?;
        self.close_held()?;
        self.source.take().ok_or(FsError::Close)?.close()?;
        let base = self.base.as_ref().ok_or(FsError::Close)?;
        for (name, expected) in &self.copied {
            budget.check_io()?;
            base.check(budget)?;
            if !identity_at(base.file()?, "active")?.is_some_and(|v| v.same_node(&self.active_id)) {
                return Err(FsError::Changed);
            }
            let file = open_read(self.active()?, name)?;
            let actual = validate(&file, self.user, self.active_id.device, false);
            close(file)?;
            if actual? != expected.id
                || identity_at(self.active()?, name)?.as_ref() != Some(&expected.id)
            {
                return Err(FsError::Changed);
            }
            posix::unlink(self.active()?, name, false)?;
        }
        budget.check_io()?;
        base.check(budget)?;
        if !names(self.active()?)?.is_empty()
            || !identity_at(base.file()?, "active")?.is_some_and(|v| v.same_node(&self.active_id))
        {
            return Err(FsError::Changed);
        }
        close(self.active.take().ok_or(FsError::Close)?)?;
        posix::unlink(base.file()?, "active", true)?;
        base.check(budget)?;
        self.base.take().ok_or(FsError::Close)?.close()?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "source_schema_fs_tests.rs"]
mod tests;
