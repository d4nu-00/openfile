//! Generate Repertoire and the Repertoire Report (see docs/repertoire.md):
//! White and Black repertoire databases built from a player's own games,
//! and the games of a database that reach a repertoire's marked positions.
use crate::{commands::AppError, registry::Registry, trees::san};
use cbh_reader::{
    name_list::NameKind,
    repertoire::{self, MarkedPosition},
    tree::GameStats,
    v2::write::Writer,
};
use shakmaty::{Chess, EnPassantMode, Position, fen::Fen, uci::UciMove};
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Frequent players offered, as the reference app offers (`0x140b19720`).
pub const FREQUENT_PLAYERS: usize = 8;
/// Players chosen at most.
pub const MAX_PLAYERS: usize = 8;
/// File names of the two databases, as the reference app names them
/// (`0x14079b440`), in the 2CBH format.
pub const WHITE_FILE: &str = "Repertoire White.2cbh";
pub const BLACK_FILE: &str = "Repertoire Black.2cbh";
/// Entries a report scans at most: the latest.
pub const MAX_SCAN: usize = 2_000_000;
/// Plies of each scanned main line.
pub const SCAN_PLIES: usize = 40;
/// Best games kept per position, as the reference app keeps (`0x14070d5e0`).
pub const MAX_HITS: usize = 100;
/// Games of a position named in the report; the rest can be listed.
const NAMED_HITS: usize = 10;
/// Positions with games shown at most.
const MAX_REPORT_POSITIONS: usize = 1_000;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepertoireProgress {
    /// "games" while the players' games are found, "lines" while read,
    /// "writing"; for a report "repertoire", then "scan".
    pub stage: &'static str,
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrequentPlayer {
    pub id: u32,
    pub name: Option<String>,
    pub games: u32,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepertoireSide {
    pub white: bool,
    /// The database written; absent when the side has no repertoire game.
    pub path: Option<String>,
    /// The players' games with the colour, deleted ones left out.
    pub games: u32,
    /// Games read (the latest [`repertoire::MAX_GAMES`]).
    pub read: u32,
    pub truncated: bool,
    /// Repertoire games written, and the games merged into them.
    pub repertoire_games: u32,
    pub merged: u32,
    /// Games the opening table does not classify, set-ups and unreadable
    /// games.
    pub unclassified: u32,
    pub rare: u32,
    /// Games whose moves did not all fit a repertoire game.
    pub cut: u32,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedRepertoire {
    pub players: Vec<String>,
    pub sides: Vec<RepertoireSide>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HitGame {
    pub ordinal: String,
    pub white: Option<String>,
    pub black: Option<String>,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    pub year: Option<u16>,
    pub result: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionHits {
    /// "1.e4 c5 2.Nf3".
    pub text: String,
    pub fen: String,
    /// Games whose deepest repertoire position this is.
    pub games: u64,
    /// The best [`MAX_HITS`] of them, best first.
    pub ordinals: Vec<String>,
    /// The first of those, named.
    pub top: Vec<HitGame>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    /// The repertoire game's names, "White (Black)".
    pub name: String,
    pub eco: Option<String>,
    pub positions: Vec<PositionHits>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepertoireReport {
    pub repertoire: String,
    /// Repertoire games read and the marked positions found in them.
    pub repertoire_games: u32,
    pub repertoire_truncated: bool,
    pub positions: u32,
    pub scanned: String,
    pub scan_truncated: bool,
    /// Games reaching a repertoire position.
    pub matched: String,
    /// Topics with games, in repertoire order.
    pub topics: Vec<Topic>,
    pub positions_truncated: bool,
    pub cancelled: bool,
}

/// The reference app's game rank for the report (`0x140464ca0`): the rating
/// percentage curve of a weighted rating, the side to move counted twice.
/// The factor for the game's age is left out (its reference date was not
/// traced).
pub fn game_score(stats: &GameStats, white_to_move: bool) -> f64 {
    let (w, b) = (f64::from(stats.white_elo), f64::from(stats.black_elo));
    let r = if white_to_move {
        (b + 2.0 * w) / 3.0
    } else {
        (w + 2.0 * b) / 3.0
    };
    if r >= 2850.0 {
        return 100.0;
    }
    (89.42282 / (((2850.0 - r) / 308.9042).powf(4.364275) + 1.0) + 6.76487).clamp(0.0, 100.0)
}

/// "1.e4 c5 2.Nf3" for UCI moves from the start position.
fn move_text(moves: &[String]) -> (String, Chess) {
    let mut pos = Chess::default();
    let mut out = String::new();
    for (i, uci) in moves.iter().enumerate() {
        if i % 2 == 0 {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&format!("{}.", i / 2 + 1));
        } else {
            out.push(' ');
        }
        out.push_str(&san(&pos, uci));
        let Some(m) = UciMove::from_ascii(uci.as_bytes())
            .ok()
            .and_then(|u| u.to_move(&pos).ok())
        else {
            break;
        };
        pos.play_unchecked(m);
    }
    (out, pos)
}

/// Running jobs, by database id, for cancelling.
#[derive(Default)]
pub struct RepertoireJobs(std::sync::Mutex<HashMap<String, Arc<AtomicBool>>>);

impl RepertoireJobs {
    fn begin(&self, database_id: &str) -> Arc<AtomicBool> {
        let cancel = Arc::new(AtomicBool::new(false));
        if let Ok(mut j) = self.0.lock()
            && let Some(old) = j.insert(database_id.to_string(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        cancel
    }
    fn end(&self, database_id: &str, cancel: &Arc<AtomicBool>) {
        if let Ok(mut j) = self.0.lock()
            && j.get(database_id).is_some_and(|c| Arc::ptr_eq(c, cancel))
        {
            j.remove(database_id);
        }
    }
    pub fn cancel(&self, database_id: &str) {
        if let Ok(j) = self.0.lock()
            && let Some(c) = j.get(database_id)
        {
            c.store(true, Ordering::Relaxed);
        }
    }
}

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
}

impl Registry {
    /// The most frequent players of the database's player list, which must
    /// have been prepared (the Players list).
    pub fn frequent_players(&self, database_id: String) -> Result<Vec<FrequentPlayer>, AppError> {
        let list = self
            .lock()?
            .name_lists
            .get(&(database_id.clone(), NameKind::Players))
            .cloned()
            .ok_or_else(|| AppError::new("noNameList", "The list has not been prepared"))?;
        let mut best: Vec<(u32, u32)> = vec![];
        {
            let mut list = list
                .lock()
                .map_err(|_| AppError::new("state", "List unavailable"))?;
            let n = list.len();
            let mut at = 0;
            while at < n {
                let rows = list.read(at, 4096)?;
                if rows.is_empty() {
                    break;
                }
                at += rows.len() as u64;
                for r in rows {
                    best.push((r.games, r.id));
                }
                best.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                best.truncate(FREQUENT_PLAYERS);
            }
        }
        let mut db = self.own_handle(&database_id)?;
        best.into_iter()
            .map(|(games, id)| {
                Ok(FrequentPlayer {
                    id,
                    name: db.player_name(id)?,
                    games,
                })
            })
            .collect()
    }

    /// Writes "Repertoire White.2cbh" and "Repertoire Black.2cbh" into
    /// `folder` from the games of `players` in the database. None when
    /// cancelled; nothing is written then.
    pub fn generate_repertoire(
        &self,
        database_id: String,
        players: Vec<u32>,
        folder: PathBuf,
        cancel: &AtomicBool,
        mut progress: impl FnMut(&'static str, u64, u64),
    ) -> Result<Option<GeneratedRepertoire>, AppError> {
        if players.is_empty() || players.len() > MAX_PLAYERS {
            return Err(AppError::new(
                "bounds",
                format!("Choose between 1 and {MAX_PLAYERS} players"),
            ));
        }
        if !folder.is_dir() {
            return Err(AppError::new("notFound", "The folder does not exist"));
        }
        for name in [WHITE_FILE, BLACK_FILE] {
            if folder.join(name).exists() {
                return Err(AppError::new(
                    "exists",
                    format!("“{name}” already exists in that folder. Choose another folder."),
                ));
            }
        }
        let mut reader = self.own_handle(&database_id)?;
        let mut names = vec![];
        let mut by_colour = [BTreeSet::new(), BTreeSet::new()];
        let total = players.len() as u64;
        for (i, &id) in players.iter().enumerate() {
            names.push(reader.player_name(id)?.unwrap_or_else(|| "?".into()));
            let Some(found) = reader.player_games(id, workers(), cancel, |d, t| {
                progress("games", i as u64 * t + d, total * t.max(1))
            })?
            else {
                return Ok(None);
            };
            for g in found.games {
                by_colour[usize::from(!g.white)].insert(g.ordinal);
            }
        }
        let mut built = vec![];
        for (side, ordinals) in by_colour.iter().enumerate() {
            let white = side == 0;
            let all: Vec<u64> = ordinals.iter().copied().collect();
            let read = &all[all.len().saturating_sub(repertoire::MAX_GAMES)..];
            let Some(lines) =
                reader.uci_lines(read, repertoire::READ_PLIES, workers(), cancel, |d, t| {
                    progress("lines", d, t)
                })?
            else {
                return Ok(None);
            };
            let mut rep = repertoire::build(&lines, white);
            // Set-ups and unreadable games count as not classified.
            rep.unclassified += read.len() - lines.len();
            built.push((white, all.len(), read.len(), rep));
        }
        let mut sides = vec![];
        for (white, games, read, rep) in built {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let path = folder.join(if white { WHITE_FILE } else { BLACK_FILE });
            let written = if rep.games.is_empty() {
                None
            } else {
                progress("writing", 0, rep.games.len() as u64);
                let mut w = Writer::create(&path)?;
                let games: Vec<chess_core::Game> =
                    rep.games.iter().map(|g| g.game.clone()).collect();
                w.append_all(&games)?;
                progress("writing", games.len() as u64, games.len() as u64);
                Some(path.to_string_lossy().into_owned())
            };
            sides.push(RepertoireSide {
                white,
                path: written,
                games: games as u32,
                read: read as u32,
                truncated: games > read,
                repertoire_games: rep.games.len() as u32,
                merged: rep.games.iter().map(|g| g.games as u32).sum(),
                unclassified: rep.unclassified as u32,
                rare: rep.rare as u32,
                cut: rep.games.iter().map(|g| g.cut as u32).sum(),
            });
        }
        Ok(Some(GeneratedRepertoire {
            players: names,
            sides,
        }))
    }

    /// The games of the database reaching the positions marked in the
    /// repertoire database at `repertoire`, each by the deepest such
    /// position it reaches, best first.
    pub fn repertoire_report(
        &self,
        database_id: String,
        repertoire: &Path,
        cancel: &AtomicBool,
        mut progress: impl FnMut(&'static str, u64, u64),
    ) -> Result<RepertoireReport, AppError> {
        let mut rep_db = cbh_reader::Database::open(repertoire, cbh_reader::ReadLimits::default())?;
        let count = rep_db.info().entry_count;
        let read = count.min(repertoire::MAX_REPERTOIRE_GAMES as u64);
        let mut topics: Vec<(String, Option<String>)> = vec![];
        let mut marked: Vec<MarkedPosition> = vec![];
        for ordinal in 1..=read {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if ordinal % 50 == 0 || ordinal == read {
                progress("repertoire", ordinal, read);
            }
            let loaded = rep_db.load(ordinal)?;
            let (Some(game), false) = (&loaded.game, loaded.summary.deleted) else {
                continue;
            };
            let first = game.nodes[0].comments.first().cloned();
            let name = first.unwrap_or_else(|| {
                let w = loaded.summary.white.clone().unwrap_or_default();
                match loaded.summary.black.as_deref() {
                    Some(b) if !b.is_empty() && b != "?" => format!("{w} ({b})"),
                    _ => w,
                }
            });
            let left = repertoire::MAX_POSITIONS.saturating_sub(marked.len());
            let found = repertoire::marked_positions(game, topics.len(), left);
            if !found.is_empty() {
                topics.push((name, loaded.summary.eco.clone()));
                marked.extend(found);
            }
        }
        let file = repertoire
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut report = RepertoireReport {
            repertoire: file,
            repertoire_games: read as u32,
            repertoire_truncated: count > read,
            positions: marked.len() as u32,
            scanned: "0".into(),
            scan_truncated: false,
            matched: "0".into(),
            topics: vec![],
            positions_truncated: false,
            cancelled: cancel.load(Ordering::Relaxed),
        };
        if report.cancelled || marked.is_empty() {
            return Ok(report);
        }
        // Each game goes to the deepest marked position it reaches, ties in
        // repertoire order. (The reference app takes the first in repertoire
        // order, which sends nearly every game to a shallow mark such as
        // 1.e4.) A position marked twice keeps its first place.
        let mut order: Vec<usize> = (0..marked.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(marked[i].moves.len()), i));
        let mut ranks: HashMap<u64, usize> = HashMap::new();
        for (rank, &i) in order.iter().enumerate() {
            ranks.entry(marked[i].hash).or_insert(rank);
        }
        let reader = self.own_handle(&database_id)?;
        let entries = reader.info().entry_count;
        let first = entries.saturating_sub(MAX_SCAN as u64) + 1;
        let ordinals: Vec<u64> = (first..=entries).collect();
        let Some(found) =
            reader.first_marked(&ordinals, SCAN_PLIES, &ranks, workers(), cancel, |d, t| {
                progress("scan", d, t)
            })?
        else {
            report.cancelled = true;
            return Ok(report);
        };
        report.scanned = ordinals.len().to_string();
        report.scan_truncated = first > 1;
        // Hits per position: (score, ordinal), best first.
        let mut hits: HashMap<usize, (u64, Vec<(f64, u64)>)> = HashMap::new();
        let mut matched = 0u64;
        for (&ordinal, hit) in ordinals.iter().zip(found) {
            let Some((rank, stats)) = hit else {
                continue;
            };
            matched += 1;
            let index = order[rank];
            let white_to_move = marked[index].moves.len().is_multiple_of(2);
            let entry = hits.entry(index).or_default();
            entry.0 += 1;
            entry.1.push((game_score(&stats, white_to_move), ordinal));
            if entry.1.len() > 4 * MAX_HITS {
                prune(&mut entry.1);
            }
        }
        report.matched = matched.to_string();
        let mut ranked: Vec<usize> = hits.keys().copied().collect();
        ranked.sort_unstable();
        report.positions_truncated = ranked.len() > MAX_REPORT_POSITIONS;
        ranked.truncate(MAX_REPORT_POSITIONS);
        let mut db = self.own_handle(&database_id)?;
        let mut by_topic: Vec<Vec<PositionHits>> = (0..topics.len()).map(|_| vec![]).collect();
        for index in ranked {
            let (games, mut list) = hits.remove(&index).unwrap_or_default();
            prune(&mut list);
            let m = &marked[index];
            let (text, pos) = move_text(&m.moves);
            let mut top = vec![];
            for &(_, ordinal) in list.iter().take(NAMED_HITS) {
                if let Some(e) = db.page(ordinal, 1)?.entries.into_iter().next() {
                    top.push(HitGame {
                        ordinal: ordinal.to_string(),
                        white: e.white,
                        black: e.black,
                        white_elo: e.white_elo,
                        black_elo: e.black_elo,
                        year: e.date.as_deref().and_then(|d| d.get(..4)?.parse().ok()),
                        result: e.result,
                    });
                }
            }
            by_topic[m.topic].push(PositionHits {
                text,
                fen: Fen::from_position(&pos, EnPassantMode::Legal).to_string(),
                games,
                ordinals: list.iter().map(|(_, o)| o.to_string()).collect(),
                top,
            });
        }
        report.topics = topics
            .into_iter()
            .zip(by_topic)
            .filter(|(_, p)| !p.is_empty())
            .map(|((name, eco), positions)| Topic {
                name,
                eco,
                positions,
            })
            .collect();
        Ok(report)
    }
}

/// Keeps the best [`MAX_HITS`], best first (later entries first on ties).
fn prune(list: &mut Vec<(f64, u64)>) {
    list.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)));
    list.truncate(MAX_HITS);
}

#[tauri::command]
pub async fn frequent_players(
    registry: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<Vec<FrequentPlayer>, AppError> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || registry.frequent_players(database_id))
        .await
        .map_err(|e| AppError::new("state", e.to_string()))?
}

fn sender(events: tauri::ipc::Channel<RepertoireProgress>) -> impl FnMut(&'static str, u64, u64) {
    let mut last = std::time::Instant::now();
    move |stage, done, total| {
        if last.elapsed().as_millis() >= 250 || done == total {
            last = std::time::Instant::now();
            let _ = events.send(RepertoireProgress {
                stage,
                done: done.to_string(),
                total: total.to_string(),
            });
        }
    }
}

#[tauri::command]
pub async fn generate_repertoire(
    registry: tauri::State<'_, Arc<Registry>>,
    jobs: tauri::State<'_, Arc<RepertoireJobs>>,
    database_id: String,
    players: Vec<u32>,
    folder: PathBuf,
    events: tauri::ipc::Channel<RepertoireProgress>,
) -> Result<Option<GeneratedRepertoire>, AppError> {
    let registry = registry.inner().clone();
    let jobs = jobs.inner().clone();
    let cancel = jobs.begin(&database_id);
    let (id, flag) = (database_id.clone(), cancel.clone());
    let result = tauri::async_runtime::spawn_blocking(move || {
        registry.generate_repertoire(id, players, folder, &flag, sender(events))
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()));
    jobs.end(&database_id, &cancel);
    result?
}

#[tauri::command]
pub async fn repertoire_report(
    registry: tauri::State<'_, Arc<Registry>>,
    jobs: tauri::State<'_, Arc<RepertoireJobs>>,
    database_id: String,
    repertoire: PathBuf,
    events: tauri::ipc::Channel<RepertoireProgress>,
) -> Result<RepertoireReport, AppError> {
    let registry = registry.inner().clone();
    let jobs = jobs.inner().clone();
    let cancel = jobs.begin(&database_id);
    let (id, flag) = (database_id.clone(), cancel.clone());
    let result = tauri::async_runtime::spawn_blocking(move || {
        registry.repertoire_report(id, &repertoire, &flag, sender(events))
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()));
    jobs.end(&database_id, &cancel);
    result?
}

#[tauri::command]
pub fn cancel_repertoire(
    jobs: tauri::State<'_, Arc<RepertoireJobs>>,
    database_id: String,
) -> Result<(), AppError> {
    jobs.cancel(&database_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_follow_the_rating_curve() {
        let s = |w, b| GameStats {
            result: 1,
            white_elo: w,
            black_elo: b,
            year: 2020,
        };
        assert_eq!(game_score(&s(2900, 2900), true), 100.0);
        let strong = game_score(&s(2700, 2700), true);
        let weak = game_score(&s(2000, 2000), true);
        assert!(strong > weak);
        // The side to move counts twice.
        assert!(game_score(&s(2700, 2000), true) > game_score(&s(2700, 2000), false));
        // Unrated games still score, at the curve's floor.
        assert!((game_score(&s(0, 0), true) - 6.76487).abs() < 0.01);
    }

    #[test]
    fn prune_keeps_the_best() {
        let mut v: Vec<(f64, u64)> = (0..300).map(|i| ((i % 7) as f64, i)).collect();
        prune(&mut v);
        assert_eq!(v.len(), MAX_HITS);
        assert_eq!(v[0].0, 6.0);
        assert!(v.windows(2).all(|w| w[0].0 >= w[1].0));
    }
}
