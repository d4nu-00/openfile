//! Database maintenance that only reads: Check Integrity and Find Double
//! Games. Both scan the whole database in parallel through their own read
//! handles. See docs/maintenance.md for ChessBase's versions.
use crate::{CbhError, Database, DatabaseFormat, EntryKind, header::RawEntry};
use std::{collections::HashMap, sync::atomic::AtomicBool};

/// Problems listed at most; all are counted.
pub const MAX_PROBLEMS: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// The entry, or 0 for a problem of the files.
    pub ordinal: u64,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityReport {
    pub checked: u64,
    pub games: u64,
    pub texts: u64,
    pub deleted: u64,
    /// Problems by code, and the first `MAX_PROBLEMS` of them.
    pub counts: std::collections::BTreeMap<String, u64>,
    pub problems: Vec<Problem>,
    pub cancelled: bool,
}

fn problem(ordinal: u64, code: &str, message: impl Into<String>) -> Problem {
    Problem {
        ordinal,
        code: code.into(),
        message: message.into(),
    }
}

/// The highest id of each name list, to check the entries' references.
struct NameBounds {
    players: Option<u32>,
    tournaments: Option<u32>,
    annotators: Option<u32>,
    sources: Option<u32>,
}

impl NameBounds {
    fn of(db: &Database) -> Result<Self, CbhError> {
        let (mut players, mut tournaments, mut annotators, mut sources) = (None, None, None, None);
        let top =
            |slot: &mut Option<u32>, id: u32| *slot = Some(slot.map_or(id, |t: u32| t.max(id)));
        db.names_each_player(&mut |id, _, _| {
            top(&mut players, id);
            true
        })?;
        db.names_each_tournament(&mut |id, _, _| {
            top(&mut tournaments, id);
            true
        })?;
        db.names_each_annotator(&mut |id, _| {
            top(&mut annotators, id);
            true
        })?;
        db.names_each_source(&mut |id, _| {
            top(&mut sources, id);
            true
        })?;
        // 2CBH annotators are player records.
        if db.info().format == DatabaseFormat::New {
            annotators = players;
        }
        Ok(Self {
            players,
            tournaments,
            annotators,
            sources,
        })
    }
    fn check(&self, r: &RawEntry, out: &mut Vec<Problem>) {
        let mut bad = |what: &str, id: Option<u32>, top: Option<u32>| {
            if let Some(id) = id
                && top.is_none_or(|t| id > t)
            {
                out.push(problem(
                    r.ordinal,
                    "unknownName",
                    format!("{what} {id} is not in the name list"),
                ));
            }
        };
        bad("Player", r.white_id, self.players);
        bad("Player", r.black_id, self.players);
        bad("Tournament", r.tournament_id, self.tournaments);
        bad("Annotator", r.annotator_id, self.annotators);
        bad("Source", r.source_id, self.sources);
    }
}

impl Database {
    /// Check Integrity: reads every entry's record, moves and annotations
    /// and the names it refers to, and reports what cannot be read. Nothing
    /// is changed. `progress` gets the entries done.
    pub fn check_integrity(
        &self,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64),
    ) -> Result<IntegrityReport, CbhError> {
        let mut report = IntegrityReport::default();
        let mut files = vec![];
        if self.format == DatabaseFormat::New {
            // 2CBG and 2CBA declare their used length in a 12-byte header.
            for (file, ext) in [(&self.moves, "2cbg"), (&self.annotations, "2cba")] {
                match file {
                    None => files.push(problem(0, "missingFile", format!(".{ext} is missing"))),
                    Some(f) => {
                        let head = f.read_exact_range(0, 8)?;
                        let used = u64::from_le_bytes(head.try_into().unwrap());
                        if used < crate::v2::block::FILE_HEADER || used > f.len() {
                            files.push(problem(
                                0,
                                "streamLength",
                                format!(".{ext} declares {used} bytes but holds {}", f.len()),
                            ));
                        }
                    }
                }
            }
        }
        let names = NameBounds::of(self)?;
        let counts = std::sync::Mutex::new((0u64, 0u64, 0u64));
        let found = crate::scan::parallel(
            self,
            crate::scan::default_workers(),
            256,
            usize::MAX,
            cancel,
            |done, _| progress(done),
            |_| Ok(()),
            |db, _, start, n, out| {
                let (mut games, mut texts, mut deleted) = (0, 0, 0);
                for r in db.raw_chunk(start, n)? {
                    let mut found = vec![];
                    deleted += r.deleted as u64;
                    match r.kind {
                        EntryKind::Game => {
                            games += 1;
                            names.check(&r, &mut found);
                            db.check_game(&r, &mut found)?;
                        }
                        EntryKind::Text => texts += 1,
                        EntryKind::Unknown(k) => found.push(problem(
                            r.ordinal,
                            "unknownEntry",
                            format!("Entry kind {k} is not known"),
                        )),
                    }
                    out.extend(found.into_iter().map(|p| (r.ordinal, p)));
                }
                let mut c = counts.lock().unwrap();
                c.0 += games;
                c.1 += texts;
                c.2 += deleted;
                Ok(())
            },
        )?;
        let (games, texts, deleted) = *counts.lock().unwrap();
        report.checked = found.scanned;
        report.games = games;
        report.texts = texts;
        report.deleted = deleted;
        report.cancelled = found.cancelled;
        for p in files
            .into_iter()
            .chain(found.hits.into_iter().map(|(_, p)| p))
        {
            *report.counts.entry(p.code.clone()).or_default() += 1;
            if report.problems.len() < MAX_PROBLEMS {
                report.problems.push(p);
            }
        }
        Ok(report)
    }

    /// Reads a game's moves and annotations; problems go to `out`. Only a
    /// changed source is an error.
    fn check_game(&self, r: &RawEntry, out: &mut Vec<Problem>) -> Result<(), CbhError> {
        out.extend(
            r.diagnostics
                .iter()
                .map(|d| problem(r.ordinal, &d.code, d.message.clone())),
        );
        match (&self.moves, r.moves_offset) {
            (None, _) => {}
            (Some(_), None) => out.push(problem(
                r.ordinal,
                "noMoves",
                "The game has no moves record",
            )),
            (Some(file), Some(offset)) => match self.decode_moves_at(file, offset) {
                Ok(_) => {}
                Err(CbhError::SourceChanged) => return Err(CbhError::SourceChanged),
                Err(e @ (CbhError::Unsupported(_) | CbhError::LimitExceeded)) => {
                    out.push(problem(r.ordinal, "unsupportedMoves", e.to_string()))
                }
                Err(e) => out.push(problem(r.ordinal, "damagedMoves", e.to_string())),
            },
        }
        if let (Some(file), Some(offset)) = (&self.annotations, r.annotation_offset) {
            match self.parse_annotations(file, offset, r.ordinal) {
                Ok(_) => {}
                Err(CbhError::SourceChanged) => return Err(CbhError::SourceChanged),
                Err(e) => out.push(problem(r.ordinal, "damagedAnnotations", e.to_string())),
            }
        }
        Ok(())
    }
}

// --- Find Double Games ---------------------------------------------------

/// How names or moves are compared (ChessBase's Exact / Similar / Ignore).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Compare {
    Exact,
    Similar,
    Ignore,
}

/// Which game of a pair is marked (ChessBase's Decision).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    /// The first (lower-numbered) game.
    First,
    /// The second game.
    Second,
    /// The game with less data; see `completeness`.
    KeepBetter,
}

/// ChessBase's Kill Doubles settings (`KillDouble` section, dialog 894)
/// that the workbench ports.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoubleOptions {
    pub decision: Decision,
    pub names: Compare,
    /// Exact or Similar; Ignore is treated as Similar.
    pub moves: Compare,
    /// Shorter games are not compared (`MinGameLen`, in moves).
    pub min_moves: u16,
    pub ignore_result: bool,
    pub ignore_year: bool,
    /// Two games with different annotators are both kept.
    pub keep_annotators: bool,
    /// Of two annotated doubles, one is marked too (`KillVCs`); otherwise
    /// both are kept.
    pub delete_annotated: bool,
}

impl Default for DoubleOptions {
    /// ChessBase's defaults (`0x1407ab920`).
    fn default() -> Self {
        Self {
            decision: Decision::KeepBetter,
            names: Compare::Similar,
            moves: Compare::Similar,
            min_moves: 0,
            ignore_result: true,
            ignore_year: false,
            keep_annotators: false,
            delete_annotated: false,
        }
    }
}

/// Entries compared at most; larger databases are refused.
pub const MAX_DOUBLE_ENTRIES: u64 = 2_000_000;
/// Games with one key compared pairwise at most.
const MAX_GROUP: usize = 1_000;

/// What a game contributes to the comparison.
#[derive(Clone, Debug)]
struct Fingerprint {
    ordinal: u64,
    key: u64,
    /// The main line's moves, hashed.
    moves: u64,
    /// The main line's last position, hashed.
    last: u64,
    plies: u32,
    annotated: bool,
    annotator: Option<u32>,
    score: u32,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoublesReport {
    pub checked: u64,
    /// Pairs found, as (game marked, game kept), in order of the marked game.
    pub pairs: Vec<(u64, u64)>,
    /// Pairs of which both games are kept (annotated, or annotators differ).
    pub kept_both: u64,
    pub cancelled: bool,
}

fn fnv(bytes: &[u8], mut h: u64) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}
const FNV: u64 = 0xcbf2_9ce4_8422_2325;

/// "Last, First" of a player, case-folded without accents.
type Names = HashMap<u32, (String, String)>;

fn folded_names(db: &Database) -> Result<Names, CbhError> {
    let mut names = HashMap::new();
    db.names_each_player(&mut |id, last, first| {
        names.insert(
            id,
            (
                crate::sort::collation_key(last),
                crate::sort::collation_key(first),
            ),
        );
        true
    })?;
    Ok(names)
}

/// The completeness score of ChessBase's "keep better" (`0x140714c90`):
/// annotations count 10, and every field that is filled in counts 1.
fn completeness(r: &RawEntry, names: &Names, annotated: bool, longer: bool) -> u32 {
    let mut score = 10 * annotated as u32 + longer as u32;
    score += (r.white_elo.is_some() || r.black_elo.is_some()) as u32;
    score += (r.white_elo.is_some() && r.black_elo.is_some()) as u32;
    for id in [r.white_id, r.black_id] {
        if let Some((last, first)) = id.and_then(|i| names.get(&i)) {
            score += !last.is_empty() as u32 + !first.is_empty() as u32;
        }
    }
    score += r.result_code.is_some_and(|c| c <= 2) as u32;
    if let Some(date) = &r.date {
        score += date
            .split('.')
            .filter(|p| !p.is_empty() && !p.contains('?'))
            .count() as u32;
    }
    score += r.source_id.is_some() as u32;
    score += r.tournament_id.is_some() as u32;
    score += r
        .round
        .as_deref()
        .is_some_and(|v| !v.is_empty() && v != "?") as u32;
    score += r.eco_raw.is_some_and(|e| e >> 7 != 0) as u32;
    score
}

impl Database {
    /// Find Double Games: compares every game with the games that share its
    /// key (players, year and result as the options say) and decides, for
    /// each pair of doubles, which one to mark. Nothing is changed.
    pub fn find_doubles(
        &self,
        options: &DoubleOptions,
        cancel: &AtomicBool,
        progress: impl FnMut(u64, u64),
    ) -> Result<DoublesReport, CbhError> {
        let count = self.info().entry_count;
        if count > MAX_DOUBLE_ENTRIES {
            return Err(CbhError::Unsupported(format!(
                "Find Double Games works on databases of at most {MAX_DOUBLE_ENTRIES} entries"
            )));
        }
        let names = folded_names(self)?;
        let found = crate::scan::parallel(
            self,
            crate::scan::default_workers(),
            256,
            usize::MAX,
            cancel,
            progress,
            |_| Ok(()),
            |db, _, start, n, out| {
                for r in db.raw_chunk(start, n)? {
                    if r.deleted || r.kind != EntryKind::Game {
                        continue;
                    }
                    if let Some(f) = db.fingerprint_of(&r, &names, options)? {
                        out.push((r.ordinal, f));
                    }
                }
                Ok(())
            },
        )?;
        let mut report = DoublesReport {
            checked: found.scanned,
            cancelled: found.cancelled,
            ..Default::default()
        };
        if report.cancelled {
            return Ok(report);
        }
        let mut prints: Vec<Fingerprint> = found.hits.into_iter().map(|(_, f)| f).collect();
        prints.sort_by_key(|f| (f.key, f.ordinal));
        let mut marked = std::collections::HashSet::new();
        for group in prints.chunk_by(|a, b| a.key == b.key) {
            let group = &group[..group.len().min(MAX_GROUP)];
            for (i, a) in group.iter().enumerate() {
                if marked.contains(&a.ordinal) {
                    continue;
                }
                for b in &group[i + 1..] {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        report.cancelled = true;
                        return Ok(report);
                    }
                    if marked.contains(&b.ordinal) || !self.same_game(a, b, options.moves)? {
                        continue;
                    }
                    match decide(a, b, options) {
                        Some(gone) => {
                            let kept = if gone == a.ordinal {
                                b.ordinal
                            } else {
                                a.ordinal
                            };
                            marked.insert(gone);
                            report.pairs.push((gone, kept));
                            if gone == a.ordinal {
                                break;
                            }
                        }
                        None => report.kept_both += 1,
                    }
                }
            }
        }
        // A game kept against one double may be marked against another:
        // name the game finally kept.
        let partner: HashMap<u64, u64> = report.pairs.iter().copied().collect();
        for pair in &mut report.pairs {
            let mut kept = pair.1;
            for _ in 0..partner.len() {
                match partner.get(&kept) {
                    Some(&next) => kept = next,
                    None => break,
                }
            }
            pair.1 = kept;
        }
        report.pairs.sort_unstable();
        Ok(report)
    }

    fn fingerprint_of(
        &self,
        r: &RawEntry,
        names: &Names,
        options: &DoubleOptions,
    ) -> Result<Option<Fingerprint>, CbhError> {
        let Some(game) = self.moves_of(r)? else {
            return Ok(None);
        };
        let Some((plies, moves, last)) = main_line(&game, None) else {
            return Ok(None);
        };
        // As ChessBase compares only games of more than four plies.
        if plies < MIN_PLIES || plies.div_ceil(2) < options.min_moves as u32 {
            return Ok(None);
        }
        let variations = game.nodes.iter().any(|n| n.children.len() > 1);
        let notes = match self.annotation_texts(r)? {
            Some(t) => !t.is_empty(),
            None => false,
        };
        let annotated = variations
            || notes
            || game
                .nodes
                .iter()
                .any(|n| !n.nags.is_empty() || !n.squares.is_empty());
        // The key: players, year and result, as the options say.
        let mut key = FNV;
        for id in [r.white_id, r.black_id] {
            let (last, first) = id.and_then(|i| names.get(&i)).cloned().unwrap_or_default();
            match options.names {
                Compare::Exact => {
                    key = fnv(last.as_bytes(), key);
                    key = fnv(b",", key);
                    key = fnv(first.as_bytes(), key);
                }
                Compare::Similar => {
                    let prefix: String = last.chars().take(4).collect();
                    key = fnv(prefix.as_bytes(), key);
                }
                Compare::Ignore => {}
            }
            key = fnv(b"|", key);
        }
        if !options.ignore_year {
            key = fnv(&r.year.unwrap_or(0).to_le_bytes(), key);
        }
        if !options.ignore_result {
            key = fnv(&[r.result_code.unwrap_or(0xff)], key);
        }
        if options.moves == Compare::Exact {
            key = fnv(&plies.to_le_bytes(), key);
        }
        Ok(Some(Fingerprint {
            ordinal: r.ordinal,
            key,
            moves,
            last,
            plies,
            annotated,
            annotator: r.annotator_id,
            score: completeness(r, names, annotated, false),
        }))
    }
}

/// Games shorter than this are not compared.
const MIN_PLIES: u32 = 5;
/// A shorter game is a truncated copy of a longer one when it has at least
/// this many plies and this share of the longer one's.
const TRUNCATED_PLIES: u32 = 10;
const TRUNCATED_SHARE: f64 = 0.8;

/// The main line's length, moves hash and last position hash, or the
/// position hash after `stop` plies.
fn main_line(game: &chess_core::Game, stop: Option<u32>) -> Option<(u32, u64, u64)> {
    let hash = |p: &shakmaty::Chess| {
        let fen = shakmaty::fen::Fen::from_position(p, shakmaty::EnPassantMode::Legal);
        fnv(fen.to_string().as_bytes(), FNV)
    };
    let mut plies = 0u32;
    let mut moves = FNV;
    let mut pos = chess_core::model::parse_position(&game.start_fen).ok()?;
    let mut at = 0usize;
    while let Some(&child) = game.nodes[at].children.first() {
        if stop == Some(plies) {
            break;
        }
        let uci = game.nodes[child as usize].uci.as_deref()?;
        moves = fnv(uci.as_bytes(), fnv(b" ", moves));
        pos = chess_core::model::apply_uci(pos, uci).ok()?;
        plies += 1;
        at = child as usize;
    }
    Some((plies, moves, hash(&pos)))
}

impl Database {
    /// The position hash of entry `ordinal`'s main line after `ply` plies.
    fn position_after(&self, ordinal: u64, ply: u32) -> Result<Option<u64>, CbhError> {
        let raw = self.raw_chunk(ordinal, 1)?.remove(0);
        Ok(self
            .moves_of(&raw)?
            .and_then(|g| main_line(&g, Some(ply)))
            .filter(|(p, _, _)| *p == ply)
            .map(|(_, _, h)| h))
    }

    /// Doubles by their moves: the same main line; for Similar also the same
    /// last position, or a shorter game that ends where the longer one
    /// stands at that ply (a truncated copy).
    fn same_game(
        &self,
        a: &Fingerprint,
        b: &Fingerprint,
        moves: Compare,
    ) -> Result<bool, CbhError> {
        if a.plies == b.plies && a.moves == b.moves {
            return Ok(true);
        }
        if moves == Compare::Exact {
            return Ok(false);
        }
        if a.last == b.last {
            return Ok(true);
        }
        let (short, long) = if a.plies < b.plies { (a, b) } else { (b, a) };
        if short.plies < TRUNCATED_PLIES
            || (short.plies as f64) < TRUNCATED_SHARE * long.plies as f64
        {
            return Ok(false);
        }
        Ok(self.position_after(long.ordinal, short.plies)? == Some(short.last))
    }
}

/// The game of a pair to mark, as ChessBase's decision (`0x140715720`)
/// chooses it; `None` keeps both.
fn decide(a: &Fingerprint, b: &Fingerprint, o: &DoubleOptions) -> Option<u64> {
    let (first, second) = if a.ordinal < b.ordinal {
        (a, b)
    } else {
        (b, a)
    };
    if o.keep_annotators
        && let (Some(x), Some(y)) = (first.annotator, second.annotator)
        && x != y
    {
        return None;
    }
    match (first.annotated, second.annotated) {
        // The annotated game is kept, whatever the decision.
        (false, true) => return Some(first.ordinal),
        (true, false) => return Some(second.ordinal),
        (true, true) if !o.delete_annotated => return None,
        _ => {}
    }
    Some(match o.decision {
        Decision::First => first.ordinal,
        Decision::Second => second.ordinal,
        Decision::KeepBetter => {
            if first.annotated && second.annotated {
                // The game without an annotator goes first.
                match (first.annotator, second.annotator) {
                    (Some(_), None) => return Some(second.ordinal),
                    (None, Some(_)) => return Some(first.ordinal),
                    _ => {}
                }
            }
            let longer = |x: &Fingerprint, y: &Fingerprint| (x.plies > y.plies) as u32;
            let (s1, s2) = (
                first.score + longer(first, second),
                second.score + longer(second, first),
            );
            if s1 > s2 {
                second.ordinal
            } else if s2 > s1 {
                first.ordinal
            } else {
                second.ordinal
            }
        }
    })
}
