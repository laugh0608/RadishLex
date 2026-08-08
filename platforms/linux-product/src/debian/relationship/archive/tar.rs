use std::collections::BTreeSet;
use std::io::Read;

use md5::Md5;
use sha2::{Digest, Sha256};

use super::super::{MAX_CONTROL_BYTES, MAX_MANIFEST_BYTES, PRODUCT_MANIFEST_PATH};
use super::ar::{archive_error, read_exact};
use super::{PayloadDirectory, PayloadFile, PayloadInventory};
use crate::debian::relationship::DebianRelationshipError;

const TAR_BLOCK_BYTES: u64 = 512;
const TAR_RECORD_BYTES: u64 = 20 * TAR_BLOCK_BYTES;
const CONTROL_TAR_ENTRIES: usize = 2;
const MAX_PACKAGE_TAR_ENTRIES: usize = 100_000;
const MAX_DATA_TAR_ENTRIES: usize = MAX_PACKAGE_TAR_ENTRIES - CONTROL_TAR_ENTRIES;
const MAX_MD5SUMS_BYTES: usize = 32 * 1024 * 1024;

pub(super) struct ControlArchive {
    pub(super) control: Vec<u8>,
    pub(super) control_sha256: String,
    md5sums: Vec<u8>,
    pub(super) md5sums_sha256: String,
}

pub(super) struct DataArchive {
    pub(super) product_manifest: Vec<u8>,
    pub(super) product_manifest_sha256: String,
    pub(super) inventory: PayloadInventory,
    pub(super) md5sums: Vec<PayloadMd5Sum>,
    pub(super) installed_size_kib: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PayloadMd5Sum {
    absolute_path: String,
    md5: String,
}

pub(super) fn parse_control_tar(
    reader: &mut impl Read,
    size: u64,
) -> Result<ControlArchive, DebianRelationshipError> {
    let mut archive = TarStream::new(reader, size)?;
    let mut control = None;
    let mut control_sha256 = None;
    let mut md5sums = None;
    let mut md5sums_sha256 = None;
    let expected = ["/control", "/md5sums"];

    for expected_path in expected {
        let header = archive.next_header()?.ok_or_else(archive_error)?;
        if header.kind != TarEntryKind::Regular
            || header.absolute_path != expected_path
            || header.mode != 0o644
            || header.size == 0
        {
            return Err(archive_error());
        }
        let mut hasher = Sha256::new();
        if expected_path == "/control" {
            let content_size = usize::try_from(header.size).map_err(|_| archive_error())?;
            if content_size > MAX_CONTROL_BYTES {
                return Err(archive_error());
            }
            let mut bytes = Vec::with_capacity(content_size);
            archive.read_entry(header.size, |chunk| {
                hasher.update(chunk);
                bytes.extend_from_slice(chunk);
            })?;
            control = Some(bytes);
            control_sha256 = Some(format!("{:x}", hasher.finalize()));
        } else {
            let content_size = usize::try_from(header.size).map_err(|_| archive_error())?;
            if content_size > MAX_MD5SUMS_BYTES {
                return Err(archive_error());
            }
            let mut bytes = Vec::with_capacity(content_size);
            archive.read_entry(header.size, |chunk| {
                hasher.update(chunk);
                bytes.extend_from_slice(chunk);
            })?;
            md5sums = Some(bytes);
            md5sums_sha256 = Some(format!("{:x}", hasher.finalize()));
        }
    }
    if archive.next_header()?.is_some() {
        return Err(archive_error());
    }
    Ok(ControlArchive {
        control: control.ok_or_else(archive_error)?,
        control_sha256: control_sha256.ok_or_else(archive_error)?,
        md5sums: md5sums.ok_or_else(archive_error)?,
        md5sums_sha256: md5sums_sha256.ok_or_else(archive_error)?,
    })
}

pub(super) fn parse_data_tar(
    reader: &mut impl Read,
    size: u64,
) -> Result<DataArchive, DebianRelationshipError> {
    let mut archive = TarStream::new(reader, size)?;
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut known_directories = BTreeSet::new();
    let mut known_paths = BTreeSet::new();
    let mut previous_path: Option<String> = None;
    let mut product_manifest = None;
    let mut product_manifest_sha256 = None;
    let mut md5sums = Vec::new();
    let mut installed_size_kib = 0_u64;
    let mut entry_budget = TarEntryBudget::new();

    while let Some(header) = archive.next_header()? {
        entry_budget.consume()?;
        if !known_paths.insert(header.absolute_path.clone())
            || previous_path
                .as_deref()
                .is_some_and(|previous| previous >= header.absolute_path.as_str())
        {
            return Err(archive_error());
        }
        previous_path = Some(header.absolute_path.clone());
        let parent = parent_path(&header.absolute_path)?;
        if parent != "/" && !known_directories.contains(parent) {
            return Err(archive_error());
        }

        match header.kind {
            TarEntryKind::Directory => {
                if header.size != 0 || header.mode != 0o755 {
                    return Err(archive_error());
                }
                known_directories.insert(header.absolute_path.clone());
                directories.push(PayloadDirectory {
                    path: header.absolute_path,
                    mode: header.mode,
                    uid: header.uid,
                    gid: header.gid,
                });
                archive.read_entry(0, |_| {})?;
            }
            TarEntryKind::Regular => {
                if header.size == 0 || !matches!(header.mode, 0o644 | 0o755) {
                    return Err(archive_error());
                }
                let file_size_kib = header.size.checked_add(1023).ok_or_else(archive_error)? / 1024;
                installed_size_kib = installed_size_kib
                    .checked_add(file_size_kib)
                    .ok_or_else(archive_error)?;
                let mut sha256 = Sha256::new();
                let mut md5 = Md5::new();
                if header.absolute_path == PRODUCT_MANIFEST_PATH {
                    if product_manifest.is_some()
                        || header.mode != 0o644
                        || header.size > MAX_MANIFEST_BYTES as u64
                    {
                        return Err(archive_error());
                    }
                    let content_size = usize::try_from(header.size).map_err(|_| archive_error())?;
                    let mut bytes = Vec::with_capacity(content_size);
                    archive.read_entry(header.size, |chunk| {
                        sha256.update(chunk);
                        md5.update(chunk);
                        bytes.extend_from_slice(chunk);
                    })?;
                    product_manifest_sha256 = Some(format!("{:x}", sha256.finalize()));
                    product_manifest = Some(bytes);
                } else {
                    archive.read_entry(header.size, |chunk| {
                        sha256.update(chunk);
                        md5.update(chunk);
                    })?;
                    files.push(PayloadFile {
                        path: header.absolute_path.clone(),
                        mode: header.mode,
                        uid: header.uid,
                        gid: header.gid,
                        size: header.size,
                        sha256: format!("{:x}", sha256.finalize()),
                    });
                }
                md5sums.push(PayloadMd5Sum {
                    absolute_path: header.absolute_path,
                    md5: format!("{:x}", md5.finalize()),
                });
            }
        }
    }

    Ok(DataArchive {
        product_manifest: product_manifest.ok_or_else(archive_error)?,
        product_manifest_sha256: product_manifest_sha256.ok_or_else(archive_error)?,
        inventory: PayloadInventory { directories, files },
        md5sums,
        installed_size_kib,
    })
}

impl ControlArchive {
    pub(super) fn validate_payload_md5sums(
        &self,
        actual: &[PayloadMd5Sum],
    ) -> Result<(), DebianRelationshipError> {
        if parse_canonical_md5sums(&self.md5sums)? != actual {
            return Err(archive_error());
        }
        Ok(())
    }
}

fn parse_canonical_md5sums(value: &[u8]) -> Result<Vec<PayloadMd5Sum>, DebianRelationshipError> {
    if value.is_empty()
        || value.len() > MAX_MD5SUMS_BYTES
        || !value.ends_with(b"\n")
        || value.contains(&b'\r')
    {
        return Err(archive_error());
    }
    let body = std::str::from_utf8(&value[..value.len() - 1]).map_err(|_| archive_error())?;
    if body.is_empty() {
        return Err(archive_error());
    }
    let mut records = Vec::new();
    let mut previous_path: Option<String> = None;
    for line in body.split('\n') {
        let bytes = line.as_bytes();
        if bytes.len() <= 34
            || !bytes[..32]
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            || &bytes[32..34] != b"  "
        {
            return Err(archive_error());
        }
        let raw_path = &line[34..];
        let absolute_path = normalize_path(&format!("./{raw_path}"), TarEntryKind::Regular)?;
        if previous_path
            .as_deref()
            .is_some_and(|previous| previous >= absolute_path.as_str())
        {
            return Err(archive_error());
        }
        previous_path = Some(absolute_path.clone());
        records.push(PayloadMd5Sum {
            absolute_path,
            md5: line[..32].to_owned(),
        });
        if records.len() > MAX_DATA_TAR_ENTRIES {
            return Err(archive_error());
        }
    }
    Ok(records)
}

struct TarEntryBudget {
    count: usize,
}

impl TarEntryBudget {
    const fn new() -> Self {
        Self { count: 0 }
    }

    fn consume(&mut self) -> Result<(), DebianRelationshipError> {
        if self.count == MAX_DATA_TAR_ENTRIES {
            return Err(archive_error());
        }
        self.count += 1;
        Ok(())
    }
}

struct TarStream<'a, R> {
    reader: &'a mut R,
    total_size: u64,
    position: u64,
    finished: bool,
}

impl<'a, R: Read> TarStream<'a, R> {
    fn new(reader: &'a mut R, total_size: u64) -> Result<Self, DebianRelationshipError> {
        if total_size < TAR_RECORD_BYTES || total_size % TAR_RECORD_BYTES != 0 {
            return Err(archive_error());
        }
        Ok(Self {
            reader,
            total_size,
            position: 0,
            finished: false,
        })
    }

    fn next_header(&mut self) -> Result<Option<TarHeader>, DebianRelationshipError> {
        if self.finished {
            return Ok(None);
        }
        let mut header = [0_u8; TAR_BLOCK_BYTES as usize];
        self.read_exact(&mut header)?;
        if header.iter().all(|byte| *byte == 0) {
            self.finish_trailer()?;
            self.finished = true;
            return Ok(None);
        }
        Ok(Some(TarHeader::parse(&header)?))
    }

    fn read_entry(
        &mut self,
        size: u64,
        mut consume: impl FnMut(&[u8]),
    ) -> Result<(), DebianRelationshipError> {
        let mut remaining = size;
        let mut buffer = [0_u8; 16 * 1024];
        while remaining > 0 {
            let count = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("remaining tar entry fits the read buffer");
            self.read_exact(&mut buffer[..count])?;
            consume(&buffer[..count]);
            remaining -= count as u64;
        }
        let padding = (TAR_BLOCK_BYTES - size % TAR_BLOCK_BYTES) % TAR_BLOCK_BYTES;
        if padding > 0 {
            let mut zeros = [0_u8; TAR_BLOCK_BYTES as usize];
            self.read_exact(&mut zeros[..padding as usize])?;
            if zeros[..padding as usize].iter().any(|byte| *byte != 0) {
                return Err(archive_error());
            }
        }
        Ok(())
    }

    fn finish_trailer(&mut self) -> Result<(), DebianRelationshipError> {
        let mut second = [0_u8; TAR_BLOCK_BYTES as usize];
        self.read_exact(&mut second)?;
        if second.iter().any(|byte| *byte != 0) {
            return Err(archive_error());
        }
        let mut buffer = [0_u8; 16 * 1024];
        while self.position < self.total_size {
            let remaining = self.total_size - self.position;
            let count = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("remaining tar trailer fits the read buffer");
            self.read_exact(&mut buffer[..count])?;
            if buffer[..count].iter().any(|byte| *byte != 0) {
                return Err(archive_error());
            }
        }
        Ok(())
    }

    fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), DebianRelationshipError> {
        if self.position + buffer.len() as u64 > self.total_size {
            return Err(archive_error());
        }
        read_exact(self.reader, buffer)?;
        self.position += buffer.len() as u64;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TarEntryKind {
    Regular,
    Directory,
}

struct TarHeader {
    absolute_path: String,
    kind: TarEntryKind,
    mode: u32,
    uid: u32,
    gid: u32,
    size: u64,
}

impl TarHeader {
    fn parse(header: &[u8; TAR_BLOCK_BYTES as usize]) -> Result<Self, DebianRelationshipError> {
        validate_checksum(header)?;
        if &header[257..263] != b"ustar\0"
            || &header[263..265] != b"00"
            || parse_text_field(&header[265..297])? != "root"
            || parse_text_field(&header[297..329])? != "root"
            || header[157..257].iter().any(|byte| *byte != 0)
            || header[329..345].iter().any(|byte| *byte != 0)
            || header[500..512].iter().any(|byte| *byte != 0)
        {
            return Err(archive_error());
        }
        let mode = parse_octal(&header[100..108], 7)? as u32;
        let uid = parse_octal(&header[108..116], 7)? as u32;
        let gid = parse_octal(&header[116..124], 7)? as u32;
        let size = parse_octal(&header[124..136], 11)?;
        let mtime = parse_octal(&header[136..148], 11)?;
        if uid != 0 || gid != 0 || mtime != 0 || mode > 0o7777 {
            return Err(archive_error());
        }
        let kind = match header[156] {
            b'0' => TarEntryKind::Regular,
            b'5' => TarEntryKind::Directory,
            _ => return Err(archive_error()),
        };
        let name = parse_text_field(&header[..100])?;
        let prefix = parse_text_field(&header[345..500])?;
        let raw_path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        validate_ustar_split(&raw_path, name, prefix)?;
        let absolute_path = normalize_path(&raw_path, kind)?;
        Ok(Self {
            absolute_path,
            kind,
            mode,
            uid,
            gid,
            size,
        })
    }
}

fn validate_checksum(
    header: &[u8; TAR_BLOCK_BYTES as usize],
) -> Result<(), DebianRelationshipError> {
    if header[154] != 0 || header[155] != b' ' {
        return Err(archive_error());
    }
    let stored = parse_six_digit_octal(&header[148..154])?;
    let computed = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(*byte)
            }
        })
        .sum::<u64>();
    if stored != computed {
        return Err(archive_error());
    }
    Ok(())
}

fn parse_six_digit_octal(value: &[u8]) -> Result<u64, DebianRelationshipError> {
    if value.len() != 6 || value.iter().any(|byte| !(b'0'..=b'7').contains(byte)) {
        return Err(archive_error());
    }
    parse_octal_digits(value)
}

fn parse_octal(value: &[u8], digit_count: usize) -> Result<u64, DebianRelationshipError> {
    if value.len() != digit_count + 1
        || value[digit_count] != 0
        || value[..digit_count]
            .iter()
            .any(|byte| !(b'0'..=b'7').contains(byte))
    {
        return Err(archive_error());
    }
    parse_octal_digits(&value[..digit_count])
}

fn parse_octal_digits(value: &[u8]) -> Result<u64, DebianRelationshipError> {
    value.iter().try_fold(0_u64, |result, byte| {
        result
            .checked_mul(8)
            .and_then(|current| current.checked_add(u64::from(byte - b'0')))
            .ok_or_else(archive_error)
    })
}

fn parse_text_field(value: &[u8]) -> Result<&str, DebianRelationshipError> {
    let end = value
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(value.len());
    if value[end..].iter().any(|byte| *byte != 0) {
        return Err(archive_error());
    }
    std::str::from_utf8(&value[..end]).map_err(|_| archive_error())
}

fn validate_ustar_split(
    raw_path: &str,
    actual_name: &str,
    actual_prefix: &str,
) -> Result<(), DebianRelationshipError> {
    let (expected_prefix, expected_name) = canonical_ustar_split(raw_path)?;
    if actual_prefix != expected_prefix || actual_name != expected_name {
        return Err(archive_error());
    }
    Ok(())
}

fn canonical_ustar_split(value: &str) -> Result<(&str, &str), DebianRelationshipError> {
    if value.len() <= 100 {
        return Ok(("", value));
    }
    for (index, byte) in value.bytes().enumerate().rev() {
        if byte == b'/' && index + 1 < value.len() && index <= 155 && value.len() - index - 1 <= 100
        {
            return Ok((&value[..index], &value[index + 1..]));
        }
    }
    Err(archive_error())
}

fn normalize_path(raw_path: &str, kind: TarEntryKind) -> Result<String, DebianRelationshipError> {
    if !raw_path.starts_with("./")
        || raw_path.contains("//")
        || raw_path.contains('\\')
        || raw_path.chars().any(char::is_control)
    {
        return Err(archive_error());
    }
    let value = match kind {
        TarEntryKind::Regular if !raw_path.ends_with('/') => &raw_path[2..],
        TarEntryKind::Directory if raw_path.ends_with('/') => &raw_path[2..raw_path.len() - 1],
        _ => return Err(archive_error()),
    };
    if value.is_empty()
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(archive_error());
    }
    Ok(format!("/{value}"))
}

fn parent_path(value: &str) -> Result<&str, DebianRelationshipError> {
    value
        .rsplit_once('/')
        .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
        .ok_or_else(archive_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_budget_accepts_exact_limit_and_rejects_the_next_entry() {
        let mut budget = TarEntryBudget::new();
        for _ in 0..MAX_DATA_TAR_ENTRIES {
            budget.consume().expect("entry within fixed budget");
        }
        assert!(budget.consume().is_err());
    }

    #[test]
    fn long_directory_ustar_split_keeps_the_trailing_slash_in_name() {
        let path = format!("./{}/{}{}", "a".repeat(80), "b".repeat(30), "/");
        let (prefix, name) = canonical_ustar_split(&path).expect("long USTAR directory path");
        assert_eq!(prefix, format!("./{}", "a".repeat(80)));
        assert_eq!(name, format!("{}/", "b".repeat(30)));
    }
}
