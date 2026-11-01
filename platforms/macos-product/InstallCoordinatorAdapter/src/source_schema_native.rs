//! Narrow Foundation resource boundary. No UI, environment lookup or subprocess.
#![allow(unsafe_code)]

use super::filesystem::FsError;
use std::ffi::c_void;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::ptr;

type Object = *const c_void;

#[link(name = "Foundation", kind = "framework")]
extern "C" {
    fn NSSearchPathForDirectoriesInDomains(directory: usize, domain: usize, expand: i8) -> Object;
}
#[link(name = "objc")]
extern "C" {
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(object: Object);
    fn CFGetTypeID(object: Object) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFArrayGetCount(array: Object) -> isize;
    fn CFArrayGetValueAtIndex(array: Object, index: isize) -> Object;
    fn CFStringGetTypeID() -> usize;
    fn CFStringGetLength(string: Object) -> isize;
    fn CFStringGetFileSystemRepresentation(string: Object, buffer: *mut i8, size: isize) -> u8;
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: Object,
        bytes: *const u8,
        length: isize,
        directory: u8,
    ) -> Object;
    fn CFURLSetResourcePropertyForKey(
        url: Object,
        key: Object,
        value: Object,
        error: *mut Object,
    ) -> u8;
    fn CFURLCopyResourcePropertyForKey(
        url: Object,
        key: Object,
        value: *mut Object,
        error: *mut Object,
    ) -> u8;
    fn CFURLClearResourcePropertyCache(url: Object);
    fn CFDataCreate(allocator: Object, bytes: *const u8, length: isize) -> Object;
    fn CFPropertyListCreateWithData(
        allocator: Object,
        data: Object,
        options: usize,
        format: *mut isize,
        error: *mut Object,
    ) -> Object;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(value: Object) -> u8;
    static kCFBooleanTrue: Object;
    static kCFBooleanFalse: Object;
    static kCFURLIsExcludedFromBackupKey: Object;
}

struct Owned(Object);
impl Owned {
    fn new(value: Object) -> Result<Self, FsError> {
        if value.is_null() {
            Err(FsError::Platform)
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: only non-null Create/Copy results enter this single owner.
        unsafe { CFRelease(self.0) };
    }
}

pub(super) fn caches_path() -> Result<PathBuf, FsError> {
    // SAFETY: thread-local autorelease scope is paired on every return below.
    let pool = unsafe { objc_autoreleasePoolPush() };
    let result = (|| {
        // NSCachesDirectory=13, NSUserDomainMask=1. NSArray/NSString are
        // toll-free bridged to CFArray/CFString; results live until pool pop.
        // SAFETY: fixed enum values, no caller-provided pointers.
        let array = unsafe { NSSearchPathForDirectoriesInDomains(13, 1, 1) };
        // SAFETY: a non-null Foundation array remains live in this scope.
        if array.is_null()
            || unsafe { CFGetTypeID(array) != CFArrayGetTypeID() || CFArrayGetCount(array) != 1 }
        {
            return Err(FsError::Platform);
        }
        // SAFETY: the checked array has exactly one element.
        let string = unsafe { CFArrayGetValueAtIndex(array, 0) };
        if string.is_null() || unsafe { CFGetTypeID(string) != CFStringGetTypeID() } {
            return Err(FsError::Platform);
        }
        let mut bytes = [0u8; 4096];
        // SAFETY: the live string and entire writable buffer are supplied.
        if unsafe {
            CFStringGetFileSystemRepresentation(
                string,
                bytes.as_mut_ptr().cast(),
                bytes.len() as isize,
            )
        } == 0
        {
            return Err(FsError::Platform);
        }
        let length = bytes
            .iter()
            .position(|b| *b == 0)
            .ok_or(FsError::Platform)?;
        let path = PathBuf::from(std::ffi::OsString::from_vec(bytes[..length].to_vec()));
        // This first production profile is the ordinary current-user Caches
        // domain. Reject redirected Foundation home/container locations rather
        // than allowing an environment override to choose a copy destination.
        if path != account_home()?.join("Library/Caches") {
            return Err(FsError::Platform);
        }
        Ok(path)
    })();
    // SAFETY: this is the token created on this same thread above.
    unsafe { objc_autoreleasePoolPop(pool) };
    result
}

fn account_home() -> Result<PathBuf, FsError> {
    let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut returned = ptr::null_mut();
    let mut buffer = vec![0u8; 64 * 1024];
    // SAFETY: bounded writable record/buffer and result pointer; all borrowed
    // passwd strings are copied before the backing buffer leaves this scope.
    let result = unsafe {
        libc::getpwuid_r(
            libc::geteuid(),
            record.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut returned,
        )
    };
    if result != 0 || returned.is_null() || returned != record.as_mut_ptr() {
        return Err(FsError::Platform);
    }
    // SAFETY: successful getpwuid_r initialized this record in our buffer.
    let record = unsafe { record.assume_init() };
    if record.pw_dir.is_null() {
        return Err(FsError::Platform);
    }
    // SAFETY: pw_dir is a terminated string in the live getpwuid_r buffer.
    let home = unsafe { std::ffi::CStr::from_ptr(record.pw_dir) }.to_bytes();
    if home.is_empty() || home.len() > 4096 || home[0] != b'/' {
        return Err(FsError::Platform);
    }
    Ok(PathBuf::from(std::ffi::OsString::from_vec(home.to_vec())))
}

fn url(path: &Path) -> Result<Owned, FsError> {
    let bytes = path.as_os_str().as_bytes();
    if !path.is_absolute() || bytes.len() > 4096 || bytes.contains(&0) {
        return Err(FsError::Identity);
    }
    // SAFETY: buffer is live for the call, which copies its contents.
    Owned::new(unsafe {
        CFURLCreateFromFileSystemRepresentation(
            ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            1,
        )
    })
}

pub(super) fn exclude_from_backup(path: &Path, directory: &File) -> Result<(), FsError> {
    set_backup_exclusion(path, true)?;
    verify_backup_exclusion(path, directory)
}

fn set_backup_exclusion(path: &Path, excluded: bool) -> Result<(), FsError> {
    let object = url(path)?;
    let mut error = ptr::null();
    // SAFETY: all objects are live CF objects; error is valid output storage.
    let result = unsafe {
        CFURLSetResourcePropertyForKey(
            object.0,
            kCFURLIsExcludedFromBackupKey,
            if excluded {
                kCFBooleanTrue
            } else {
                kCFBooleanFalse
            },
            &mut error,
        )
    };
    let had_error = !error.is_null();
    if had_error {
        drop(Owned(error));
    }
    if result == 0 || had_error {
        return Err(FsError::BackupExclusion);
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn clear_backup_exclusion_for_test(path: &Path) -> Result<(), FsError> {
    set_backup_exclusion(path, false)
}

pub(super) fn verify_backup_exclusion(path: &Path, directory: &File) -> Result<(), FsError> {
    let before = super::filesystem::posix::identity(directory)?;
    if !before.directory() {
        return Err(FsError::BackupExclusion);
    }
    verify_directory_path(path, &before)?;
    verify_persisted_backup_marker(directory)?;
    let object = url(path)?;
    // The descriptor marker and Foundation's effective property must both
    // agree. A freshly allocated URL alone is not a cache lifetime guarantee.
    // SAFETY: this owned URL is live, local to this read and has no borrowers.
    unsafe { CFURLClearResourcePropertyCache(object.0) };
    let mut value = ptr::null();
    let mut error = ptr::null();
    // SAFETY: live URL/key and valid output locations, released exactly once.
    let result = unsafe {
        CFURLCopyResourcePropertyForKey(
            object.0,
            kCFURLIsExcludedFromBackupKey,
            &mut value,
            &mut error,
        )
    };
    let had_error = !error.is_null();
    if had_error {
        drop(Owned(error));
    }
    let value = Owned::new(value).map_err(|_| FsError::BackupExclusion)?;
    if result == 0
        || had_error
        || unsafe {
            CFGetTypeID(value.0) != CFBooleanGetTypeID() || CFBooleanGetValue(value.0) == 0
        }
    {
        return Err(FsError::BackupExclusion);
    }
    verify_persisted_backup_marker(directory)?;
    let after = super::filesystem::posix::identity(directory)?;
    verify_directory_path(path, &after)?;
    // Match Chain's directory identity contract: timestamps/size can change
    // without replacing a directory. Marker content is checked on both sides;
    // source DB/WAL timestamp checks and namespace guards remain independent.
    if !after.same_node(&before) {
        #[cfg(test)]
        eprintln!("synthetic backup rejection: directory drift before={before:?} after={after:?}");
        return Err(FsError::BackupExclusion);
    }
    Ok(())
}

fn verify_directory_path(
    path: &Path,
    expected: &super::filesystem::posix::Identity,
) -> Result<(), FsError> {
    let current = std::fs::symlink_metadata(path).map_err(|_| FsError::BackupExclusion)?;
    if !current.is_dir()
        || current.dev() != expected.device
        || current.ino() != expected.inode
        || current.uid() != expected.owner
        || current.mode() != expected.mode
    {
        return Err(FsError::BackupExclusion);
    }
    Ok(())
}

fn verify_persisted_backup_marker(directory: &File) -> Result<(), FsError> {
    // This macOS profile accepts only the system marker written by Foundation:
    // a small plist string naming com.apple.backupd. Never write raw xattrs.
    let mut bytes = [0u8; 256];
    // SAFETY: borrowed directory fd, terminated fixed name, bounded output;
    // Darwin's position/options zero request an ordinary descriptor-only read.
    let length = unsafe {
        libc::fgetxattr(
            directory.as_raw_fd(),
            c"com.apple.metadata:com_apple_backup_excludeItem".as_ptr(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            0,
            0,
        )
    };
    if length <= 0 || length as usize > bytes.len() {
        #[cfg(test)]
        eprintln!(
            "synthetic backup rejection: marker length={length} errno={:?}",
            std::io::Error::last_os_error().raw_os_error()
        );
        return Err(FsError::BackupExclusion);
    }
    // SAFETY: only the initialized, bounded bytes are copied into owned CFData.
    let data = Owned::new(unsafe { CFDataCreate(ptr::null(), bytes.as_ptr(), length) })
        .map_err(|_| FsError::BackupExclusion)?;
    let mut error = ptr::null();
    // SAFETY: live CFData, immutable plist option 0, optional format omitted;
    // Create and error results are separately owned and released below.
    let value = unsafe {
        CFPropertyListCreateWithData(ptr::null(), data.0, 0, ptr::null_mut(), &mut error)
    };
    let had_error = !error.is_null();
    if had_error {
        drop(Owned(error));
    }
    let value = Owned::new(value).map_err(|_| FsError::BackupExclusion)?;
    let mut label = [0u8; 64];
    let expected = b"com.apple.backupd\0";
    // SAFETY: type is checked before string conversion; the whole writable
    // buffer is provided. Exact NUL matching rejects prefixes and extra text.
    if had_error
        || unsafe {
            CFGetTypeID(value.0) != CFStringGetTypeID()
                || CFStringGetLength(value.0) != (expected.len() - 1) as isize
                || CFStringGetFileSystemRepresentation(
                    value.0,
                    label.as_mut_ptr().cast(),
                    label.len() as isize,
                ) == 0
        }
        || &label[..expected.len()] != expected
    {
        #[cfg(test)]
        eprintln!("synthetic backup rejection: marker plist type/value");
        return Err(FsError::BackupExclusion);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, DirBuilder};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(1);

    struct Directory {
        path: PathBuf,
        node: (u64, u64),
        file: File,
    }
    impl Directory {
        fn new() -> Self {
            let path = PathBuf::from(format!(
                "/private/tmp/radishlex-backup-property-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            DirBuilder::new().mode(0o700).create(&path).unwrap();
            let meta = fs::symlink_metadata(&path).unwrap();
            Self {
                file: File::open(&path).unwrap(),
                path,
                node: (meta.dev(), meta.ino()),
            }
        }
        fn finish(self) {
            let meta = fs::symlink_metadata(&self.path).unwrap();
            assert_eq!((meta.dev(), meta.ino()), self.node);
            assert!(meta.is_dir());
            assert_eq!(meta.mode() & 0o7777, 0o700);
            assert!(fs::read_dir(&self.path).unwrap().next().is_none());
            fs::remove_dir(self.path).unwrap();
        }
    }

    #[test]
    fn persisted_marker_and_effective_property_are_both_required() {
        for excluded in [false, true] {
            let directory = Directory::new();
            set_backup_exclusion(&directory.path, excluded).unwrap();
            let expected = if excluded {
                Ok(())
            } else {
                Err(FsError::BackupExclusion)
            };
            assert_eq!(
                verify_backup_exclusion(&directory.path, &directory.file),
                expected
            );
            directory.finish();
        }
    }

    #[test]
    fn malformed_or_oversized_persisted_markers_are_rejected() {
        for bytes in [
            b"".as_slice(),
            b"synthetic-invalid-plist".as_slice(),
            b"<plist version=\"1.0\"><string>synthetic</string></plist>".as_slice(),
            b"<plist version=\"1.0\"><string>com.apple.backupd.synthetic</string></plist>"
                .as_slice(),
            b"<plist version=\"1.0\"><integer>1</integer></plist>".as_slice(),
            &[0u8; 257],
        ] {
            let directory = Directory::new();
            // SAFETY: exact fresh fixture fd and fixed marker name; the input
            // slice remains live. This mutation exists only in synthetic tests.
            let result = unsafe {
                libc::fsetxattr(
                    directory.file.as_raw_fd(),
                    c"com.apple.metadata:com_apple_backup_excludeItem".as_ptr(),
                    bytes.as_ptr().cast(),
                    bytes.len(),
                    0,
                    0,
                )
            };
            assert_eq!(result, 0);
            assert_eq!(
                verify_backup_exclusion(&directory.path, &directory.file),
                Err(FsError::BackupExclusion)
            );
            directory.finish();
        }
    }

    #[test]
    fn valid_backup_properties_cannot_bind_a_different_directory() {
        let first = Directory::new();
        let second = Directory::new();
        exclude_from_backup(&first.path, &first.file).unwrap();
        exclude_from_backup(&second.path, &second.file).unwrap();
        assert_eq!(
            verify_backup_exclusion(&first.path, &second.file),
            Err(FsError::BackupExclusion)
        );
        assert_eq!(
            verify_backup_exclusion(&second.path, &first.file),
            Err(FsError::BackupExclusion)
        );
        first.finish();
        second.finish();
    }

    #[test]
    fn unavailable_backup_property_is_rejected() {
        let directory = Directory::new();
        set_backup_exclusion(&directory.path, true).unwrap();
        assert_eq!(
            verify_backup_exclusion(&directory.path.join("missing"), &directory.file),
            Err(FsError::BackupExclusion)
        );
        directory.finish();
    }

    #[test]
    fn foundation_current_user_caches_query_needs_no_directory_creation() {
        // Exercise the native array/string/ownership ABI without opening the
        // actual Caches directory or creating any production object there.
        let path = super::caches_path().expect("supported current-user Foundation domain");
        assert!(path.is_absolute());
        assert!(path.ends_with("Library/Caches"));
    }
}
