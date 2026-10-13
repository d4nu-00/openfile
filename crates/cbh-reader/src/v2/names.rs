//! 2LID name lists (players, tournaments, sources, annotators, teams, titles).
//!
//! The file starts with a big-endian header: header size, list count, then
//! per list a slot size, a zero word, the record count and two all-ones words,
//! then an all-ones word, an uninterpreted word (1 in the sample) and one
//! (kind, 1) pair per list. Records are
//! stored in fixed groups: group `id` holds slot `id` of every list in header
//! order, so a record lives at `header + id * sum(slots) + sum(earlier slots)`.
//! A record is a LE32 payload length and the payload; strings inside it are
//! LE32-length-prefixed UTF-8. Recovered from the Mega Database 2025 sample and
//! cross-checked against the classic CBP/CBT names of every shared game.
use crate::{
    CbhError, Diagnostic,
    companions::CompanionSet,
    entities::{Cached, EntityText, NameCache, TournamentText, missing},
    source::SourceFile,
};

/// Companions whose content this reader interprets.
pub const DECODED_COMPANIONS: &[&str] = &["2cbh", "2cbg", "2cba", "2lid"];

pub const PLAYERS: usize = 0;
pub const TOURNAMENTS: usize = 1;
pub const LISTS: usize = 6;
const MAX_HEADER: usize = 4096;
const MAX_SLOT: u32 = 64 * 1024;

#[derive(Debug)]
struct List {
    offset: u64,
    slot: u32,
    count: u32,
}

struct Lists {
    file: SourceFile,
    group: u64,
    start: u64,
    lists: Vec<List>,
}

fn be32(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes(b[i..i + 4].try_into().unwrap())
}

impl Lists {
    fn open(path: &std::path::Path) -> Result<Self, CbhError> {
        let file = SourceFile::open(path)?;
        let head = file.read_exact_range(0, 8)?;
        let (size, count) = (be32(&head, 0) as usize, be32(&head, 4) as usize);
        if count != LISTS || size != 8 + LISTS * 20 + 8 + LISTS * 8 || size > MAX_HEADER {
            return Err(CbhError::Unsupported(format!(
                "2LID header size {size} with {count} lists"
            )));
        }
        let head = file.read_exact_range(0, size)?;
        let mut lists = vec![];
        let mut offset = 0u64;
        for i in 0..LISTS {
            let at = 8 + i * 20;
            let (slot, zero, count) = (be32(&head, at), be32(&head, at + 4), be32(&head, at + 8));
            let kind_at = 8 + LISTS * 20 + 8 + i * 8;
            if zero != 0
                || !(4..=MAX_SLOT).contains(&slot)
                || be32(&head, kind_at) != i as u32
                || be32(&head, kind_at + 4) != 1
            {
                return Err(CbhError::Unsupported("Unknown 2LID list layout".into()));
            }
            lists.push(List {
                offset,
                slot,
                count,
            });
            offset += slot as u64;
        }
        Ok(Self {
            file,
            group: offset,
            start: size as u64,
            lists,
        })
    }
    fn record(&self, kind: usize, id: u32) -> Result<Vec<u8>, CbhError> {
        let list = &self.lists[kind];
        if id >= list.count {
            return Err(CbhError::Bounds);
        }
        let at = self
            .group
            .checked_mul(id as u64)
            .and_then(|v| v.checked_add(self.start + list.offset))
            .ok_or(CbhError::Bounds)?;
        let head = self.file.read_exact_range(at, 4)?;
        let length = u32::from_le_bytes(head[..].try_into().unwrap());
        if length > list.slot - 4 {
            return Err(CbhError::Decode(format!(
                "2LID record {id} exceeds its slot"
            )));
        }
        self.file.read_exact_range(at + 4, length as usize)
    }
}

/// Reads LE32-length-prefixed UTF-8 strings from a record payload.
struct Fields<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Fields<'_> {
    fn text(&mut self) -> Result<EntityText, CbhError> {
        let head = self
            .bytes
            .get(self.at..self.at + 4)
            .ok_or(CbhError::Truncated)?;
        let length = u32::from_le_bytes(head.try_into().unwrap()) as usize;
        let start = self.at + 4;
        let raw = self
            .bytes
            .get(start..start.checked_add(length).ok_or(CbhError::Bounds)?)
            .ok_or_else(|| CbhError::Decode("2LID string exceeds its record".into()))?;
        self.at = start + length;
        let mut diagnostics = vec![];
        let text = match std::str::from_utf8(raw) {
            Ok(s) => s.to_string(),
            Err(_) => {
                diagnostics.push(Diagnostic::new(
                    "unresolvedNameEncoding",
                    "Name bytes are not valid UTF-8; replacements are displayed",
                ));
                String::from_utf8_lossy(raw).into_owned()
            }
        };
        Ok(EntityText {
            text: if text.is_empty() { None } else { Some(text) },
            diagnostics,
        })
    }
}

pub struct NameIndex {
    lists: Option<Lists>,
    cache: NameCache,
}

impl NameIndex {
    pub fn open_partial(companions: &CompanionSet, budget: usize) -> (Self, Vec<Diagnostic>) {
        let mut diagnostics = vec![];
        let lists = companions
            .get("2lid")
            .and_then(|path| match Lists::open(path) {
                Ok(lists) => Some(lists),
                Err(error) => {
                    diagnostics.push(Diagnostic::new(
                        "unavailableNameTable",
                        format!("2lid: {error}"),
                    ));
                    None
                }
            });
        (
            Self {
                lists,
                cache: NameCache::new(budget),
            },
            diagnostics,
        )
    }
    pub fn player(&mut self, id: u32) -> Result<EntityText, CbhError> {
        if let Some(Cached::Player(v)) = self.cache.get((0, id)) {
            return Ok(v);
        }
        let Some(lists) = &self.lists else {
            return Ok(missing());
        };
        let bytes = lists.record(PLAYERS, id)?;
        let mut fields = Fields {
            bytes: &bytes,
            at: 0,
        };
        let last = fields.text()?;
        let first = fields.text()?;
        let mut diagnostics = last.diagnostics;
        diagnostics.extend(first.diagnostics);
        let text = match (last.text, first.text) {
            (Some(a), Some(b)) => Some(format!("{a}, {b}")),
            (a, None) => a,
            (None, b) => b,
        };
        let v = EntityText { text, diagnostics };
        self.cache.put((0, id), Cached::Player(v.clone()));
        Ok(v)
    }
    pub fn tournament(&mut self, id: u32) -> Result<TournamentText, CbhError> {
        if let Some(Cached::Tournament(v)) = self.cache.get((1, id)) {
            return Ok(v);
        }
        let Some(lists) = &self.lists else {
            return Ok(TournamentText {
                event: missing(),
                site: missing(),
            });
        };
        let bytes = lists.record(TOURNAMENTS, id)?;
        let mut fields = Fields {
            bytes: &bytes,
            at: 0,
        };
        let site = fields.text()?;
        let event = fields.text()?;
        let v = TournamentText { event, site };
        self.cache.put((1, id), Cached::Tournament(v.clone()));
        Ok(v)
    }
    pub fn verify_unchanged(&self) -> Result<(), CbhError> {
        self.lists
            .as_ref()
            .map_or(Ok(()), |l| l.file.verify_unchanged())
    }
    pub fn bytes_read(&self) -> u64 {
        self.lists.as_ref().map_or(0, |l| l.file.bytes_read())
    }
}
