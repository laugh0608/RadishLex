//! Narrow Foundation resource boundary. No UI, environment lookup or subprocess.
#![allow(unsafe_code)]

use super::filesystem::FsError;
use std::ffi::c_void;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
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

pub(super) fn exclude_from_backup(path: &Path) -> Result<(), FsError> {
    set_backup_exclusion(path, true)?;
    verify_backup_exclusion(path)
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

pub(super) fn verify_backup_exclusion(path: &Path) -> Result<(), FsError> {
    // A new URL is essential: copying a property from the setter's URL could
    // merely return its cached value without consulting the backing store.
    let object = url(path)?;
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
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn foundation_current_user_caches_query_needs_no_directory_creation() {
        // Exercise the native array/string/ownership ABI without opening the
        // actual Caches directory or creating any production object there.
        let path = super::caches_path().expect("supported current-user Foundation domain");
        assert!(path.is_absolute());
        assert!(path.ends_with("Library/Caches"));
    }
}
