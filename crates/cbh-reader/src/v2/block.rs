//! Framed payload blocks shared by 2CBG and 2CBA.
//!
//! Layout recovered from the native block reader (`0x1401af990`), writer
//! (`0x1401afc80`) and checksum (`0x140f06190`) in CBase18.exe 18.1:
//! marker, payload length, padding length, checksum, payload type, payload,
//! padding, then a total-length trailer.
use crate::{CbhError, source::SourceFile};

pub const MARKER: u64 = 0x1122_3344_5566_7788;
pub const FRAME_HEADER: usize = 26;
pub const FRAME_OVERHEAD: u64 = 34;
/// Each stream file starts with a LE64 file length and a LE32 field.
pub const FILE_HEADER: u64 = 12;

pub const TYPE_GAME: u16 = 0x0001;
pub const TYPE_TEXT: u16 = 0x1000;
pub const TYPE_ANNOTATIONS: u16 = 0x2000;

#[derive(Debug)]
pub struct Block {
    pub kind: u16,
    pub payload: Vec<u8>,
}

/// Eight byte sums over `max(len / 8, 1)`-byte chunks, packed most significant
/// first. Bytes beyond the eighth chunk are not covered.
pub fn checksum(payload: &[u8]) -> u64 {
    let step = (payload.len() / 8).max(1);
    let mut value = 0u64;
    for part in 0..8 {
        let start = (part * step).min(payload.len());
        let end = ((part + 1) * step).min(payload.len());
        let sum = payload[start..end]
            .iter()
            .fold(0u8, |a, &b| a.wrapping_add(b));
        value = (value << 8) | sum as u64;
    }
    value
}

pub fn read_block(file: &SourceFile, offset: u64, max_payload: usize) -> Result<Block, CbhError> {
    if offset < FILE_HEADER {
        return Err(CbhError::Bounds);
    }
    let head = file.read_exact_range(offset, FRAME_HEADER)?;
    let le32 = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap());
    if u64::from_le_bytes(head[..8].try_into().unwrap()) != MARKER {
        return Err(CbhError::Decode(format!("No block marker at {offset}")));
    }
    let length = le32(8) as usize;
    let padding = le32(12) as u64;
    let stored = u64::from_le_bytes(head[16..24].try_into().unwrap());
    let kind = u16::from_le_bytes([head[24], head[25]]);
    if length > max_payload {
        return Err(CbhError::LimitExceeded);
    }
    let total = FRAME_OVERHEAD + length as u64 + padding;
    let end = offset.checked_add(total).ok_or(CbhError::Bounds)?;
    if end > file.len() {
        return Err(CbhError::Truncated);
    }
    let payload = file.read_exact_range(offset + FRAME_HEADER as u64, length)?;
    let trailer = file.read_exact_range(end - 8, 8)?;
    if u64::from_le_bytes(trailer[..].try_into().unwrap()) != total {
        return Err(CbhError::Decode(format!(
            "Block trailer mismatch at {offset}"
        )));
    }
    if checksum(&payload) != stored {
        return Err(CbhError::Decode(format!(
            "Block checksum mismatch at {offset}"
        )));
    }
    Ok(Block { kind, payload })
}
