//! Descriptor-relative, read-only evidence access. This private module is the only
//! qualification-reader boundary that uses POSIX unsafe operations.
//!
//! Every returned descriptor is owned by a `File` or `DirectoryStream`; raw names
//! come from checked ASCII components. No pathname operation follows a symlink.

use super::{ReadControl, ReadError};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::Read;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

// The manifest lists at most 4096 attachments. Inventory also includes the
// manifest itself; the parent reader verifies that the listed set is closed.
const MAX_INVENTORY_FILES: usize = 4097;
const MAX_NODES: usize = 8192;
const MAX_PATH: usize = 180;
const CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EntryIdentity {
    pub(super) kind: EntryKind,
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) owner: u32,
    pub(super) group: u32,
    pub(super) mode: u32,
    pub(super) links: u64,
    pub(super) size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

pub(super) struct FileRead {
    pub(super) identity: EntryIdentity,
    pub(super) sha256: [u8; 32],
    pub(super) bytes: Option<Vec<u8>>,
}

struct Ancestor {
    descriptor: File,
    identity: EntryIdentity,
    name: Option<CString>,
    absolute: String,
}

pub(super) struct EvidenceRoot {
    // Includes '/' and every component through the evidence root. Each `name`
    // is bound relative to the preceding descriptor, never the process cwd.
    ancestors: Vec<Ancestor>,
    entries: BTreeMap<String, EntryIdentity>,
    user: u32,
}

pub(super) fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.starts_with('.')
                && !component.ends_with('.')
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
}

impl EvidenceRoot {
    pub(super) fn open(path: &Path, control: &ReadControl) -> Result<Self, ReadError> {
        control.check()?;
        let raw = path.as_os_str().as_bytes();
        // Check raw spelling: Path::components would erase '.' and repeated '/'.
        if raw.first() != Some(&b'/') || raw.len() < 2 || raw.ends_with(b"/") {
            return Err(ReadError::Malformed);
        }
        let parts = raw[1..].split(|byte| *byte == b'/').collect::<Vec<_>>();
        if parts
            .iter()
            .any(|part| part.is_empty() || *part == b"." || *part == b".." || part.contains(&0))
            || parts.len() > 256
        {
            return Err(ReadError::Malformed);
        }
        // SAFETY: geteuid has no pointer or ownership preconditions.
        let user = unsafe { libc::geteuid() };
        let slash = CString::new("/").map_err(|_| ReadError::Malformed)?;
        // SAFETY: slash is NUL terminated and flags require no variadic mode.
        let root_fd = unsafe { libc::open(slash.as_ptr(), directory_flags()) };
        let root = owned_file(root_fd)?;
        let root_identity = identity(&root)?;
        validate_ancestor(&root, &root_identity, "/", user)?;
        let mut ancestors = vec![Ancestor {
            descriptor: root,
            identity: root_identity,
            name: None,
            absolute: "/".to_owned(),
        }];
        let mut absolute = String::new();
        for (index, part) in parts.iter().enumerate() {
            control.check()?;
            let name = CString::new(*part).map_err(|_| ReadError::Malformed)?;
            let parent = &ancestors.last().ok_or(ReadError::IoFailure)?.descriptor;
            let before = identity_at(parent.as_raw_fd(), &name)?;
            if before.kind != EntryKind::Directory {
                return Err(ReadError::IdentityMismatch);
            }
            let descriptor = open_at(parent, &name, directory_flags())?;
            let observed = identity(&descriptor)?;
            if before != observed {
                return Err(ReadError::EvidenceChanged);
            }
            // Physical absolute paths need not be ASCII outside the package.
            absolute.push('/');
            absolute.push_str(std::str::from_utf8(part).map_err(|_| ReadError::Malformed)?);
            if index + 1 == parts.len() {
                validate_private(&descriptor, &observed, user, observed.device)?;
            } else {
                validate_ancestor(&descriptor, &observed, &absolute, user)?;
            }
            ancestors.push(Ancestor {
                descriptor,
                identity: observed,
                name: Some(name),
                absolute: absolute.clone(),
            });
        }
        let mut result = Self {
            ancestors,
            entries: BTreeMap::new(),
            user,
        };
        result.entries = result.scan(control)?;
        result.check_ancestors(control)?;
        result.check_physical_path(path)?;
        Ok(result)
    }

    pub(super) fn inventory(&self) -> &BTreeMap<String, EntryIdentity> {
        &self.entries
    }

    pub(super) fn read_file(
        &self,
        relative: &str,
        max_bytes: usize,
        retain: bool,
        control: &ReadControl,
    ) -> Result<FileRead, ReadError> {
        self.read_file_observed(relative, max_bytes, retain, control, || Ok(()))
    }

    fn read_file_observed(
        &self,
        relative: &str,
        max_bytes: usize,
        retain: bool,
        control: &ReadControl,
        mut after_chunk: impl FnMut() -> Result<(), ReadError>,
    ) -> Result<FileRead, ReadError> {
        control.check()?;
        if !valid_relative_path(relative) {
            return Err(ReadError::Malformed);
        }
        let expected = self
            .entries
            .get(relative)
            .ok_or(ReadError::MissingEvidence)?;
        if expected.kind != EntryKind::File {
            return Err(ReadError::IdentityMismatch);
        }
        if expected.size > max_bytes as u64 {
            return Err(ReadError::ResourceLimit);
        }
        let (mut file, parent, name) = self.open_recorded(relative, control)?;
        let before = identity(&file)?;
        self.validate_recorded(&file, &before, expected)?;
        let mut digest = Sha256::new();
        let mut bytes = retain.then(Vec::new);
        let mut length = 0usize;
        let mut buffer = [0u8; CHUNK];
        loop {
            control.check()?;
            let count = file.read(&mut buffer).map_err(|_| ReadError::IoFailure)?;
            after_chunk()?;
            control.check()?;
            if count == 0 {
                break;
            }
            length = length.checked_add(count).ok_or(ReadError::ResourceLimit)?;
            if length > max_bytes || length as u64 > expected.size {
                return Err(ReadError::ResourceLimit);
            }
            digest.update(&buffer[..count]);
            if let Some(retained) = &mut bytes {
                retained.extend_from_slice(&buffer[..count]);
            }
        }
        if length as u64 != expected.size
            || identity(&file)? != before
            || identity_at(parent.as_raw_fd(), &name)? != before
        {
            return Err(ReadError::EvidenceChanged);
        }
        validate_private(&file, &before, self.user, self.root_identity().device)?;
        self.check_ancestors(control)?;
        Ok(FileRead {
            identity: before,
            sha256: digest.finalize().into(),
            bytes,
        })
    }

    pub(super) fn verify_unchanged(&self, control: &ReadControl) -> Result<(), ReadError> {
        self.check_ancestors(control)?;
        if self.scan(control)? != self.entries {
            return Err(ReadError::EvidenceChanged);
        }
        self.check_ancestors(control)
    }

    fn root(&self) -> &File {
        &self.ancestors[self.ancestors.len() - 1].descriptor
    }

    fn root_identity(&self) -> &EntryIdentity {
        &self.ancestors[self.ancestors.len() - 1].identity
    }

    fn check_physical_path(&self, path: &Path) -> Result<(), ReadError> {
        #[cfg(target_os = "macos")]
        {
            let mut buffer = [0 as libc::c_char; libc::PATH_MAX as usize];
            // SAFETY: F_GETPATH writes at most PATH_MAX bytes into this live
            // buffer and the descriptor remains owned throughout the call.
            if unsafe {
                libc::fcntl(
                    self.root().as_raw_fd(),
                    libc::F_GETPATH,
                    buffer.as_mut_ptr(),
                )
            } != 0
            {
                return Err(ReadError::IoFailure);
            }
            // SAFETY: successful Darwin F_GETPATH returns a terminated path.
            if unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_bytes() != path.as_os_str().as_bytes()
            {
                return Err(ReadError::IdentityMismatch);
            }
        }
        #[cfg(target_os = "linux")]
        let _ = path;
        Ok(())
    }

    fn validate_recorded(
        &self,
        descriptor: &File,
        actual: &EntryIdentity,
        expected: &EntryIdentity,
    ) -> Result<(), ReadError> {
        if actual != expected {
            return Err(ReadError::EvidenceChanged);
        }
        validate_private(descriptor, actual, self.user, self.root_identity().device)
    }

    fn check_ancestors(&self, control: &ReadControl) -> Result<(), ReadError> {
        for (index, ancestor) in self.ancestors.iter().enumerate() {
            control.check()?;
            let current = identity(&ancestor.descriptor)?;
            let is_root = index + 1 == self.ancestors.len();
            // Unrelated children may change a shared system ancestor's times.
            // Its identity, access policy, and binding must still be unchanged.
            if !same_binding(&current, &ancestor.identity)
                || (is_root && current != ancestor.identity)
            {
                return Err(ReadError::EvidenceChanged);
            }
            if let Some(name) = &ancestor.name {
                let parent = &self.ancestors[index - 1].descriptor;
                let bound = identity_at(parent.as_raw_fd(), name)?;
                if !same_binding(&bound, &current) || (is_root && bound != current) {
                    return Err(ReadError::EvidenceChanged);
                }
            }
            if is_root {
                validate_private(&ancestor.descriptor, &current, self.user, current.device)?;
            } else {
                validate_ancestor(
                    &ancestor.descriptor,
                    &current,
                    &ancestor.absolute,
                    self.user,
                )?;
            }
        }
        Ok(())
    }

    fn scan(&self, control: &ReadControl) -> Result<BTreeMap<String, EntryIdentity>, ReadError> {
        let mut entries = BTreeMap::new();
        let mut folded = BTreeSet::new();
        let mut files = 0usize;
        self.scan_directory(
            self.root(),
            "",
            &mut entries,
            &mut folded,
            &mut files,
            control,
        )?;
        Ok(entries)
    }

    fn scan_directory(
        &self,
        directory: &File,
        prefix: &str,
        entries: &mut BTreeMap<String, EntryIdentity>,
        folded: &mut BTreeSet<String>,
        files: &mut usize,
        control: &ReadControl,
    ) -> Result<(), ReadError> {
        control.check()?;
        let before = identity(directory)?;
        validate_private(directory, &before, self.user, self.root_identity().device)?;
        let stream = DirectoryStream::open(directory)?;
        let mut names = BTreeSet::new();
        loop {
            control.check()?;
            let Some(name) = stream.next_name()? else {
                break;
            };
            if name == b"." || name == b".." {
                continue;
            }
            let component = std::str::from_utf8(&name).map_err(|_| ReadError::Malformed)?;
            let relative = if prefix.is_empty() {
                component.to_owned()
            } else {
                format!("{prefix}/{component}")
            };
            if !valid_relative_path(&relative) || !names.insert(name.clone()) {
                return Err(ReadError::Malformed);
            }
            if !folded.insert(relative.to_ascii_lowercase()) {
                return Err(ReadError::IdentityMismatch);
            }
            if entries.len() >= MAX_NODES {
                return Err(ReadError::ResourceLimit);
            }
            let name = CString::new(name).map_err(|_| ReadError::Malformed)?;
            let expected = identity_at(directory.as_raw_fd(), &name)?;
            let flags = match expected.kind {
                EntryKind::Directory => directory_flags(),
                EntryKind::File => file_flags(),
            };
            let child = open_at(directory, &name, flags)?;
            let actual = identity(&child)?;
            self.validate_recorded(&child, &actual, &expected)?;
            if actual.kind == EntryKind::File {
                *files += 1;
                if *files > MAX_INVENTORY_FILES {
                    return Err(ReadError::ResourceLimit);
                }
            }
            if entries.insert(relative.clone(), actual.clone()).is_some() {
                return Err(ReadError::Malformed);
            }
            if actual.kind == EntryKind::Directory {
                self.scan_directory(&child, &relative, entries, folded, files, control)?;
            }
            if identity(&child)? != actual || identity_at(directory.as_raw_fd(), &name)? != actual {
                return Err(ReadError::EvidenceChanged);
            }
        }
        if identity(directory)? != before {
            return Err(ReadError::EvidenceChanged);
        }
        Ok(())
    }

    fn open_recorded(
        &self,
        relative: &str,
        control: &ReadControl,
    ) -> Result<(File, File, CString), ReadError> {
        let dot = CString::new(".").map_err(|_| ReadError::Malformed)?;
        let mut parent = open_at(self.root(), &dot, directory_flags())?;
        self.validate_recorded(&parent, &identity(&parent)?, self.root_identity())?;
        let parts = relative.split('/').collect::<Vec<_>>();
        let mut prefix = String::new();
        for (index, part) in parts.iter().enumerate() {
            control.check()?;
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let name = CString::new(*part).map_err(|_| ReadError::Malformed)?;
            let expected = self
                .entries
                .get(&prefix)
                .ok_or(ReadError::MissingEvidence)?;
            if identity_at(parent.as_raw_fd(), &name)? != *expected {
                return Err(ReadError::EvidenceChanged);
            }
            let final_part = index + 1 == parts.len();
            let child = open_at(
                &parent,
                &name,
                if final_part {
                    file_flags()
                } else {
                    directory_flags()
                },
            )?;
            self.validate_recorded(&child, &identity(&child)?, expected)?;
            if final_part {
                return Ok((child, parent, name));
            }
            parent = child;
        }
        Err(ReadError::Malformed)
    }
}

fn same_binding(left: &EntryIdentity, right: &EntryIdentity) -> bool {
    left.kind == right.kind
        && left.device == right.device
        && left.inode == right.inode
        && left.owner == right.owner
        && left.group == right.group
        && left.mode == right.mode
}

fn directory_flags() -> libc::c_int {
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK
}

fn file_flags() -> libc::c_int {
    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK
}

fn owned_file(fd: RawFd) -> Result<File, ReadError> {
    if fd < 0 {
        return Err(last_io_error());
    }
    // SAFETY: successful open/openat returned a fresh owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn open_at(parent: &File, name: &CString, flags: libc::c_int) -> Result<File, ReadError> {
    // SAFETY: parent stays alive; name is NUL terminated; O_CREAT is never used.
    owned_file(unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) })
}

fn last_io_error() -> ReadError {
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ENOENT) => ReadError::MissingEvidence,
        Some(libc::ELOOP | libc::ENOTDIR) => ReadError::IdentityMismatch,
        _ => ReadError::IoFailure,
    }
}

fn identity(file: &File) -> Result<EntryIdentity, ReadError> {
    let mut observed = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat writes a complete stat on success; descriptor is borrowed.
    if unsafe { libc::fstat(file.as_raw_fd(), observed.as_mut_ptr()) } != 0 {
        return Err(last_io_error());
    }
    // SAFETY: successful fstat initialized the complete structure.
    from_stat(unsafe { observed.assume_init() })
}

fn identity_at(parent: RawFd, name: &CString) -> Result<EntryIdentity, ReadError> {
    let mut observed = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: parent is live, name is terminated, output storage is valid. The
    // NOFOLLOW flag observes the binding itself, including a hostile symlink.
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            observed.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(last_io_error());
    }
    // SAFETY: successful fstatat initialized the complete structure.
    from_stat(unsafe { observed.assume_init() })
}

fn from_stat(stat: libc::stat) -> Result<EntryIdentity, ReadError> {
    let kind = match stat.st_mode & libc::S_IFMT {
        libc::S_IFREG => EntryKind::File,
        libc::S_IFDIR => EntryKind::Directory,
        _ => return Err(ReadError::IdentityMismatch),
    };
    if stat.st_size < 0 {
        return Err(ReadError::Malformed);
    }
    // libc field widths vary by OS and architecture; normalize only here.
    #[allow(clippy::unnecessary_cast)]
    let (mode, device, links) = (
        stat.st_mode as u32,
        stat.st_dev as u64,
        stat.st_nlink as u64,
    );
    #[cfg(target_os = "macos")]
    let (modified, changed) = (
        (stat.st_mtime, stat.st_mtime_nsec),
        (stat.st_ctime, stat.st_ctime_nsec),
    );
    #[cfg(target_os = "linux")]
    let (modified, changed) = (
        (stat.st_mtime, stat.st_mtime_nsec),
        (stat.st_ctime, stat.st_ctime_nsec),
    );
    Ok(EntryIdentity {
        kind,
        device,
        inode: stat.st_ino,
        owner: stat.st_uid,
        group: stat.st_gid,
        mode,
        links,
        size: stat.st_size as u64,
        modified,
        changed,
    })
}

fn validate_private(
    file: &File,
    stat: &EntryIdentity,
    user: u32,
    device: u64,
) -> Result<(), ReadError> {
    let required = match stat.kind {
        EntryKind::Directory => 0o700,
        EntryKind::File => 0o600,
    };
    if stat.owner != user
        || stat.device != device
        || stat.mode & 0o7777 != required
        || (stat.kind == EntryKind::File && stat.links != 1)
    {
        return Err(ReadError::IdentityMismatch);
    }
    reject_acl(file)
}

fn validate_ancestor(
    file: &File,
    stat: &EntryIdentity,
    path: &str,
    user: u32,
) -> Result<(), ReadError> {
    let sticky_temporary = stat.owner == 0
        && stat.mode & 0o1000 != 0
        && matches!(
            path,
            "/tmp" | "/var/tmp" | "/private/tmp" | "/private/var/tmp"
        );
    if stat.kind != EntryKind::Directory
        || !(stat.owner == 0 || stat.owner == user)
        || (stat.mode & 0o022 != 0 && !sticky_temporary)
    {
        return Err(ReadError::IdentityMismatch);
    }
    reject_acl(file)
}

struct DirectoryStream(*mut libc::DIR);

impl DirectoryStream {
    fn open(file: &File) -> Result<Self, ReadError> {
        let dot = CString::new(".").map_err(|_| ReadError::Malformed)?;
        // A fresh open description avoids sharing directory cursor state with
        // held descriptors (dup would share offsets and break repeated checks).
        let descriptor = open_at(file, &dot, directory_flags())?;
        use std::os::fd::IntoRawFd;
        let fd = descriptor.into_raw_fd();
        // SAFETY: fd is a fresh owned directory; ownership transfers on success.
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            let error = last_io_error();
            // SAFETY: fdopendir failed and left ownership with this scope.
            unsafe { libc::close(fd) };
            return Err(error);
        }
        Ok(Self(stream))
    }

    fn next_name(&self) -> Result<Option<Vec<u8>>, ReadError> {
        // SAFETY: errno points to this thread's errno storage.
        unsafe { *errno_location() = 0 };
        // SAFETY: stream is owned, alive, and exclusively used by this thread.
        let entry = unsafe { libc::readdir(self.0) };
        if entry.is_null() {
            // SAFETY: errno is thread-local live storage.
            return if unsafe { *errno_location() } == 0 {
                Ok(None)
            } else {
                Err(ReadError::IoFailure)
            };
        }
        // SAFETY: readdir guarantees a terminated d_name valid until the next
        // call on this stream; copy it before that next call.
        Ok(Some(
            unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_bytes()
                .to_vec(),
        ))
    }
}

impl Drop for DirectoryStream {
    fn drop(&mut self) {
        // SAFETY: this is the unique successful fdopendir owner.
        unsafe { libc::closedir(self.0) };
    }
}

#[cfg(target_os = "macos")]
unsafe fn errno_location() -> *mut libc::c_int {
    // SAFETY: libc exposes the current thread's errno address.
    unsafe { libc::__error() }
}

#[cfg(target_os = "linux")]
unsafe fn errno_location() -> *mut libc::c_int {
    // SAFETY: libc exposes the current thread's errno address.
    unsafe { libc::__errno_location() }
}

#[cfg(target_os = "macos")]
fn reject_acl(file: &File) -> Result<(), ReadError> {
    use std::ffi::c_void;
    extern "C" {
        fn acl_get_fd(fd: libc::c_int) -> *mut c_void;
        fn acl_get_entry(
            acl: *mut c_void,
            entry_id: libc::c_int,
            entry: *mut *mut c_void,
        ) -> libc::c_int;
        fn acl_valid(acl: *mut c_void) -> libc::c_int;
        fn acl_free(object: *mut c_void) -> libc::c_int;
    }
    let before = identity(file)?;
    // SAFETY: the descriptor is live; the returned ACL is owned on success.
    let acl = unsafe { acl_get_fd(file.as_raw_fd()) };
    if acl.is_null() {
        // Darwin acl_get_fd uses fstatx_np + FILESEC_ACL. A live descriptor
        // without that property returns ENOENT, unlike an unknown ACL query
        // failure. Recheck its identity; no path lookup or fallback is used.
        let absent = std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT);
        if absent && identity(file)? == before {
            return Ok(());
        }
        return Err(ReadError::IoFailure);
    }
    // SAFETY: acl_get_fd returned a live ACL; validation disambiguates an
    // invalid ACL from the empty valid ACL's EINVAL end-of-iteration result.
    let valid = unsafe { acl_valid(acl) } == 0;
    let mut entry = std::ptr::null_mut();
    // SAFETY: ACL and entry output are valid. Darwin ACL_FIRST_ENTRY is 0;
    // Darwin returns 0 for an entry and -1/EINVAL for an empty ACL.
    unsafe { *errno_location() = 0 };
    let result = unsafe { acl_get_entry(acl, 0, &mut entry) };
    let error = std::io::Error::last_os_error().raw_os_error();
    // SAFETY: acl_get_fd returned this uniquely owned ACL.
    let freed = unsafe { acl_free(acl) };
    if freed != 0 {
        return Err(ReadError::IoFailure);
    }
    if valid && result == -1 && error == Some(libc::EINVAL) && identity(file)? == before {
        return Ok(());
    }
    if result == 0 {
        return Err(ReadError::IdentityMismatch);
    }
    Err(ReadError::IoFailure)
}

#[cfg(target_os = "linux")]
fn reject_acl(file: &File) -> Result<(), ReadError> {
    for name in [
        b"system.posix_acl_access\0".as_slice(),
        b"system.posix_acl_default\0".as_slice(),
    ] {
        // SAFETY: the attribute name is terminated; a null buffer and size zero
        // ask for length only and never read or alter attribute data.
        let size = unsafe {
            libc::fgetxattr(
                file.as_raw_fd(),
                name.as_ptr().cast(),
                std::ptr::null_mut(),
                0,
            )
        };
        if size >= 0 {
            return Err(ReadError::IdentityMismatch);
        }
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ENODATA) | Some(libc::EOPNOTSUPP) => {}
            _ => return Err(ReadError::IoFailure),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "qualification_reader_fs_tests.rs"]
mod tests;
