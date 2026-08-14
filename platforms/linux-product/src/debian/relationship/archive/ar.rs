use std::io::{self, Read};

use sha2::{Digest, Sha256};

use super::super::{
    DebianRelationshipError, DebianRelationshipErrorCode, PackageContentIdentity,
    MAX_ARTIFACT_BYTES,
};

const AR_MAGIC: &[u8; 8] = b"!<arch>\n";
const AR_HEADER_BYTES: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ArchiveMemberIdentity {
    pub(super) name: &'static str,
    pub(super) size: u64,
    pub(super) sha256: String,
}

pub(super) struct PackageReader<R> {
    inner: R,
    hasher: Sha256,
    bytes_read: u64,
    limit: u64,
}

impl<R: Read> PackageReader<R> {
    pub(super) fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes_read: 0,
            limit: MAX_ARTIFACT_BYTES,
        }
    }

    #[cfg(test)]
    pub(super) fn with_test_limit(inner: R, limit: u64) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes_read: 0,
            limit,
        }
    }

    pub(super) fn read_magic(&mut self) -> Result<(), DebianRelationshipError> {
        let mut magic = [0_u8; AR_MAGIC.len()];
        read_exact(self, &mut magic)?;
        if &magic != AR_MAGIC {
            return Err(archive_error());
        }
        Ok(())
    }

    pub(super) fn begin_member<'a>(
        &'a mut self,
        expected_name: &'static str,
    ) -> Result<MemberReader<'a, R>, DebianRelationshipError> {
        let mut header = [0_u8; AR_HEADER_BYTES];
        read_exact(self, &mut header)?;
        let size = parse_canonical_header(&header, expected_name)?;
        Ok(MemberReader {
            package: self,
            name: expected_name,
            size,
            remaining: size,
            hasher: Sha256::new(),
        })
    }

    pub(super) fn read_member_padding(
        &mut self,
        member_size: u64,
    ) -> Result<(), DebianRelationshipError> {
        if member_size % 2 == 1 {
            let mut padding = [0_u8; 1];
            read_exact(self, &mut padding)?;
            if padding != *b"\n" {
                return Err(archive_error());
            }
        }
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<PackageContentIdentity, DebianRelationshipError> {
        let mut trailing = [0_u8; 1];
        match self.read(&mut trailing) {
            Ok(0) => PackageContentIdentity::from_stream_digest(
                self.bytes_read,
                format!("{:x}", self.hasher.finalize()),
            ),
            Ok(_) | Err(_) => Err(archive_error()),
        }
    }
}

impl<R: Read> Read for PackageReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let remaining = self.limit.saturating_sub(self.bytes_read);
        if remaining == 0 {
            let mut overflow = [0_u8; 1];
            return match self.inner.read(&mut overflow)? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Debian package exceeds its byte limit",
                )),
            };
        }
        let allowed = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("remaining package limit fits the caller buffer");
        let count = self.inner.read(&mut buffer[..allowed])?;
        if count > 0 {
            self.hasher.update(&buffer[..count]);
            self.bytes_read += count as u64;
        }
        Ok(count)
    }
}

pub(super) struct MemberReader<'a, R> {
    package: &'a mut PackageReader<R>,
    name: &'static str,
    size: u64,
    remaining: u64,
    hasher: Sha256,
}

impl<R: Read> MemberReader<'_, R> {
    pub(super) const fn size(&self) -> u64 {
        self.size
    }

    pub(super) fn finish(self) -> Result<ArchiveMemberIdentity, DebianRelationshipError> {
        if self.remaining != 0 {
            return Err(archive_error());
        }
        Ok(ArchiveMemberIdentity {
            name: self.name,
            size: self.size,
            sha256: format!("{:x}", self.hasher.finalize()),
        })
    }
}

impl<R: Read> Read for MemberReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() || self.remaining == 0 {
            return Ok(0);
        }
        let allowed = usize::try_from(self.remaining.min(buffer.len() as u64))
            .expect("remaining ar member fits the caller buffer");
        let count = self.package.read(&mut buffer[..allowed])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Debian ar member is truncated",
            ));
        }
        self.hasher.update(&buffer[..count]);
        self.remaining -= count as u64;
        Ok(count)
    }
}

fn parse_canonical_header(
    header: &[u8; AR_HEADER_BYTES],
    expected_name: &str,
) -> Result<u64, DebianRelationshipError> {
    let size_text = std::str::from_utf8(&header[48..58]).map_err(|_| archive_error())?;
    let size_digits = size_text.trim_end_matches(' ');
    if size_digits.is_empty()
        || (size_digits.len() > 1 && size_digits.starts_with('0'))
        || !size_digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(archive_error());
    }
    let size = size_digits.parse::<u64>().map_err(|_| archive_error())?;
    if size == 0 || size > MAX_ARTIFACT_BYTES {
        return Err(archive_error());
    }
    let stored_name = format!("{expected_name}/");
    let expected = format!(
        "{stored_name:<16}{:<12}{:<6}{:<6}{:<8o}{size:<10}`\n",
        0, 0, 0, 0o100644
    );
    if expected.as_bytes() != header {
        return Err(archive_error());
    }
    Ok(size)
}

pub(super) fn read_exact(
    reader: &mut impl Read,
    buffer: &mut [u8],
) -> Result<(), DebianRelationshipError> {
    reader.read_exact(buffer).map_err(|_| archive_error())
}

pub(super) fn archive_error() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::ArtifactContentMismatch,
        "Debian package archive is not canonical",
    )
}
