//! 2CBH header and 192-byte entry records.
//!
//! Stride (LE16 at byte 10) and header-inclusive count (LE64 at byte 16) come
//! from `0x1401ceab0` and `0x1401cd6a0`. Field offsets come from the classic to
//! 192-byte conversion `0x1401cd6b0` and the 2CBH getters (`0x1401ceef0` and
//! neighbours). See docs/compatibility.md for what remains uninterpreted.
use crate::{CbhError, Diagnostic, EntryKind, header::RawEntry};

pub const RECORD: usize = 192;
const TYPE_GAME: u16 = 0x0101;
const TYPE_TEXT: u16 = 0x0301;

#[derive(Debug)]
pub struct NewHeader {
    pub entry_count: u64,
    pub diagnostics: Vec<Diagnostic>,
}

impl NewHeader {
    pub fn parse(bytes: &[u8], length: u64) -> Result<Self, CbhError> {
        if bytes.len() != RECORD {
            return Err(CbhError::InvalidHeader);
        }
        let stride = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
        if stride != RECORD {
            return Err(CbhError::Unsupported(format!(
                "2CBH record stride {stride} (only {RECORD} is supported)"
            )));
        }
        let count = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        if count == 0 || count > length / RECORD as u64 {
            return Err(CbhError::InvalidHeader);
        }
        let diagnostics = if count < length / RECORD as u64 {
            vec![Diagnostic::new(
                "unusedCapacity",
                "Complete trailing records are outside the declared entry count",
            )]
        } else {
            vec![]
        };
        Ok(Self {
            entry_count: count - 1,
            diagnostics,
        })
    }
}

fn le16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn le32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn le64(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}
/// Entity ids are LE64; all-ones means none.
fn id(b: &[u8], i: usize, diagnostics: &mut Vec<Diagnostic>) -> Option<u32> {
    let value = le64(b, i);
    if value == u64::MAX {
        return None;
    }
    u32::try_from(value).ok().or_else(|| {
        diagnostics.push(Diagnostic::new("idOutOfRange", "Entity id exceeds 32 bits"));
        None
    })
}

pub fn parse_record(bytes: &[u8], ordinal: u64) -> Result<RawEntry, CbhError> {
    if bytes.len() != RECORD {
        return Err(CbhError::Truncated);
    }
    let flags = le16(bytes, 0);
    let kind = match le16(bytes, 2) {
        TYPE_GAME => EntryKind::Game,
        TYPE_TEXT => EntryKind::Text,
        n => EntryKind::Unknown((n >> 8) as u8),
    };
    let mut r = RawEntry {
        ordinal,
        kind,
        // `0x1401cd6b0` copies classic byte 0 into these flags unchanged.
        deleted: flags & 0x80 != 0,
        moves_offset: None,
        annotation_offset: None,
        white_id: None,
        black_id: None,
        tournament_id: None,
        annotator_id: None,
        source_id: None,
        white_team_id: None,
        black_team_id: None,
        date: None,
        result: None,
        result_code: None,
        white_elo: None,
        black_elo: None,
        round: None,
        eco_raw: None,
        year: None,
        move_count: 0,
        diagnostics: vec![],
    };
    let offset = |v: u64| if v == 0 { None } else { Some(v) };
    r.moves_offset = offset(le64(bytes, 8));
    if r.kind != EntryKind::Game {
        return Ok(r);
    }
    r.annotation_offset = offset(le64(bytes, 16));
    let d = &mut r.diagnostics;
    r.white_id = id(bytes, 24, d);
    r.black_id = id(bytes, 32, d);
    r.tournament_id = id(bytes, 40, d);
    r.annotator_id = id(bytes, 48, d);
    r.source_id = id(bytes, 56, d);
    // Same team names as the classic CBJ references (see formats.md).
    r.white_team_id = id(bytes, 64, d);
    r.black_team_id = id(bytes, 72, d);
    let packed = le32(bytes, 188);
    r.date = crate::header::date(packed);
    r.year = crate::header::year(packed);
    r.move_count = bytes[138];
    if packed != 0 && r.date.is_none() {
        r.diagnostics.push(Diagnostic::new(
            "invalidDate",
            "Date has an invalid calendar value",
        ));
    }
    r.result_code = Some(bytes[88]);
    r.result = crate::header::result(bytes[88], bytes[89]);
    if r.result.is_none() {
        r.diagnostics.push(Diagnostic::new(
            "unknownResult",
            "Result code is not interpreted",
        ));
    }
    let rating = |i| {
        // Same bits as the classic big-endian field; Elo is the low 12 bits.
        let elo = le16(bytes, i) & 4095;
        if elo == 0 { None } else { Some(elo) }
    };
    r.white_elo = rating(96);
    r.black_elo = rating(112);
    let (round, subround) = (le16(bytes, 90), le16(bytes, 92));
    if round > 0 {
        r.round = Some(if subround > 0 {
            format!("{round}.{subround}")
        } else {
            round.to_string()
        });
    }
    r.eco_raw = Some(le16(bytes, 128));
    Ok(r)
}
