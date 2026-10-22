//! Game annotations from classic CBA records and 2CBA blocks.
//!
//! Both encodings carry the same entries. Each entry has a position, a type
//! and type-specific data. Position -1 is the start of the game and position
//! k >= 0 is the (k+1)-th move in an order that depends on the encoding:
//!
//! * Classic CBA uses stream (pre-)order: a move's continuation before its
//!   alternatives, which is the classic CBG order.
//! * 2CBA uses line order: walk a line move by move, and right after a move
//!   number its alternative variations completely before continuing.
//!
//! Both orders were established on the test database. All 16,202 texts that
//! occur once in their game land on the same move from either file.
//!
//! Type ids come from the annotation factory `0x1401a2ae0` in the reference binary.
//! Classic record layouts come from the classic serializers (`0x140190020`
//! text, `0x14018fdc0` symbols, `0x14018fd10` squares, `0x14018f310` arrows).
//! 2CBA layouts come from each class's stream reader (vtable slot 4). Colour
//! codes come from the PGN `[%csl]` importer `0x140e98150`, and language codes
//! from the table read by `0x140e94770`. See docs/formats.md.
use crate::{CbhError, DataStatus, Diagnostic};
use chess_core::{ArrowMark, Game, NodeId, SquareMark};
use std::collections::BTreeMap;

pub const TEXT_AFTER: u16 = 0x02;
pub const SYMBOLS: u16 = 0x03;
pub const SQUARES: u16 = 0x04;
pub const ARROWS: u16 = 0x05;
pub const TEXT_ALT: u16 = 0x20;
pub const TEXT_BEFORE: u16 = 0x82;
/// A critical position: one byte, 1 opening, 2 middlegame, 3 endgame.
pub const CRITICAL: u16 = 0x18;
/// A training question: a presence flag, the item's kind and the item.
pub const TRAINING: u16 = 0x09;
/// An evaluation profile: one packed evaluation per index.
pub const EVAL_PROFILE: u16 = 0x26;
/// Entries an evaluation profile holds at most (`EvaluationArr`, 2,001).
pub const PROFILE_ENTRIES: usize = 2001;

/// Classic language byte for each 2CBA language index. Index 7 (byte 0) is
/// "no language"; indices 0, 1 and 2 (English, German, French) are confirmed
/// by the sample texts.
const LANGUAGE_CODES: [u8; 21] = [
    42, 53, 49, 43, 70, 103, 117, 0, 129, 134, 252, 128, 116, 36, 251, 142, 62, 121, 55, 120, 30,
];
pub const NEUTRAL_LANGUAGE: u16 = 7;
const LANGUAGE_NAMES: [&str; 7] = ["en", "de", "fr", "es", "it", "nl", "sl"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Text {
        before: bool,
        /// 2CBA language index; `None` for an unknown classic language byte.
        language: Option<u16>,
        text: String,
    },
    /// Move, position and prefix symbols as PGN NAG numbers (0 = none).
    Symbols([u8; 3]),
    /// (colour code, square 1..=64) pairs.
    Squares(Vec<(u8, u8)>),
    /// (colour code, from, to) triples.
    Arrows(Vec<(u8, u8, u8)>),
    /// A critical position (type 0x18): 1 opening, 2 middlegame, 3 endgame,
    /// as the reference app's three commands set it (`0x1408e0cb0`, `0x1408e0c50`,
    /// `0x1408e0bf0`). Kept for searching; not shown.
    Critical(u8),
    /// A training question (type 0x09), see docs/training.md.
    Training(Box<chess_core::training::Training>),
    /// An evaluation profile (type 0x26): packed evaluations by index, see
    /// docs/evaluation-profile.md.
    EvalProfile(Vec<u32>),
    /// A recognised type this reader does not represent.
    Other(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub position: i32,
    pub kind: Kind,
}

#[derive(Debug, Default)]
pub struct Parsed {
    pub entries: Vec<Entry>,
    /// Set when parsing stopped at an entry whose size cannot be determined.
    pub stopped_at: Option<(i32, u16)>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Windows-1252 for 0x80..=0x9f; other bytes map to the same code point.
const CP1252: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// The reference app marks text as UTF-8 when it is valid UTF-8 with non-ASCII bytes
/// (`SString::MayBeUTF8`); other text is read as Windows-1252. Text ends at
/// the first NUL, as the reference app's own 2CBA copy of test database entry 7289 shows.
pub fn decode_text(bytes: &[u8]) -> String {
    let bytes = &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())];
    if !bytes.is_ascii()
        && let Ok(s) = std::str::from_utf8(bytes)
    {
        return s.to_string();
    }
    bytes.iter().map(|&b| cp1252(b)).collect()
}

/// One Windows-1252 byte.
pub(crate) fn cp1252(b: u8) -> char {
    match b {
        0x80..=0x9f => CP1252[(b - 0x80) as usize],
        _ => b as char,
    }
}

fn classic_language(code: u8) -> Option<u16> {
    LANGUAGE_CODES
        .iter()
        .position(|&c| c == code)
        .map(|i| i as u16)
}

pub fn language_name(index: Option<u16>) -> String {
    match index {
        Some(i) if (i as usize) < LANGUAGE_NAMES.len() => LANGUAGE_NAMES[i as usize].into(),
        Some(NEUTRAL_LANGUAGE) => "none".into(),
        Some(i) => format!("language {i}"),
        None => "unknown".into(),
    }
}

fn text(ty: u16, language: Option<u16>, bytes: &[u8]) -> Kind {
    Kind::Text {
        before: ty == TEXT_BEFORE,
        language,
        text: decode_text(bytes),
    }
}

fn pairs(data: &[u8]) -> Option<Kind> {
    data.len().is_multiple_of(2).then(|| {
        Kind::Squares(
            data.as_chunks::<2>()
                .0
                .iter()
                .map(|c| (c[0], c[1]))
                .collect(),
        )
    })
}

fn triples(data: &[u8]) -> Option<Kind> {
    data.len().is_multiple_of(3).then(|| {
        Kind::Arrows(
            data.as_chunks::<3>()
                .0
                .iter()
                .map(|c| (c[0], c[1], c[2]))
                .collect(),
        )
    })
}

/// Classic CBA: a 14-byte header (BE24 game number, BE32 total length at 10),
/// then records of BE24 signed position, type byte, BE16 length including the
/// six-byte record header, and data.
pub fn parse_classic(bytes: &[u8], ordinal: u64) -> Result<Parsed, CbhError> {
    if bytes.len() < 14 {
        return Err(CbhError::Truncated);
    }
    let total = u32::from_be_bytes(bytes[10..14].try_into().unwrap()) as usize;
    if total != bytes.len() {
        return Err(CbhError::Decode(
            "CBA length does not match its header".into(),
        ));
    }
    let mut parsed = Parsed::default();
    let game = crate::header::be24(&bytes[..3]) as u64;
    if game != ordinal {
        parsed.diagnostics.push(Diagnostic::new(
            "annotationGameNumber",
            format!("CBA record names game {game}"),
        ));
    }
    let mut at = 14;
    while at < total {
        let head = bytes.get(at..at + 6).ok_or(CbhError::Truncated)?;
        let position = (i32::from_be_bytes([head[0], head[1], head[2], 0])) >> 8;
        let ty = head[3] as u16;
        let length = u16::from_be_bytes([head[4], head[5]]) as usize;
        if length < 6 || at + length > total {
            return Err(CbhError::Decode(format!(
                "CBA record length {length} at {at}"
            )));
        }
        let data = &bytes[at + 6..at + length];
        let kind = match ty {
            TEXT_AFTER | TEXT_BEFORE | TEXT_ALT => {
                if data.len() < 2 {
                    return Err(CbhError::Decode("Short CBA text record".into()));
                }
                text(ty, classic_language(data[1]), &data[2..])
            }
            SYMBOLS if data.len() <= 3 => {
                let mut nags = [0; 3];
                nags[..data.len()].copy_from_slice(data);
                Kind::Symbols(nags)
            }
            SQUARES => pairs(data).ok_or_else(|| CbhError::Decode("Odd square list".into()))?,
            ARROWS => triples(data).ok_or_else(|| CbhError::Decode("Uneven arrow list".into()))?,
            SYMBOLS => return Err(CbhError::Decode("Long CBA symbol record".into())),
            0x18 if !data.is_empty() => Kind::Critical(data[0]),
            EVAL_PROFILE => classic_profile(data)?,
            TRAINING => match classic_training(data)? {
                Some(t) => Kind::Training(Box::new(t)),
                None => Kind::Other(TRAINING),
            },
            other => Kind::Other(other),
        };
        parsed.entries.push(Entry { position, kind });
        at += length;
    }
    Ok(parsed)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], CbhError> {
        let end = self.at.checked_add(n).ok_or(CbhError::Bounds)?;
        let slice = self.bytes.get(self.at..end).ok_or(CbhError::Truncated)?;
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, CbhError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, CbhError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, CbhError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    /// LE32 length and bytes, at most 0xfff0 as in the native reader.
    fn string(&mut self) -> Result<&[u8], CbhError> {
        let length = self.u32()? as usize;
        if length > 0xfff0 {
            return Err(CbhError::Decode("2CBA string exceeds 0xfff0 bytes".into()));
        }
        self.take(length)
    }
}

/// 2CBA payload: repeated LE32 position and LE32 count, then per entry a LE16
/// type and the type's stream data, until position 0x7fffffff.
pub fn parse_new(payload: &[u8]) -> Result<Parsed, CbhError> {
    let mut parsed = Parsed::default();
    let mut c = Cursor {
        bytes: payload,
        at: 0,
    };
    let mut last = i32::MIN;
    loop {
        if c.at == payload.len() {
            break;
        }
        let position = c.u32()? as i32;
        if position == i32::MAX {
            if c.at != payload.len() {
                return Err(CbhError::Decode("Data after the 2CBA end marker".into()));
            }
            break;
        }
        if position <= last || position < -1 {
            return Err(CbhError::Decode(format!(
                "2CBA position {position} out of order"
            )));
        }
        last = position;
        let count = c.u32()?;
        if count as usize > payload.len() {
            return Err(CbhError::Decode(
                "2CBA entry count exceeds the block".into(),
            ));
        }
        for _ in 0..count {
            let ty = c.u16()?;
            let kind = match ty {
                TEXT_AFTER | TEXT_BEFORE | TEXT_ALT => {
                    let _flags = c.u16()?;
                    let language = c.u16()?;
                    let bytes = c.string()?;
                    text(ty, Some(language), bytes)
                }
                SYMBOLS => {
                    let b = c.take(3)?;
                    Kind::Symbols([b[0], b[1], b[2]])
                }
                SQUARES => {
                    pairs(c.string()?).ok_or_else(|| CbhError::Decode("Odd square list".into()))?
                }
                ARROWS => triples(c.string()?)
                    .ok_or_else(|| CbhError::Decode("Uneven arrow list".into()))?,
                // Pre-move squares/arrows, piece path, audio, picture, video.
                0x84 | 0x85 | 0x15 | 0x10 | 0x11 | 0x12 => {
                    c.string()?;
                    Kind::Other(ty)
                }
                0x06 => Kind::Other(ty),
                0x01 | 0x07 | 0x08 | 0x16 | 0x17 | 0x22 | 0x23 | 0x25 => {
                    c.u32()?;
                    Kind::Other(ty)
                }
                0x14 => {
                    c.u8()?;
                    Kind::Other(ty)
                }
                0x18 => Kind::Critical(c.u8()?),
                // Two presence bytes (`0x140190a10`, `0x140b93fa0`), the kind
                // as LE32, then the item; an item of an unknown kind cannot be
                // sized.
                TRAINING => {
                    if c.u8()? == 0 || c.u8()? == 0 {
                        continue;
                    }
                    let code = c.u32()?;
                    let Some(kind) = chess_core::training::TrainingKind::from_code(code) else {
                        parsed.stopped_at = Some((position, ty));
                        return Ok(parsed);
                    };
                    Kind::Training(Box::new(training_item(&mut c, kind, false)?))
                }
                0x27 => {
                    c.u16()?;
                    Kind::Other(ty)
                }
                0x28 => {
                    c.take(4)?;
                    Kind::Other(ty)
                }
                0x21 => {
                    c.take(6)?;
                    Kind::Other(ty)
                }
                0x0a => {
                    c.string()?;
                    c.u32()?;
                    Kind::Other(ty)
                }
                0x1c => {
                    if c.u8()? != 0 {
                        c.string()?;
                        c.string()?;
                    }
                    Kind::Other(ty)
                }
                0x24 => {
                    if c.u8()? != 0 {
                        c.take(3 * 11)?;
                    }
                    c.take(4)?;
                    Kind::Other(ty)
                }
                // Evaluation profile: presence byte, then a length-prefixed
                // sub-block (`0x140f04eb0`).
                EVAL_PROFILE => {
                    if c.u8()? == 0 {
                        continue;
                    }
                    let length = c.u32()? as usize;
                    new_profile(c.take(length)?)?
                }
                // Training, game quotation, correspondence and unknown types
                // nest further structures whose size is not recovered.
                other => {
                    parsed.stopped_at = Some((position, other));
                    return Ok(parsed);
                }
            };
            parsed.entries.push(Entry { position, kind });
        }
    }
    Ok(parsed)
}

/// A classic profile (`0x140c4dfe0`): BE16 count, then BE32 evaluations.
fn classic_profile(data: &[u8]) -> Result<Kind, CbhError> {
    let count = data
        .get(..2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
        .ok_or_else(|| CbhError::Decode("Short evaluation profile".into()))?;
    if count > PROFILE_ENTRIES || data.len() != 2 + 4 * count {
        return Err(CbhError::Decode("Evaluation profile length".into()));
    }
    Ok(Kind::EvalProfile(
        data[2..2 + 4 * count]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_be_bytes(*b))
            .collect(),
    ))
}
/// A 2CBA profile sub-block (`0x140c4e9b0`): LE16 count, then LE32
/// evaluations.
fn new_profile(block: &[u8]) -> Result<Kind, CbhError> {
    let count = block
        .get(..2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
        .ok_or_else(|| CbhError::Decode("Short evaluation profile".into()))?;
    if count > PROFILE_ENTRIES || block.len() != 2 + 4 * count {
        return Err(CbhError::Decode("Evaluation profile length".into()));
    }
    Ok(Kind::EvalProfile(
        block[2..2 + 4 * count]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect(),
    ))
}

/// A classic CBA training record's data (`0x140b92060`): an LE16 presence
/// word, the kind, the item's LE16 length (counting itself), then the item
/// with LE16-length strings. None for no item or an unknown kind.
fn classic_training(data: &[u8]) -> Result<Option<chess_core::training::Training>, CbhError> {
    let mut c = Cursor { bytes: data, at: 0 };
    if c.u16()? == 0 {
        return Ok(None);
    }
    let Some(kind) = chess_core::training::TrainingKind::from_code(u32::from(c.u8()?)) else {
        return Ok(None);
    };
    let _length = c.u16()?;
    let t = training_item(&mut c, kind, true)?;
    if c.at != data.len() {
        return Err(CbhError::Decode(
            "CBA training record has trailing data".into(),
        ));
    }
    Ok(Some(t))
}

/// Strings of one training text block at most (9 slots × 2).
const MAX_BLOCK_STRINGS: u16 = 18;

/// A training item (`0x140b94030` and the kind's reader): seconds, score,
/// four text blocks, then the answers. Strings have an LE16 length in CBA
/// and an LE32 length in 2CBA.
fn training_item(
    c: &mut Cursor,
    kind: chess_core::training::TrainingKind,
    classic: bool,
) -> Result<chess_core::training::Training, CbhError> {
    use chess_core::training::{Answer, Training, TrainingKind};
    let seconds = c.u32()? as i32;
    let score = c.u16()? as i16;
    let question = training_block(c, classic)?;
    let wrong = training_block(c, classic)?;
    let help = [training_block(c, classic)?, training_block(c, classic)?];
    let count = c.u8()?;
    let mut answers = Vec::with_capacity(count as usize);
    for _ in 0..count {
        answers.push(match kind {
            TrainingKind::MoveInput => {
                let b = c.take(4)?;
                let (from, to, extra, points) = (b[0], b[1], b[2], b[3]);
                Answer {
                    from,
                    to,
                    extra,
                    points,
                    feedback: training_block(c, classic)?,
                    ..Default::default()
                }
            }
            TrainingKind::MultipleChoice => {
                let points = c.u8()?;
                let choice = training_block(c, classic)?;
                Answer {
                    points,
                    choice,
                    feedback: training_block(c, classic)?,
                    ..Default::default()
                }
            }
        });
    }
    Ok(Training {
        kind,
        seconds,
        score,
        question,
        wrong,
        help,
        answers,
    })
}

/// A training text block: a count, then per string its slot (0–6 a
/// language, 7 arrows, 8 coloured squares), its index and the string.
fn training_block(c: &mut Cursor, classic: bool) -> Result<chess_core::training::Block, CbhError> {
    use chess_core::training::{ARROW_SLOT, Block, RawString, SQUARE_SLOT, Text};
    let count = c.u16()?;
    if count > MAX_BLOCK_STRINGS {
        return Err(CbhError::Decode(format!(
            "Training text block of {count} strings"
        )));
    }
    let mut block = Block::default();
    for _ in 0..count {
        let slot = c.u8()?;
        let index = c.u8()?;
        let bytes = if classic {
            let length = c.u16()? as usize;
            c.take(length)?
        } else {
            c.string()?
        };
        let raw = || RawString {
            slot,
            index,
            bytes: bytes.to_vec(),
        };
        match (slot, index) {
            (0..=6, 0..=1) => block.texts.push(Text {
                language: slot,
                index,
                text: decode_text(bytes),
            }),
            (ARROW_SLOT, 0) => {
                let marks: Option<Vec<ArrowMark>> = bytes
                    .chunks(3)
                    .map(|t| {
                        (t.len() == 3).then_some(())?;
                        Some(ArrowMark {
                            color: color(t[0])?,
                            from: square(t[1])?,
                            to: square(t[2])?,
                        })
                    })
                    .collect();
                match marks {
                    Some(m) => block.arrows.extend(m),
                    None => block.other.push(raw()),
                }
            }
            (SQUARE_SLOT, 0) => {
                let marks: Option<Vec<SquareMark>> = bytes
                    .chunks(2)
                    .map(|t| {
                        (t.len() == 2).then_some(())?;
                        Some(SquareMark {
                            color: color(t[0])?,
                            square: square(t[1])?,
                        })
                    })
                    .collect();
                match marks {
                    Some(m) => block.squares.extend(m),
                    None => block.other.push(raw()),
                }
            }
            _ => block.other.push(raw()),
        }
    }
    Ok(block)
}

fn color(code: u8) -> Option<char> {
    // Stored codes are the importer's colour index plus one.
    match code {
        2 => Some('G'),
        3 => Some('Y'),
        4 => Some('R'),
        7 => Some('B'),
        8 => Some('C'),
        9 => Some('O'),
        _ => None,
    }
}

fn square(code: u8) -> Option<String> {
    let index = code.checked_sub(1).filter(|&i| i < 64)?;
    Some(format!(
        "{}{}",
        (b'a' + index / 8) as char,
        (b'1' + index % 8) as char
    ))
}

/// Applies parsed entries to a decoded game and returns the resulting status
/// and diagnostics. Text is kept in one language per game (English when
/// present, otherwise the most common one) plus language-neutral text; other
/// translations are counted, not shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Classic CBA positions.
    Stream,
    /// 2CBA positions.
    Line,
}

/// Node ids for positions 0, 1, ... in the given order.
pub fn positions(game: &Game, order: Order) -> Vec<NodeId> {
    let mut out = Vec::with_capacity(game.nodes.len());
    match order {
        Order::Stream => {
            let mut stack: Vec<NodeId> = game.nodes[0].children.iter().rev().copied().collect();
            while let Some(id) = stack.pop() {
                out.push(id);
                stack.extend(game.nodes[id as usize].children.iter().rev());
            }
        }
        Order::Line => {
            // Explicit stack of line cursors instead of recursion.
            let mut lines: Vec<Option<NodeId>> = vec![game.nodes[0].children.first().copied()];
            while let Some(top) = lines.last_mut() {
                let Some(id) = *top else {
                    lines.pop();
                    continue;
                };
                out.push(id);
                let node = &game.nodes[id as usize];
                *top = node.children.first().copied();
                let siblings = node
                    .parent
                    .map(|p| &game.nodes[p as usize].children[..])
                    .unwrap_or(&[]);
                if siblings.first() == Some(&id) {
                    for &alt in siblings[1..].iter().rev() {
                        lines.push(Some(alt));
                    }
                }
            }
        }
    }
    out
}

pub fn apply(game: &mut Game, parsed: Parsed, order: Order) -> (DataStatus, Vec<Diagnostic>) {
    let positions = positions(game, order);
    let parsed_empty = parsed.entries.is_empty() && parsed.stopped_at.is_none();
    let mut diagnostics = parsed.diagnostics;
    let mut complete = true;
    let mut languages: BTreeMap<Option<u16>, usize> = BTreeMap::new();
    for entry in &parsed.entries {
        if let Kind::Text { language, .. } = &entry.kind
            && *language != Some(NEUTRAL_LANGUAGE)
        {
            *languages.entry(*language).or_default() += 1;
        }
    }
    let chosen = if languages.contains_key(&Some(0)) {
        Some(Some(0))
    } else {
        languages
            .iter()
            .max_by_key(|(lang, n)| (**n, std::cmp::Reverse(**lang)))
            .map(|(l, _)| *l)
    };
    let (mut omitted_text, mut bad_marks, mut out_of_range) = (0, 0, 0);
    let mut other: BTreeMap<u16, usize> = BTreeMap::new();
    for entry in parsed.entries {
        // A profile belongs to the game's start and its main line.
        if let (-1, Kind::EvalProfile(values)) = (entry.position, &entry.kind) {
            game.profile = values.clone();
            continue;
        }
        let node = match entry.position {
            -1 => 0,
            k if k >= 0 && (k as usize) < positions.len() => positions[k as usize] as usize,
            _ => {
                out_of_range += 1;
                continue;
            }
        };
        let node = &mut game.nodes[node];
        match entry.kind {
            Kind::Text {
                before,
                language,
                text,
            } => {
                if language != Some(NEUTRAL_LANGUAGE) && Some(language) != chosen {
                    omitted_text += 1;
                } else if before && node.parent.is_some() {
                    node.pre_comments.push(text);
                } else {
                    node.comments.push(text);
                }
            }
            Kind::Symbols(nags) => {
                for nag in nags.into_iter().filter(|&n| n != 0).map(u16::from) {
                    if !node.nags.contains(&nag) {
                        node.nags.push(nag);
                    }
                }
            }
            Kind::Squares(items) => {
                for (c, s) in items {
                    match (color(c), square(s)) {
                        (Some(color), Some(square)) => {
                            node.squares.push(SquareMark { color, square })
                        }
                        _ => bad_marks += 1,
                    }
                }
            }
            Kind::Arrows(items) => {
                for (c, f, t) in items {
                    match (color(c), square(f), square(t)) {
                        (Some(color), Some(from), Some(to)) => {
                            node.arrows.push(ArrowMark { color, from, to })
                        }
                        _ => bad_marks += 1,
                    }
                }
            }
            Kind::Other(ty) => *other.entry(ty).or_default() += 1,
            Kind::EvalProfile(_) => *other.entry(EVAL_PROFILE).or_default() += 1,
            Kind::Critical(kind @ 1..=3) => node.critical = Some(kind),
            Kind::Training(t) => node.training = Some(t),
            Kind::Critical(_) => *other.entry(CRITICAL).or_default() += 1,
        }
    }
    if omitted_text > 0 {
        let name = language_name(chosen.flatten());
        diagnostics.push(Diagnostic::new(
            "otherLanguageText",
            format!(
                "{omitted_text} text annotation(s) in languages other than {name} are not shown"
            ),
        ));
    }
    if !other.is_empty() {
        complete = false;
        let list: Vec<_> = other
            .iter()
            .map(|(ty, n)| format!("{ty:#04x}×{n}"))
            .collect();
        diagnostics.push(Diagnostic::new(
            "unsupportedAnnotation",
            format!("Annotation types not shown: {}", list.join(", ")),
        ));
    }
    if bad_marks > 0 {
        complete = false;
        diagnostics.push(Diagnostic::new(
            "unsupportedMark",
            format!("{bad_marks} square or arrow mark(s) have unknown colours or squares"),
        ));
    }
    if out_of_range > 0 {
        complete = false;
        diagnostics.push(Diagnostic::new(
            "annotationOutOfRange",
            format!("{out_of_range} annotation(s) refer to moves outside the game"),
        ));
    }
    if let Some((position, ty)) = parsed.stopped_at {
        complete = false;
        diagnostics.push(Diagnostic::new(
            "annotationsStopped",
            format!(
                "2CBA annotation type {ty:#04x} at position {position} is not decoded; \
                 later annotations of this game are not shown"
            ),
        ));
    }
    let status = if parsed_empty {
        DataStatus::Missing
    } else if complete && omitted_text == 0 {
        DataStatus::Available
    } else {
        DataStatus::Partial
    };
    (status, diagnostics)
}
