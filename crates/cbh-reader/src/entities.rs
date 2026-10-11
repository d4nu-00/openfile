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
fn missing() -> EntityText {
    EntityText {
        text: None,
        diagnostics: vec![Diagnostic::new(
            "missingNameTable",
            "Name companion is unavailable",
        )],
    }
}
#[derive(Clone)]
enum Cached {
    Player(EntityText),
    Tournament(TournamentText),
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
            Self::Player(v) => size(v),
            Self::Tournament(v) => size(&v.event) + size(&v.site),
        }
    }
}
pub struct EntityTables {
    players: Option<Table>,
    tournaments: Option<Table>,
    budget: usize,
    used: usize,
    cache: HashMap<(u8, u32), Cached>,
    order: VecDeque<(u8, u32)>,
}
impl EntityTables {
    pub fn open_partial(companions: &CompanionSet, budget: usize) -> (Self, Vec<Diagnostic>) {
        let mut diagnostics = vec![];
        let mut open = |ext, stride| {
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
        };
        let players = open("cbp", 67);
        let tournaments = open("cbt", 99);
        (
            Self {
                players,
                tournaments,
                budget,
                used: 0,
                cache: HashMap::new(),
                order: VecDeque::new(),
            },
            diagnostics,
        )
    }
    pub fn verify_unchanged(&self) -> Result<(), CbhError> {
        for table in [&self.players, &self.tournaments].into_iter().flatten() {
            table.file.verify_unchanged()?;
        }
        Ok(())
    }
    pub fn open(companions: &CompanionSet, budget: usize) -> Result<Self, CbhError> {
        let players = companions
            .get("cbp")
            .map(|p| Table::open(p, 67))
            .transpose()?;
        let tournaments = companions
            .get("cbt")
            .map(|p| Table::open(p, 99))
            .transpose()?;
        Ok(Self {
            players,
            tournaments,
            budget,
            used: 0,
            cache: HashMap::new(),
            order: VecDeque::new(),
        })
    }
    fn get(&mut self, key: (u8, u32)) -> Option<Cached> {
        let value = self.cache.get(&key)?.clone();
        self.order.retain(|&k| k != key);
        self.order.push_back(key);
        Some(value)
    }
    fn put(&mut self, key: (u8, u32), value: Cached) {
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
    pub fn player(&mut self, id: u32) -> Result<EntityText, CbhError> {
        if let Some(Cached::Player(v)) = self.get((0, id)) {
            return Ok(v);
        }
        let Some(table) = &self.players else {
            return Ok(missing());
        };
        let row = table.record(id)?;
        let last = field(&row[9..39]);
        let first = field(&row[39..59]);
        let mut diagnostics = last.diagnostics;
        diagnostics.extend(first.diagnostics);
        let text = match (last.text, first.text) {
            (Some(a), Some(b)) => Some(format!("{a}, {b}")),
            (a, None) => a,
            (None, b) => b,
        };
        let v = EntityText { text, diagnostics };
        self.put((0, id), Cached::Player(v.clone()));
        Ok(v)
    }
    pub fn tournament(&mut self, id: u32) -> Result<TournamentText, CbhError> {
        if let Some(Cached::Tournament(v)) = self.get((1, id)) {
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
        self.put((1, id), Cached::Tournament(v.clone()));
        Ok(v)
    }
    pub fn cached_bytes(&self) -> usize {
        self.used
    }
    pub fn bytes_read(&self) -> u64 {
        self.players.as_ref().map_or(0, |t| t.file.bytes_read())
            + self.tournaments.as_ref().map_or(0, |t| t.file.bytes_read())
    }
}
