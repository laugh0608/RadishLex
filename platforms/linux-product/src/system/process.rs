use std::collections::BTreeSet;
#[cfg(target_os = "linux")]
use std::fs::{self, File};
#[cfg(target_os = "linux")]
use std::io::{self, Read};
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "linux")]
use std::path::Path;

#[cfg(target_os = "linux")]
use crate::startup::{
    SYSTEM_FCITX_ADDON_PATH, SYSTEM_FCITX_FFI_PATH, SYSTEM_MANAGER_FFI_PATH, SYSTEM_MANAGER_PATH,
};

use super::observer::{LinuxSystemObservationError, LinuxSystemObservationErrorCode};

#[cfg(target_os = "linux")]
const MAX_PROCESS_MAP_BYTES: u64 = 16 * 1024 * 1024;

#[cfg(target_os = "linux")]
pub(super) fn prove_system_processes_quiescent() -> Result<(), LinuxSystemObservationError> {
    let target_paths = [
        SYSTEM_MANAGER_PATH,
        SYSTEM_MANAGER_FFI_PATH,
        SYSTEM_FCITX_ADDON_PATH,
        SYSTEM_FCITX_FFI_PATH,
    ];
    let target_files = target_paths
        .iter()
        .filter_map(|path| fs::symlink_metadata(path).ok())
        .filter(|metadata| metadata.file_type().is_file())
        .map(|metadata| {
            let device = metadata.dev();
            (
                linux_device_major(device),
                linux_device_minor(device),
                metadata.ino(),
            )
        })
        .collect::<BTreeSet<_>>();
    for entry in fs::read_dir("/proc").map_err(process_io_error)? {
        let entry = entry.map_err(process_io_error)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let maps_path = entry.path().join("maps");
        let maps = match read_process_maps(&maps_path) {
            Ok(value) => value,
            Err(error) if error.code() == LinuxSystemObservationErrorCode::ProcessDisappeared => {
                continue;
            }
            Err(error) => return Err(error),
        };
        for line in maps.lines() {
            if mapping_matches_product(line, &target_paths, &target_files)? {
                return Err(LinuxSystemObservationError::new(
                    LinuxSystemObservationErrorCode::ProgramsRunning,
                    "a process still maps a RadishLex product component",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn prove_system_processes_quiescent() -> Result<(), LinuxSystemObservationError> {
    Err(LinuxSystemObservationError::new(
        LinuxSystemObservationErrorCode::EnvironmentUnsupported,
        "Linux procfs process inspection is unavailable on this platform",
    ))
}

#[cfg(target_os = "linux")]
fn read_process_maps(path: &Path) -> Result<String, LinuxSystemObservationError> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(LinuxSystemObservationError::new(
                LinuxSystemObservationErrorCode::ProcessDisappeared,
                "process disappeared during procfs inspection",
            ))
        }
        Err(error) => return Err(process_io_error(error)),
    };
    let mut value = Vec::new();
    file.by_ref()
        .take(MAX_PROCESS_MAP_BYTES + 1)
        .read_to_end(&mut value)
        .map_err(process_io_error)?;
    if value.len() as u64 > MAX_PROCESS_MAP_BYTES
        || value.contains(&0)
        || (!value.is_empty() && !value.ends_with(b"\n"))
    {
        return Err(LinuxSystemObservationError::new(
            LinuxSystemObservationErrorCode::ProcessInspectionUnavailable,
            "procfs process mapping is malformed or exceeds its limit",
        ));
    }
    String::from_utf8(value).map_err(|_| {
        LinuxSystemObservationError::new(
            LinuxSystemObservationErrorCode::ProcessInspectionUnavailable,
            "procfs process mapping is not UTF-8",
        )
    })
}

fn mapping_matches_product(
    line: &str,
    target_paths: &[&str],
    target_files: &BTreeSet<(u64, u64, u64)>,
) -> Result<bool, LinuxSystemObservationError> {
    let mut remainder = line;
    for _ in 0..3 {
        let (_, rest) = take_mapping_token(remainder)?;
        remainder = rest;
    }
    let (device, rest) = take_mapping_token(remainder)?;
    let (inode, path) = take_final_mapping_token(rest)?;
    let (device_major, device_minor) = device.split_once(':').ok_or_else(malformed_mapping)?;
    let device_major = u64::from_str_radix(device_major, 16).map_err(|_| malformed_mapping())?;
    let device_minor = u64::from_str_radix(device_minor, 16).map_err(|_| malformed_mapping())?;
    let inode = inode.parse::<u64>().map_err(|_| malformed_mapping())?;
    let path = path.trim_start();
    if path.is_empty() || path.starts_with('[') {
        return Ok(false);
    }
    if target_paths
        .iter()
        .any(|target| path == *target || path == format!("{target} (deleted)"))
    {
        return Ok(true);
    }
    Ok(inode != 0 && target_files.contains(&(device_major, device_minor, inode)))
}

fn take_mapping_token(value: &str) -> Result<(&str, &str), LinuxSystemObservationError> {
    let value = value.trim_start();
    let end = value
        .find(char::is_whitespace)
        .ok_or_else(malformed_mapping)?;
    let token = &value[..end];
    if token.is_empty() {
        return Err(malformed_mapping());
    }
    Ok((token, &value[end..]))
}

fn take_final_mapping_token(value: &str) -> Result<(&str, &str), LinuxSystemObservationError> {
    let value = value.trim_start();
    if value.is_empty() {
        return Err(malformed_mapping());
    }
    Ok(match value.find(char::is_whitespace) {
        Some(end) => (&value[..end], &value[end..]),
        None => (value, ""),
    })
}

fn malformed_mapping() -> LinuxSystemObservationError {
    LinuxSystemObservationError::new(
        LinuxSystemObservationErrorCode::ProcessInspectionUnavailable,
        "procfs process mapping line is malformed",
    )
}

#[cfg(target_os = "linux")]
const fn linux_device_major(device: u64) -> u64 {
    ((device >> 8) & 0x0fff) | ((device >> 32) & 0xffff_f000)
}

#[cfg(target_os = "linux")]
const fn linux_device_minor(device: u64) -> u64 {
    (device & 0x00ff) | ((device >> 12) & 0xffff_ff00)
}

#[cfg(target_os = "linux")]
fn process_io_error(error: io::Error) -> LinuxSystemObservationError {
    LinuxSystemObservationError::new(
        if error.kind() == io::ErrorKind::PermissionDenied {
            LinuxSystemObservationErrorCode::PermissionDenied
        } else {
            LinuxSystemObservationErrorCode::ProcessInspectionUnavailable
        },
        "procfs process inspection failed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_parser_accepts_anonymous_and_named_non_product_mappings() {
        let paths = ["/usr/lib/radishlex.so"];
        let identities = BTreeSet::new();
        assert!(!mapping_matches_product(
            "7f000000-7f001000 rw-p 00000000 00:00 0",
            &paths,
            &identities,
        )
        .expect("anonymous mapping"));
        assert!(!mapping_matches_product(
            "7f000000-7f001000 r--p 00000000 08:01 42 /usr/lib/unrelated.so",
            &paths,
            &identities,
        )
        .expect("unrelated mapping"));
    }

    #[test]
    fn mapping_parser_matches_exact_deleted_and_device_inode_identity() {
        let paths = ["/usr/lib/radishlex.so"];
        let identities = BTreeSet::from([(8, 1, 42)]);
        for line in [
            "7f000000-7f001000 r-xp 00000000 08:01 11 /usr/lib/radishlex.so",
            "7f000000-7f001000 r-xp 00000000 08:01 11 /usr/lib/radishlex.so (deleted)",
            "7f000000-7f001000 r-xp 00000000 08:01 42 /tmp/renamed.so",
        ] {
            assert!(mapping_matches_product(line, &paths, &identities).expect("product mapping"));
        }
        assert!(!mapping_matches_product(
            "7f000000-7f001000 r-xp 00000000 08:02 42 /tmp/collision.so",
            &paths,
            &identities,
        )
        .expect("different device"));
    }

    #[test]
    fn mapping_parser_rejects_malformed_lines_instead_of_guessing() {
        let error = mapping_matches_product("not-a-mapping", &[], &BTreeSet::new())
            .expect_err("malformed mapping");
        assert_eq!(
            error.code(),
            LinuxSystemObservationErrorCode::ProcessInspectionUnavailable
        );
    }
}
