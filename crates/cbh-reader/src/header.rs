use crate::{CbhError, Diagnostic, EntryKind};
#[derive(Debug)]
pub struct ClassicHeader {
    pub entry_count: u64,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug)]
pub struct RawEntry {
    pub ordinal: u64,
    pub kind: EntryKind,
    pub deleted: bool,
    pub moves_offset: Option<u64>,
    pub annotation_offset: Option<u64>,
    pub white_id: Option<u32>,
    pub black_id: Option<u32>,
    pub tournament_id: Option<u32>,
    pub annotator_id: Option<u32>,
    pub source_id: Option<u32>,
    pub date: Option<String>,
    pub result: Option<String>,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    pub round: Option<String>,
    pub eco_raw: Option<u16>,
    pub diagnostics: Vec<Diagnostic>,
}
pub fn be24(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]])
}
impl ClassicHeader {
    pub fn parse(bytes: &[u8], length: u64) -> Result<Self, CbhError> {
        if bytes.len() != 46 || length < 46 || !length.is_multiple_of(46) {
            return Err(CbhError::InvalidHeader);
        }
        if ![&[0, 0, 0x2c, 0, 0x2e, 1][..], &[0, 0, 0x24, 0, 0x2e, 1][..]].contains(&&bytes[..6]) {
            return Err(CbhError::Unsupported("Unknown classic CBH header".into()));
        }
        let short = be24(&bytes[7..10]) as u64;
        let version = u16::from_be_bytes([bytes[1], bytes[2]]);
        let count = if short == 0xffffff && version > 0x24 {
            u64::from_be_bytes(
                bytes[36..44]
                    .try_into()
                    .map_err(|_| CbhError::InvalidHeader)?,
            )
        } else {
            short
        };
        if count == 0 || count > length / 46 {
            return Err(CbhError::InvalidHeader);
        }
        let diagnostics = if count < length / 46 {
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
pub(crate) fn date(value: u32) -> Option<String> {
    if value == 0 {
        return None;
    }
    let y = (value >> 9) & 4095;
    let m = (value >> 5) & 15;
    let d = value & 31;
    if m > 12 || (m == 0 && d > 0) {
        return None;
    }
    let leap = y == 0 || (y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)));
    let max = match m {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if d > max {
        return None;
    }
    Some(format!(
        "{}.{}.{}",
        if y == 0 {
            "????".into()
        } else {
            format!("{y:04}")
        },
        if m == 0 {
            "??".into()
        } else {
            format!("{m:02}")
        },
        if d == 0 {
            "??".into()
        } else {
            format!("{d:02}")
        }
    ))
}
/// Packed ECO: bits 7-15 hold 1..=500 for A00..E99; 0 means none. The low
/// seven bits are a ChessBase sub-classification and are not shown.
pub(crate) fn eco(raw: u16) -> Option<String> {
    let code = raw >> 7;
    if code == 0 || code > 500 {
        return None;
    }
    let code = code - 1;
    Some(format!(
        "{}{:02}",
        (b'A' + (code / 100) as u8) as char,
        code % 100
    ))
}
pub(crate) fn result(code: u8) -> Option<String> {
    match code {
        0 => Some("0-1".into()),
        1 => Some("1/2-1/2".into()),
        2 => Some("1-0".into()),
        _ => None,
    }
}
pub fn parse_record(bytes: &[u8], ordinal: u64) -> Result<RawEntry, CbhError> {
    if bytes.len() != 46 {
        return Err(CbhError::Truncated);
    }
    if ordinal == 0 {
        return Err(CbhError::Bounds);
    }
    let kind = match bytes[0] & 15 {
        1 => EntryKind::Game,
        3 => EntryKind::Text,
        n => EntryKind::Unknown(n),
    };
    let mut r = RawEntry {
        ordinal,
        kind,
        deleted: bytes[0] & 0x80 != 0,
        moves_offset: None,
        annotation_offset: None,
        white_id: None,
        black_id: None,
        tournament_id: None,
        annotator_id: None,
        source_id: None,
        date: None,
        result: None,
        white_elo: None,
        black_elo: None,
        round: None,
        eco_raw: None,
        diagnostics: vec![],
    };
    if r.kind != EntryKind::Game {
        return Ok(r);
    }
    r.moves_offset = Some(u32::from_be_bytes(bytes[1..5].try_into().unwrap()).into());
    let a = u32::from_be_bytes(bytes[5..9].try_into().unwrap());
    r.annotation_offset = if a == 0 { None } else { Some(a.into()) };
    r.white_id = Some(be24(&bytes[9..12]));
    r.black_id = Some(be24(&bytes[12..15]));
    r.tournament_id = Some(be24(&bytes[15..18]));
    r.annotator_id = Some(be24(&bytes[18..21]));
    r.source_id = Some(be24(&bytes[21..24]));
    let packed = be24(&bytes[24..27]);
    r.date = date(packed);
    if packed != 0 && r.date.is_none() {
        r.diagnostics.push(Diagnostic::new(
            "invalidDate",
            "Date has an invalid calendar value",
        ));
    }
    r.result = result(bytes[27]);
    if r.result.is_none() {
        r.diagnostics.push(Diagnostic::new(
            "unknownResult",
            "Result code is not interpreted",
        ));
    }
    let rating = |i| {
        let elo = u16::from_be_bytes([bytes[i], bytes[i + 1]]) & 4095;
        if elo == 0 { None } else { Some(elo) }
    };
    r.white_elo = rating(31);
    r.black_elo = rating(33);
    if bytes[29] > 0 {
        r.round = Some(if bytes[30] > 0 {
            format!("{}.{}", bytes[29], bytes[30])
        } else {
            bytes[29].to_string()
        });
    }
    r.eco_raw = Some(u16::from_be_bytes([bytes[35], bytes[36]]));
    Ok(r)
}
