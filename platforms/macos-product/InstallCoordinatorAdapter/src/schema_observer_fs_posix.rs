//! Minimal, descriptor-relative POSIX boundary for the synthetic observer.
//! No existing object is repaired and no cleanup traverses an unowned tree.

use super::FsError;
use std::collections::BTreeSet;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};

/// Single-threaded, deterministic synthetic failure injection. It cannot change
/// an input path, install a process hook, or affect another test thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FsOperation {
    Close,
    Unlink,
    Rmdir,
}
thread_local! { static FAULT: std::cell::Cell<Option<(FsOperation, usize)>> = const { std::cell::Cell::new(None) }; }
pub(crate) fn inject_fault(operation: FsOperation, successful_calls: usize) {
    FAULT.with(|fault| fault.set(Some((operation, successful_calls))));
}
pub(crate) fn clear_fault() {
    FAULT.with(|fault| fault.set(None));
}
fn fail(operation: FsOperation) -> bool {
    FAULT.with(|fault| match fault.get() {
        Some((wanted, remaining)) if wanted == operation => {
            if remaining == 0 {
                fault.set(None);
                true
            } else {
                fault.set(Some((wanted, remaining - 1)));
                false
            }
        }
        _ => false,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub device: u64,
    pub inode: u64,
    pub owner: u32,
    pub mode: u32,
    pub links: u64,
    pub size: u64,
    pub modified: (i64, i64),
    pub changed: (i64, i64),
}
impl Identity {
    pub fn same_node(&self, other: &Self) -> bool {
        self.device == other.device
            && self.inode == other.inode
            && self.owner == other.owner
            && self.mode == other.mode
            && (self.directory() || self.links == other.links)
    }
    pub fn directory(&self) -> bool {
        self.mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32
    }
}

pub(super) fn name(value: &str) -> Result<CString, FsError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(FsError::Identity);
    }
    CString::new(value).map_err(|_| FsError::Identity)
}
fn owned(fd: i32) -> Result<File, FsError> {
    if fd < 0 {
        return Err(last_error());
    }
    // SAFETY: successful open/openat returns a fresh owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
pub(super) fn last_error() -> FsError {
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ENOENT) => FsError::Missing,
        Some(libc::EEXIST) => FsError::ScratchOccupied,
        _ => FsError::Io,
    }
}
pub(super) fn close(file: File) -> Result<(), FsError> {
    let injected = fail(FsOperation::Close);
    let fd = file.into_raw_fd();
    // SAFETY: ownership was removed from File and close is called exactly once.
    // EINTR is never retried: descriptor state is uncertain, so fail closed.
    let result = unsafe { libc::close(fd) };
    if injected {
        // Fixture teardown is authorized only for the planned synthetic error.
        // A coincident real close failure must instead stop the test and retain
        // its objects; it cannot be misclassified as confirmed disposal.
        assert_eq!(
            result, 0,
            "real close failed before planned fault injection"
        );
    }
    if result != 0 || injected {
        return Err(FsError::Close);
    }
    Ok(())
}
pub(super) fn open_slash() -> Result<File, FsError> {
    // SAFETY: literal is terminated, flags use no variadic mode argument.
    owned(unsafe { libc::open(c"/".as_ptr(), directory_flags()) })
}
fn directory_flags() -> i32 {
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
}
pub(super) fn open_dir(parent: &File, component: &str) -> Result<File, FsError> {
    let component = name(component)?;
    // SAFETY: borrowed parent and terminated single component remain live.
    owned(unsafe { libc::openat(parent.as_raw_fd(), component.as_ptr(), directory_flags()) })
}
pub(super) fn open_read(parent: &File, component: &str) -> Result<File, FsError> {
    let component = name(component)?;
    // O_NONBLOCK prevents a replaced FIFO/device from blocking before fstat.
    // SAFETY: parent and terminated single component are valid; no creation.
    owned(unsafe {
        libc::openat(
            parent.as_raw_fd(),
            component.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    })
}
pub(super) fn open_write_for_planned_fault(
    parent: &File,
    component: &str,
) -> Result<File, FsError> {
    let component = name(component)?;
    // SAFETY: fixed fixture-host component, live parent, no creation/following.
    owned(unsafe {
        libc::openat(
            parent.as_raw_fd(),
            component.as_ptr(),
            libc::O_WRONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    })
}
pub(super) fn create_file(parent: &File, component: &str) -> Result<File, FsError> {
    let component = name(component)?;
    // SAFETY: mode is provided for O_CREAT; O_EXCL refuses all existing entries.
    owned(unsafe {
        libc::openat(
            parent.as_raw_fd(),
            component.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600 as libc::c_uint,
        )
    })
}
pub(super) fn create_dir(parent: &File, component: &str) -> Result<File, FsError> {
    let component_c = name(component)?;
    // SAFETY: live parent, checked component, fixed private mode. No umask change.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), component_c.as_ptr(), 0o700) } != 0 {
        return Err(last_error());
    }
    open_dir(parent, component)
}
fn stat_value(value: libc::stat) -> Result<Identity, FsError> {
    if value.st_size < 0 {
        return Err(FsError::Identity);
    }
    Ok(Identity {
        device: value.st_dev as u64,
        inode: value.st_ino,
        owner: value.st_uid,
        mode: value.st_mode as u32,
        links: value.st_nlink as u64,
        size: value.st_size as u64,
        modified: (value.st_mtime, value.st_mtime_nsec),
        changed: (value.st_ctime, value.st_ctime_nsec),
    })
}
pub(super) fn identity(file: &File) -> Result<Identity, FsError> {
    let mut value = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat initializes valid output on success, descriptor is borrowed.
    if unsafe { libc::fstat(file.as_raw_fd(), value.as_mut_ptr()) } != 0 {
        return Err(last_error());
    }
    // SAFETY: successful fstat initialized the complete value.
    stat_value(unsafe { value.assume_init() })
}
pub(super) fn identity_at(parent: &File, component: &str) -> Result<Option<Identity>, FsError> {
    let component = name(component)?;
    let mut value = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: live descriptor, checked name and initialized storage; no symlink follow.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            component.as_ptr(),
            value.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return match last_error() {
            FsError::Missing => Ok(None),
            error => Err(error),
        };
    }
    // SAFETY: successful fstatat initialized the complete value.
    stat_value(unsafe { value.assume_init() }).map(Some)
}
pub(super) fn validate(
    file: &File,
    user: u32,
    device: u64,
    directory: bool,
) -> Result<Identity, FsError> {
    let id = identity(file)?;
    let expected = if directory {
        libc::S_IFDIR as u32 | 0o700
    } else {
        libc::S_IFREG as u32 | 0o600
    };
    if id.device != device
        || id.owner != user
        || id.mode != expected
        || (!directory && id.links != 1)
        || id.inode == 0
    {
        return Err(FsError::Identity);
    }
    reject_acl(file)?;
    Ok(id)
}
pub(super) fn reject_acl(file: &File) -> Result<(), FsError> {
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
    // SAFETY: acl_get_fd borrows a valid descriptor and returns an owned ACL.
    let acl = unsafe { acl_get_fd(file.as_raw_fd()) };
    if acl.is_null() {
        if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT)
            && identity(file)? == before
        {
            return Ok(());
        }
        return Err(FsError::Io);
    }
    // SAFETY: ACL is live; validation and lookup borrow it before freeing once.
    let valid = unsafe { acl_valid(acl) } == 0;
    let mut entry = std::ptr::null_mut();
    // SAFETY: errno is thread-local; entry points to valid output storage.
    unsafe {
        *libc::__error() = 0;
    }
    // SAFETY: ACL remains live and Darwin ACL_FIRST_ENTRY is zero.
    let result = unsafe { acl_get_entry(acl, 0, &mut entry) };
    let error = std::io::Error::last_os_error().raw_os_error();
    // SAFETY: uniquely owned ACL is released exactly once, result is checked.
    let freed = unsafe { acl_free(acl) };
    if freed != 0 {
        return Err(FsError::Close);
    }
    if valid && result == -1 && error == Some(libc::EINVAL) && identity(file)? == before {
        return Ok(());
    }
    Err(FsError::Identity)
}
pub(super) fn names(file: &File) -> Result<BTreeSet<String>, FsError> {
    let descriptor = {
        // The public component validator deliberately rejects dot; this fixed
        // internal open obtains an independent directory cursor, unlike dup.
        // SAFETY: fixed terminated dot, live parent, no creation.
        owned(unsafe { libc::openat(file.as_raw_fd(), c".".as_ptr(), directory_flags()) })
    }?;
    let fd = descriptor.into_raw_fd();
    // SAFETY: fd is fresh owned directory; ownership transfers only on success.
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        // SAFETY: failed fdopendir leaves descriptor ownership with caller.
        let result = close(unsafe { File::from_raw_fd(fd) });
        result?;
        return Err(FsError::Io);
    }
    let result = (|| {
        let mut entries = BTreeSet::new();
        loop {
            // SAFETY: current thread owns the stream and its errno storage.
            unsafe {
                *libc::__error() = 0;
            }
            // SAFETY: uniquely borrowed stream remains live until checked close.
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                // SAFETY: errno is this thread's live storage.
                if unsafe { *libc::__error() } != 0 {
                    return Err(FsError::Io);
                }
                break;
            }
            // SAFETY: readdir returns terminated d_name valid until next call.
            let raw = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if raw == b"." || raw == b".." {
                continue;
            }
            let text = std::str::from_utf8(raw)
                .map_err(|_| FsError::UnknownEntry)?
                .to_owned();
            name(&text).map_err(|_| FsError::UnknownEntry)?;
            if !entries.insert(text) || entries.len() > 128 {
                return Err(FsError::UnknownEntry);
            }
        }
        Ok(entries)
    })();
    // SAFETY: stream uniquely owns fd; closed exactly once, including error path.
    let closed = unsafe { libc::closedir(stream) };
    if closed != 0 {
        return Err(FsError::Close);
    }
    result
}
pub(super) fn unlink(parent: &File, component: &str, directory: bool) -> Result<(), FsError> {
    let component_c = name(component)?;
    if fail(if directory {
        FsOperation::Rmdir
    } else {
        FsOperation::Unlink
    }) {
        return Err(FsError::Unlink);
    }
    // SAFETY: live parent, checked single component; AT_REMOVEDIR only for empty owned directory.
    if unsafe {
        libc::unlinkat(
            parent.as_raw_fd(),
            component_c.as_ptr(),
            if directory { libc::AT_REMOVEDIR } else { 0 },
        )
    } != 0
    {
        return Err(FsError::Unlink);
    }
    if identity_at(parent, component)?.is_some() {
        return Err(FsError::Changed);
    }
    Ok(())
}
pub(super) fn free_space(file: &File) -> Result<(), FsError> {
    let mut value = MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: fstatvfs only reads filesystem capacity for this live directory.
    if unsafe { libc::fstatvfs(file.as_raw_fd(), value.as_mut_ptr()) } != 0 {
        return Err(FsError::Io);
    }
    // SAFETY: successful fstatvfs initialized output.
    let value = unsafe { value.assume_init() };
    let free = (value.f_bavail as u64)
        .checked_mul(value.f_frsize)
        .ok_or(FsError::Capacity)?;
    if free < super::MIN_FREE {
        return Err(FsError::Capacity);
    }
    Ok(())
}
