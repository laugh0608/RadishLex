use super::*;
use sha2::Digest;

#[test]
fn components_cannot_escape_or_name_family_via_normalization() {
    for value in [
        "",
        "/absolute",
        "../outside",
        "a/../../outside",
        "a/./b",
        "a//b",
    ] {
        // Path normalizes repeated separators/dot. Reject their raw spelling
        // separately as well as their parsed component types.
        assert!(checked_components(Path::new(value)).is_err(), "{value}");
    }
    assert!(checked_components(Path::new(".radishlex-upgrade-v1/preparation.json")).is_ok());
}

#[test]
fn directory_and_file_identity_have_distinct_cleanup_rules() {
    let value = Identity {
        device: 1,
        inode: 2,
        owner: 501,
        mode: libc::S_IFDIR as u32 | 0o700,
        links: 3,
        size: 0,
        modified: (0, 0),
        changed: (0, 0),
    };
    assert!(value.directory());
    let mut changed = value.clone();
    changed.inode += 1;
    assert!(!value.same_node(&changed));
    changed = value.clone();
    changed.mode |= 0o020;
    assert!(!value.same_node(&changed));
    changed = value.clone();
    changed.modified.0 += 1;
    assert!(value.same_node(&changed));
    assert_ne!(value, changed);
}

/// Called by the single batch integration test; it never creates a second root
/// or adopts residual objects. Every injected name is known before creation.
pub(crate) fn check_owned_negative_objects(case: &mut SyntheticCase) -> Result<(), FsError> {
    use std::os::unix::fs::symlink;
    let scratch = case.scratch.as_ref().ok_or(FsError::ScratchRetained)?;
    case.check()?;
    let file = posix::create_file(scratch.file()?, "negative-sentinel")?;
    let sentinel = validate(&file, case.user, scratch.id.device, false)?;
    assert_eq!(
        expect_names(scratch.file()?, &[]),
        Err(FsError::UnknownEntry)
    );
    close(file)?;
    let link_path = scratch.path.join("negative-link");
    std::fs::hard_link(scratch.path.join("negative-sentinel"), &link_path)
        .map_err(|_| FsError::Io)?;
    let linked = posix::open_read(scratch.file()?, "negative-link")?;
    let linked_id = identity(&linked)?;
    assert_eq!(
        validate(&linked, case.user, scratch.id.device, false),
        Err(FsError::Identity)
    );
    assert_eq!(linked_id.inode, sentinel.inode);
    assert_eq!(linked_id.links, 2);
    close(linked)?;
    // The fixture host created this exact hard link and registers its current
    // identity before dismantling it; observer cleanup never accepts links.
    if identity_at(scratch.file()?, "negative-link")?.as_ref() != Some(&linked_id) {
        return Err(FsError::Changed);
    }
    posix::unlink(scratch.file()?, "negative-link", false)?;
    let current = identity_at(scratch.file()?, "negative-sentinel")?.ok_or(FsError::Changed)?;
    assert_eq!(current.inode, sentinel.inode);
    assert_eq!(current.links, 1);
    let sym_path = scratch.path.join("negative-symlink");
    symlink("negative-sentinel", &sym_path).map_err(|_| FsError::Io)?;
    assert!(posix::open_read(scratch.file()?, "negative-symlink").is_err());
    let sym_id = identity_at(scratch.file()?, "negative-symlink")?.ok_or(FsError::Changed)?;
    assert_eq!(sym_id.mode & libc::S_IFMT as u32, libc::S_IFLNK as u32);
    if identity_at(scratch.file()?, "negative-symlink")?.as_ref() != Some(&sym_id) {
        return Err(FsError::Changed);
    }
    posix::unlink(scratch.file()?, "negative-symlink", false)?;
    let reopened = posix::open_read(scratch.file()?, "negative-sentinel")?;
    inject_fault(FsOperation::Close, 0);
    assert_eq!(close(reopened), Err(FsError::Close));
    clear_fault();
    inject_fault(FsOperation::Unlink, 0);
    assert_eq!(
        remove_file(scratch.file()?, "negative-sentinel", &current, case.user),
        Err(FsError::Unlink)
    );
    clear_fault();
    assert_eq!(
        identity_at(scratch.file()?, "negative-sentinel")?.as_ref(),
        Some(&current)
    );
    remove_file(scratch.file()?, "negative-sentinel", &current, case.user)?;
    let directory = posix::create_dir(scratch.file()?, "negative-directory")?;
    let dir_id = validate(&directory, case.user, scratch.id.device, true)?;
    close(directory)?;
    inject_fault(FsOperation::Rmdir, 0);
    assert_eq!(
        posix::unlink(scratch.file()?, "negative-directory", true),
        Err(FsError::Unlink)
    );
    clear_fault();
    assert!(
        identity_at(scratch.file()?, "negative-directory")?.is_some_and(|id| id.same_node(&dir_id))
    );
    posix::unlink(scratch.file()?, "negative-directory", true)?;
    expect_names(scratch.file()?, &[])?;
    // Parent/capability substitution and changed device are rejected before IO.
    let mut wrong = scratch.id.clone();
    wrong.inode = wrong.inode.checked_add(1).ok_or(FsError::Identity)?;
    assert!(!identity(scratch.file()?)?.same_node(&wrong));
    let file = posix::create_file(scratch.file()?, "negative-device")?;
    let id = identity(&file)?;
    assert_eq!(
        validate(&file, case.user, id.device ^ 1, false),
        Err(FsError::Identity)
    );
    assert_eq!(
        validate(&file, case.user ^ 1, id.device, false),
        Err(FsError::Identity)
    );
    close(file)?;
    remove_file(scratch.file()?, "negative-device", &id, case.user)?;
    case.check()?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PlannedDrift {
    SameBytesDatabaseReplacement,
    WalAppend,
    WalTruncate,
    AddShm,
    RemoveShm,
    SourceDetach,
    ScratchDetach,
}

/// One independent, deliberately destructive synthetic case. A successful
/// capture precedes the mutation, so rejection cannot be caused by an earlier
/// fault. No failed observation is repaired or reused. The fixture host tears
/// down only the precise original/new objects declared by this plan.
pub(crate) fn check_planned_drift(
    mut case: SyntheticCase,
    budget: &mut ObserverBudget,
    fault: PlannedDrift,
) -> Result<(), FsError> {
    assert!(
        case.registered.is_empty(),
        "planned case has no core material"
    );
    let family = case.baseline.clone().ok_or(FsError::Identity)?;
    let original_ids = case.sealed_ids.clone().ok_or(FsError::Identity)?;
    let original_hashes = case.generated_hashes.clone().ok_or(FsError::Identity)?;
    let source_fd = open_fixed_dot(case.source.file()?)?;
    let source_path = case.source.path.clone();
    let root_fd = open_fixed_dot(case.root.file()?)?;
    let root_path = case.root.path.clone();
    let device = case.root.id.device;
    let user = case.user;
    let sentinel = write_record(
        &root_fd,
        "planned-sentinel",
        user,
        device,
        "synthetic unrelated sentinel\n",
    )?;
    let sentinel_digest = sha2::Sha256::digest(b"synthetic unrelated sentinel\n");
    let mut expected_ids = original_ids.clone();
    let mut backups = BTreeMap::new();
    let mut source_leaf = "source";
    let mut scratch_leaf = "scratch";
    let mut replacement_directory = None;
    let mut copy = case.capture(&family, budget)?;
    copy.revalidate_source(budget)?;
    copy.verify_copy(budget)?;
    match fault {
        PlannedDrift::SameBytesDatabaseReplacement => {
            let original = original_ids[0].as_ref().ok_or(FsError::Identity)?;
            std::fs::rename(
                source_path.join(FAMILY[0]),
                root_path.join("planned-original-db"),
            )
            .map_err(|_| FsError::Io)?;
            let mut old = posix::open_read(&root_fd, "planned-original-db")?;
            let old_id = validate(&old, user, device, false)?;
            if !old_id.same_node(original)
                || old_id.size != original.size
                || old_id.modified != original.modified
            {
                return Err(FsError::Changed);
            }
            let mut replacement = posix::create_file(&source_fd, FAMILY[0])?;
            let mut buffer = [0_u8; CHUNK];
            loop {
                let count = old.read(&mut buffer).map_err(|_| FsError::Io)?;
                if count == 0 {
                    break;
                }
                replacement
                    .write_all(&buffer[..count])
                    .map_err(|_| FsError::Io)?;
            }
            let replacement_id = validate(&replacement, user, device, false)?;
            if replacement_id.inode == original.inode || replacement_id.size != original.size {
                return Err(FsError::Identity);
            }
            if identity(&old)? != old_id {
                return Err(FsError::Changed);
            }
            close(old)?;
            close(replacement)?;
            expected_ids[0] = Some(replacement_id);
            backups.insert("planned-original-db", old_id);
        }
        PlannedDrift::WalAppend | PlannedDrift::WalTruncate => {
            use std::io::{Seek, SeekFrom};
            let original = original_ids[1].as_ref().ok_or(FsError::Identity)?;
            let mut wal = posix::open_write_for_planned_fault(&source_fd, FAMILY[1])?;
            if validate(&wal, user, device, false)? != *original {
                return Err(FsError::Changed);
            }
            let expected_size = if matches!(fault, PlannedDrift::WalAppend) {
                wal.seek(SeekFrom::End(0)).map_err(|_| FsError::Io)?;
                wal.write_all(&[0x5a]).map_err(|_| FsError::Io)?;
                original.size.checked_add(1).ok_or(FsError::Capacity)?
            } else {
                let length = original.size.checked_sub(1).ok_or(FsError::Identity)?;
                wal.set_len(length).map_err(|_| FsError::Io)?;
                length
            };
            let changed = validate(&wal, user, device, false)?;
            if !changed.same_node(original) || changed.size != expected_size {
                return Err(FsError::Changed);
            }
            close(wal)?;
            expected_ids[1] = Some(changed);
        }
        PlannedDrift::AddShm => {
            if original_ids[2].is_some() {
                return Err(FsError::Identity);
            }
            let mut shm = posix::create_file(&source_fd, FAMILY[2])?;
            shm.write_all(b"synthetic index injection")
                .map_err(|_| FsError::Io)?;
            let created = validate(&shm, user, device, false)?;
            close(shm)?;
            expected_ids[2] = Some(created);
        }
        PlannedDrift::RemoveShm => {
            let original = original_ids[2].as_ref().ok_or(FsError::Identity)?;
            std::fs::rename(
                source_path.join(FAMILY[2]),
                root_path.join("planned-original-shm"),
            )
            .map_err(|_| FsError::Io)?;
            let old = posix::open_read(&root_fd, "planned-original-shm")?;
            let moved = validate(&old, user, device, false)?;
            close(old)?;
            if !moved.same_node(original)
                || moved.size != original.size
                || moved.modified != original.modified
            {
                return Err(FsError::Changed);
            }
            backups.insert("planned-original-shm", moved);
            expected_ids[2] = None;
        }
        PlannedDrift::SourceDetach | PlannedDrift::ScratchDetach => {
            let (original_name, moved_name) = if matches!(fault, PlannedDrift::SourceDetach) {
                ("source", "planned-source")
            } else {
                ("scratch", "planned-scratch")
            };
            std::fs::rename(root_path.join(original_name), root_path.join(moved_name))
                .map_err(|_| FsError::Io)?;
            let new_directory = Directory::new(
                posix::create_dir(&root_fd, original_name)?,
                user,
                device,
                root_path.join(original_name),
            )?;
            replacement_directory = Some((original_name, new_directory));
            if original_name == "source" {
                source_leaf = moved_name;
            } else {
                scratch_leaf = moved_name;
            }
        }
    }
    assert!(
        copy.revalidate_source(budget).is_err(),
        "{fault:?} must reject held observation"
    );
    let copied = copy.close_for_planned_fault()?;
    assert!(case.stopped.get());
    // A non-target sentinel is checked by both full identity and content. It
    // lives outside source/scratch and is never a mutation target.
    let mut witness = posix::open_read(&root_fd, "planned-sentinel")?;
    assert_eq!(validate(&witness, user, device, false)?, sentinel);
    let mut content = [0_u8; 64];
    let length = usize::try_from(sentinel.size).map_err(|_| FsError::Capacity)?;
    if length >= content.len() {
        return Err(FsError::Capacity);
    }
    witness
        .read_exact(&mut content[..length])
        .map_err(|_| FsError::Io)?;
    if witness
        .read(&mut content[length..length + 1])
        .map_err(|_| FsError::Io)?
        != 0
    {
        return Err(FsError::Changed);
    }
    assert_eq!(sha2::Sha256::digest(&content[..length]), sentinel_digest);
    close(witness)?;
    assert_eq!(
        identity_at(&root_fd, "planned-sentinel")?.as_ref(),
        Some(&sentinel)
    );
    // Independently establish that all non-target original family members
    // retain their complete identity/hash, even when their directory detached.
    let (observed_ids, observed_hashes) =
        capture::generated_source(&source_fd, user, device, budget)?;
    assert_eq!(observed_ids, expected_ids);
    for index in 0..4 {
        let changed = match fault {
            PlannedDrift::SameBytesDatabaseReplacement => index == 0,
            PlannedDrift::WalAppend | PlannedDrift::WalTruncate => index == 1,
            PlannedDrift::AddShm | PlannedDrift::RemoveShm => index == 2,
            PlannedDrift::SourceDetach | PlannedDrift::ScratchDetach => false,
        };
        if !changed {
            assert_eq!(observed_hashes[index], original_hashes[index]);
        }
    }
    if matches!(fault, PlannedDrift::SameBytesDatabaseReplacement) {
        assert_eq!(
            observed_hashes[0], original_hashes[0],
            "same bytes still must reject changed inode"
        );
    }
    close(source_fd)?;
    // Observer failure retained both directories. The explicitly declared
    // fixture plan now disposes only its closed, identity-checked objects.
    budget.begin_cleanup()?;
    let mut expected_root = ["case.json", "planned-sentinel", source_leaf, scratch_leaf]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    expected_root.extend(backups.keys().map(|name| (*name).to_owned()));
    if let Some((name, _)) = &replacement_directory {
        expected_root.insert((*name).to_owned());
    }
    assert_eq!(names(&root_fd)?, expected_root);
    let source_entries = FAMILY
        .into_iter()
        .zip(&expected_ids)
        .filter_map(|(name, id)| id.clone().map(|id| (name.to_owned(), id)))
        .collect::<BTreeMap<_, _>>();
    teardown_planned_directory(
        &mut case.source,
        &root_fd,
        source_leaf,
        &source_entries,
        user,
    )?;
    teardown_planned_directory(
        case.scratch.as_mut().ok_or(FsError::ScratchRetained)?,
        &root_fd,
        scratch_leaf,
        &copied,
        user,
    )?;
    case.scratch = None;
    if let Some((name, mut directory)) = replacement_directory {
        teardown_planned_directory(&mut directory, &root_fd, name, &BTreeMap::new(), user)?;
    }
    for (name, id) in backups {
        remove_file(&root_fd, name, &id, user)?;
    }
    remove_file(&root_fd, "planned-sentinel", &sentinel, user)?;
    expect_names(&root_fd, &["case.json"])?;
    remove_file(&root_fd, "case.json", &case.record, user)?;
    case.root.check(&case.parent, &case.component, user)?;
    close(root_fd)?;
    case.root.close()?;
    posix::unlink(&case.parent, &case.component, true)?;
    close(case.parent)?;
    case.active.set(false);
    Ok(())
}

fn teardown_planned_directory(
    directory: &mut Directory,
    parent: &File,
    leaf: &str,
    entries: &BTreeMap<String, Identity>,
    user: u32,
) -> Result<(), FsError> {
    directory.check(parent, leaf, user)?;
    if names(directory.file()?)? != entries.keys().cloned().collect() {
        return Err(FsError::UnknownEntry);
    }
    for (name, id) in entries {
        remove_file(directory.file()?, name, id, user)?;
    }
    expect_names(directory.file()?, &[])?;
    directory.check(parent, leaf, user)?;
    directory.close()?;
    posix::unlink(parent, leaf, true)
}

#[test]
fn public_ancestor_owner_mode_and_sticky_rules_fail_closed() {
    let directory = Identity {
        device: 1,
        inode: 2,
        owner: 0,
        mode: libc::S_IFDIR as u32 | 0o755,
        links: 2,
        size: 0,
        modified: (0, 0),
        changed: (0, 0),
    };
    assert_eq!(validate_public_ancestor_identity(&directory, 0), Ok(()));
    assert_eq!(validate_public_ancestor_identity(&directory, 1), Ok(()));
    for mode in [0o775, 0o777, 0o1755, 0o4755] {
        let mut changed = directory.clone();
        changed.mode = libc::S_IFDIR as u32 | mode;
        assert_eq!(
            validate_public_ancestor_identity(&changed, 0),
            Err(FsError::Identity)
        );
    }
    let mut temporary = directory.clone();
    temporary.mode = libc::S_IFDIR as u32 | 0o1777;
    assert_eq!(validate_public_ancestor_identity(&temporary, 2), Ok(()));
    for mode in [0o777, 0o1700, 0o3777, 0o4777] {
        let mut changed = temporary.clone();
        changed.mode = libc::S_IFDIR as u32 | mode;
        assert_eq!(
            validate_public_ancestor_identity(&changed, 2),
            Err(FsError::Identity)
        );
    }
    temporary.owner = 501;
    assert_eq!(
        validate_public_ancestor_identity(&temporary, 2),
        Err(FsError::Identity)
    );
    let mut wrong_owner = directory.clone();
    wrong_owner.owner = 501;
    assert_eq!(
        validate_public_ancestor_identity(&wrong_owner, 0),
        Err(FsError::Identity)
    );
    let mut not_directory = directory;
    not_directory.mode = libc::S_IFREG as u32 | 0o755;
    assert_eq!(
        validate_public_ancestor_identity(&not_directory, 0),
        Err(FsError::Identity)
    );
}

#[test]
fn retained_batch_and_scratch_limits_include_material_and_controls() {
    let source = 256 * MIB;
    let scratch = MAX_SCRATCH;
    let material = MAX_BATCH - source - scratch - CONTROL_RECORD_RESERVE;
    assert_eq!(check_retained_totals(source, material, scratch), Ok(()));
    assert_eq!(
        check_retained_totals(source, material + 1, scratch),
        Err(FsError::Capacity)
    );
    assert_eq!(check_retained_totals(0, 0, MAX_SCRATCH), Ok(()));
    assert_eq!(
        check_retained_totals(0, 0, MAX_SCRATCH + 1),
        Err(FsError::Capacity)
    );
    assert_eq!(
        check_retained_totals(u64::MAX, 1, 0),
        Err(FsError::Capacity)
    );
    assert_eq!(
        check_retained_totals(MAX_BATCH - CONTROL_RECORD_RESERVE, 0, 0),
        Ok(())
    );
    assert_eq!(
        check_retained_totals(MAX_BATCH, 0, 0),
        Err(FsError::Capacity)
    );
}

#[test]
fn generated_index_identity_can_refresh_only_within_the_created_node() {
    let original = Identity {
        device: 1,
        inode: 2,
        owner: 501,
        mode: libc::S_IFREG as u32 | 0o600,
        links: 1,
        size: 32,
        modified: (1, 2),
        changed: (1, 2),
    };
    let mut live_index = original.clone();
    live_index.size = 32768;
    live_index.modified = (3, 4);
    live_index.changed = (3, 4);
    assert!(live_index.same_node(&original));
    // Exact disposal still rejects the pre-close snapshot. Only an explicit
    // later verification can register the final index metadata for removal.
    assert_ne!(live_index, original);
    let mut replacement = live_index.clone();
    replacement.inode += 1;
    assert!(!replacement.same_node(&original));
    replacement = live_index.clone();
    replacement.device += 1;
    assert!(!replacement.same_node(&original));
    replacement = live_index.clone();
    replacement.links += 1;
    assert!(!replacement.same_node(&original));
    replacement = live_index;
    replacement.mode |= 0o020;
    assert!(!replacement.same_node(&original));
}

#[test]
fn observed_sqlite_index_cannot_disappear_or_be_replaced() {
    let created = Identity {
        device: 1,
        inode: 2,
        owner: 501,
        mode: libc::S_IFREG as u32 | 0o600,
        links: 1,
        size: 32,
        modified: (1, 2),
        changed: (1, 2),
    };
    assert_eq!(capture::validate_generated_index(None, None, false), Ok(()));
    assert_eq!(
        capture::validate_generated_index(None, Some(&created), false),
        Err(FsError::UnknownEntry)
    );
    assert_eq!(
        capture::validate_generated_index(None, Some(&created), true),
        Ok(())
    );
    assert_eq!(
        capture::validate_generated_index(Some(&created), None, true),
        Err(FsError::Changed)
    );
    let mut closed = created.clone();
    closed.modified = (3, 4);
    closed.changed = (3, 4);
    assert_eq!(
        capture::validate_generated_index(Some(&created), Some(&closed), true),
        Ok(())
    );
    let mut replacement = closed;
    replacement.inode += 1;
    assert_eq!(
        capture::validate_generated_index(Some(&created), Some(&replacement), true),
        Err(FsError::Changed)
    );
}
