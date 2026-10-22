//! Bounded byte capture and exact owned scratch disposition.
use super::*;
use radishlex_ime_product_upgrade::PreparationFileIdentity;
use sha2::{Digest, Sha256};
use std::io::{Seek, SeekFrom};

struct HeldSource {
    name: &'static str,
    file: Option<File>,
    identity: Identity,
}
pub(crate) struct CapturedCopy<'a> {
    case: &'a mut SyntheticCase,
    family: PreparationFamily,
    source_directory: Identity,
    source: Vec<HeldSource>,
    copied: BTreeMap<&'static str, (Identity, String)>,
    sqlite_started: bool,
}
type SourceRegistration = ([Option<Identity>; 4], [Option<String>; 4]);
pub(super) fn family_identities(parent: &File) -> Result<[Option<Identity>; 4], FsError> {
    Ok([
        identity_at(parent, FAMILY[0])?,
        identity_at(parent, FAMILY[1])?,
        identity_at(parent, FAMILY[2])?,
        identity_at(parent, FAMILY[3])?,
    ])
}
pub(super) fn family_members(family: &PreparationFamily) -> [Option<&PreparationFileIdentity>; 4] {
    [
        Some(&family.database),
        family.wal.as_ref(),
        family.shm.as_ref(),
        family.journal.as_ref(),
    ]
}
pub(super) fn matches_member(id: &Identity, member: &PreparationFileIdentity) -> bool {
    id.device == member.device_id
        && id.inode == member.inode
        && id.owner == member.owner_id
        && id.mode & 0o7777 == member.mode
        && id.links == member.link_count
        && id.size == member.byte_len
}
fn member(id: &Identity, sha256: String) -> PreparationFileIdentity {
    PreparationFileIdentity {
        device_id: id.device,
        inode: id.inode,
        owner_id: id.owner,
        mode: id.mode & 0o7777,
        link_count: id.links,
        byte_len: id.size,
        sha256,
    }
}
/// A caller-supplied length never controls the read loop. Read the exact observed
/// size and then require EOF, checking the descriptor and directory binding.
fn hash_file(
    file: &mut File,
    expected: &Identity,
    budget: &mut ObserverBudget,
) -> Result<String, FsError> {
    file.seek(SeekFrom::Start(0)).map_err(|_| FsError::Io)?;
    let mut remaining = expected.size;
    let mut buffer = [0_u8; CHUNK];
    let mut hash = Sha256::new();
    while remaining != 0 {
        budget.check_io()?;
        let wanted = remaining.min(CHUNK as u64) as usize;
        budget.charge_read(wanted as u64)?;
        let count = file.read(&mut buffer[..wanted]).map_err(|_| FsError::Io)?;
        if count == 0 {
            return Err(FsError::Changed);
        }
        hash.update(&buffer[..count]);
        remaining -= count as u64;
    }
    // EOF probing is charged if an unexpected byte is returned.
    budget.charge_read(1)?;
    let count = file.read(&mut buffer[..1]).map_err(|_| FsError::Io)?;
    if count != 0 || identity(file)? != *expected {
        return Err(FsError::Changed);
    }
    Ok(hex(&hash.finalize()))
}
/// Fixture host registration is separate from observer admission: it can record
/// a deliberately missing DB or journal without making either a valid family.
pub(super) fn generated_source(
    parent: &File,
    user: u32,
    device: u64,
    budget: &mut ObserverBudget,
) -> Result<SourceRegistration, FsError> {
    let ids = family_identities(parent)?;
    let mut total = 0u64;
    for (id, limit) in ids.iter().zip([256 * MIB, 128 * MIB, 8 * MIB, 8 * MIB]) {
        let size = id.as_ref().map_or(0, |id| id.size);
        if size > limit {
            return Err(FsError::Capacity);
        }
        total = total.checked_add(size).ok_or(FsError::Capacity)?;
    }
    if total > 256 * MIB {
        return Err(FsError::Capacity);
    }
    let mut hashes: [Option<String>; 4] = std::array::from_fn(|_| None);
    for (index, component) in FAMILY.into_iter().enumerate() {
        let Some(expected) = &ids[index] else {
            continue;
        };
        let mut file = posix::open_read(parent, component)?;
        if validate(&file, user, device, false)? != *expected {
            return Err(FsError::Changed);
        }
        let digest = hash_file(&mut file, expected, budget);
        close(file)?;
        hashes[index] = Some(digest?);
    }
    if family_identities(parent)? != ids {
        return Err(FsError::Changed);
    }
    Ok((ids, hashes))
}
pub(super) fn read_family(
    parent: &File,
    user: u32,
    device: u64,
    budget: &mut ObserverBudget,
) -> Result<PreparationFamily, FsError> {
    let mut ids = Vec::new();
    for component in FAMILY {
        ids.push(identity_at(parent, component)?);
    }
    let lengths = std::array::from_fn(|index| ids[index].as_ref().map(|id| id.size));
    budget.admit_family(lengths)?;
    if ids[0].is_none() {
        return Err(FsError::Missing);
    }
    if ids[3].is_some() {
        return Err(FsError::Identity);
    }
    let mut output = Vec::new();
    let mut nodes = BTreeSet::new();
    for (component, observed) in FAMILY.into_iter().zip(ids) {
        let Some(id) = observed else {
            output.push(None);
            continue;
        };
        let mut file = posix::open_read(parent, component)?;
        let private = validate(&file, user, device, false)?;
        if private != id || !nodes.insert((id.device, id.inode)) {
            return Err(FsError::Changed);
        }
        let result = hash_file(&mut file, &id, budget);
        let binding = identity_at(parent, component)?;
        close(file)?;
        if binding.as_ref() != Some(&id) {
            return Err(FsError::Changed);
        }
        output.push(Some(member(&id, result?)));
    }
    let mut output = output.into_iter();
    Ok(PreparationFamily {
        database: output.next().flatten().ok_or(FsError::Missing)?,
        wal: output.next().flatten(),
        shm: output.next().flatten(),
        journal: output.next().flatten(),
    })
}
pub(super) fn capture<'a>(
    case: &'a mut SyntheticCase,
    expected: &PreparationFamily,
    budget: &mut ObserverBudget,
) -> Result<CapturedCopy<'a>, FsError> {
    let result = (|| {
        budget.checkpoint(Phase::Capture)?;
        case.check()?;
        case.check_source_inventory()?;
        let source_directory =
            validate(case.source.file()?, case.user, case.source.id.device, true)?;
        if &family_identities(case.source.file()?)?
            != case.sealed_ids.as_ref().ok_or(FsError::Identity)?
        {
            return Err(FsError::Changed);
        }
        if case.baseline.as_ref() != Some(expected) {
            return Err(FsError::Changed);
        }
        let projection = expected
            .database
            .byte_len
            .checked_add(expected.wal.as_ref().map_or(0, |wal| wal.byte_len))
            .and_then(|size| size.checked_add(8 * MIB))
            .ok_or(FsError::Capacity)?;
        case.check_retained_budget(Some(projection))?;
        let scratch = case.scratch.as_ref().ok_or(FsError::ScratchRetained)?;
        if !names(scratch.file()?)?.is_empty() {
            return Err(FsError::UnknownEntry);
        }
        posix::free_space(scratch.file()?)?;
        if read_family(
            case.source.file()?,
            case.user,
            case.source.id.device,
            budget,
        )? != *expected
        {
            return Err(FsError::Changed);
        }
        let mut held = Vec::new();
        for (component, expected_member) in FAMILY.into_iter().zip(family_members(expected)) {
            if let Some(expected_member) = expected_member {
                let file = posix::open_read(case.source.file()?, component)?;
                let id = validate(&file, case.user, case.source.id.device, false)?;
                if !matches_member(&id, expected_member) {
                    return Err(FsError::Changed);
                }
                held.push(HeldSource {
                    name: component,
                    file: Some(file),
                    identity: id,
                });
            }
        }
        let mut copied = BTreeMap::new();
        for source in &mut held {
            if source.name == FAMILY[2] {
                continue;
            }
            let source_file = source.file.as_mut().ok_or(FsError::Close)?;
            source_file
                .seek(SeekFrom::Start(0))
                .map_err(|_| FsError::Io)?;
            let mut destination = posix::create_file(scratch.file()?, source.name)?;
            let initial = validate(&destination, case.user, scratch.id.device, false)?;
            if held_node_matches(&initial, &source.identity) {
                return Err(FsError::Identity);
            }
            let mut buffer = [0_u8; CHUNK];
            let mut remaining = source.identity.size;
            let mut hash = Sha256::new();
            while remaining != 0 {
                budget.checkpoint(Phase::Capture)?;
                let wanted = remaining.min(CHUNK as u64) as usize;
                budget.charge_read(wanted as u64)?;
                let count = source_file
                    .read(&mut buffer[..wanted])
                    .map_err(|_| FsError::Io)?;
                if count == 0 {
                    return Err(FsError::Changed);
                }
                budget.charge_write(count as u64)?;
                destination
                    .write_all(&buffer[..count])
                    .map_err(|_| FsError::Io)?;
                hash.update(&buffer[..count]);
                remaining -= count as u64;
            }
            budget.charge_read(1)?;
            let count = source_file
                .read(&mut buffer[..1])
                .map_err(|_| FsError::Io)?;
            if count != 0 || identity(source_file)? != source.identity {
                return Err(FsError::Changed);
            }
            let id = validate(&destination, case.user, scratch.id.device, false)?;
            let digest = hex(&hash.finalize());
            close(destination)?;
            let expected_member = if source.name == FAMILY[0] {
                &expected.database
            } else {
                expected.wal.as_ref().ok_or(FsError::Changed)?
            };
            if digest != expected_member.sha256
                || id.size != source.identity.size
                || identity_at(scratch.file()?, source.name)?.as_ref() != Some(&id)
            {
                return Err(FsError::Changed);
            }
            case.scratch_owned
                .borrow_mut()
                .insert(source.name.to_owned(), id.clone());
            copied.insert(source.name, (id, digest));
        }
        case.check()?;
        if read_family(
            case.source.file()?,
            case.user,
            case.source.id.device,
            budget,
        )? != *expected
        {
            return Err(FsError::Changed);
        }
        if validate(case.source.file()?, case.user, case.source.id.device, true)?
            != source_directory
        {
            return Err(FsError::Changed);
        }
        Ok((held, copied, source_directory))
    })();
    match result {
        Ok((source, copied, source_directory)) => Ok(CapturedCopy {
            case,
            family: expected.clone(),
            source_directory,
            source,
            copied,
            sqlite_started: false,
        }),
        Err(error) => {
            case.stopped.set(true);
            Err(FsError::Retained(Box::new(error)))
        }
    }
}
fn held_node_matches(left: &Identity, right: &Identity) -> bool {
    left.device == right.device && left.inode == right.inode
}
pub(super) fn validate_generated_index(
    recorded: Option<&Identity>,
    observed: Option<&Identity>,
    sqlite_started: bool,
) -> Result<(), FsError> {
    match (recorded, observed) {
        (Some(_), None) => Err(FsError::Changed),
        (_, Some(_)) if !sqlite_started => Err(FsError::UnknownEntry),
        (Some(created), Some(current)) if !created.same_node(current) => Err(FsError::Changed),
        _ => Ok(()),
    }
}

impl CapturedCopy<'_> {
    /// Called immediately before the sole SQLite connection is opened. An SHM
    /// entry before this transition is unknown rather than adopted as an index.
    pub(crate) fn mark_sqlite_started(&mut self) -> Result<(), FsError> {
        if self.sqlite_started {
            return Err(FsError::Identity);
        }
        self.case.check()?;
        let scratch = self.case.scratch.as_ref().ok_or(FsError::ScratchRetained)?;
        if identity_at(scratch.file()?, FAMILY[2])?.is_some() {
            return Err(FsError::UnknownEntry);
        }
        self.sqlite_started = true;
        Ok(())
    }
    pub(crate) fn revalidate_source(&mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        self.case.check()?;
        if validate(
            self.case.source.file()?,
            self.case.user,
            self.case.source.id.device,
            true,
        )? != self.source_directory
        {
            return Err(FsError::Changed);
        }
        self.case.check_source_inventory()?;
        for source in &self.source {
            let file = source.file.as_ref().ok_or(FsError::Close)?;
            if validate(file, self.case.user, self.case.source.id.device, false)? != source.identity
                || identity_at(self.case.source.file()?, source.name)?.as_ref()
                    != Some(&source.identity)
            {
                return Err(FsError::Changed);
            }
        }
        if read_family(
            self.case.source.file()?,
            self.case.user,
            self.case.source.id.device,
            budget,
        )? != self.family
        {
            return Err(FsError::Changed);
        }
        Ok(())
    }
    pub(crate) fn verify_copy(&self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        budget.check_io()?;
        self.case.check()?;
        self.case.check_retained_budget(None)?;
        let scratch = self.case.scratch.as_ref().ok_or(FsError::ScratchRetained)?;
        let entries = names(scratch.file()?)?;
        let recorded_index = self.case.scratch_owned.borrow().get(FAMILY[2]).cloned();
        if !entries.contains(FAMILY[2]) {
            // This profile has no contract allowing an already observed index
            // to disappear, even if SQLite might normally unlink one on close.
            validate_generated_index(recorded_index.as_ref(), None, self.sqlite_started)?;
        }
        let mut allowed = self
            .copied
            .keys()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>();
        if entries.contains(FAMILY[2]) {
            if !self.sqlite_started {
                return Err(FsError::UnknownEntry);
            }
            allowed.insert(FAMILY[2].to_owned());
        }
        if allowed != entries {
            return Err(FsError::UnknownEntry);
        }
        let mut total = 0_u64;
        for (component, (expected, digest)) in &self.copied {
            let mut file = posix::open_read(scratch.file()?, component)?;
            let id = validate(&file, self.case.user, scratch.id.device, false)?;
            if id != *expected {
                return Err(FsError::Changed);
            }
            let actual = hash_file(&mut file, expected, budget);
            close(file)?;
            if actual? != *digest
                || identity_at(scratch.file()?, component)?.as_ref() != Some(expected)
            {
                return Err(FsError::Changed);
            }
            total = total.checked_add(id.size).ok_or(FsError::Capacity)?;
        }
        if entries.contains(FAMILY[2]) {
            let file = posix::open_read(scratch.file()?, FAMILY[2])?;
            let id = validate(&file, self.case.user, scratch.id.device, false)?;
            close(file)?;
            if id.size > 8 * MIB || identity_at(scratch.file()?, FAMILY[2])?.as_ref() != Some(&id) {
                return Err(FsError::Capacity);
            }
            // SQLite may change its index's length/timestamps until explicit
            // close, but a later verification must never adopt another inode.
            // The final post-close verification records the stable identity
            // consumed by exact fixture-host/observer disposal.
            validate_generated_index(recorded_index.as_ref(), Some(&id), self.sqlite_started)?;
            self.case
                .scratch_owned
                .borrow_mut()
                .insert(FAMILY[2].to_owned(), id.clone());
            total = total.checked_add(id.size).ok_or(FsError::Capacity)?;
        }
        if total > MAX_SCRATCH {
            return Err(FsError::Capacity);
        }
        self.case.check_retained_budget(None)?;
        Ok(())
    }
    /// Close held source descriptors explicitly while retaining every on-disk
    /// object. This is also the ordinary close-unconfirmed SQLite disposition:
    /// uncertainty never grants deletion authority or a successful observation.
    pub(crate) fn retain(mut self) -> Result<(), FsError> {
        self.case.stopped.set(true);
        let mut failure = None;
        for source in &mut self.source {
            let result = source.file.take().ok_or(FsError::Close).and_then(close);
            if let Err(error) = result {
                if failure.is_none() {
                    failure = Some(error);
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
    /// Explicit fixture-host handoff after a deliberate source/namespace fault.
    /// No unlink is attempted and the batch remains stopped. Only the dedicated
    /// planned-fault test consumes these creation records for its own teardown.
    pub(super) fn close_for_planned_fault(mut self) -> Result<BTreeMap<String, Identity>, FsError> {
        self.case.stopped.set(true);
        for source in &mut self.source {
            close(source.file.take().ok_or(FsError::Close)?)?;
        }
        Ok(self
            .copied
            .iter()
            .map(|(name, (id, _))| ((*name).to_owned(), id.clone()))
            .collect())
    }
    /// Caller must already have explicitly closed SQLite. No connection is
    /// stored here and there is deliberately no Drop that attempts unlink.
    pub(crate) fn cleanup(mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        budget.begin_cleanup()?;
        let result = self.cleanup_inner(budget);
        if let Err(error) = result {
            self.case.stopped.set(true);
            return Err(FsError::Retained(Box::new(error)));
        }
        result
    }
    fn cleanup_inner(&mut self, budget: &mut ObserverBudget) -> Result<(), FsError> {
        budget.checkpoint(Phase::Cleanup)?;
        self.revalidate_source(budget)?;
        self.verify_copy(budget)?;
        for source in &mut self.source {
            close(source.file.take().ok_or(FsError::Close)?)?;
        }
        let scratch = self.case.scratch.as_mut().ok_or(FsError::ScratchRetained)?;
        // Inventory is closed before the first unlink. Use only the exact SHM
        // identity registered by the verification above; observing its current
        // pathname again must never grant ownership of a replacement inode.
        let mut owned = self
            .copied
            .iter()
            .map(|(name, (id, _))| ((*name).to_owned(), id.clone()))
            .collect::<BTreeMap<_, _>>();
        if let Some(shm) = self.case.scratch_owned.borrow().get(FAMILY[2]).cloned() {
            owned.insert(FAMILY[2].to_owned(), shm);
        }
        if names(scratch.file()?)? != owned.keys().cloned().collect() {
            return Err(FsError::UnknownEntry);
        }
        for (component, expected) in owned {
            budget.checkpoint(Phase::Cleanup)?;
            scratch.check(self.case.root.file()?, "scratch", self.case.user)?;
            remove_file(scratch.file()?, &component, &expected, self.case.user)?;
            self.case.scratch_owned.borrow_mut().remove(&component);
        }
        if !names(scratch.file()?)?.is_empty() {
            return Err(FsError::UnknownEntry);
        }
        scratch.check(self.case.root.file()?, "scratch", self.case.user)?;
        scratch.close()?;
        posix::unlink(self.case.root.file()?, "scratch", true)?;
        self.case.scratch = None;
        self.case.check()?;
        Ok(())
    }
}
