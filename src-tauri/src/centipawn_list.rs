//! Centipawn Analysis of the selected games of a database list
//! (`DBCentipawnAnalysisCmd`, `0x1403fef00`): each game is analysed as in
//! the game window and replaced by the annotated game, and per-player
//! statistics are kept for a CSV file (`0x14041e730`, `0x14041faf0`). See
//! docs/centipawn-analysis.md.
use crate::{
    centipawn::{
        CentipawnParams, analyse_game, check as check_params, has_moves_after_book,
        millis_per_move, start_engine,
    },
    clipboard::RowRange,
    commands::AppError,
    documents::{AnalysisItem, add_analysis},
    registry::Registry,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Games one run takes at most; the reference app warns above this.
pub const MAX_GAMES: usize = 100_000;
/// Skipped games listed in the report at most.
pub const MAX_SKIPPED_LISTED: usize = 200;
/// Games a player's CSV row lists at most, largest weights first.
pub const MAX_CSV_GAMES: usize = 1_000;

/// One game of a player in the statistics.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerGame {
    pub opponent: String,
    /// 0 when the opponent has no rating.
    pub opponent_elo: u16,
    /// The player's result in half points; None for an unknown result.
    pub half_points: Option<u8>,
    /// The game's value for the player ("BV").
    pub value: i32,
    /// The game's weight × 100.
    pub weight: i32,
}

/// A player's row: the reference app's CSV columns.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRow {
    pub name: String,
    pub elo: u16,
    pub perform: Option<i32>,
    pub points: f64,
    /// Largest weight first.
    pub games: Vec<PlayerGame>,
    pub weighted_average: Option<f64>,
}

/// The reference app's performance (`0x1404191c0`, `0x140c492c0`): over games
/// against rated opponents, their average rating plus 400 log10(p / (1 − p))
/// for the score p, ±800 at 0 % and 100 %.
pub fn perform(games: &[PlayerGame]) -> Option<i32> {
    let rated: Vec<_> = games
        .iter()
        .filter(|g| g.opponent_elo != 0 && g.half_points.is_some())
        .collect();
    if rated.is_empty() {
        return None;
    }
    let n = rated.len() as i64;
    let elo: i64 = rated.iter().map(|g| i64::from(g.opponent_elo)).sum();
    let half: i64 = rated
        .iter()
        .map(|g| i64::from(g.half_points.unwrap()))
        .sum();
    // The score as a whole percentage, as the reference app truncates it.
    let p = ((half * 50) / n) as f64 / 100.0;
    let d = if p <= 0.0 {
        -800.0
    } else if p >= 1.0 {
        800.0
    } else {
        400.0 * (p / (1.0 - p)).log10()
    };
    Some((elo as f64 / n as f64 + d + 0.5).floor() as i32)
}

/// The weighted average of the values (`0x140419610`).
pub fn weighted_average(games: &[PlayerGame]) -> Option<f64> {
    let w: i64 = games.iter().map(|g| i64::from(g.weight)).sum();
    (w > 0).then(|| {
        games
            .iter()
            .map(|g| i64::from(g.value) * i64::from(g.weight))
            .sum::<i64>() as f64
            / w as f64
    })
}

/// The statistics key: the name with commas made spaces, as the reference
/// app does.
fn key(name: &str) -> String {
    name.replace(',', " ")
}

/// White's result in half points.
pub fn white_half_points(result: Option<&str>) -> Option<u8> {
    match result.map(str::trim) {
        Some("1-0") => Some(2),
        Some("1/2-1/2" | "½-½") => Some(1),
        Some("0-1") => Some(0),
        _ => None,
    }
}

/// Per-player statistics of a run.
#[derive(Default)]
pub struct PlayerStats(BTreeMap<String, (u16, Vec<PlayerGame>)>);
impl PlayerStats {
    /// Adds a game for both players. A player keeps the rating of their first
    /// game.
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        white: &str,
        black: &str,
        white_elo: u16,
        black_elo: u16,
        white_half: Option<u8>,
        values: [i32; 2],
        weight: i32,
    ) {
        for (me, elo, opponent, opponent_elo, half, value) in [
            (white, white_elo, black, black_elo, white_half, values[0]),
            (
                black,
                black_elo,
                white,
                white_elo,
                white_half.map(|h| 2 - h),
                values[1],
            ),
        ] {
            self.0
                .entry(key(me))
                .or_insert_with(|| (elo, vec![]))
                .1
                .push(PlayerGame {
                    opponent: key(opponent),
                    opponent_elo,
                    half_points: half,
                    value,
                    weight,
                });
        }
    }
    /// The rows: each player's games by weight, largest first, and the players
    /// by weighted average, lowest first (`0x1403f2ac0`, `0x1403ee9a0`).
    pub fn rows(&self) -> Vec<PlayerRow> {
        let mut rows: Vec<PlayerRow> = self
            .0
            .iter()
            .map(|(name, (elo, games))| {
                let mut games = games.clone();
                games.sort_by_key(|g| std::cmp::Reverse(g.weight));
                PlayerRow {
                    name: name.clone(),
                    elo: *elo,
                    perform: perform(&games),
                    points: games
                        .iter()
                        .filter_map(|g| g.half_points)
                        .map(f64::from)
                        .sum::<f64>()
                        / 2.0,
                    weighted_average: weighted_average(&games),
                    games,
                }
            })
            .collect();
        rows.sort_by(|a, b| {
            a.weighted_average
                .unwrap_or(f64::MAX)
                .total_cmp(&b.weighted_average.unwrap_or(f64::MAX))
                .then_with(|| a.name.cmp(&b.name))
        });
        rows
    }
}

/// The CSV file (`0x14041faf0`): "Player Name,Elo,Perform,Points", a value
/// per game ("BV-n"), "Weighted Av", then each game's opponent, rating,
/// points and weight ("Opponent-n").
pub fn csv(rows: &[PlayerRow]) -> String {
    let n = rows
        .iter()
        .map(|r| r.games.len().min(MAX_CSV_GAMES))
        .max()
        .unwrap_or(0);
    let mut header = vec![
        "Player Name".to_string(),
        "Elo".into(),
        "Perform".into(),
        "Points".into(),
    ];
    header.extend((1..=n).map(|i| format!("BV-{i}")));
    header.push("Weighted Av".into());
    header.extend((1..=n).map(|i| format!("Opponent-{i}")));
    let mut out = header.join(",");
    out.push('\n');
    for r in rows {
        let games = &r.games[..r.games.len().min(MAX_CSV_GAMES)];
        let pad = n - games.len();
        let mut cells = vec![
            r.name.clone(),
            r.elo.to_string(),
            r.perform.map(|p| p.to_string()).unwrap_or_default(),
            format!("{:.1}", r.points),
        ];
        cells.extend(games.iter().map(|g| g.value.to_string()));
        cells.extend(std::iter::repeat_n(" ".to_string(), pad));
        cells.push(
            r.weighted_average
                .map(|v| format!("{v:.2}"))
                .unwrap_or_default(),
        );
        cells.extend(games.iter().map(|g| {
            format!(
                "{} {} {:.1} wgt={:.1}",
                g.opponent,
                g.opponent_elo,
                f64::from(g.half_points.unwrap_or(0)) * 0.5,
                f64::from(g.weight) * 0.01
            )
        }));
        cells.extend(std::iter::repeat_n(" ".to_string(), pad));
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

/// A game left out, and why.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedGame {
    pub ordinal: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CentipawnListReport {
    pub cancelled: bool,
    /// The engine failed; the games before were kept.
    pub error: Option<String>,
    pub games: u32,
    pub analysed: u32,
    pub replaced: u32,
    pub skipped_count: u32,
    /// At most [`MAX_SKIPPED_LISTED`].
    pub skipped: Vec<SkippedGame>,
    pub millis_per_move: u64,
    pub players: Vec<PlayerRow>,
    /// The CSV file's text.
    pub csv: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CentipawnListProgress {
    /// The game being analysed, from 1.
    pub game: u32,
    pub games: u32,
    /// Positions of this game.
    pub done: u32,
    pub total: u32,
    pub label: String,
}

impl Registry {
    /// Centipawn Analysis of the selected games of a 2CBH database opened for
    /// editing; each annotated game replaces its entry.
    pub fn centipawn_list(
        &self,
        database_id: &str,
        ranges: &[RowRange],
        params: &CentipawnParams,
        cancel: &AtomicBool,
        mut progress: impl FnMut(&CentipawnListProgress),
    ) -> Result<CentipawnListReport, AppError> {
        check_params(params)?;
        self.writer_of(database_id)?;
        let (ordinals, truncated) = self.ordinals_of_rows(database_id, ranges, MAX_GAMES)?;
        if truncated {
            return Err(AppError::new(
                "limitExceeded",
                format!("Centipawn Analysis takes at most {MAX_GAMES} games at once"),
            ));
        }
        let mut report = CentipawnListReport {
            cancelled: false,
            error: None,
            games: ordinals.len() as u32,
            analysed: 0,
            replaced: 0,
            skipped_count: 0,
            skipped: vec![],
            millis_per_move: millis_per_move(params.seconds),
            players: vec![],
            csv: String::new(),
        };
        let mut stats = PlayerStats::default();
        let skip = |report: &mut CentipawnListReport, ordinal: u64, reason: String| {
            report.skipped_count += 1;
            if report.skipped.len() < MAX_SKIPPED_LISTED {
                report.skipped.push(SkippedGame {
                    ordinal: ordinal.to_string(),
                    reason,
                });
            }
        };
        let mut session = start_engine(params)?;
        for (i, &ordinal) in ordinals.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                break;
            }
            if self.open_in_tab(database_id, ordinal)? {
                skip(&mut report, ordinal, "The game is open in a tab".into());
                continue;
            }
            let (game, summary) = match self.replaceable_game(database_id, ordinal)? {
                Ok(found) => found,
                Err(reason) => {
                    skip(&mut report, ordinal, reason);
                    continue;
                }
            };
            if summary.deleted {
                skip(&mut report, ordinal, "The game is marked as deleted".into());
                continue;
            }
            if !has_moves_after_book(&game) {
                skip(&mut report, ordinal, "No moves after its opening".into());
                continue;
            }
            let white = summary.white.clone().unwrap_or_default();
            let black = summary.black.clone().unwrap_or_default();
            let mut state = CentipawnListProgress {
                game: i as u32 + 1,
                games: report.games,
                done: 0,
                total: 0,
                label: format!("{white} – {black}"),
            };
            progress(&state);
            let analysis = match analyse_game(&game, params, &mut session, cancel, |done, total| {
                state.done = done;
                state.total = total;
                progress(&state);
            }) {
                Ok(a) => a,
                Err(e) if e.code == "noMoves" => {
                    skip(&mut report, ordinal, e.message);
                    continue;
                }
                Err(e) => {
                    report.error = Some(e.message);
                    break;
                }
            };
            if analysis.report.cancelled {
                report.cancelled = true;
                break;
            }
            report.analysed += 1;
            let mut annotated = game.clone();
            let items = analysis
                .report
                .items
                .iter()
                .map(|t| AnalysisItem {
                    node: t.node,
                    nag: None,
                    evaluation: None,
                    comment: Some(t.comment.clone()),
                    line: vec![],
                    line_comment: None,
                })
                .collect();
            add_analysis(&mut annotated, items)?;
            if !analysis.profile.is_empty() {
                annotated.profile = analysis.profile.clone();
            }
            match self.replace_if_unchanged(database_id, ordinal, &game, &annotated) {
                Ok(()) => report.replaced += 1,
                Err(e) if e.code == "staleGame" => {
                    skip(&mut report, ordinal, e.message);
                    continue;
                }
                Err(e) => {
                    report.error = Some(e.message);
                    break;
                }
            }
            stats.add(
                &white,
                &black,
                summary.white_elo.unwrap_or(0),
                summary.black_elo.unwrap_or(0),
                white_half_points(summary.result.as_deref()),
                analysis.values,
                analysis.weight,
            );
        }
        let _ = session.shutdown();
        report.players = stats.rows();
        report.csv = csv(&report.players);
        Ok(report)
    }
}

/// The running list analysis, for Stop.
#[derive(Default)]
pub struct CentipawnListJob(Mutex<Option<Arc<AtomicBool>>>);

#[tauri::command]
pub async fn centipawn_list(
    registry: tauri::State<'_, Arc<Registry>>,
    job: tauri::State<'_, Arc<CentipawnListJob>>,
    database_id: String,
    ranges: Vec<RowRange>,
    params: CentipawnParams,
    events: tauri::ipc::Channel<CentipawnListProgress>,
) -> Result<CentipawnListReport, AppError> {
    let registry = registry.inner().clone();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut current = job
            .0
            .lock()
            .map_err(|_| AppError::new("state", "Centipawn Analysis unavailable"))?;
        if current.as_ref().is_some_and(|c| !c.load(Ordering::Relaxed)) {
            return Err(AppError::new(
                "busy",
                "Centipawn Analysis of a list is already running",
            ));
        }
        *current = Some(cancel.clone());
    }
    let flag = cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        registry.centipawn_list(&database_id, &ranges, &params, &flag, |p| {
            let _ = events.send(p.clone());
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()));
    cancel.store(true, Ordering::Relaxed);
    result?
}

#[tauri::command]
pub fn cancel_centipawn_list(job: tauri::State<'_, Arc<CentipawnListJob>>) {
    if let Ok(current) = job.0.lock()
        && let Some(c) = current.as_ref()
    {
        c.store(true, Ordering::Relaxed);
    }
}

/// Saves the statistics' CSV text where the user chose; only `.csv` files.
#[tauri::command]
pub async fn save_csv(path: PathBuf, text: String) -> Result<(), AppError> {
    let csv = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    if !csv || !path.is_absolute() {
        return Err(AppError::new(
            "invalidPath",
            "Choose an absolute path ending in .csv",
        ));
    }
    if text.len() > 64 * 1024 * 1024 {
        return Err(AppError::new("limitExceeded", "The CSV text is too large"));
    }
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&path, text))
        .await
        .map_err(|e| AppError::new("state", e.to_string()))?
        .map_err(|e| AppError::new("io", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game(opponent_elo: u16, half: Option<u8>, value: i32, weight: i32) -> PlayerGame {
        PlayerGame {
            opponent: "X".into(),
            opponent_elo,
            half_points: half,
            value,
            weight,
        }
    }
    #[test]
    fn performance_follows_the_reference_formula() {
        // 1.5 of 2 against 2400 and 2600: 75 %, +191.
        assert_eq!(
            perform(&[game(2400, Some(2), 0, 1), game(2600, Some(1), 0, 1)]),
            Some(2691)
        );
        assert_eq!(perform(&[game(2500, Some(2), 0, 1)]), Some(3300));
        assert_eq!(perform(&[game(2500, Some(0), 0, 1)]), Some(1700));
        // Unrated opponents and unknown results do not count.
        assert_eq!(
            perform(&[game(0, Some(2), 0, 1), game(2500, None, 0, 1)]),
            None
        );
    }
    #[test]
    fn statistics_and_csv_follow_the_reference_layout() {
        let mut s = PlayerStats::default();
        s.add(
            "Player, A",
            "Player, B",
            2500,
            2400,
            Some(2),
            [10, 40],
            3000,
        );
        s.add(
            "Player, B",
            "Player, A",
            2410,
            2500,
            Some(1),
            [20, 30],
            1000,
        );
        let rows = s.rows();
        // A: (10 × 3000 + 30 × 1000) / 4000 = 15; B: (40 × 3000 + 20 × 1000) / 4000 = 35.
        assert_eq!(rows[0].name, "Player  A");
        assert_eq!(rows[0].weighted_average, Some(15.0));
        assert_eq!(rows[0].points, 1.5);
        assert_eq!(rows[0].elo, 2500);
        assert_eq!(rows[1].elo, 2400);
        assert_eq!(rows[0].games[0].weight, 3000);
        let text = csv(&rows);
        let mut lines = text.lines();
        assert_eq!(
            lines.next().unwrap(),
            "Player Name,Elo,Perform,Points,BV-1,BV-2,Weighted Av,Opponent-1,Opponent-2"
        );
        assert_eq!(
            lines.next().unwrap(),
            "Player  A,2500,2596,1.5,10,30,15.00,Player  B 2400 1.0 wgt=30.0,Player  B 2410 0.5 wgt=10.0"
        );
        assert_eq!(white_half_points(Some("0-1")), Some(0));
        assert_eq!(white_half_points(Some("*")), None);
    }
}
