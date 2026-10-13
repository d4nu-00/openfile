use crate::{CbhError, Diagnostic, companions::CompanionSet, source::SourceFile};
use std::collections::{HashMap, VecDeque};
#[derive(Clone, Debug)]
pub struct EntityText {
    pub text: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone, Debug)]
pub struct TournamentText {
    pub event: EntityText,
    pub site: EntityText,
}
/// A source: title, publisher and publication date (`YYYY.MM.DD`).
#[derive(Clone, Debug)]
pub struct SourceText {
    pub title: EntityText,
    pub publisher: EntityText,
    pub date: Option<String>,
}
/// Name-cache key kinds.
pub(crate) const PLAYER: u8 = 0;
pub(crate) const TOURNAMENT: u8 = 1;
pub(crate) const ANNOTATOR: u8 = 2;
pub(crate) const SOURCE: u8 = 3;
pub(crate) const TEAM: u8 = 4;
struct Table {
    file: SourceFile,
    start: u64,
    stride: usize,
    count: u32,
}
impl Table {
    fn open(path: &std::path::Path, stride: usize) -> Result<Self, CbhError> {
        let file = SourceFile::open(path)?;
        let h = file.read_exact_range(0, 28)?;
        let count = u32::from_le_bytes(h[..4].try_into().unwrap());
        let magic = u32::from_le_bytes(h[8..12].try_into().unwrap());
        let payload = u32::from_le_bytes(h[12..16].try_into().unwrap());
        if magic != 1234567890 || payload as usize != stride - 9 || ![0, 4].contains(&h[24]) {
            return Err(CbhError::Unsupported(
                "Unknown entity table header or stride".into(),
            ));
        }
        let start = 28 + h[24] as u64;
        let expected = start
            .checked_add(
                (count as u64)
                    .checked_mul(stride as u64)
                    .ok_or(CbhError::Bounds)?,
            )
            .ok_or(CbhError::Bounds)?;
        if expected > file.len() || !(file.len() - start).is_multiple_of(stride as u64) {
            return Err(CbhError::Truncated);
        }
        Ok(Self {
            file,
            start,
            stride,
            count,
        })
    }
    fn record(&self, id: u32) -> Result<Vec<u8>, CbhError> {
        if id >= self.count {
            return Err(CbhError::Bounds);
        }
        self.file
            .read_exact_range(self.start + id as u64 * self.stride as u64, self.stride)
    }
}
fn field(bytes: &[u8]) -> EntityText {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let mut diagnostics = vec![];
    if end == bytes.len() {
        diagnostics.push(Diagnostic::new(
            "fullWidthName",
            "Name fills its fixed-width field without a terminator",
        ));
    }
    let text = match std::str::from_utf8(&bytes[..end]) {
        Ok(s) => s.to_string(),
        Err(_) => {
            diagnostics.push(Diagnostic::new(
                "unresolvedNameEncoding",
                "Name bytes are not valid UTF-8; replacements are displayed",
            ));
            String::from_utf8_lossy(&bytes[..end]).into_owned()
        }
    };
    EntityText {
        text: if text.is_empty() { None } else { Some(text) },
        diagnostics,
    }
}
pub(crate) fn missing() -> EntityText {
    EntityText {
        text: None,
        diagnostics: vec![Diagnostic::new(
            "missingNameTable",
            "Name companion is unavailable",
        )],
    }
}
#[derive(Clone)]
pub(crate) enum Cached {
    /// A single name: player, annotator or team.
    Name(EntityText),
    Tournament(TournamentText),
    Source(SourceText),
}
impl Cached {
    fn cost(&self) -> usize {
        let size = |v: &EntityText| {
            v.text.as_ref().map_or(0, String::capacity)
                + v.diagnostics
                    .iter()
                    .map(|d| {
                        d.code.capacity() + d.message.capacity() + std::mem::size_of::<Diagnostic>()
                    })
                    .sum::<usize>()
        };
        512 + match self {
            Self::Name(v) => size(v),
            Self::Tournament(v) => size(&v.event) + size(&v.site),
            Self::Source(v) => size(&v.title) + size(&v.publisher),
        }
    }
}
/// Byte-budgeted LRU cache of resolved names, keyed by (kind, id).
pub(crate) struct NameCache {
    budget: usize,
    used: usize,
    cache: HashMap<(u8, u32), Cached>,
    order: VecDeque<(u8, u32)>,
}
impl NameCache {
    pub(crate) fn new(budget: usize) -> Self {
        Self {
            budget,
            used: 0,
            cache: HashMap::new(),
            order: VecDeque::new(),
        }
    }
    pub(crate) fn get(&mut self, key: (u8, u32)) -> Option<Cached> {
        let value = self.cache.get(&key)?.clone();
        self.order.retain(|&k| k != key);
        self.order.push_back(key);
        Some(value)
    }
    pub(crate) fn put(&mut self, key: (u8, u32), value: Cached) {
        let cost = value.cost();
        if cost > self.budget {
            return;
        }
        while self.used + cost > self.budget {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some(v) = self.cache.remove(&old) {
                self.used -= v.cost();
            }
        }
        self.used += cost;
        self.cache.insert(key, value);
        self.order.push_back(key);
    }
}
/// Classic name tables: CBP players, CBT tournaments, CBC annotators,
/// CBS sources and CBE teams. Each record starts with a nine-byte tree prefix
/// (left, right, balance) and ends with the game count and first game.
pub struct EntityTables {
    players: Option<Table>,
    tournaments: Option<Table>,
    annotators: Option<Table>,
    sources: Option<Table>,
    teams: Option<Table>,
    cache: NameCache,
}
/// Table extension and record stride, in field order.
const TABLES: [(&str, usize); 5] = [
    ("cbp", 67),
    ("cbt", 99),
    ("cbc", 62),
    ("cbs", 68),
    ("cbe", 72),
];
impl EntityTables {
    fn from_tables(mut t: Vec<Option<Table>>, budget: usize) -> Self {
        let mut next = || t.remove(0);
        Self {
            players: next(),
            tournaments: next(),
            annotators: next(),
            sources: next(),
            teams: next(),
            cache: NameCache::new(budget),
        }
    }
    pub fn open_partial(companions: &CompanionSet, budget: usize) -> (Self, Vec<Diagnostic>) {
        let mut diagnostics = vec![];
        let tables = TABLES
            .iter()
            .map(|&(ext, stride)| {
                companions
                    .get(ext)
                    .and_then(|path| match Table::open(path, stride) {
                        Ok(table) => Some(table),
                        Err(error) => {
                            diagnostics.push(Diagnostic::new(
                                "unavailableNameTable",
                                format!("{ext}: {error}"),
                            ));
                            None
                        }
                    })
            })
            .collect();
        (Self::from_tables(tables, budget), diagnostics)
    }
    fn all(&self) -> impl Iterator<Item = &Table> {
        [
            &self.players,
            &self.tournaments,
            &self.annotators,
            &self.sources,
            &self.teams,
        ]
        .into_iter()
        .flatten()
    }
    pub fn verify_unchanged(&self) -> Result<(), CbhError> {
        for table in self.all() {
            table.file.verify_unchanged()?;
        }
        Ok(())
    }
    pub fn open(companions: &CompanionSet, budget: usize) -> Result<Self, CbhError> {
        let tables = TABLES
            .iter()
            .map(|&(ext, stride)| {
                companions
                    .get(ext)
                    .map(|p| Table::open(p, stride))
                    .transpose()
            })
            .collect::<Result<_, _>>()?;
        Ok(Self::from_tables(tables, budget))
    }
    pub fn player(&mut self, id: u32) -> Result<EntityText, CbhError> {
        if let Some(Cached::Name(v)) = self.cache.get((PLAYER, id)) {
            return Ok(v);
        }
        let Some(table) = &self.players else {
            return Ok(missing());
        };
        let row = table.record(id)?;
        let last = field(&row[9..39]);
        let first = field(&row[39..59]);
        let v = join_names(last, first);
        self.cache.put((PLAYER, id), Cached::Name(v.clone()));
        Ok(v)
    }
    pub fn tournament(&mut self, id: u32) -> Result<TournamentText, CbhError> {
        if let Some(Cached::Tournament(v)) = self.cache.get((TOURNAMENT, id)) {
            return Ok(v);
        }
        let Some(table) = &self.tournaments else {
            return Ok(TournamentText {
                event: missing(),
                site: missing(),
            });
        };
        let row = table.record(id)?;
        let v = TournamentText {
            event: field(&row[9..49]),
            site: field(&row[49..79]),
        };
        self.cache
            .put((TOURNAMENT, id), Cached::Tournament(v.clone()));
        Ok(v)
    }
    /// CBC: one 45-byte name, stored as ChessBase shows it ("Last,First").
    pub fn annotator(&mut self, id: u32) -> Result<EntityText, CbhError> {
        self.single(ANNOTATOR, id)
    }
    /// CBE: a 45-byte name, then team number, season flag, year and nation,
    /// which are not interpreted.
    pub fn team(&mut self, id: u32) -> Result<EntityText, CbhError> {
        self.single(TEAM, id)
    }
    fn single(&mut self, kind: u8, id: u32) -> Result<EntityText, CbhError> {
        if let Some(Cached::Name(v)) = self.cache.get((kind, id)) {
            return Ok(v);
        }
        let table = if kind == ANNOTATOR {
            &self.annotators
        } else {
            &self.teams
        };
        let Some(table) = table else {
            return Ok(missing());
        };
        let v = field(&table.record(id)?[9..54]);
        self.cache.put((kind, id), Cached::Name(v.clone()));
        Ok(v)
    }
    /// CBS: 25-byte title, 16-byte publisher, LE32 publication and version
    /// dates, version and quality bytes.
    pub fn source(&mut self, id: u32) -> Result<SourceText, CbhError> {
        if let Some(Cached::Source(v)) = self.cache.get((SOURCE, id)) {
            return Ok(v);
        }
        let Some(table) = &self.sources else {
            return Ok(SourceText {
                title: missing(),
                publisher: missing(),
                date: None,
            });
        };
        let row = table.record(id)?;
        let v = SourceText {
            title: field(&row[9..34]),
            publisher: field(&row[34..50]),
            date: crate::header::date(u32::from_le_bytes(row[50..54].try_into().unwrap())),
        };
        self.cache.put((SOURCE, id), Cached::Source(v.clone()));
        Ok(v)
    }
    pub fn cached_bytes(&self) -> usize {
        self.cache.used
    }
    pub fn bytes_read(&self) -> u64 {
        self.all().map(|t| t.file.bytes_read()).sum()
    }
}
/// "Last, First" from the two player name fields.
pub(crate) fn join_names(last: EntityText, first: EntityText) -> EntityText {
    let mut diagnostics = last.diagnostics;
    diagnostics.extend(first.diagnostics);
    let text = match (last.text, first.text) {
        (Some(a), Some(b)) => Some(format!("{a}, {b}")),
        (a, None) => a,
        (None, b) => b,
    };
    EntityText { text, diagnostics }
}
