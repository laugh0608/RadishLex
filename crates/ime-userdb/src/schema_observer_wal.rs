//! Bounded WAL admission, not a pager or a schema interpreter.
use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::{ObserverControl, ObserverError, ReadCharge};

pub(super) const MAX_DB: u64 = 256 * 1024 * 1024;
pub(super) const MAX_WAL: u64 = 128 * 1024 * 1024;
const MAX_FRAMES: u64 = 262_144;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WalAdmission {
    pub page_size: u32,
    pub committed_pages: Option<u32>,
    pub frames: u64,
    pub commits: u64,
}

pub(super) fn exact_read(
    file: &mut File,
    bytes: &mut [u8],
    control: &ObserverControl,
    charge: &mut ReadCharge<'_>,
) -> Result<(), ObserverError> {
    for chunk in bytes.chunks_mut(64 * 1024) {
        control()?;
        // Reserve the requested read before IO; a short read is conservatively
        // charged in full and cannot consume unreserved budget.
        let mut offset = 0;
        while offset < chunk.len() {
            control()?;
            charge((chunk.len() - offset) as u64)?;
            let count = file
                .read(&mut chunk[offset..])
                .map_err(|_| ObserverError::Io)?;
            if count == 0 {
                return Err(ObserverError::InputFormat);
            }
            offset += count;
        }
        control()?;
    }
    Ok(())
}

pub(super) fn be(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes[..4].try_into().expect("four-byte format field"))
}

pub(super) fn checksum(bytes: &[u8], little: bool, mut state: [u32; 2]) -> [u32; 2] {
    for pair in bytes.chunks_exact(8) {
        let read = |word: &[u8]| {
            let word: [u8; 4] = word.try_into().expect("four-byte checksum word");
            if little {
                u32::from_le_bytes(word)
            } else {
                u32::from_be_bytes(word)
            }
        };
        state[0] = state[0]
            .wrapping_add(read(&pair[..4]))
            .wrapping_add(state[1]);
        state[1] = state[1]
            .wrapping_add(read(&pair[4..]))
            .wrapping_add(state[0]);
    }
    state
}

pub(super) fn admit(
    path: &Path,
    control: &ObserverControl,
    charge: &mut ReadCharge<'_>,
) -> Result<WalAdmission, ObserverError> {
    control()?;
    let mut db = File::open(path).map_err(|_| ObserverError::Io)?;
    let db_len = db.metadata().map_err(|_| ObserverError::Io)?.len();
    if db_len > MAX_DB {
        return Err(ObserverError::LimitExceeded);
    }
    if db_len < 100 {
        return Err(ObserverError::InputFormat);
    }
    let mut header = [0; 100];
    exact_read(&mut db, &mut header, control, charge)?;
    if &header[..16] != b"SQLite format 3\0" {
        return Err(ObserverError::InputFormat);
    }
    let raw = u16::from_be_bytes([header[16], header[17]]);
    let page_size = if raw == 1 { 65536 } else { u32::from(raw) };
    if !(512..=65536).contains(&page_size)
        || !page_size.is_power_of_two()
        || db_len % u64::from(page_size) != 0
        || !matches!((header[18], header[19]), (1, 1) | (2, 2))
    {
        return Err(ObserverError::InputFormat);
    }
    let mut admission = WalAdmission {
        page_size,
        committed_pages: None,
        frames: 0,
        commits: 0,
    };
    let wal_path = path.with_file_name("userdb.sqlite3-wal");
    let mut wal = match File::open(wal_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return if header[18] == 1 {
                Ok(admission)
            } else {
                Err(ObserverError::WalUnsupported)
            };
        }
        Err(_) => return Err(ObserverError::Io),
    };
    let length = wal.metadata().map_err(|_| ObserverError::Io)?.len();
    if length > MAX_WAL
        || db_len
            .checked_add(length)
            .ok_or(ObserverError::LimitExceeded)?
            > MAX_DB
    {
        return Err(ObserverError::LimitExceeded);
    }
    if length == 0 {
        return Ok(admission);
    }
    if header[18] != 2 || length < 32 {
        return Err(ObserverError::WalUnsupported);
    }
    let mut wal_header = [0; 32];
    exact_read(&mut wal, &mut wal_header, control, charge)?;
    let magic = be(&wal_header);
    if !matches!(magic, 0x377f0682 | 0x377f0683)
        || be(&wal_header[4..]) != 3_007_000
        || be(&wal_header[8..]) != page_size
    {
        return Err(ObserverError::WalUnsupported);
    }
    let little = magic == 0x377f0682;
    let mut state = checksum(&wal_header[..24], little, [0, 0]);
    if state != [be(&wal_header[24..]), be(&wal_header[28..])] {
        return Err(ObserverError::WalChecksum);
    }
    let frame_size = u64::from(page_size) + 24;
    if (length - 32) % frame_size != 0 {
        return Err(ObserverError::InputFormat);
    }
    admission.frames = (length - 32) / frame_size;
    if admission.frames > MAX_FRAMES {
        return Err(ObserverError::LimitExceeded);
    }
    let mut page = vec![0; page_size as usize];
    let mut frame = [0; 24];
    for _ in 0..admission.frames {
        exact_read(&mut wal, &mut frame, control, charge)?;
        exact_read(&mut wal, &mut page, control, charge)?;
        let page_number = be(&frame);
        let commit_pages = be(&frame[4..]);
        if page_number == 0 || u64::from(page_number) > MAX_DB / u64::from(page_size) {
            return Err(ObserverError::InputFormat);
        }
        if frame[8..16] != wal_header[16..24] {
            return Err(ObserverError::WalUnsupported);
        }
        state = checksum(&frame[..8], little, state);
        state = checksum(&page, little, state);
        if state != [be(&frame[16..]), be(&frame[20..])] {
            return Err(ObserverError::WalChecksum);
        }
        if commit_pages != 0 {
            if u64::from(commit_pages)
                .checked_mul(u64::from(page_size))
                .ok_or(ObserverError::LimitExceeded)?
                > MAX_DB
            {
                return Err(ObserverError::LimitExceeded);
            }
            admission.committed_pages = Some(commit_pages);
            admission.commits += 1;
        }
    }
    control()?;
    Ok(admission)
}
