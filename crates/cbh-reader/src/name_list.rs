//! Player and tournament lists for the database window's panes (see
//! docs/game-list.md). A list is the workbench's own file: every name that
//! occurs in a game, in alphabetical order, with its number of games and
//! the first and last year it occurs. It is built from the name table and
//! one parallel scan of the entry index.
use crate::{
    CbhError, Database,
    sort::{SortColumn, collation_key, fields},
};
use std::{
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

const MAGIC: &[u8; 8] = b"CWNAME01";
/// Magic, kind and version, rows, entry count, database fingerprint.
const HEADER: u64 = 8 + 8 + 8 + 8 + 32;
const VERSION: u32 = 1;
const ROW: u64 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NameKind {
    Players,
    Tournaments,
}

impl NameKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Players => "players",
            Self::Tournaments => "tournaments",
        }
    }
    fn code(self) -> u32 {
        match self {
            Self::Players => 1,
            Self::Tournaments => 2,
        }
    }
}

/// One name of a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameRow {
    pub id: u32,
    /// Game entries naming it, deleted ones included.
    pub games: u32,
    /// First and last year of its dated games; 0 when none is dated.
    pub first_year: u16,
    pub last_year: u16,
}

/// Per-id counts while building.
#[derive(Clone, Copy, Default)]
struct Count {
    games: u32,
    first: u16,
    last: u16,
}

impl Count {
    fn add(&mut self, year: u16) {
        self.games = self.games.saturating_add(1);
        if year > 0 {
            self.first = if self.first == 0 {
                year
            } else {
                self.first.min(year)
            };
            self.last = self.last.max(year);
        }
    }
    fn merge(&mut self, o: &Count) {
        self.games = self.games.saturating_add(o.games);
        if o.first > 0 {
            self.first = if self.first == 0 {
                o.first
            } else {
                self.first.min(o.first)
            };
        }
        self.last = self.last.max(o.last);
    }
}

/// A list file opened for reading.
pub struct NameList {
    file: std::fs::File,
    rows: u64,
}

impl NameList {
    /// Opens `path` if it holds `kind` built from `db` as it is now.
    pub fn open(path: &Path, db: &Database, kind: NameKind) -> Result<Option<Self>, CbhError> {
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut head = [0u8; HEADER as usize];
        if file.read_exact(&mut head).is_err() || &head[..8] != MAGIC {
            return Ok(None);
        }
        let le32 = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap());
        let le64 = |i: usize| u64::from_le_bytes(head[i..i + 8].try_into().unwrap());
        let rows = le64(16);
        let stamp: Vec<u64> = (0..4).map(|i| le64(32 + 8 * i)).collect();
        if le32(8) != kind.code()
            || le32(12) != VERSION
            || le64(24) != db.info().entry_count
            || stamp != db.fingerprint()
            || file.metadata()?.len() != HEADER + rows * ROW
        {
            return Ok(None);
        }
        Ok(Some(Self { file, rows }))
    }

    pub fn len(&self) -> u64 {
        self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

    /// Rows `from .. from + n` (counted from 0).
    pub fn read(&mut self, from: u64, n: usize) -> Result<Vec<NameRow>, CbhError> {
        if from >= self.rows {
            return Ok(vec![]);
        }
        let n = (n as u64).min(self.rows - from);
        self.file.seek(SeekFrom::Start(HEADER + from * ROW))?;
        let mut bytes = vec![0u8; (n * ROW) as usize];
        self.file.read_exact(&mut bytes)?;
        Ok(bytes
            .as_chunks::<12>()
            .0
            .iter()
            .map(|b| NameRow {
                id: u32::from_le_bytes(b[0..4].try_into().unwrap()),
                games: u32::from_le_bytes(b[4..8].try_into().unwrap()),
                first_year: u16::from_le_bytes([b[8], b[9]]),
                last_year: u16::from_le_bytes([b[10], b[11]]),
            })
            .collect())
    }

    /// The first row whose name sorts at or after `prefix`. Players take
    /// "Last, First"; tournaments take the event name.
    pub fn find(
        &mut self,
        db: &mut Database,
        kind: NameKind,
        prefix: &str,
    ) -> Result<u64, CbhError> {
        let target = match kind {
            NameKind::Players => match prefix.split_once(',') {
                Some((last, first)) => {
                    format!("{}\u{1}{}", collation_key(last), collation_key(first))
                }
                None => collation_key(prefix),
            },
            NameKind::Tournaments => collation_key(prefix),
        };
        let (mut lo, mut hi) = (0, self.rows);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let id = self.read(mid, 1)?[0].id;
            let key = match kind {
                NameKind::Players => {
                    let name = db.player_name(id)?.unwrap_or_default();
                    let (last, first) = name.split_once(',').unwrap_or((&name, ""));
                    format!("{}\u{1}{}", collation_key(last), collation_key(first))
                }
                NameKind::Tournaments => {
                    let (event, site) = db.tournament_name(id)?;
                    format!(
                        "{}\u{1}{}",
                        collation_key(&event.unwrap_or_default()),
                        collation_key(&site.unwrap_or_default())
                    )
                }
            };
            if key.as_str() < target.as_str() {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        Ok(lo)
    }

    /// Builds the list of `kind` for `db` at `path`, replacing any older one.
    /// Returns false when cancelled; nothing is left behind.
    pub fn build(
        db: &Database,
        kind: NameKind,
        path: &Path,
        workers: usize,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<bool, CbhError> {
        db.verify()?;
        let count = db.info().entry_count;
        let column = match kind {
            NameKind::Players => SortColumn::White,
            NameKind::Tournaments => SortColumn::Tournament,
        };
        let names = db.sorted_names(column, cancel)?.unwrap_or_default();
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        // Ids beyond the name table are not counted.
        let slots = names.iter().map(|n| n.1 as usize + 1).max().unwrap_or(0);
        if slots > 1 << 24 {
            return Err(CbhError::Unsupported(
                "Name table is too large to list".into(),
            ));
        }
        let format = db.format();
        // Each worker counts into its own table; together at most 512 MB.
        let budget = ((512usize << 20) / (slots * std::mem::size_of::<Count>()).max(1)).max(1);
        let workers = workers.clamp(1, 64).min(budget) as u64;
        let handles = (1..workers)
            .map(|_| db.reopen())
            .collect::<Result<Vec<_>, _>>()?;
        let per = count.div_ceil(workers).max(1);
        let done = AtomicU64::new(0);
        let parts = std::thread::scope(|scope| -> Result<Option<Vec<Vec<Count>>>, CbhError> {
            let mut running = vec![];
            for i in 0..workers {
                let first = 1 + i * per;
                if first > count {
                    break;
                }
                let last = (first + per - 1).min(count);
                let handle = if i == 0 { db } else { &handles[i as usize - 1] };
                let done = &done;
                running.push(
                    scope.spawn(move || -> Result<Option<Vec<Count>>, CbhError> {
                        const CHUNK: u64 = 8192;
                        let mut counts = vec![Count::default(); slots];
                        let size = format.record_size();
                        let mut at = first;
                        while at <= last {
                            if cancel.load(Ordering::Relaxed) {
                                return Ok(None);
                            }
                            let n = CHUNK.min(last + 1 - at);
                            for record in handle.record_bytes(at, n)?.chunks_exact(size) {
                                let f = fields(format, record);
                                if !f.game {
                                    continue;
                                }
                                let year = ((f.date >> 9) & 4095) as u16;
                                let mut add = |id: Option<u32>| {
                                    if let Some(c) = id.and_then(|id| counts.get_mut(id as usize)) {
                                        c.add(year);
                                    }
                                };
                                match kind {
                                    NameKind::Players => {
                                        add(f.white);
                                        // A player's game counts once.
                                        if f.black != f.white {
                                            add(f.black);
                                        }
                                    }
                                    NameKind::Tournaments => add(f.tournament),
                                }
                            }
                            at += n;
                            done.fetch_add(n, Ordering::Relaxed);
                        }
                        Ok(Some(counts))
                    }),
                );
            }
            loop {
                let finished = running.iter().all(|p| p.is_finished());
                progress(done.load(Ordering::Relaxed), count);
                if finished {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let mut out = vec![];
            for p in running {
                match p
                    .join()
                    .map_err(|_| CbhError::Io(std::io::Error::other("list worker stopped")))??
                {
                    Some(c) => out.push(c),
                    None => return Ok(None),
                }
            }
            Ok(Some(out))
        })?;
        let Some(mut parts) = parts else {
            return Ok(false);
        };
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let mut counts = parts.pop().unwrap_or_else(|| vec![Count::default(); slots]);
        for part in &parts {
            for (c, o) in counts.iter_mut().zip(part) {
                c.merge(o);
            }
        }
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let mut temp = tempfile::NamedTempFile::new_in(dir)?;
        let rows = names
            .iter()
            .filter(|(_, id)| counts[*id as usize].games > 0)
            .count() as u64;
        {
            let mut out = BufWriter::new(temp.as_file_mut());
            out.write_all(MAGIC)?;
            out.write_all(&kind.code().to_le_bytes())?;
            out.write_all(&VERSION.to_le_bytes())?;
            out.write_all(&rows.to_le_bytes())?;
            out.write_all(&count.to_le_bytes())?;
            for v in db.fingerprint() {
                out.write_all(&v.to_le_bytes())?;
            }
            for (_, id) in &names {
                let c = counts[*id as usize];
                if c.games == 0 {
                    continue;
                }
                out.write_all(&id.to_le_bytes())?;
                out.write_all(&c.games.to_le_bytes())?;
                out.write_all(&c.first.to_le_bytes())?;
                out.write_all(&c.last.to_le_bytes())?;
            }
            out.flush()?;
        }
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| CbhError::Io(e.error))?;
        Ok(true)
    }
}
