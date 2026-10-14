//! Sort orders for the game list (see docs/game-list.md). A sort order is the
//! workbench's own file: every entry number of a database, arranged by one
//! column. It is built from the entry index and the name tables, without
//! decoding any game, and kept in the app's data folder.
use crate::{CbhError, Database, DatabaseFormat};
use std::{
    collections::HashMap,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

const MAGIC: &[u8; 8] = b"CWSORT01";
/// Magic, column and version, entry count, database fingerprint.
const HEADER: u64 = 8 + 8 + 8 + 32;
/// Version of the keys; files with another version are rebuilt.
/// 2: classic names decoded as Windows-1252 with figurines, not lossy UTF-8.
const KEY_VERSION: u32 = 2;

/// A game list column the list can be sorted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SortColumn {
    White,
    WhiteElo,
    Black,
    BlackElo,
    Result,
    Eco,
    Tournament,
    Round,
    Date,
    Annotator,
    Source,
    Moves,
}

impl SortColumn {
    pub const ALL: [SortColumn; 12] = [
        Self::White,
        Self::WhiteElo,
        Self::Black,
        Self::BlackElo,
        Self::Result,
        Self::Eco,
        Self::Tournament,
        Self::Round,
        Self::Date,
        Self::Annotator,
        Self::Source,
        Self::Moves,
    ];
    /// A short name, used in file names.
    pub fn name(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::WhiteElo => "white-elo",
            Self::Black => "black",
            Self::BlackElo => "black-elo",
            Self::Result => "result",
            Self::Eco => "eco",
            Self::Tournament => "tournament",
            Self::Round => "round",
            Self::Date => "date",
            Self::Annotator => "annotator",
            Self::Source => "source",
            Self::Moves => "moves",
        }
    }
    fn code(self) -> u32 {
        Self::ALL.iter().position(|c| *c == self).unwrap() as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SortStage {
    /// Reading names; counts names.
    Names,
    /// Reading the entry index; counts entries.
    Reading,
    /// Arranging the keys; counts entries written.
    Sorting,
}

/// How a sort order is built.
#[derive(Clone, Debug)]
pub struct SortOptions {
    /// Threads reading the entry index.
    pub workers: usize,
    /// Largest database sorted: keys take 16 bytes per entry in memory.
    pub max_entries: u64,
}

impl Default for SortOptions {
    fn default() -> Self {
        Self {
            workers: std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .clamp(1, 8),
            max_entries: 32 << 20,
        }
    }
}

/// The fields of one entry-index record that sort keys use.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Fields {
    pub game: bool,
    pub deleted: bool,
    pub white: Option<u32>,
    pub black: Option<u32>,
    pub tournament: Option<u32>,
    annotator: Option<u32>,
    source: Option<u32>,
    pub date: u32,
    pub result: u8,
    round: u16,
    subround: u16,
    /// 0 when unrated.
    pub white_elo: u16,
    pub black_elo: u16,
    /// Packed ECO: bits 7–15 hold 1..=500 for A00..E99, 0 for none.
    pub eco: u16,
    moves: u8,
}

fn be24(b: &[u8]) -> u32 {
    u32::from_be_bytes([0, b[0], b[1], b[2]])
}
fn le16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

/// Reads the sort fields from a record. The offsets are those of
/// `header::parse_record` (classic) and `v2::record::parse_record` (2CBH).
pub(crate) fn fields(format: DatabaseFormat, b: &[u8]) -> Fields {
    match format {
        DatabaseFormat::Classic => {
            if b[0] & 15 != 1 {
                return Fields::default();
            }
            Fields {
                game: true,
                deleted: b[0] & 0x80 != 0,
                white: Some(be24(&b[9..12])),
                black: Some(be24(&b[12..15])),
                tournament: Some(be24(&b[15..18])),
                annotator: Some(be24(&b[18..21])),
                source: Some(be24(&b[21..24])),
                date: be24(&b[24..27]),
                result: b[27],
                round: b[29] as u16,
                subround: b[30] as u16,
                white_elo: u16::from_be_bytes([b[31], b[32]]) & 4095,
                black_elo: u16::from_be_bytes([b[33], b[34]]) & 4095,
                eco: u16::from_be_bytes([b[35], b[36]]),
                moves: b[45],
            }
        }
        DatabaseFormat::New => {
            if le16(b, 2) != 0x0101 {
                return Fields::default();
            }
            let id = |i: usize| {
                let v = u64::from_le_bytes(b[i..i + 8].try_into().unwrap());
                u32::try_from(v).ok()
            };
            Fields {
                game: true,
                deleted: le16(b, 0) & 0x80 != 0,
                white: id(24),
                black: id(32),
                tournament: id(40),
                annotator: id(48),
                source: id(56),
                date: u32::from_le_bytes(b[188..192].try_into().unwrap()),
                result: b[88],
                round: le16(b, 90),
                subround: le16(b, 92),
                white_elo: le16(b, 96) & 4095,
                black_elo: le16(b, 112) & 4095,
                eco: le16(b, 128),
                moves: b[138],
            }
        }
    }
}

/// Order of the stored result codes: White wins, draw, Black wins, then the
/// forfeits, "0-0" and lines.
fn result_rank(code: u8) -> u64 {
    match code {
        2 => 1,
        1 => 2,
        0 => 3,
        6 => 4,
        5 => 5,
        4 => 6,
        7 => 7,
        3 => 8,
        _ => 9,
    }
}

/// Each name's place in alphabetical order, by id; 0 for unknown ids.
enum Ranks {
    Dense(Vec<u32>),
    Sparse(HashMap<u32, u32>),
}

impl Ranks {
    fn get(&self, id: Option<u32>) -> u64 {
        let Some(id) = id else { return 0 };
        match self {
            Self::Dense(v) => v.get(id as usize).copied().unwrap_or(0) as u64,
            Self::Sparse(m) => m.get(&id).copied().unwrap_or(0) as u64,
        }
    }
}

/// Collation key of a name: case-folded, with common Latin accents removed,
/// so "Ánand" sorts beside "Anand".
pub fn collation_key(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        let base = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'ď' | 'đ' => 'd',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
            'ğ' => 'g',
            'ì' | 'í' | 'î' | 'ï' | 'ı' | 'ī' => 'i',
            'ł' | 'ľ' => 'l',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ő' | 'ō' => 'o',
            'ř' => 'r',
            'ś' | 'š' | 'ş' | 'ș' => 's',
            'ť' | 'ţ' | 'ț' => 't',
            'ù' | 'ú' | 'û' | 'ü' | 'ů' | 'ű' | 'ū' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            'ß' => {
                out.push_str("ss");
                continue;
            }
            c => c,
        };
        out.push(base);
    }
    out
}

/// Ranks of names given as (id, collation key); ties keep id order.
fn rank(names: Vec<(String, u32)>) -> Ranks {
    let max = names.iter().map(|n| n.1).max().unwrap_or(0) as usize;
    // Equal keys share a rank, so equal names sort by entry number.
    let mut ranks = Vec::with_capacity(names.len());
    let mut last: Option<&str> = None;
    let mut r = 0u32;
    for (key, id) in &names {
        if last != Some(key.as_str()) {
            r += 1;
            last = Some(key.as_str());
        }
        ranks.push((*id, r));
    }
    if max < (16 << 20) {
        let mut dense = vec![0; max + 1];
        for (id, r) in ranks {
            dense[id as usize] = r;
        }
        Ranks::Dense(dense)
    } else {
        Ranks::Sparse(ranks.into_iter().collect())
    }
}

impl Database {
    /// Name ranks for a column, or None when the column has no names.
    fn column_ranks(
        &self,
        column: SortColumn,
        cancel: &AtomicBool,
    ) -> Result<Option<Ranks>, CbhError> {
        Ok(self.sorted_names(column, cancel)?.map(rank))
    }

    /// The names behind a column as (collation key, id), in alphabetical
    /// order, or None when the column has no names.
    pub(crate) fn sorted_names(
        &self,
        column: SortColumn,
        cancel: &AtomicBool,
    ) -> Result<Option<Vec<(String, u32)>>, CbhError> {
        let mut names = vec![];
        let go = || !cancel.load(Ordering::Relaxed);
        match column {
            SortColumn::White | SortColumn::Black => {
                self.names_each_player(&mut |id, last, first| {
                    names.push((
                        format!("{}\u{1}{}", collation_key(last), collation_key(first)),
                        id,
                    ));
                    go()
                })?;
            }
            SortColumn::Tournament => self.names_each_tournament(&mut |id, event, site| {
                names.push((
                    format!("{}\u{1}{}", collation_key(event), collation_key(site)),
                    id,
                ));
                go()
            })?,
            SortColumn::Annotator => self.names_each_annotator(&mut |id, name| {
                names.push((collation_key(name), id));
                go()
            })?,
            SortColumn::Source => self.names_each_source(&mut |id, title| {
                names.push((collation_key(title), id));
                go()
            })?,
            _ => return Ok(None),
        }
        names.sort_unstable();
        Ok(Some(names))
    }
}

/// The key of one record; entries that are not games sort as 0.
fn key(column: SortColumn, f: &Fields, ranks: Option<&Ranks>) -> u64 {
    if !f.game {
        return 0;
    }
    let names = |id| ranks.map_or(0, |r| r.get(id));
    match column {
        SortColumn::White => names(f.white),
        SortColumn::Black => names(f.black),
        SortColumn::Annotator => names(f.annotator),
        SortColumn::Source => names(f.source),
        // A tournament's games in round order.
        SortColumn::Tournament => {
            names(f.tournament) << 32 | (f.round as u64) << 16 | f.subround as u64
        }
        SortColumn::WhiteElo => f.white_elo as u64,
        SortColumn::BlackElo => f.black_elo as u64,
        SortColumn::Result => result_rank(f.result),
        SortColumn::Eco => f.eco as u64,
        SortColumn::Round => (f.round as u64) << 16 | f.subround as u64,
        SortColumn::Date => f.date as u64,
        SortColumn::Moves => f.moves as u64,
    }
}

/// A sort order file opened for reading.
pub struct SortOrder {
    file: std::fs::File,
    entries: u64,
    column: SortColumn,
}

impl SortOrder {
    /// Opens `path` if it holds `column` built from `db` as it is now.
    pub fn open(path: &Path, db: &Database, column: SortColumn) -> Result<Option<Self>, CbhError> {
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut head = [0u8; HEADER as usize];
        if file.read_exact(&mut head).is_err() || &head[..8] != MAGIC {
            return Ok(None);
        }
        let code = u32::from_le_bytes(head[8..12].try_into().unwrap());
        let version = u32::from_le_bytes(head[12..16].try_into().unwrap());
        let entries = u64::from_le_bytes(head[16..24].try_into().unwrap());
        let stamp: Vec<u64> = head[24..]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| u64::from_le_bytes(*c))
            .collect();
        if code != column.code()
            || version != KEY_VERSION
            || entries != db.info().entry_count
            || stamp != db.fingerprint()
            || file.metadata()?.len() != HEADER + entries * 4
        {
            return Ok(None);
        }
        Ok(Some(Self {
            file,
            entries,
            column,
        }))
    }

    pub fn column(&self) -> SortColumn {
        self.column
    }

    pub fn len(&self) -> u64 {
        self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries == 0
    }

    /// Entry numbers at sorted places `from .. from + n` (counted from 0).
    pub fn read(&mut self, from: u64, n: usize, descending: bool) -> Result<Vec<u64>, CbhError> {
        if from >= self.entries {
            return Ok(vec![]);
        }
        let n = (n as u64).min(self.entries - from);
        // Descending places count from the end of the file.
        let first = if descending {
            self.entries - from - n
        } else {
            from
        };
        self.file.seek(SeekFrom::Start(HEADER + first * 4))?;
        let mut bytes = vec![0u8; n as usize * 4];
        self.file.read_exact(&mut bytes)?;
        let mut out: Vec<u64> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c) as u64)
            .collect();
        if descending {
            out.reverse();
        }
        Ok(out)
    }

    /// Calls `f` with every entry number in sorted order; stops when it
    /// returns false.
    fn each(&mut self, descending: bool, mut f: impl FnMut(u64) -> bool) -> Result<(), CbhError> {
        const CHUNK: usize = 1 << 16;
        let mut at = 0;
        while at < self.entries {
            for ordinal in self.read(at, CHUNK, descending)? {
                if !f(ordinal) {
                    return Ok(());
                }
            }
            at += CHUNK as u64;
        }
        Ok(())
    }

    /// The sorted place of an entry (from 0), if it is in the database.
    pub fn place_of(&mut self, ordinal: u64, descending: bool) -> Result<Option<u64>, CbhError> {
        let mut found = None;
        let mut i = 0;
        self.each(descending, |o| {
            if o == ordinal {
                found = Some(i);
                return false;
            }
            i += 1;
            true
        })?;
        Ok(found)
    }

    /// Indexes into `ordinals` arranged in sort order: `ordinals[result[0]]`
    /// comes first. Entries outside the database are left out. Returns None
    /// when cancelled.
    pub fn arrange(
        &mut self,
        ordinals: &[u64],
        descending: bool,
        cancel: &AtomicBool,
    ) -> Result<Option<Vec<u32>>, CbhError> {
        let mut by_ordinal: Vec<(u64, u32)> = ordinals
            .iter()
            .enumerate()
            .map(|(i, &o)| (o, i as u32))
            .collect();
        by_ordinal.sort_unstable();
        let mut out = Vec::with_capacity(ordinals.len());
        let mut cancelled = false;
        let mut seen = 0u64;
        self.each(descending, |o| {
            seen += 1;
            if seen.is_multiple_of(1 << 20) && cancel.load(Ordering::Relaxed) {
                cancelled = true;
                return false;
            }
            if let Ok(i) = by_ordinal.binary_search_by_key(&o, |p| p.0) {
                out.push(by_ordinal[i].1);
            }
            out.len() < ordinals.len()
        })?;
        Ok((!cancelled).then_some(out))
    }

    /// Builds the sort order of `column` for `db` at `path`, replacing any
    /// older one. Returns false when cancelled; nothing is left behind.
    pub fn build(
        db: &Database,
        column: SortColumn,
        path: &Path,
        options: &SortOptions,
        cancel: &AtomicBool,
        mut progress: impl FnMut(SortStage, u64, u64),
    ) -> Result<bool, CbhError> {
        let count = db.info().entry_count;
        if count > options.max_entries || count > u32::MAX as u64 {
            return Err(CbhError::Unsupported(format!(
                "Sorting is limited to {} entries",
                options.max_entries.min(u32::MAX as u64)
            )));
        }
        db.verify()?;
        progress(SortStage::Names, 0, 1);
        let ranks = db.column_ranks(column, cancel)?;
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        progress(SortStage::Reading, 0, count);
        let format = db.format();
        let workers = options.workers.clamp(1, 64) as u64;
        let handles = (1..workers)
            .map(|_| db.reopen())
            .collect::<Result<Vec<_>, _>>()?;
        let mut keys = vec![(0u64, 0u32); count as usize];
        let per = count.div_ceil(workers).max(1);
        let done = AtomicU64::new(0);
        let read = std::thread::scope(|scope| -> Result<bool, CbhError> {
            let mut parts = vec![];
            for (i, slice) in keys.chunks_mut(per as usize).enumerate() {
                let handle = if i == 0 { db } else { &handles[i - 1] };
                let (ranks, done) = (ranks.as_ref(), &done);
                parts.push(scope.spawn(move || -> Result<bool, CbhError> {
                    const CHUNK: u64 = 8192;
                    let first = 1 + i as u64 * per;
                    let size = format.record_size();
                    let mut at = 0u64;
                    while at < slice.len() as u64 {
                        if cancel.load(Ordering::Relaxed) {
                            return Ok(false);
                        }
                        let n = CHUNK.min(slice.len() as u64 - at);
                        let bytes = handle.record_bytes(first + at, n)?;
                        for (j, record) in bytes.chunks_exact(size).enumerate() {
                            let ordinal = first + at + j as u64;
                            slice[(at as usize) + j] =
                                (key(column, &fields(format, record), ranks), ordinal as u32);
                        }
                        at += n;
                        done.fetch_add(n, Ordering::Relaxed);
                    }
                    // Each part is sorted here; the parts are merged below.
                    slice.sort_unstable();
                    Ok(true)
                }));
            }
            loop {
                let finished = parts.iter().all(|p| p.is_finished());
                progress(SortStage::Reading, done.load(Ordering::Relaxed), count);
                if finished {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let mut ok = true;
            for p in parts {
                ok &= p
                    .join()
                    .map_err(|_| CbhError::Io(std::io::Error::other("sort worker stopped")))??;
            }
            Ok(ok)
        })?;
        if !read || cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let mut temp = tempfile::NamedTempFile::new_in(dir)?;
        {
            let mut out = BufWriter::with_capacity(1 << 20, temp.as_file_mut());
            out.write_all(MAGIC)?;
            out.write_all(&column.code().to_le_bytes())?;
            out.write_all(&KEY_VERSION.to_le_bytes())?;
            out.write_all(&count.to_le_bytes())?;
            for v in db.fingerprint() {
                out.write_all(&v.to_le_bytes())?;
            }
            // Merge the sorted parts.
            let parts: Vec<&[(u64, u32)]> = keys.chunks(per as usize).collect();
            let mut heads = vec![0usize; parts.len()];
            let mut heap = std::collections::BinaryHeap::new();
            for (p, part) in parts.iter().enumerate() {
                if let Some(first) = part.first() {
                    heap.push(std::cmp::Reverse((*first, p)));
                }
            }
            let mut written = 0u64;
            while let Some(std::cmp::Reverse(((_, ordinal), p))) = heap.pop() {
                out.write_all(&ordinal.to_le_bytes())?;
                heads[p] += 1;
                if let Some(next) = parts[p].get(heads[p]) {
                    heap.push(std::cmp::Reverse((*next, p)));
                }
                written += 1;
                if written.is_multiple_of(1 << 20) {
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(false);
                    }
                    progress(SortStage::Sorting, written, count);
                }
            }
            out.flush()?;
        }
        drop(keys);
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| CbhError::Io(e.error))?;
        progress(SortStage::Sorting, count, count);
        Ok(true)
    }
}
