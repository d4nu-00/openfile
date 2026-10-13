//! Game-data search over a whole database (ChessBase's search mask, game data
//! tab). See docs/search.md. Name fields are resolved to id sets first; the
//! entry index is then scanned in chunks without decoding any game.
use crate::{CbhError, Database, DatabaseFormat, EntryKind, header::RawEntry};
use std::{
    collections::HashSet,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerQuery {
    /// Prefix of the last name.
    pub last: String,
    /// Prefix of the first name.
    pub first: String,
}

impl PlayerQuery {
    fn is_empty(&self) -> bool {
        self.last.trim().is_empty() && self.first.trim().is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EloMode {
    /// Neither rating needs to be in range (the range is ignored).
    #[default]
    None,
    One,
    Both,
    Average,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameFilter {
    pub white: PlayerQuery,
    pub black: PlayerQuery,
    pub ignore_colors: bool,
    pub wins_only: bool,
    pub tournament: String,
    pub annotator: String,
    pub elo_min: Option<u16>,
    pub elo_max: Option<u16>,
    pub elo_mode: EloMode,
    /// At most this rating difference between the players; both must be
    /// rated.
    pub elo_difference: Option<u16>,
    pub year_min: Option<u16>,
    pub year_max: Option<u16>,
    /// Inclusive ECO range, e.g. "B20".."B99".
    pub eco_from: String,
    pub eco_to: String,
    pub moves_min: Option<u16>,
    pub moves_max: Option<u16>,
    /// Allowed PGN results ("1-0", "0-1", "1/2-1/2", "0-0"); empty means any.
    pub results: Vec<String>,
    pub exclude_deleted: bool,
    /// Quick filter: every word must match a player, the tournament or its
    /// site, the annotator, the source, the ECO code, the year or the result.
    pub text: String,
    /// Games of this player id, with either colour (the players pane).
    pub player_id: Option<u32>,
    /// Games of this tournament id (the tournaments pane).
    pub tournament_id: Option<u32>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub ordinals: Vec<u64>,
    /// Entries examined; equals the entry count unless stopped early.
    pub scanned: u64,
    /// Stopped at the hit limit.
    pub truncated: bool,
    pub cancelled: bool,
}

fn fold(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Lower-case prefix test that ignores empty queries.
fn prefix(value: &str, query: &str) -> bool {
    query.is_empty() || value.to_lowercase().starts_with(query)
}

/// Packed ECO index (1..=500) of "A00".."E99".
fn eco_index(code: &str) -> Option<u16> {
    let b = code.trim().as_bytes();
    if b.len() != 3 || !(b'A'..=b'E').contains(&b[0].to_ascii_uppercase()) {
        return None;
    }
    let n: u16 = std::str::from_utf8(&b[1..]).ok()?.parse().ok()?;
    (n < 100).then(|| (b[0].to_ascii_uppercase() - b'A') as u16 * 100 + n + 1)
}

/// The player sides a name matched: white, black.
struct Sides {
    white: Option<HashSet<u32>>,
    black: Option<HashSet<u32>>,
}

struct Compiled {
    sides: Sides,
    tournaments: Option<HashSet<u32>>,
    annotators: Option<HashSet<u32>>,
    eco: Option<(u16, u16)>,
    words: Vec<Word>,
}

/// What one quick-filter word matches.
#[derive(Default)]
struct Word {
    players: HashSet<u32>,
    tournaments: HashSet<u32>,
    annotators: HashSet<u32>,
    sources: HashSet<u32>,
    eco: Option<(u16, u16)>,
    year: Option<u16>,
    /// Stored result codes.
    results: Vec<u8>,
}

impl Word {
    fn matches(&self, r: &RawEntry) -> bool {
        let has = |set: &HashSet<u32>, id: Option<u32>| id.is_some_and(|id| set.contains(&id));
        has(&self.players, r.white_id)
            || has(&self.players, r.black_id)
            || has(&self.tournaments, r.tournament_id)
            || has(&self.annotators, r.annotator_id)
            || has(&self.sources, r.source_id)
            || self.eco.is_some_and(|(from, to)| {
                let code = r.eco_raw.map_or(0, |e| e >> 7);
                code >= from && code <= to
            })
            || self.year.is_some_and(|y| r.year == Some(y))
            || r.result_code.is_some_and(|c| self.results.contains(&c))
    }
}

/// The ECO codes a word names: "B" (B00–B99), "B9" (B90–B99) or "B90".
fn eco_prefix(word: &str) -> Option<(u16, u16)> {
    let b = word.as_bytes();
    if b.is_empty() || b.len() > 3 || !(b'a'..=b'e').contains(&b[0]) {
        return None;
    }
    let digits = std::str::from_utf8(&b[1..]).ok()?;
    if !digits.bytes().all(|d| d.is_ascii_digit()) {
        return None;
    }
    let letter = (b[0] - b'a') as u16 * 100;
    let (lo, hi) = match digits.len() {
        0 => (0, 99),
        1 => {
            let d: u16 = digits.parse().ok()?;
            (d * 10, d * 10 + 9)
        }
        _ => {
            let d: u16 = digits.parse().ok()?;
            (d, d)
        }
    };
    Some((letter + lo + 1, letter + hi + 1))
}

impl GameFilter {
    fn matches(&self, c: &Compiled, r: &RawEntry) -> bool {
        if r.kind != EntryKind::Game || (self.exclude_deleted && r.deleted) {
            return false;
        }
        let has = |set: &Option<HashSet<u32>>, id: Option<u32>| {
            set.as_ref()
                .is_none_or(|s| id.is_some_and(|id| s.contains(&id)))
        };
        let code = r.result_code.unwrap_or(3);
        let pgn = crate::header::pgn_result(code);
        let (white_won, black_won) = (pgn == Some("1-0"), pgn == Some("0-1"));
        let straight = has(&c.sides.white, r.white_id) && has(&c.sides.black, r.black_id);
        let swapped = self.ignore_colors
            && has(&c.sides.white, r.black_id)
            && has(&c.sides.black, r.white_id);
        if !(straight || swapped) {
            return false;
        }
        if self.wins_only {
            // The first named player won: White, or Black when swapped.
            let won = (straight && white_won) || (swapped && black_won);
            if !won {
                return false;
            }
        }
        if !has(&c.tournaments, r.tournament_id) || !has(&c.annotators, r.annotator_id) {
            return false;
        }
        if self
            .player_id
            .is_some_and(|p| r.white_id != Some(p) && r.black_id != Some(p))
            || self
                .tournament_id
                .is_some_and(|t| r.tournament_id != Some(t))
        {
            return false;
        }
        if self.elo_mode != EloMode::None && (self.elo_min.is_some() || self.elo_max.is_some()) {
            let range = |e: Option<u16>| {
                e.is_some_and(|e| {
                    self.elo_min.is_none_or(|m| e >= m) && self.elo_max.is_none_or(|m| e <= m)
                })
            };
            let ok = match self.elo_mode {
                EloMode::One => range(r.white_elo) || range(r.black_elo),
                EloMode::Both => range(r.white_elo) && range(r.black_elo),
                EloMode::Average => match (r.white_elo, r.black_elo) {
                    (Some(w), Some(b)) => range(Some((w + b) / 2)),
                    _ => false,
                },
                EloMode::None => true,
            };
            if !ok {
                return false;
            }
        }
        if let Some(max) = self.elo_difference {
            match (r.white_elo, r.black_elo) {
                (Some(w), Some(b)) if w > 0 && b > 0 && w.abs_diff(b) <= max => {}
                _ => return false,
            }
        }
        if self.year_min.is_some() || self.year_max.is_some() {
            let Some(y) = r.year else { return false };
            if self.year_min.is_some_and(|m| y < m) || self.year_max.is_some_and(|m| y > m) {
                return false;
            }
        }
        if let Some((from, to)) = c.eco {
            let code = r.eco_raw.map_or(0, |e| e >> 7);
            if code < from || code > to {
                return false;
            }
        }
        let moves = r.move_count as u16;
        if self.moves_min.is_some_and(|m| moves < m) || self.moves_max.is_some_and(|m| moves > m) {
            return false;
        }
        if !self.results.is_empty() {
            let shown = if code == 7 { Some("0-0") } else { pgn };
            if !shown.is_some_and(|s| self.results.iter().any(|r| r == s)) {
                return false;
            }
        }
        c.words.iter().all(|w| w.matches(r))
    }
}

impl Database {
    fn players(&self, q: &PlayerQuery) -> Result<Option<HashSet<u32>>, CbhError> {
        if q.is_empty() {
            return Ok(None);
        }
        let (last, first) = (fold(&q.last), fold(&q.first));
        let mut set = HashSet::new();
        self.names_each_player(&mut |id, l, f| {
            if prefix(l, &last) && prefix(f, &first) {
                set.insert(id);
            }
            true
        })?;
        Ok(Some(set))
    }

    /// Compiles the quick filter's words; at most 8 are used.
    fn words(&self, text: &str) -> Result<Vec<Word>, CbhError> {
        let needles: Vec<String> = text
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .take(8)
            .collect();
        let mut words: Vec<Word> = needles
            .iter()
            .map(|n| Word {
                eco: eco_prefix(n),
                year: (n.len() == 4)
                    .then(|| n.parse().ok())
                    .flatten()
                    .filter(|y| *y > 0),
                results: match n.as_str() {
                    "1-0" => vec![2, 6],
                    "0-1" => vec![0, 4],
                    "1/2-1/2" | "½-½" | "1/2" | "½" | "=" => vec![1, 5],
                    _ => vec![],
                },
                ..Word::default()
            })
            .collect();
        if needles.is_empty() {
            return Ok(words);
        }
        let contains = |hay: &str, needle: &str| hay.to_lowercase().contains(needle);
        self.names_each_player(&mut |id, last, first| {
            for (w, n) in words.iter_mut().zip(&needles) {
                if contains(last, n) || contains(first, n) {
                    w.players.insert(id);
                }
            }
            true
        })?;
        self.names_each_tournament(&mut |id, event, site| {
            for (w, n) in words.iter_mut().zip(&needles) {
                if contains(event, n) || contains(site, n) {
                    w.tournaments.insert(id);
                }
            }
            true
        })?;
        self.names_each_annotator(&mut |id, name| {
            for (w, n) in words.iter_mut().zip(&needles) {
                if contains(name, n) {
                    w.annotators.insert(id);
                }
            }
            true
        })?;
        self.names_each_source(&mut |id, title| {
            for (w, n) in words.iter_mut().zip(&needles) {
                if contains(title, n) {
                    w.sources.insert(id);
                }
            }
            true
        })?;
        Ok(words)
    }

    /// Searches every entry for games matching `filter`. `progress` receives
    /// the number of entries scanned so far; `cancel` stops the scan.
    pub fn search(
        &mut self,
        filter: &GameFilter,
        max_hits: usize,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64),
    ) -> Result<SearchResult, CbhError> {
        self.verify()?;
        let tournaments = if filter.tournament.trim().is_empty() {
            None
        } else {
            let needle = fold(&filter.tournament);
            let mut set = HashSet::new();
            self.names_each_tournament(&mut |id, event, site| {
                if event.to_lowercase().contains(&needle) || site.to_lowercase().contains(&needle) {
                    set.insert(id);
                }
                true
            })?;
            Some(set)
        };
        let annotators = if filter.annotator.trim().is_empty() {
            None
        } else {
            let needle = fold(&filter.annotator);
            let mut set = HashSet::new();
            self.names_each_annotator(&mut |id, name| {
                if name.to_lowercase().contains(&needle) {
                    set.insert(id);
                }
                true
            })?;
            Some(set)
        };
        let eco = match (eco_index(&filter.eco_from), eco_index(&filter.eco_to)) {
            (None, None) => None,
            (a, b) => Some((a.unwrap_or(1), b.unwrap_or(500))),
        };
        let compiled = Compiled {
            sides: Sides {
                white: self.players(&filter.white)?,
                black: self.players(&filter.black)?,
            },
            tournaments,
            annotators,
            eco,
            words: self.words(&filter.text)?,
        };
        let count = self.info().entry_count;
        let mut result = SearchResult {
            ordinals: vec![],
            scanned: 0,
            truncated: false,
            cancelled: false,
        };
        const CHUNK: u64 = 8192;
        let mut start = 1;
        while start <= count {
            if cancel.load(Ordering::Relaxed) {
                result.cancelled = true;
                break;
            }
            let n = CHUNK.min(count + 1 - start);
            for raw in self.raw_chunk(start, n)? {
                result.scanned += 1;
                if filter.matches(&compiled, &raw) {
                    if result.ordinals.len() == max_hits {
                        result.truncated = true;
                        return Ok(result);
                    }
                    result.ordinals.push(raw.ordinal);
                }
            }
            progress(result.scanned);
            start += n;
        }
        Ok(result)
    }
}

impl DatabaseFormat {
    pub(crate) fn record_size(self) -> usize {
        match self {
            Self::Classic => 46,
            Self::New => crate::v2::record::RECORD,
        }
    }
}
