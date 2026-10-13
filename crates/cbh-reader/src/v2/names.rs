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
    entities::{
        ANNOTATOR, Cached, EntityText, NameCache, PLAYER, SOURCE, SourceText, TEAM, TOURNAMENT,
        TournamentText, join_names, missing,
    },
    source::SourceFile,
};

/// Companions whose content this reader interprets.
pub const DECODED_COMPANIONS: &[&str] = &["2cbh", "2cbg", "2cba", "2lid"];

pub const PLAYERS: usize = 0;
pub const TOURNAMENTS: usize = 1;
pub const SOURCES: usize = 2;
/// Never referenced in the sample: 2CBH annotator ids index the player list.
pub const ANNOTATORS: usize = 3;
pub const TEAMS: usize = 4;
/// Multilingual titles of text entries.
pub const TITLES: usize = 5;
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
    fn le32(&mut self) -> Result<u32, CbhError> {
        let v = self
            .bytes
            .get(self.at..self.at + 4)
            .ok_or(CbhError::Truncated)?;
        self.at += 4;
        Ok(u32::from_le_bytes(v.try_into().unwrap()))
    }
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
    fn fields(&self, kind: usize, id: u32) -> Result<Option<Vec<u8>>, CbhError> {
        self.lists.as_ref().map(|l| l.record(kind, id)).transpose()
    }
    /// Player names: last name, then first name.
    fn person(&mut self, key: u8, id: u32) -> Result<EntityText, CbhError> {
        if let Some(Cached::Name(v)) = self.cache.get((key, id)) {
            return Ok(v);
        }
        let Some(bytes) = self.fields(PLAYERS, id)? else {
            return Ok(missing());
        };
        let mut fields = Fields {
            bytes: &bytes,
            at: 0,
        };
        let last = fields.text()?;
        let v = join_names(last, fields.text()?);
        self.cache.put((key, id), Cached::Name(v.clone()));
        Ok(v)
    }
    pub fn player(&mut self, id: u32) -> Result<EntityText, CbhError> {
        self.person(PLAYER, id)
    }
    /// 2CBH annotator ids refer to the player list: every annotated Mega game
    /// names the same person here as the classic CBC annotator.
    pub fn annotator(&mut self, id: u32) -> Result<EntityText, CbhError> {
        self.person(ANNOTATOR, id)
    }
    pub fn tournament(&mut self, id: u32) -> Result<TournamentText, CbhError> {
        if let Some(Cached::Tournament(v)) = self.cache.get((TOURNAMENT, id)) {
            return Ok(v);
        }
        let Some(bytes) = self.fields(TOURNAMENTS, id)? else {
            return Ok(TournamentText {
                event: missing(),
                site: missing(),
            });
        };
        let mut fields = Fields {
            bytes: &bytes,
            at: 0,
        };
        let site = fields.text()?;
        let event = fields.text()?;
        let v = TournamentText { event, site };
        self.cache
            .put((TOURNAMENT, id), Cached::Tournament(v.clone()));
        Ok(v)
    }
    /// Sources: title, publisher, then LE32 publication and version dates
    /// packed as in CBH; the remaining four bytes are not interpreted.
    pub fn source(&mut self, id: u32) -> Result<SourceText, CbhError> {
        if let Some(Cached::Source(v)) = self.cache.get((SOURCE, id)) {
            return Ok(v);
        }
        let Some(bytes) = self.fields(SOURCES, id)? else {
            return Ok(SourceText {
                title: missing(),
                publisher: missing(),
                date: None,
            });
        };
        let mut fields = Fields {
            bytes: &bytes,
            at: 0,
        };
        let title = fields.text()?;
        let publisher = fields.text()?;
        let date = crate::header::date(fields.le32()?);
        let v = SourceText {
            title,
            publisher,
            date,
        };
        self.cache.put((SOURCE, id), Cached::Source(v.clone()));
        Ok(v)
    }
    /// Teams: the name, then five bytes that hold the year (LE16 at +2) and
    /// nation (+4) as in CBE; they are not interpreted.
    pub fn team(&mut self, id: u32) -> Result<EntityText, CbhError> {
        if let Some(Cached::Name(v)) = self.cache.get((TEAM, id)) {
            return Ok(v);
        }
        let Some(bytes) = self.fields(TEAMS, id)? else {
            return Ok(missing());
        };
        let v = Fields {
            bytes: &bytes,
            at: 0,
        }
        .text()?;
        self.cache.put((TEAM, id), Cached::Name(v.clone()));
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
