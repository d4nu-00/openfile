//! Error Report (`BLUNDER_REPORT`; see docs/error-report.md): an engine of
//! the user's analyses the selected games to a fixed depth, and the losses of
//! the counted moves give the error value, the reference app's Blunder Elo curve and
//! the workbench's blunder statistics.
use crate::{
    blunder_check::{MATE, evaluate_to},
    clipboard::RowRange,
    commands::AppError,
    registry::Registry,
};
use engine::{AnalysisRequest, EngineSession, EngineSpec};
use serde::{Deserialize, Serialize};
use shakmaty::{Chess, Color, Position, Role, san::SanPlus, uci::UciMove};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::atomic::AtomicBool,
    time::Duration,
};

/// Games analysed at most.
pub const MAX_GAMES: usize = 1_000;
/// Plies of each main line analysed, as the reference app cuts games off.
pub const MAX_PLIES: usize = 120;
/// The reference app's depths for Quick and Slower (`0x1407a82b0`).
pub const QUICK_DEPTH: u32 = 9;
pub const SLOW_DEPTH: u32 = 12;
/// The longest search of one position, in milliseconds.
const QUICK_CAP: u64 = 2_000;
const SLOW_CAP: u64 = 6_000;
/// Losses are clamped to this, as the reference app does.
pub const MAX_LOSS: i32 = 400;
/// Counted moves needed for a Blunder Elo.
pub const MIN_MOVES: u32 = 20;
const MAX_WORST: usize = 30;
/// A loss from this counts as an error in the contexts and transitions.
const ERROR: i32 = 50;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorReportParams {
    pub engine: PathBuf,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// The reference app's "Slower (top games)": depth 12 instead of 9.
    pub slower: bool,
    /// Count this player's moves; both sides when none.
    pub player: Option<u32>,
    /// Rows of the game list.
    pub ranges: Vec<RowRange>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Opening,
    Middlegame,
    Endgame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Strength {
    Stronger,
    Equal,
    Weaker,
    Unrated,
}

/// One counted move.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveRecord {
    pub ordinal: u64,
    pub node: u32,
    /// Ply from the start, 0 for the first move.
    pub ply: u16,
    pub white: bool,
    pub san: String,
    pub best: Option<String>,
    /// Centipawns, 0..=[`MAX_LOSS`].
    pub loss: i32,
    pub weight: f64,
    /// The mover's evaluation before and after the move.
    pub before: i32,
    pub after: i32,
    pub phase: Phase,
    pub opponent: Strength,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextRow {
    pub moves: u32,
    /// Moves losing [`ERROR`] centipawns or more.
    pub errors: u32,
    pub average_loss: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorstMove {
    pub ordinal: String,
    pub node: u32,
    /// "23." or "23..."
    pub number: String,
    pub san: String,
    pub best: Option<String>,
    pub loss: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorReport {
    pub cancelled: bool,
    pub depth: u32,
    pub games: u32,
    /// Games without moves the engine could analyse.
    pub skipped: u32,
    /// More games were selected than [`MAX_GAMES`].
    pub truncated: bool,
    /// The player counted, when one was chosen.
    pub player: Option<String>,
    pub moves: u32,
    /// Average loss over the counted moves.
    pub average_loss: Option<f64>,
    pub error_value: Option<f64>,
    pub plies_per_game: Option<f64>,
    pub blunder_elo: Option<i32>,
    /// The counted side's average rating in the analysed games.
    pub rating: Option<u32>,
    pub difference: Option<i32>,
    /// Inaccuracies (25–49), mistakes (50–99), blunders (100–299) and losing
    /// blunders (300+).
    pub severity: [u32; 4],
    /// Losses from 25 in bins of 25 up to 400.
    pub distribution: Vec<u32>,
    pub phases: BTreeMap<String, ContextRow>,
    pub opponents: BTreeMap<String, ContextRow>,
    /// Errors by band before (row) and after (column): winning, better,
    /// equal, worse, lost.
    pub transitions: [[u32; 5]; 5],
    pub worst: Vec<WorstMove>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorProgress {
    pub games_done: u32,
    pub games: u32,
    pub positions: u32,
}

/// A player of the selection, for choosing whose moves count.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionPlayer {
    pub id: u32,
    pub name: Option<String>,
    pub games: u32,
}

/// The reference app's Blunder Elo for error value `e` and `plies` analysed per game
/// (`0x1404705d0`), without its slope term.
pub fn blunder_elo(e: f64, plies: f64) -> i32 {
    let base = if e < 15.0 {
        1.16 * e * e - 73.0 * e + 3141.5
    } else if e <= 20.0 {
        3110.0 - 53.5 * e
    } else {
        2680.0 - 31.0 * e
    };
    let length = (2.0 * (plies - 90.0)).max(-20.0);
    let elo = ((base + length + 5.0) / 10.0).floor() * 10.0;
    elo.max(500.0) as i32
}

/// The band of a mover's evaluation: winning, better, equal, worse, lost.
pub fn band(cp: i32) -> usize {
    match cp {
        200.. => 0,
        70..200 => 1,
        -69..70 => 2,
        -199..-69 => 3,
        _ => 4,
    }
}

/// The weight of an ordinary move: the reference app gives 1 + max(1, complexity /
/// 100) (`0x1404035c0`, "CPLX"); without its complexity this is the least.
pub const ORDINARY: f64 = 2.0;
/// The weight of a recapture ("RECAPT").
pub const RECAPTURE: f64 = 0.2;

/// The error value as the reference app forms it (`0x14046f140`): the sum of the
/// losses over the sum of the weights, moves of weight 0 left out.
pub fn error_value(records: &[MoveRecord]) -> Option<f64> {
    let counted = records.iter().filter(|r| r.weight > 0.0);
    let weights: f64 = counted.clone().map(|r| r.weight).sum();
    (weights > 0.0).then(|| counted.map(|r| f64::from(r.loss)).sum::<f64>() / weights)
}

fn severity(loss: i32) -> Option<usize> {
    match loss {
        25..50 => Some(0),
        50..100 => Some(1),
        100..300 => Some(2),
        300.. => Some(3),
        _ => None,
    }
}

/// The statistics of the counted moves of `games` games of `plies` plies.
pub fn summarize(records: &[MoveRecord], games: u32, plies: u64, ratings: &[u16]) -> ErrorReport {
    let counted: Vec<&MoveRecord> = records.iter().filter(|r| r.weight > 0.0).collect();
    let moves = counted.len() as u32;
    let mut report = ErrorReport {
        games,
        moves,
        distribution: vec![0; 16],
        ..Default::default()
    };
    if moves > 0 {
        report.average_loss =
            Some(counted.iter().map(|r| f64::from(r.loss)).sum::<f64>() / f64::from(moves));
    }
    report.error_value = error_value(records);
    if games > 0 {
        report.plies_per_game = Some(plies as f64 / f64::from(games));
    }
    if let (Some(e), Some(p), true) = (
        report.error_value,
        report.plies_per_game,
        moves >= MIN_MOVES,
    ) {
        report.blunder_elo = Some(blunder_elo(e, p));
    }
    let rated: Vec<u32> = ratings
        .iter()
        .filter(|r| **r > 0)
        .map(|r| u32::from(*r))
        .collect();
    if !rated.is_empty() {
        report.rating = Some(rated.iter().sum::<u32>() / rated.len() as u32);
    }
    report.difference = report
        .blunder_elo
        .zip(report.rating)
        .map(|(b, r)| b - r as i32);
    let key = |s: &str| s.to_string();
    for r in &counted {
        if let Some(s) = severity(r.loss) {
            report.severity[s] += 1;
        }
        if r.loss >= 25 {
            report.distribution[((r.loss.min(MAX_LOSS) - 25) / 25).min(15) as usize] += 1;
        }
        let phase = match r.phase {
            Phase::Opening => "opening",
            Phase::Middlegame => "middlegame",
            Phase::Endgame => "endgame",
        };
        let opponent = match r.opponent {
            Strength::Stronger => "stronger",
            Strength::Equal => "equal",
            Strength::Weaker => "weaker",
            Strength::Unrated => "unrated",
        };
        for (map, k) in [
            (&mut report.phases, key(phase)),
            (&mut report.opponents, key(opponent)),
        ] {
            let row = map.entry(k).or_default();
            let total = row.average_loss.unwrap_or(0.0) * f64::from(row.moves);
            row.moves += 1;
            row.average_loss = Some((total + f64::from(r.loss)) / f64::from(row.moves));
            if r.loss >= ERROR {
                row.errors += 1;
            }
        }
        if r.loss >= ERROR {
            report.transitions[band(r.before)][band(r.after)] += 1;
        }
    }
    let mut worst: Vec<&&MoveRecord> = counted.iter().filter(|r| r.loss > 0).collect();
    worst.sort_by(|a, b| b.loss.cmp(&a.loss).then(a.ordinal.cmp(&b.ordinal)));
    report.worst = worst
        .into_iter()
        .take(MAX_WORST)
        .map(|r| WorstMove {
            ordinal: r.ordinal.to_string(),
            node: r.node,
            number: format!("{}{}", r.ply / 2 + 1, if r.white { "." } else { "..." }),
            san: r.san.clone(),
            best: r.best.clone(),
            loss: r.loss,
        })
        .collect();
    report
}

/// Each side's pieces in points (queen 9, rook 5, bishop and knight 3).
fn pieces(position: &Chess, color: Color) -> u32 {
    let board = position.board();
    let count = |role: Role| (board.by_role(role) & board.by_color(color)).count() as u32;
    9 * count(Role::Queen) + 5 * count(Role::Rook) + 3 * (count(Role::Bishop) + count(Role::Knight))
}

/// The phase of the position before ply `ply`.
pub fn phase(position: &Chess, ply: usize) -> Phase {
    if pieces(position, Color::White) <= 13 && pieces(position, Color::Black) <= 13 {
        Phase::Endgame
    } else if ply < 30 {
        Phase::Opening
    } else {
        Phase::Middlegame
    }
}

pub fn strength(mine: Option<u16>, theirs: Option<u16>) -> Strength {
    match (mine.filter(|e| *e > 0), theirs.filter(|e| *e > 0)) {
        (Some(m), Some(t)) if i32::from(t) - i32::from(m) >= 100 => Strength::Stronger,
        (Some(m), Some(t)) if i32::from(m) - i32::from(t) >= 100 => Strength::Weaker,
        (Some(_), Some(_)) => Strength::Equal,
        _ => Strength::Unrated,
    }
}

fn san(position: &Chess, uci: &str) -> Option<String> {
    let m = UciMove::from_ascii(uci.as_bytes())
        .ok()?
        .to_move(position)
        .ok()?;
    Some(SanPlus::from_move(position.clone(), m).to_string())
}

/// One position's evaluation for the side to move, and the engine's line.
pub(crate) type LineEval = Option<(i32, Vec<String>)>;

/// Evaluates the positions `line[i]` (after `moves[..i]` from `start_fen`)
/// whose `needed[i]` is set, each to `depth` or for `cap` milliseconds at
/// most; `each` is called after every position. None when cancelled. A
/// set-up the engine refuses at its first position gives no evaluations.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_line(
    session: &mut EngineSession,
    start_fen: &str,
    moves: &[String],
    line: &[Chess],
    needed: &[bool],
    cap: u64,
    depth: Option<u32>,
    cancel: &AtomicBool,
    tag: &str,
    mut each: impl FnMut(),
) -> Result<Option<Vec<LineEval>>, AppError> {
    let mut evals = vec![None; line.len()];
    for i in 0..line.len() {
        if !needed[i] {
            continue;
        }
        let request = AnalysisRequest {
            request_id: format!("{tag}-{i}"),
            search_moves: Vec::new(),
            start_fen: start_fen.to_string(),
            history: moves[..i].to_vec(),
            root_turn: if line[i].turn().is_white() {
                "white"
            } else {
                "black"
            }
            .into(),
        };
        match evaluate_to(session, request, &line[i], cap, depth, cancel) {
            Ok(Some(e)) => evals[i] = Some((e.cp, e.pv)),
            Ok(None) => return Ok(None),
            Err(e) if e.code == "engine" && evals.iter().all(Option::is_none) => break,
            Err(e) => return Err(e),
        }
        each();
    }
    Ok(Some(evals))
}

/// Move `i`'s loss as the Error Report counts it.
pub(crate) struct MoveLoss {
    /// Centipawns, 0..=[`MAX_LOSS`]; 0 for the engine's best move.
    pub loss: i32,
    pub weight: f64,
    /// The mover's evaluation before and after the move, clamped to mate.
    pub before: i32,
    pub after: i32,
    /// The engine's best move before it (UCI).
    pub best: Option<String>,
}

/// The loss of `moves[i]` from the evaluations of the positions before and
/// after it; None when either is missing.
pub(crate) fn move_loss(
    line: &[Chess],
    moves: &[String],
    evals: &[LineEval],
    i: usize,
) -> Option<MoveLoss> {
    let (Some((before, best)), Some((next, _))) = (&evals[i], &evals[i + 1]) else {
        return None;
    };
    let (before, after) = (*before, -*next);
    let loss = if best.first().map(String::as_str) == Some(moves[i].as_str()) {
        0
    } else {
        (before.clamp(-MATE, MATE) - after.clamp(-MATE, MATE)).clamp(0, MAX_LOSS)
    };
    let only = line[i].legal_moves().len() == 1;
    let recapture = i > 0 && {
        let last = UciMove::from_ascii(moves[i - 1].as_bytes())
            .ok()
            .and_then(|u| u.to_move(&line[i - 1]).ok());
        let this = UciMove::from_ascii(moves[i].as_bytes())
            .ok()
            .and_then(|u| u.to_move(&line[i]).ok());
        matches!((last, this), (Some(l), Some(t))
            if l.is_capture() && t.is_capture() && l.to() == t.to())
    };
    Some(MoveLoss {
        loss,
        weight: if only {
            0.0
        } else if recapture {
            RECAPTURE
        } else {
            ORDINARY
        },
        before: before.clamp(-MATE, MATE),
        after: after.clamp(-MATE, MATE),
        best: best.first().cloned(),
    })
}

impl Registry {
    /// The players of the selection, most games first (at most 10).
    pub fn selection_players(
        &self,
        database_id: String,
        ranges: Vec<RowRange>,
    ) -> Result<Vec<SelectionPlayer>, AppError> {
        let (ordinals, _) = self.ordinals_of_rows(&database_id, &ranges, MAX_GAMES)?;
        let mut db = self.own_handle(&database_id)?;
        let mut count: HashMap<u32, (Option<String>, u32)> = HashMap::new();
        for &ordinal in &ordinals {
            for e in db.page(ordinal, 1)?.entries {
                for (id, name) in [(e.white_id, e.white), (e.black_id, e.black)] {
                    if let Some(id) = id {
                        let c = count.entry(id).or_insert((name, 0));
                        c.1 += 1;
                    }
                }
            }
        }
        let mut players: Vec<SelectionPlayer> = count
            .into_iter()
            .map(|(id, (name, games))| SelectionPlayer { id, name, games })
            .collect();
        players.sort_by(|a, b| b.games.cmp(&a.games).then(a.name.cmp(&b.name)));
        players.truncate(10);
        Ok(players)
    }

    /// Runs the Error Report on the selected games.
    pub fn error_report(
        &self,
        database_id: String,
        params: &ErrorReportParams,
        cancel: &AtomicBool,
        mut progress: impl FnMut(ErrorProgress),
    ) -> Result<ErrorReport, AppError> {
        let (ordinals, truncated) =
            self.ordinals_of_rows(&database_id, &params.ranges, MAX_GAMES)?;
        if ordinals.is_empty() {
            return Err(AppError::new(
                "noSelection",
                "Select the games to report on",
            ));
        }
        let mut db = self.own_handle(&database_id)?;
        let depth = if params.slower {
            SLOW_DEPTH
        } else {
            QUICK_DEPTH
        };
        let cap = if params.slower { SLOW_CAP } else { QUICK_CAP };
        let mut session = EngineSession::spawn(
            EngineSpec {
                executable: params.engine.clone(),
                arguments: vec![],
            },
            Duration::from_secs(5),
        )
        .map_err(|e| AppError::new("engine", e.to_string()))?;
        crate::tablebases::configure_engine(&mut session);
        let player_name = match params.player {
            Some(id) => db.player_name(id)?,
            None => None,
        };
        let result = (|| -> Result<ErrorReport, AppError> {
            for (name, value) in &params.options {
                crate::engine_commands::standard_option(name, value)?;
                if session.options().iter().any(|o| &o.name == name) {
                    session
                        .set_option(name, value)
                        .map_err(|e| AppError::new("engine", e.to_string()))?;
                }
            }
            if session.options().iter().any(|o| o.name == "MultiPV") {
                session
                    .set_option("MultiPV", "1")
                    .map_err(|e| AppError::new("engine", e.to_string()))?;
            }
            let total = ordinals.len() as u32;
            let mut records = vec![];
            let (mut games, mut skipped, mut plies, mut positions) = (0u32, 0u32, 0u64, 0u32);
            let mut ratings = vec![];
            for (done, &ordinal) in ordinals.iter().enumerate() {
                progress(ErrorProgress {
                    games_done: done as u32,
                    games: total,
                    positions,
                });
                let loaded = db.load(ordinal)?;
                let summary = &loaded.summary;
                let counts = |white: bool| match params.player {
                    Some(id) => {
                        (if white {
                            summary.white_id
                        } else {
                            summary.black_id
                        }) == Some(id)
                    }
                    None => true,
                };
                let Some(game) = loaded
                    .game
                    .as_ref()
                    .filter(|_| counts(true) || counts(false))
                else {
                    skipped += 1;
                    continue;
                };
                // The main line, up to the cut-off.
                let mut nodes = vec![0u32];
                while let Some(&next) = game.nodes[*nodes.last().unwrap() as usize].children.first()
                {
                    if nodes.len() > MAX_PLIES {
                        break;
                    }
                    nodes.push(next);
                }
                let Ok(start) = chess_core::model::parse_position(&game.start_fen) else {
                    skipped += 1;
                    continue;
                };
                let mut line = vec![start];
                let mut moves = vec![];
                for &n in &nodes[1..] {
                    let Some(uci) = game.nodes[n as usize].uci.clone() else {
                        break;
                    };
                    let Ok(next) = chess_core::model::apply_uci(line.last().unwrap().clone(), &uci)
                    else {
                        break;
                    };
                    moves.push(uci);
                    line.push(next);
                }
                if moves.is_empty() {
                    skipped += 1;
                    continue;
                }
                let needed: Vec<bool> = (0..line.len())
                    .map(|i| {
                        let before = i < moves.len() && counts(line[i].turn().is_white());
                        let after = i > 0 && counts(line[i - 1].turn().is_white());
                        before || after
                    })
                    .collect();
                let tag = format!("error-{ordinal}");
                let Some(evals) = evaluate_line(
                    &mut session,
                    &game.start_fen,
                    &moves,
                    &line,
                    &needed,
                    cap,
                    Some(depth),
                    cancel,
                    &tag,
                    || positions += 1,
                )?
                else {
                    let mut report = summarize(&records, games, plies, &ratings);
                    report.cancelled = true;
                    report.depth = depth;
                    report.truncated = truncated;
                    report.skipped = skipped;
                    report.player = player_name.clone();
                    return Ok(report);
                };
                if evals.iter().all(Option::is_none) {
                    skipped += 1;
                    continue;
                }
                games += 1;
                plies += moves.len() as u64;
                for white in [true, false] {
                    if counts(white) {
                        ratings.push(if white {
                            summary.white_elo.unwrap_or(0)
                        } else {
                            summary.black_elo.unwrap_or(0)
                        });
                    }
                }
                for i in 0..moves.len() {
                    let white = line[i].turn().is_white();
                    if !counts(white) {
                        continue;
                    }
                    let Some(m) = move_loss(&line, &moves, &evals, i) else {
                        continue;
                    };
                    let (mine, theirs) = if white {
                        (summary.white_elo, summary.black_elo)
                    } else {
                        (summary.black_elo, summary.white_elo)
                    };
                    records.push(MoveRecord {
                        ordinal,
                        node: nodes[i + 1],
                        ply: i as u16,
                        white,
                        san: san(&line[i], &moves[i]).unwrap_or_else(|| moves[i].clone()),
                        best: m.best.as_deref().and_then(|b| san(&line[i], b)),
                        loss: m.loss,
                        weight: m.weight,
                        before: m.before,
                        after: m.after,
                        phase: phase(&line[i], i),
                        opponent: strength(mine, theirs),
                    });
                }
            }
            progress(ErrorProgress {
                games_done: total,
                games: total,
                positions,
            });
            let mut report = summarize(&records, games, plies, &ratings);
            report.depth = depth;
            report.truncated = truncated;
            report.skipped = skipped;
            report.player = player_name.clone();
            Ok(report)
        })();
        let _ = session.shutdown();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(loss: i32, weight: f64) -> MoveRecord {
        MoveRecord {
            ordinal: 1,
            node: 1,
            ply: 10,
            white: true,
            san: "e4".into(),
            best: Some("d4".into()),
            loss,
            weight,
            before: 30,
            after: 30 - loss,
            phase: Phase::Middlegame,
            opponent: Strength::Equal,
        }
    }

    #[test]
    fn blunder_elo_follows_reference_curve() {
        // e = 10: 1.16 × 100 − 730 + 3141.5 = 2527.5; 80 plies: −20.
        assert_eq!(blunder_elo(10.0, 80.0), 2510);
        // e = 18: 3110 − 963 = 2147; 100 plies: +20.
        assert_eq!(blunder_elo(18.0, 100.0), 2170);
        // e = 40: 2680 − 1240 = 1440.
        assert_eq!(blunder_elo(40.0, 90.0), 1440);
        // Never below 500.
        assert_eq!(blunder_elo(200.0, 90.0), 500);
    }

    #[test]
    fn error_value_divides_the_losses_by_the_weights() {
        let r = [
            record(100, ORDINARY),
            record(0, ORDINARY),
            record(10, RECAPTURE),
            // The only legal move does not count.
            record(400, 0.0),
        ];
        // (100 + 0 + 10) / (2 + 2 + 0.2)
        assert!((error_value(&r).unwrap() - 110.0 / 4.2).abs() < 1e-9);
    }

    #[test]
    fn bands_and_severity() {
        assert_eq!(
            [band(250), band(100), band(0), band(-100), band(-300)],
            [0, 1, 2, 3, 4]
        );
        assert_eq!(band(-69), 2);
        assert_eq!(band(-70), 3);
        let mut records: Vec<MoveRecord> = (0..20).map(|_| record(0, 1.0)).collect();
        records.push(record(30, 1.0));
        records.push(record(120, 1.0));
        records.push(record(400, 1.0));
        let report = summarize(&records, 1, 80, &[2500, 0]);
        assert_eq!(report.moves, 23);
        assert_eq!(report.severity, [1, 0, 1, 1]);
        assert_eq!(report.distribution[0], 1);
        assert_eq!(report.distribution[3], 1);
        assert_eq!(report.distribution[15], 1);
        assert_eq!(report.rating, Some(2500));
        assert!(report.blunder_elo.is_some());
        // 120 cp from +0.30 to −0.90: equal to worse.
        assert_eq!(report.transitions[2][3], 1);
        assert_eq!(report.worst[0].loss, 400);
        assert_eq!(report.phases["middlegame"].errors, 2);
    }

    #[test]
    fn phase_and_strength() {
        let start = Chess::default();
        assert_eq!(phase(&start, 0), Phase::Opening);
        assert_eq!(phase(&start, 40), Phase::Middlegame);
        let ending: Chess = "4k3/8/8/8/8/8/4P3/R3K3 w - - 0 1"
            .parse::<shakmaty::fen::Fen>()
            .unwrap()
            .into_position(shakmaty::CastlingMode::Standard)
            .unwrap();
        assert_eq!(phase(&ending, 0), Phase::Endgame);
        assert_eq!(strength(Some(2400), Some(2550)), Strength::Stronger);
        assert_eq!(strength(Some(2400), Some(2300)), Strength::Weaker);
        assert_eq!(strength(Some(2400), Some(2450)), Strength::Equal);
        assert_eq!(strength(None, Some(2450)), Strength::Unrated);
    }
}

/// Running Error Reports, by database id, for cancelling.
#[derive(Default)]
pub struct ErrorReportJobs(std::sync::Mutex<HashMap<String, std::sync::Arc<AtomicBool>>>);

#[tauri::command]
pub async fn error_report(
    registry: tauri::State<'_, std::sync::Arc<Registry>>,
    jobs: tauri::State<'_, std::sync::Arc<ErrorReportJobs>>,
    database_id: String,
    params: ErrorReportParams,
    events: tauri::ipc::Channel<ErrorProgress>,
) -> Result<ErrorReport, AppError> {
    use std::sync::atomic::Ordering;
    let registry = registry.inner().clone();
    let jobs = jobs.inner().clone();
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    if let Ok(mut j) = jobs.0.lock()
        && let Some(old) = j.insert(database_id.clone(), cancel.clone())
    {
        old.store(true, Ordering::Relaxed);
    }
    let id = database_id.clone();
    let flag = cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut last = std::time::Instant::now();
        registry.error_report(id, &params, &flag, |p| {
            if last.elapsed().as_millis() >= 250 || p.games_done == p.games {
                last = std::time::Instant::now();
                let _ = events.send(p);
            }
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?;
    if let Ok(mut j) = jobs.0.lock()
        && j.get(&database_id)
            .is_some_and(|c| std::sync::Arc::ptr_eq(c, &cancel))
    {
        j.remove(&database_id);
    }
    result
}

#[tauri::command]
pub fn cancel_error_report(
    jobs: tauri::State<'_, std::sync::Arc<ErrorReportJobs>>,
    database_id: String,
) -> Result<(), AppError> {
    if let Ok(j) = jobs.0.lock()
        && let Some(c) = j.get(&database_id)
    {
        c.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn selection_players(
    registry: tauri::State<'_, std::sync::Arc<Registry>>,
    database_id: String,
    ranges: Vec<RowRange>,
) -> Result<Vec<SelectionPlayer>, AppError> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || registry.selection_players(database_id, ranges))
        .await
        .map_err(|e| AppError::new("state", e.to_string()))?
}
