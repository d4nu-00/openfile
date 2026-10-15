//! Opening classification: Classify Opening for the shown game and Set ECO
//! Codes for a database opened for editing (see docs/openings.md).
use crate::{commands::AppError, registry::Registry};
use std::sync::Arc;

/// Entries whose codes are written per sync.
const ECO_CHUNK: usize = 50_000;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    /// "B90", or none for a set-up or a game the table does not know.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eco: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The ply of the main line whose position the table holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ply: Option<u16>,
    /// The game's own ECO tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EcoResponse {
    pub games: String,
    /// Codes written.
    pub changed: String,
    pub unchanged: String,
    /// Games with another code, left alone ("only games without a code").
    pub kept: String,
    pub unclassified: String,
    pub unreadable: String,
    /// Coded games that already had the table's code.
    pub agreed: String,
    pub had_code: String,
    pub cancelled: bool,
}

impl Registry {
    /// Classify Opening (`CLASSIFYGAME`): the document's main line.
    pub fn classify_game(&self, document_id: &str) -> Result<Classification, AppError> {
        let d = self.document(document_id)?;
        let d = d
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        let found = chess_core::eco::classify_game(&d.game);
        Ok(Classification {
            eco: found.as_ref().map(|c| c.opening.eco.clone()),
            name: found.as_ref().map(|c| c.opening.name.clone()),
            ply: found.as_ref().map(|c| c.ply),
            current: d.game.tags.get("ECO").filter(|e| !e.is_empty()).cloned(),
        })
    }

    /// Set ECO Codes (`DBTOOLSSETECOCODE`): classifies every game of a 2CBH
    /// opened for editing and writes the codes that change, in chunks.
    /// Progress counts the scan, then the writing.
    pub fn set_eco_codes(
        &self,
        database_id: String,
        only_missing: bool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<EcoResponse, AppError> {
        // Refused before the long scan when the database is read-only.
        self.writer_of(&database_id)?;
        let db = self.own_handle(&database_id)?;
        let count = db.info().entry_count;
        let cancel = self.maintenance_cancel(&database_id)?;
        let found = db.classify_eco(
            cbh_reader::eco::EcoOptions { only_missing },
            &cancel,
            &mut progress,
        );
        let result = (|| {
            let found = found?;
            let mut response = EcoResponse {
                games: found.games.to_string(),
                changed: "0".into(),
                unchanged: found.unchanged.to_string(),
                kept: found.kept.to_string(),
                unclassified: found.unclassified.to_string(),
                unreadable: found.unreadable.to_string(),
                agreed: found.agreed.to_string(),
                had_code: found.had_code.to_string(),
                cancelled: found.cancelled,
            };
            if found.cancelled || found.changes.is_empty() {
                return Ok(response);
            }
            let _save = self
                .save_lock
                .lock()
                .map_err(|_| AppError::new("state", "Save service unavailable"))?;
            let (writer, path) = self.writer_of(&database_id)?;
            if self
                .database(&database_id)?
                .lock()
                .map_err(|_| AppError::new("state", "Database unavailable"))?
                .info()
                .entry_count
                != count
            {
                return Err(AppError::new(
                    "sourceChanged",
                    "The database changed during the classification; run it again",
                ));
            }
            let tree = self.tree_before_change(&database_id, None);
            let total = found.changes.len() as u64;
            let mut written = 0u64;
            let outcome = (|| -> Result<(), AppError> {
                for chunk in found.changes.chunks(ECO_CHUNK) {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        response.cancelled = true;
                        return Ok(());
                    }
                    writer
                        .lock()
                        .map_err(|_| AppError::new("state", "Database writer unavailable"))?
                        .set_eco_many(chunk)?;
                    written += chunk.len() as u64;
                    progress(written, total);
                }
                Ok(())
            })();
            if written > 0 {
                self.refresh_after_write(&database_id, &path)?;
                self.tree_after_change(&database_id, tree, None);
            }
            outcome?;
            response.changed = written.to_string();
            Ok(response)
        })();
        self.maintenance_done(&database_id, &cancel);
        result
    }
}

#[tauri::command]
pub fn classify_game(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
) -> Result<Classification, AppError> {
    state.classify_game(&document_id)
}

#[tauri::command]
pub async fn set_eco_codes(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    only_missing: bool,
    events: tauri::ipc::Channel<crate::maintenance::MaintenanceProgress>,
) -> Result<EcoResponse, AppError> {
    let s = state.inner().clone();
    let mut last = std::time::Instant::now();
    tauri::async_runtime::spawn_blocking(move || {
        s.set_eco_codes(database_id, only_missing, |done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::maintenance::MaintenanceProgress {
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}

// Opening Report (`OpeningReport`, `OPENINGREPORTBOARDCMD`): see
// docs/openings.md for what ChessBase's report holds and what is ported.

/// Games whose moves are read for the main line and "played by", at most:
/// the strongest by rating.
const REPORT_DECODE: usize = 5_000;
/// Plies of a game read past the report's position.
const REPORT_PLIES: usize = 20;
/// Rating from which a player counts as strong (the workbench's choice;
/// ChessBase's "strong grandmasters" filter was not traced).
const STRONG_ELO: u16 = 2_500;
/// Database games a fashion bucket needs for an index (the workbench's
/// choice).
const FASHION_MIN_GAMES: u64 = 100;
/// Move rows listed (ChessBase prepares 20).
const REPORT_MOVES: usize = 20;
/// ChessBase's "Main line" follows a move only while at least this many
/// games play it (`0x140708a30`).
const MAIN_LINE_GAMES: usize = 3;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportGame {
    pub ordinal: String,
    pub white: Option<String>,
    pub black: Option<String>,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    pub date: Option<String>,
    pub result: Option<String>,
    pub event: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPlayer {
    pub name: String,
    pub games: u32,
    /// The player's score in these games, in percent.
    pub score: f64,
    pub best_elo: Option<u16>,
    pub average_elo: Option<u16>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportMove {
    pub uci: String,
    pub san: String,
    pub games: u64,
    pub share: f64,
    pub score: Option<f64>,
    pub draws: Option<f64>,
    pub average_elo: Option<u32>,
    pub performance: Option<u32>,
    pub last_year: Option<u16>,
    /// The strongest players who chose it, among the games read.
    pub played_by: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FashionBucket {
    pub from: u16,
    pub to: u16,
    pub games: u32,
    /// The bucket's share of the position's games over its share of the
    /// database's games, in percent; 100 is average.
    pub index: Option<f64>,
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpeningReport {
    pub eco: Option<String>,
    pub name: Option<String>,
    /// The line to the position in SAN.
    pub line: Vec<String>,
    pub white_to_move: bool,
    pub games: u64,
    /// More games reach the position than the report reads (100,000).
    pub truncated: bool,
    pub cancelled: bool,
    pub white_wins: u64,
    pub draws: u64,
    pub black_wins: u64,
    pub other_results: u64,
    /// All games when there are fewer than 10, as ChessBase lists them.
    pub listed: Vec<ReportGame>,
    pub earliest: Option<ReportGame>,
    pub latest_annotated: Option<ReportGame>,
    pub latest_strong: Option<ReportGame>,
    pub latest: Option<ReportGame>,
    /// Games per year.
    pub years: Vec<(u16, u32)>,
    pub fashion: Vec<FashionBucket>,
    pub strong_white: Vec<ReportPlayer>,
    pub strong_black: Vec<ReportPlayer>,
    pub notable: Vec<ReportPlayer>,
    pub moves: Vec<ReportMove>,
    /// The move rows come from the opening tree (all games) rather than
    /// from the games read.
    pub moves_from_tree: bool,
    /// Games whose moves were read.
    pub read: u64,
    pub main_line: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportProgress {
    pub stage: String,
    pub done: String,
    pub total: String,
}

/// A date that sorts: "1946.??.??" as "1946.00.00".
fn sortable(date: &Option<String>) -> String {
    date.as_deref().unwrap_or("").replace('?', "0")
}

fn report_game(e: &crate::commands::EntryDto) -> ReportGame {
    ReportGame {
        ordinal: e.ordinal.clone(),
        white: e.white.clone(),
        black: e.black.clone(),
        white_elo: e.white_elo,
        black_elo: e.black_elo,
        date: e.date.clone(),
        result: e.result.clone(),
        event: e.event.clone(),
    }
}

/// Points of White in half points (2 win, 1 draw, 0 loss); None for other
/// results.
fn white_points(result: Option<&str>) -> Option<u32> {
    match result? {
        "1-0" => Some(2),
        "1/2-1/2" => Some(1),
        "0-1" => Some(0),
        _ => None,
    }
}

/// ChessBase's bucket size for the fashion chart, by the number of games
/// (`0x140704f20`).
pub fn fashion_bucket(games: u64) -> u16 {
    match games {
        n if n < 50 => 10,
        n if n < 301 => 5,
        n if n < 801 => 3,
        n if n < 2_001 => 2,
        _ => 1,
    }
}

/// The fashion index of `years` (the position's games per year) against
/// `all` (the database's), in buckets of `size` years aligned to multiples
/// of `size`.
pub fn fashion_buckets(
    years: &std::collections::BTreeMap<u16, u32>,
    all: &std::collections::BTreeMap<u16, u64>,
    size: u16,
) -> Vec<FashionBucket> {
    let (Some(&first), Some(&last)) = (years.keys().next(), years.keys().last()) else {
        return vec![];
    };
    let ours: u64 = years.values().map(|&n| u64::from(n)).sum();
    let theirs: u64 = all.values().sum();
    let mut out = vec![];
    let mut from = first - first % size;
    while from <= last {
        let to = from + size - 1;
        let games: u32 = years.range(from..=to).map(|(_, &n)| n).sum();
        let db: u64 = all.range(from..=to).map(|(_, &n)| n).sum();
        out.push(FashionBucket {
            from,
            to,
            games,
            // Buckets with few database games say little; the workbench
            // leaves them without an index.
            index: (db >= FASHION_MIN_GAMES && ours > 0)
                .then(|| (f64::from(games) / ours as f64) / (db as f64 / theirs as f64) * 100.0),
        });
        from += size;
    }
    out
}

#[derive(Default)]
struct PlayerTally {
    games: u32,
    points: u32,
    best: Option<u16>,
    elo_sum: u64,
    rated: u32,
}

impl PlayerTally {
    fn add(&mut self, points: Option<u32>, elo: Option<u16>) {
        self.games += 1;
        self.points += points.unwrap_or(1);
        if let Some(e) = elo.filter(|&e| e > 0) {
            self.best = Some(self.best.map_or(e, |b| b.max(e)));
            self.elo_sum += u64::from(e);
            self.rated += 1;
        }
    }
    fn row(&self, name: &str) -> ReportPlayer {
        ReportPlayer {
            name: name.into(),
            games: self.games,
            score: f64::from(self.points) * 50.0 / f64::from(self.games.max(1)),
            best_elo: self.best,
            average_elo: (self.rated > 0).then(|| (self.elo_sum / u64::from(self.rated)) as u16),
        }
    }
}

impl Registry {
    /// Games per year of the whole database, counted once per entry count.
    fn database_years(
        &self,
        database_id: &str,
        cancel: &std::sync::atomic::AtomicBool,
        progress: &mut impl FnMut(&str, u64, u64),
    ) -> Result<Option<Arc<std::collections::BTreeMap<u16, u64>>>, AppError> {
        let db = self.own_handle(database_id)?;
        let count = db.info().entry_count;
        if let Some((n, years)) = self.lock()?.year_counts.get(database_id)
            && *n == count
        {
            return Ok(Some(years.clone()));
        }
        let counted = db.year_counts(cancel, |d, t| {
            progress("Counting the database's years", d, t)
        })?;
        if counted.cancelled {
            return Ok(None);
        }
        let years = Arc::new(counted.years);
        self.lock()?
            .year_counts
            .insert(database_id.into(), (count, years.clone()));
        Ok(Some(years))
    }

    /// The opening report of the line's last position in `database_id`.
    pub fn opening_report(
        &self,
        database_id: String,
        line: crate::trees::Line,
        mut progress: impl FnMut(&str, u64, u64),
    ) -> Result<OpeningReport, AppError> {
        let cancel = self.maintenance_cancel(&database_id)?;
        let result = self.build_report(&database_id, &line, &cancel, &mut progress);
        self.maintenance_done(&database_id, &cancel);
        result
    }

    fn build_report(
        &self,
        database_id: &str,
        line: &crate::trees::Line,
        cancel: &std::sync::atomic::AtomicBool,
        progress: &mut impl FnMut(&str, u64, u64),
    ) -> Result<OpeningReport, AppError> {
        use std::{collections::HashMap, sync::atomic::Ordering::Relaxed};
        let (pos, hashes, _) = crate::trees::replay(line)?;
        let target = *hashes.last().unwrap_or(&0);
        let start = chess_core::model::parse_position(&line.start_fen)?;
        let mut positions = vec![start.clone()];
        for uci in &line.moves {
            let next = chess_core::model::apply_uci(positions.last().unwrap().clone(), uci)?;
            positions.push(next);
        }
        let named = chess_core::eco::classify(&positions);
        let mut report = OpeningReport {
            eco: named.as_ref().map(|c| c.opening.eco.clone()),
            name: named.map(|c| c.opening.name),
            line: crate::deep_analysis::san_line(&start, &line.moves),
            white_to_move: shakmaty::Position::turn(&pos).is_white(),
            ..Default::default()
        };
        progress("Searching the position", 0, 1);
        let (hits, truncated, _, _) = self.reference_hits(database_id, line, cancel, true)?;
        if cancel.load(Relaxed) {
            report.cancelled = true;
            return Ok(report);
        }
        report.truncated = truncated;
        // Summaries, in chunks so Stop and progress work.
        let total = hits.ordinals.len() as u64;
        let mut entries = Vec::with_capacity(hits.ordinals.len());
        for chunk in hits.ordinals.chunks(1_000) {
            if cancel.load(Relaxed) {
                report.cancelled = true;
                return Ok(report);
            }
            entries.extend(
                self.entries_of(database_id, chunk)?
                    .into_iter()
                    .filter(|e| e.kind == "game"),
            );
            progress("Reading the games' data", entries.len() as u64, total);
        }
        report.games = entries.len() as u64;
        // Results.
        for e in &entries {
            match white_points(e.result.as_deref()) {
                Some(2) => report.white_wins += 1,
                Some(1) => report.draws += 1,
                Some(_) => report.black_wins += 1,
                None => report.other_results += 1,
            }
        }
        // History.
        if entries.len() < 10 {
            report.listed = entries.iter().map(report_game).collect();
        }
        let dated: Vec<&crate::commands::EntryDto> = entries
            .iter()
            .filter(|e| {
                e.date
                    .as_deref()
                    .and_then(cbh_reader::history::year_of)
                    .is_some()
            })
            .collect();
        let key = |e: &&&crate::commands::EntryDto| {
            (sortable(&e.date), e.ordinal.parse::<u64>().unwrap_or(0))
        };
        report.earliest = dated.iter().min_by_key(key).map(|e| report_game(e));
        let latest = |f: &dyn Fn(&crate::commands::EntryDto) -> bool| {
            dated
                .iter()
                .filter(|e| f(e))
                .max_by_key(key)
                .map(|e| report_game(e))
        };
        // As ChessBase: the latest annotated game, the latest strong game if
        // it is another, and the latest game if it is yet another.
        report.latest_annotated =
            latest(&|e| e.annotator.as_deref().is_some_and(|a| !a.is_empty()));
        let strong = latest(&|e| {
            e.white_elo.unwrap_or(0) >= STRONG_ELO && e.black_elo.unwrap_or(0) >= STRONG_ELO
        });
        let last = latest(&|_| true);
        let same = |a: &Option<ReportGame>, b: &Option<ReportGame>| matches!((a, b), (Some(a), Some(b)) if a.ordinal == b.ordinal);
        if !same(&strong, &report.latest_annotated) {
            report.latest_strong = strong;
        }
        if !same(&last, &report.latest_annotated) && !same(&last, &report.latest_strong) {
            report.latest = last;
        }
        let mut years: std::collections::BTreeMap<u16, u32> = Default::default();
        for e in &dated {
            if let Some(y) = e.date.as_deref().and_then(cbh_reader::history::year_of) {
                *years.entry(y).or_default() += 1;
            }
        }
        report.years = years.iter().map(|(&y, &n)| (y, n)).collect();
        if entries.len() > 50 && !years.is_empty() {
            if let Some(all) = self.database_years(database_id, cancel, progress)? {
                report.fashion =
                    fashion_buckets(&years, &all, fashion_bucket(entries.len() as u64));
            } else {
                report.cancelled = true;
                return Ok(report);
            }
        }
        // Players.
        let mut white: HashMap<String, PlayerTally> = HashMap::new();
        let mut black: HashMap<String, PlayerTally> = HashMap::new();
        let mut both: HashMap<String, PlayerTally> = HashMap::new();
        for e in &entries {
            let p = white_points(e.result.as_deref());
            if let Some(w) = e.white.as_deref().filter(|n| !n.is_empty()) {
                white.entry(w.into()).or_default().add(p, e.white_elo);
                both.entry(w.into()).or_default().add(p, e.white_elo);
            }
            if let Some(b) = e.black.as_deref().filter(|n| !n.is_empty()) {
                let q = p.map(|p| 2 - p);
                black.entry(b.into()).or_default().add(q, e.black_elo);
                both.entry(b.into()).or_default().add(q, e.black_elo);
            }
        }
        let strongest = |m: &HashMap<String, PlayerTally>| {
            let mut v: Vec<ReportPlayer> = m
                .iter()
                .filter(|(_, t)| t.best.unwrap_or(0) >= STRONG_ELO)
                .map(|(n, t)| t.row(n))
                .collect();
            v.sort_by(|a, b| b.best_elo.cmp(&a.best_elo).then(b.games.cmp(&a.games)));
            v.truncate(10);
            v
        };
        report.strong_white = strongest(&white);
        report.strong_black = strongest(&black);
        // ChessBase's "Other notable players" (`0x140706c50`): more than a
        // hundredth of the games or more than 10, or an average rating over
        // 2500; at least 2 games; not listed above; at most 8.
        let listed: std::collections::HashSet<&str> = report
            .strong_white
            .iter()
            .chain(&report.strong_black)
            .map(|p| p.name.as_str())
            .collect();
        let hundredth = entries.len() as u32 / 100;
        let mut notable: Vec<ReportPlayer> = both
            .iter()
            .filter(|(n, t)| {
                let avg = if t.rated > 0 {
                    t.elo_sum / u64::from(t.rated)
                } else {
                    0
                };
                (t.games > hundredth || t.games > 10 || avg > 2_500)
                    && t.games >= 2
                    && !listed.contains(n.as_str())
            })
            .map(|(n, t)| t.row(n))
            .collect();
        notable.sort_by(|a, b| b.games.cmp(&a.games).then(a.name.cmp(&b.name)));
        notable.truncate(8);
        report.notable = notable;
        // The strongest games' moves from the position on.
        let mut by_strength: Vec<&crate::commands::EntryDto> = entries.iter().collect();
        by_strength.sort_by_key(|e| {
            std::cmp::Reverse(e.white_elo.unwrap_or(0).max(e.black_elo.unwrap_or(0)))
        });
        by_strength.truncate(REPORT_DECODE);
        let reader = self.game_reader(database_id)?;
        // (moves after the position, the mover's name and rating, White's
        // points, year)
        type Read = (
            Vec<String>,
            Option<String>,
            Option<u16>,
            Option<u32>,
            Option<u16>,
        );
        let mut read: Vec<Read> = vec![];
        let white_moves = report.white_to_move;
        for (i, e) in by_strength.iter().enumerate() {
            if cancel.load(Relaxed) {
                report.cancelled = true;
                return Ok(report);
            }
            if i % 100 == 0 {
                progress(
                    "Reading the strongest games' moves",
                    i as u64,
                    by_strength.len() as u64,
                );
            }
            let Ok(ordinal) = e.ordinal.parse::<u64>() else {
                continue;
            };
            let loaded = reader
                .lock()
                .map_err(|_| AppError::new("state", "Database unavailable"))?
                .load(ordinal);
            let Some(game) = loaded.ok().and_then(|l| l.game) else {
                continue;
            };
            let Some(after) = continuation(&game, target) else {
                continue;
            };
            let (name, elo) = if white_moves {
                (e.white.clone(), e.white_elo)
            } else {
                (e.black.clone(), e.black_elo)
            };
            let year = e.date.as_deref().and_then(cbh_reader::history::year_of);
            read.push((after, name, elo, white_points(e.result.as_deref()), year));
        }
        report.read = read.len() as u64;
        // Move rows: from the tree when it covers the position, otherwise
        // from the games read.
        let tree = self.opening_tree(database_id.into(), line.clone(), "report".into())?;
        let from_tree = matches!(
            tree.status,
            crate::trees::TreeStatus::Tree | crate::trees::TreeStatus::Line
        ) && !tree.moves.is_empty();
        let played_by = |uci: &str| {
            let mut v: Vec<(u16, String)> = read
                .iter()
                .filter(|r| r.0.first().map(String::as_str) == Some(uci))
                .filter_map(|r| r.1.clone().map(|n| (r.2.unwrap_or(0), n)))
                .collect();
            v.sort_by_key(|p| std::cmp::Reverse(p.0));
            let mut names: Vec<String> = vec![];
            for (_, n) in v {
                if !names.contains(&n) {
                    names.push(n);
                }
                if names.len() == 5 {
                    break;
                }
            }
            names
        };
        if from_tree {
            report.moves_from_tree = true;
            report.moves = tree
                .moves
                .iter()
                .take(REPORT_MOVES)
                .map(|m| ReportMove {
                    uci: m.uci.clone(),
                    san: m.san.clone(),
                    games: m.games,
                    share: m.share,
                    score: m.score,
                    draws: m.draws,
                    average_elo: m.average_elo,
                    performance: m.performance,
                    last_year: m.last_year,
                    played_by: played_by(&m.uci),
                })
                .collect();
        } else {
            #[derive(Default)]
            struct Row {
                games: u64,
                points: u64,
                draws: u64,
                decided: u64,
                elo: u64,
                rated: u64,
                last: Option<u16>,
            }
            let mut rows: HashMap<String, Row> = HashMap::new();
            for (after, _, elo, points, year) in &read {
                let Some(m) = after.first() else { continue };
                let r = rows.entry(m.clone()).or_default();
                r.games += 1;
                if let Some(p) = points {
                    let mine = if white_moves { *p } else { 2 - p };
                    r.points += u64::from(mine);
                    r.draws += u64::from(*p == 1);
                    r.decided += 1;
                }
                if let Some(e) = elo.filter(|&e| e > 0) {
                    r.elo += u64::from(e);
                    r.rated += 1;
                }
                if year.is_some() {
                    r.last = r.last.max(*year);
                }
            }
            let all: u64 = rows.values().map(|r| r.games).sum();
            let mut moves: Vec<ReportMove> = rows
                .into_iter()
                .map(|(uci, r)| ReportMove {
                    san: crate::deep_analysis::san_line(&pos, std::slice::from_ref(&uci))
                        .pop()
                        .unwrap_or_else(|| uci.clone()),
                    played_by: played_by(&uci),
                    uci,
                    games: r.games,
                    share: r.games as f64 * 100.0 / all.max(1) as f64,
                    score: (r.decided > 0).then(|| r.points as f64 * 50.0 / r.decided as f64),
                    draws: (r.decided > 0).then(|| r.draws as f64 * 100.0 / r.decided as f64),
                    average_elo: (r.rated > 0).then(|| (r.elo / r.rated) as u32),
                    performance: None,
                    last_year: r.last,
                })
                .collect();
            moves.sort_by(|a, b| b.games.cmp(&a.games).then(a.uci.cmp(&b.uci)));
            moves.truncate(REPORT_MOVES);
            report.moves = moves;
        }
        // Main line: the most played move while at least 3 of the games read
        // play it.
        let mut prefix: Vec<String> = vec![];
        let mut at = pos.clone();
        while prefix.len() < REPORT_PLIES {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for (after, ..) in &read {
                if after.len() > prefix.len() && after[..prefix.len()] == prefix[..] {
                    *counts.entry(after[prefix.len()].as_str()).or_default() += 1;
                }
            }
            let Some((m, n)) = counts
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
            else {
                break;
            };
            if n < MAIN_LINE_GAMES {
                break;
            }
            let m = m.to_string();
            report.main_line.extend(crate::deep_analysis::san_line(
                &at,
                std::slice::from_ref(&m),
            ));
            at = chess_core::model::apply_uci(at, &m)?;
            prefix.push(m);
        }
        progress("Done", 1, 1);
        Ok(report)
    }
}

/// The main-line moves of `game` after the first position whose board and
/// side to move hash to `target`, at most `REPORT_PLIES`.
fn continuation(game: &chess_core::Game, target: u64) -> Option<Vec<String>> {
    let mut pos = chess_core::model::parse_position(&game.start_fen).ok()?;
    let mut at = 0usize;
    let mut found = cbh_reader::tree::position_hash(
        shakmaty::Position::board(&pos),
        shakmaty::Position::turn(&pos),
    ) == target;
    let mut out = vec![];
    while let Some(&next) = game.nodes[at].children.first() {
        let uci = game.nodes[next as usize].uci.clone()?;
        if found {
            out.push(uci.clone());
            if out.len() >= REPORT_PLIES {
                break;
            }
        }
        pos = chess_core::model::apply_uci(pos, &uci).ok()?;
        at = next as usize;
        if !found
            && cbh_reader::tree::position_hash(
                shakmaty::Position::board(&pos),
                shakmaty::Position::turn(&pos),
            ) == target
        {
            found = true;
        }
    }
    found.then_some(out)
}

#[tauri::command]
pub async fn opening_report(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    line: crate::trees::Line,
    events: tauri::ipc::Channel<ReportProgress>,
) -> Result<OpeningReport, AppError> {
    let s = state.inner().clone();
    let mut last = std::time::Instant::now();
    tauri::async_runtime::spawn_blocking(move || {
        s.opening_report(database_id, line, |stage, done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(ReportProgress {
                    stage: stage.into(),
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fashion_index_compares_shares() {
        // ChessBase's bucket sizes.
        assert_eq!(
            [10, 49, 50, 300, 301, 800, 801, 2_000, 2_001].map(fashion_bucket),
            [10, 10, 5, 5, 3, 3, 2, 2, 1]
        );
        let years = [(2001, 30u32), (2006, 10)].into_iter().collect();
        // The database: 1,000 games in 2000-2004, 1,000 in 2005-2009, and
        // 50 in 2010.
        let all = [(2000, 1_000u64), (2005, 1_000), (2010, 50)]
            .into_iter()
            .collect();
        let b = fashion_buckets(&years, &all, 5);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].from, b[0].to, b[0].games), (2000, 2004, 30));
        // 75 % of the position's games against 1000/2050 of the database's.
        let i = b[0].index.unwrap();
        assert!((i - 75.0 / (1000.0 / 2050.0)).abs() < 1e-9, "{i}");
        assert!(b[1].index.unwrap() < 100.0);
        // A bucket with under 100 database games gets no index.
        let few = fashion_buckets(&[(2010, 5u32)].into_iter().collect(), &all, 5);
        assert_eq!(few[0].index, None);
    }
}
