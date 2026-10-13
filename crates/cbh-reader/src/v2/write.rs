//! Creating and changing 2CBH databases. See docs/writing.md for the
//! evidence behind each layout and for the write order.
//!
//! The writer owns the four files it changes (`.2cbh`, `.2cbg`, `.2cba`,
//! `.2lid`). It never writes `.2lcd` or `.2lgd`, which are ChessBase indexes
//! over the name lists. Every operation first checks that no other program
//! changed the files since the writer last saw them.
use super::{
    block::{FILE_HEADER, MARKER, TYPE_ANNOTATIONS, TYPE_GAME, checksum},
    encode::{Omitted, encode_annotations, encode_moves},
    names::LISTS,
    record::RECORD,
};
use crate::CbhError;
use chess_core::Game;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// 2LID slot sizes: players, tournaments, sources, annotators, teams, titles.
const SLOTS: [u32; LISTS] = [1024, 1120, 220, 1024, 314, 532];
const LID_HEADER: u64 = 184;
/// Block slack after the payload, as in the Mega copy (see writing.md).
const GAME_SLACK: usize = 104;
const NOTES_SLACK: usize = 200;
/// Tail of every player record in the sample.
const PLAYER_TAIL: [u8; 24] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 8, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff,
];
const TOURNAMENT_TAIL: usize = 108;
const SOURCE_TAIL: [u8; 4] = [1, 0, 1, 0];
const NONE: u64 = u64::MAX;

/// File extensions the writer creates and changes.
pub const WRITTEN: [&str; 4] = ["2cbh", "2cbg", "2cba", "2lid"];

struct Tracked {
    path: PathBuf,
    file: File,
    seen: (u64, Option<SystemTime>),
}

impl Tracked {
    fn stamp(file: &File) -> Result<(u64, Option<SystemTime>), CbhError> {
        let meta = file.metadata()?;
        Ok((meta.len(), meta.modified().ok()))
    }
    fn open(path: PathBuf, create: bool) -> Result<Self, CbhError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(create)
            .open(&path)?;
        let seen = Self::stamp(&file)?;
        Ok(Self { path, file, seen })
    }
    fn check(&self) -> Result<(), CbhError> {
        let now = std::fs::metadata(&self.path)?;
        if (now.len(), now.modified().ok()) != self.seen {
            return Err(CbhError::SourceChanged);
        }
        Ok(())
    }
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> Result<(), CbhError> {
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(bytes)?;
        Ok(())
    }
    fn read_at(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, CbhError> {
        let mut bytes = vec![0; length];
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
    fn sync(&mut self) -> Result<(), CbhError> {
        self.file.sync_all()?;
        self.seen = Self::stamp(&self.file)?;
        Ok(())
    }
}

/// A frame around one payload; padding brings the block to a multiple of
/// eight bytes plus the slack ChessBase leaves.
fn frame(kind: u16, payload: &[u8], slack: usize) -> Vec<u8> {
    let padding = slack - (34 + payload.len()) % 8;
    let total = (34 + payload.len() + padding) as u64;
    let mut b = Vec::with_capacity(total as usize);
    b.extend(MARKER.to_le_bytes());
    b.extend((payload.len() as u32).to_le_bytes());
    b.extend((padding as u32).to_le_bytes());
    b.extend(checksum(payload).to_le_bytes());
    b.extend(kind.to_le_bytes());
    b.extend(payload);
    b.resize(b.len() + padding, 0);
    b.extend(total.to_le_bytes());
    b
}

/// A stream file (2CBG or 2CBA): LE64 used length, LE16 header size 12 and a
/// LE16 version word.
struct Stream {
    file: Tracked,
    used: u64,
}

impl Stream {
    fn create(path: PathBuf, version: u16) -> Result<Self, CbhError> {
        let mut file = Tracked::open(path, true)?;
        let mut head = FILE_HEADER.to_le_bytes().to_vec();
        head.extend(12u16.to_le_bytes());
        head.extend(version.to_le_bytes());
        file.write_at(0, &head)?;
        Ok(Self {
            file,
            used: FILE_HEADER,
        })
    }
    fn open(path: PathBuf) -> Result<Self, CbhError> {
        let mut file = Tracked::open(path, false)?;
        let head = file.read_at(0, FILE_HEADER as usize)?;
        let used = u64::from_le_bytes(head[..8].try_into().unwrap());
        if u16::from_le_bytes([head[8], head[9]]) != 12 || used < FILE_HEADER || used > file.seen.0
        {
            return Err(CbhError::Unsupported(format!(
                "{}: unexpected stream header",
                file.path.display()
            )));
        }
        Ok(Self { file, used })
    }
    /// Appends a framed block at the used length; returns its offset.
    fn append(&mut self, block: &[u8]) -> Result<u64, CbhError> {
        let at = self.used;
        self.file.write_at(at, block)?;
        self.used += block.len() as u64;
        Ok(at)
    }
    fn commit(&mut self) -> Result<(), CbhError> {
        self.file.sync()?;
        self.file.write_at(0, &self.used.to_le_bytes())?;
        self.file.sync()
    }
}

/// Name kinds the writer fills.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Player = 0,
    Tournament = 1,
    Source = 2,
}

struct Lists {
    file: Tracked,
    counts: [u32; LISTS],
    /// Identifying strings to id, per kind, filled on first use.
    known: HashMap<Kind, HashMap<Vec<String>, u32>>,
}

fn lid_header(counts: &[u32; LISTS]) -> Vec<u8> {
    let mut h = vec![];
    for v in [LID_HEADER as u32, LISTS as u32] {
        h.extend(v.to_be_bytes());
    }
    for (slot, count) in SLOTS.iter().zip(counts) {
        for v in [*slot, 0, *count, u32::MAX, u32::MAX] {
            h.extend(v.to_be_bytes());
        }
    }
    for v in [u32::MAX, 1] {
        h.extend(v.to_be_bytes());
    }
    for kind in 0..LISTS as u32 {
        h.extend(kind.to_be_bytes());
        h.extend(1u32.to_be_bytes());
    }
    h
}

fn strings(values: &[&str]) -> Vec<u8> {
    let mut b = vec![];
    for v in values {
        b.extend((v.len() as u32).to_le_bytes());
        b.extend(v.as_bytes());
    }
    b
}

fn read_strings(payload: &[u8], n: usize) -> Option<Vec<String>> {
    let mut at = 0;
    let mut out = vec![];
    for _ in 0..n {
        let length = u32::from_le_bytes(payload.get(at..at + 4)?.try_into().ok()?) as usize;
        let bytes = payload.get(at + 4..at + 4 + length)?;
        out.push(String::from_utf8_lossy(bytes).into_owned());
        at += 4 + length;
    }
    Some(out)
}

impl Lists {
    fn group() -> u64 {
        SLOTS.iter().map(|&s| s as u64).sum()
    }
    fn offset(kind: usize, id: u32) -> u64 {
        LID_HEADER
            + id as u64 * Self::group()
            + SLOTS[..kind].iter().map(|&s| s as u64).sum::<u64>()
    }
    fn create(path: PathBuf) -> Result<Self, CbhError> {
        let mut file = Tracked::open(path, true)?;
        let counts = [0; LISTS];
        file.write_at(0, &lid_header(&counts))?;
        Ok(Self {
            file,
            counts,
            known: HashMap::new(),
        })
    }
    fn open(path: PathBuf) -> Result<Self, CbhError> {
        let mut file = Tracked::open(path, false)?;
        let head = file.read_at(0, LID_HEADER as usize)?;
        let be = |i: usize| u32::from_be_bytes(head[i..i + 4].try_into().unwrap());
        let mut counts = [0; LISTS];
        for (i, count) in counts.iter_mut().enumerate() {
            *count = be(8 + i * 20 + 8);
        }
        if head != lid_header(&counts) {
            return Err(CbhError::Unsupported(
                "2LID header differs from the layout the writer knows".into(),
            ));
        }
        Ok(Self {
            file,
            counts,
            known: HashMap::new(),
        })
    }
    fn record(&mut self, kind: usize, id: u32) -> Result<Vec<u8>, CbhError> {
        let at = Self::offset(kind, id);
        if at + 4 > self.file.seen.0 {
            return Ok(vec![]);
        }
        let length = u32::from_le_bytes(self.file.read_at(at, 4)?.try_into().unwrap());
        if length > SLOTS[kind] - 4 {
            return Err(CbhError::Decode(format!(
                "2LID record {id} exceeds its slot"
            )));
        }
        self.file.read_at(at + 4, length as usize)
    }
    /// Returns the id of the record whose leading strings are `key`, adding
    /// `payload` as a new record when there is none.
    fn id(&mut self, kind: Kind, key: Vec<String>, payload: Vec<u8>) -> Result<u32, CbhError> {
        let list = kind as usize;
        if !self.known.contains_key(&kind) {
            let mut map = HashMap::new();
            for id in 0..self.counts[list] {
                let bytes = self.record(list, id)?;
                if let Some(k) = read_strings(&bytes, key.len()) {
                    map.entry(k).or_insert(id);
                }
            }
            self.known.insert(kind, map);
        }
        if let Some(&id) = self.known[&kind].get(&key) {
            return Ok(id);
        }
        if payload.len() + 4 > SLOTS[list] as usize {
            return Err(CbhError::Unsupported(format!(
                "Name record of {} bytes exceeds its 2LID slot",
                payload.len()
            )));
        }
        let id = self.counts[list];
        // Records live in group `id`; keep the file at whole groups.
        let end = LID_HEADER + (id as u64 + 1) * Self::group();
        if self.file.seen.0 < end {
            self.file.file.set_len(end)?;
            self.file.seen.0 = end;
        }
        let mut bytes = (payload.len() as u32).to_le_bytes().to_vec();
        bytes.extend(payload);
        self.file.write_at(Self::offset(list, id), &bytes)?;
        self.counts[list] += 1;
        self.known.get_mut(&kind).unwrap().insert(key, id);
        Ok(id)
    }
    fn commit(&mut self) -> Result<(), CbhError> {
        self.file.sync()?;
        self.file.write_at(0, &lid_header(&self.counts))?;
        self.file.sync()
    }
}

/// Packed `year << 9 | month << 5 | day` from a PGN date; `?` parts are 0.
pub fn pack_date(date: &str) -> u32 {
    let mut parts = date.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (y, m, d) = (
        parts.next().unwrap_or(0).min(4095),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    if m > 12 || d > 31 {
        return y << 9;
    }
    y << 9 | m << 5 | d
}

/// Packed ECO: `(index + 1) << 7` for A00..E99.
pub fn pack_eco(eco: &str) -> u16 {
    let b = eco.as_bytes();
    if b.len() < 3 || !(b'A'..=b'E').contains(&b[0]) {
        return 0;
    }
    match eco[1..3].parse::<u16>() {
        Ok(n) if n < 100 => ((b[0] - b'A') as u16 * 100 + n + 1) << 7,
        _ => 0,
    }
}

fn result_code(result: Option<&str>) -> u8 {
    match result {
        Some("1-0") => 2,
        Some("0-1") => 0,
        Some("1/2-1/2") => 1,
        _ => 3,
    }
}

/// Main-line move count as ChessBase stores it (classic byte 45), capped.
fn move_count(game: &Game) -> u8 {
    let mut plies = 0usize;
    let mut at = 0usize;
    while let Some(&child) = game.nodes[at].children.first() {
        plies += 1;
        at = child as usize;
    }
    (plies.div_ceil(2)).min(255) as u8
}

/// A writable 2CBH database.
pub struct Writer {
    header: Tracked,
    games: Stream,
    notes: Stream,
    names: Lists,
    /// Records including the header record.
    records: u64,
}

impl Writer {
    /// Creates an empty family at `path` (a `.2cbh` path). Fails when any of
    /// the family's files already exists.
    pub fn create(path: &Path) -> Result<Self, CbhError> {
        let family = Family::new(path)?;
        for ext in WRITTEN.iter().chain(&["2lcd", "2lgd"]) {
            if family.path(ext).exists() {
                return Err(CbhError::Unsupported(format!(
                    "A database file already exists: {}",
                    family.path(ext).display()
                )));
            }
        }
        let mut header = Tracked::open(family.path("2cbh"), true)?;
        // Create path `0x14019a0e0`: zeroed record with these fields set.
        let mut head = [0u8; RECORD];
        head[8] = 0x26;
        head[10..12].copy_from_slice(&(RECORD as u16).to_le_bytes());
        head[13] = 5;
        head[16..24].copy_from_slice(&1u64.to_le_bytes());
        header.write_at(0, &head)?;
        let mut writer = Self {
            header,
            games: Stream::create(family.path("2cbg"), 1)?,
            notes: Stream::create(family.path("2cba"), 0)?,
            names: Lists::create(family.path("2lid"))?,
            records: 1,
        };
        writer.games.commit()?;
        writer.notes.commit()?;
        writer.names.commit()?;
        writer.header.sync()?;
        Ok(writer)
    }

    /// Opens an existing family for writing.
    pub fn open(path: &Path) -> Result<Self, CbhError> {
        let family = Family::new(path)?;
        let mut header = Tracked::open(family.path("2cbh"), false)?;
        let head = header.read_at(0, RECORD)?;
        let parsed = super::record::NewHeader::parse(&head, header.seen.0)?;
        if head[8] != 0x26 || head[13] != 5 {
            return Err(CbhError::Unsupported(
                "2CBH header version the writer does not know".into(),
            ));
        }
        Ok(Self {
            header,
            games: Stream::open(family.path("2cbg"))?,
            notes: Stream::open(family.path("2cba"))?,
            names: Lists::open(family.path("2lid"))?,
            records: parsed.entry_count + 1,
        })
    }

    pub fn entry_count(&self) -> u64 {
        self.records - 1
    }

    fn check(&self) -> Result<(), CbhError> {
        self.header.check()?;
        self.games.file.check()?;
        self.notes.file.check()?;
        self.names.file.check()
    }

    /// Builds the record for `game`, appending its blocks and new names.
    fn prepare(&mut self, game: &Game, deleted: bool) -> Result<([u8; RECORD], Omitted), CbhError> {
        let moves = encode_moves(game)?;
        let (notes, omitted) = encode_annotations(game);
        let tag = |key: &str| {
            game.tags
                .get(key)
                .map(String::as_str)
                .filter(|v| !v.is_empty() && *v != "?" && !v.chars().all(|c| c == '?' || c == '.'))
        };
        let mut r = [0u8; RECORD];
        r[0] = if deleted { 0x81 } else { 0x01 };
        r[2..4].copy_from_slice(&0x0101u16.to_le_bytes());
        let mut ids = [NONE; 7];
        for (i, key) in ["White", "Black"].iter().enumerate() {
            if let Some(name) = tag(key) {
                ids[i] = self.player(name)? as u64;
            }
        }
        if tag("Event").is_some() || tag("Site").is_some() {
            let (site, event) = (tag("Site").unwrap_or(""), tag("Event").unwrap_or(""));
            let mut payload = strings(&[site, event]);
            let mut tail = [0u8; TOURNAMENT_TAIL];
            tail[..4].copy_from_slice(&pack_date(tag("Date").unwrap_or("")).to_le_bytes());
            for at in [34, 45, 56] {
                tail[at] = 7;
            }
            payload.extend(tail);
            ids[2] = self
                .names
                .id(Kind::Tournament, vec![site.into(), event.into()], payload)?
                as u64;
        }
        if let Some(name) = tag("Annotator") {
            ids[3] = self.player(name)? as u64;
        }
        if let Some(title) = tag("SourceTitle") {
            let publisher = tag("Source").unwrap_or("");
            let date = pack_date(tag("SourceDate").unwrap_or("")).to_le_bytes();
            let mut payload = strings(&[title, publisher]);
            payload.extend(date);
            payload.extend(date);
            payload.extend(SOURCE_TAIL);
            ids[4] = self
                .names
                .id(Kind::Source, vec![title.into(), publisher.into()], payload)?
                as u64;
        }
        for (i, id) in ids.iter().enumerate() {
            let at = 24 + i * 8;
            r[at..at + 8].copy_from_slice(&id.to_le_bytes());
        }
        r[80..88].copy_from_slice(&2u64.to_le_bytes());
        r[88] = result_code(tag("Result").or(game.tags.get("Result").map(String::as_str)));
        if let Some(round) = tag("Round") {
            let mut parts = round.split('.').map(|p| p.parse::<u16>().unwrap_or(0));
            r[90..92].copy_from_slice(&parts.next().unwrap_or(0).to_le_bytes());
            r[92..94].copy_from_slice(&parts.next().unwrap_or(0).to_le_bytes());
        }
        for (key, at) in [("WhiteElo", 96), ("BlackElo", 112)] {
            let elo = tag(key).and_then(|v| v.parse::<u16>().ok()).unwrap_or(0);
            if elo < 4096 {
                r[at..at + 2].copy_from_slice(&elo.to_le_bytes());
            }
        }
        r[128..130].copy_from_slice(&pack_eco(tag("ECO").unwrap_or("")).to_le_bytes());
        r[138] = move_count(game);
        r[188..192].copy_from_slice(&pack_date(tag("Date").unwrap_or("")).to_le_bytes());
        let at = self.games.append(&frame(TYPE_GAME, &moves, GAME_SLACK))?;
        r[8..16].copy_from_slice(&at.to_le_bytes());
        let at = self
            .notes
            .append(&frame(TYPE_ANNOTATIONS, &notes, NOTES_SLACK))?;
        r[16..24].copy_from_slice(&at.to_le_bytes());
        Ok((r, omitted))
    }

    /// "Last, First" split at the first ", ", the inverse of how the reader
    /// joins the two fields. Anything else, such as consultation names or
    /// classic "Last,First" annotators, stays one last name.
    fn player(&mut self, name: &str) -> Result<u32, CbhError> {
        let (last, first) = name.split_once(", ").unwrap_or((name, ""));
        let mut payload = strings(&[last, first]);
        payload.extend(PLAYER_TAIL);
        self.names
            .id(Kind::Player, vec![last.into(), first.into()], payload)
    }

    /// Writes data before the records that refer to it (writing.md).
    fn commit_data(&mut self) -> Result<(), CbhError> {
        self.games.commit()?;
        self.notes.commit()?;
        self.names.commit()
    }

    fn write_count(&mut self) -> Result<(), CbhError> {
        self.header.write_at(16, &self.records.to_le_bytes())?;
        self.header.sync()
    }

    /// Appends `game` as a new entry and returns its number.
    pub fn append(&mut self, game: &Game) -> Result<(u64, Omitted), CbhError> {
        let mut out = self.append_all(std::slice::from_ref(game))?;
        Ok(out.remove(0))
    }

    /// Appends several games with one set of syncs, in the same order as a
    /// single append: data, names, records, then the count.
    pub fn append_all(&mut self, games: &[Game]) -> Result<Vec<(u64, Omitted)>, CbhError> {
        self.check()?;
        let mut prepared = Vec::with_capacity(games.len());
        for game in games {
            prepared.push(self.prepare(game, false)?);
        }
        self.commit_data()?;
        let mut out = Vec::with_capacity(games.len());
        for (i, (record, omitted)) in prepared.into_iter().enumerate() {
            let ordinal = self.records + i as u64;
            self.header.write_at(ordinal * RECORD as u64, &record)?;
            out.push((ordinal, omitted));
        }
        self.header.sync()?;
        self.records += games.len() as u64;
        self.write_count()?;
        Ok(out)
    }

    fn existing(&mut self, ordinal: u64) -> Result<Vec<u8>, CbhError> {
        if ordinal == 0 || ordinal >= self.records {
            return Err(CbhError::Bounds);
        }
        let bytes = self.header.read_at(ordinal * RECORD as u64, RECORD)?;
        if bytes[2..4] != 0x0101u16.to_le_bytes() {
            return Err(CbhError::Unsupported("Entry is not a game".into()));
        }
        Ok(bytes)
    }

    /// Replaces entry `ordinal` with `game`. A forfeit or line evaluation
    /// stored for the old game is kept when the PGN result is unchanged.
    pub fn replace(&mut self, ordinal: u64, game: &Game) -> Result<Omitted, CbhError> {
        self.check()?;
        let old = self.existing(ordinal)?;
        let (mut record, omitted) = self.prepare(game, old[0] & 0x80 != 0)?;
        if crate::header::pgn_result(old[88]) == crate::header::pgn_result(record[88])
            && old[88] < 8
        {
            record[88] = old[88];
            record[89] = old[89];
        }
        self.commit_data()?;
        self.header.write_at(ordinal * RECORD as u64, &record)?;
        self.header.sync()?;
        Ok(omitted)
    }

    /// Sets or clears the deleted flag (bit 7 of byte 0) of entry `ordinal`.
    pub fn set_deleted(&mut self, ordinal: u64, deleted: bool) -> Result<(), CbhError> {
        self.check()?;
        let old = self.existing(ordinal)?;
        let flags = if deleted {
            old[0] | 0x80
        } else {
            old[0] & !0x80
        };
        self.header.write_at(ordinal * RECORD as u64, &[flags])?;
        self.header.sync()
    }
}

/// Paths of one family, from its `.2cbh` path.
struct Family {
    base: PathBuf,
}

impl Family {
    fn new(path: &Path) -> Result<Self, CbhError> {
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("2cbh"))
        {
            return Err(CbhError::Unsupported(
                "Only .2cbh databases can be written".into(),
            ));
        }
        Ok(Self {
            base: path.to_path_buf(),
        })
    }
    fn path(&self, ext: &str) -> PathBuf {
        self.base.with_extension(ext)
    }
}
