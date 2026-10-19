//! Centipawn Analysis (`CENTIPAWN`, Ctrl+Alt+Shift+Z; see
//! docs/centipawn-analysis.md): the Error Report's engine pass on the shown
//! game's main line after its opening, each move classified by the
//! reference app's rule, with a summary at the start of the game. The
//! texts are applied as one edit (`Edit::AddAnalysis`).
use crate::{
    blunder_check::MATE,
    commands::AppError,
    error_report::{MAX_PLIES, error_value, evaluate_line, move_loss},
    registry::Registry,
};
use engine::{EngineSession, EngineSpec};
use serde::{Deserialize, Serialize};
use shakmaty::Position;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Seconds per move the dialog offers at most.
pub const MAX_SECONDS: u32 = 60;
/// Plies of opening the reference app always skips (`0x1404161f0`).
pub const MIN_BOOK_PLIES: u16 = 10;
/// From this evaluation a side counts as winning, and losing from its
/// negative. The reference app's thresholds were not traced.
pub const WINNING: i32 = 200;
/// A drop of more than this turns a win away or a game into a loss
/// (`0x140418d40`).
pub const TURN: i32 = 160;
/// A loss from this is a mistake (the reference app's blunder flag was not
/// traced).
pub const MISTAKE: i32 = 100;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CentipawnParams {
    pub engine: PathBuf,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// The reference app's "Seconds" (0 to [`MAX_SECONDS`]).
    pub seconds: u32,
    /// Comment every classified move, not only the errors.
    #[serde(default)]
    pub every_move: bool,
}

/// The reference app's time per position (`0x140a09480`): 750 ms per
/// second asked, at least 500 ms; 200 ms for 0.
pub fn millis_per_move(seconds: u32) -> u64 {
    if seconds == 0 {
        200
    } else {
        (u64::from(seconds) * 750).max(500)
    }
}

/// The reference app's move categories (`0x140418b10`), in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// The engine's move, found only by a deep search (1, 2): a UCI engine
    /// does not say, so these are never given.
    Brilliant,
    Strong,
    /// The engine's best move (3).
    Best,
    /// The engine's second choice, close to the best (4): not given, since
    /// the pass asks for one line.
    Good,
    /// Loses over 25 cp and over 0.3 of the evaluation (5).
    Inaccuracy,
    /// Loses [`MISTAKE`] or more (6).
    Mistake,
    /// Turns a winning position into one that is not (7).
    MissedWin,
    /// Turns a position that was not lost into a lost one (8).
    Losing,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Brilliant,
        Category::Strong,
        Category::Best,
        Category::Good,
        Category::Inaccuracy,
        Category::Mistake,
        Category::MissedWin,
        Category::Losing,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Category::Brilliant => "Brilliant move",
            Category::Strong => "Strong move",
            Category::Best => "Engine's move",
            Category::Good => "Good alternative",
            Category::Inaccuracy => "Inaccuracy",
            Category::Mistake => "Mistake",
            Category::MissedWin => "Lets the win slip",
            Category::Losing => "Losing move",
        }
    }
    fn is_error(self) -> bool {
        self >= Category::Inaccuracy
    }
}

/// The reference app's classifier (`0x140418d40`) on what a UCI engine
/// gives: whether the move was the engine's, and the mover's evaluation
/// before and after it.
pub fn classify(engine_move: bool, before: i32, after: i32) -> Option<Category> {
    if engine_move {
        return Some(Category::Best);
    }
    let drop = before - after;
    if before >= WINNING && after < WINNING && drop > TURN {
        return Some(Category::MissedWin);
    }
    if after <= -WINNING && before > -WINNING && drop > TURN {
        return Some(Category::Losing);
    }
    let loss = drop.max(0);
    if loss >= MISTAKE {
        return Some(Category::Mistake);
    }
    if loss > 25 && f64::from(loss) > 0.3 * f64::from(before.abs()) {
        return Some(Category::Inaccuracy);
    }
    None
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SideSummary {
    pub moves: u32,
    pub average_loss: Option<f64>,
    /// Mean of 100 × exp(−centipawn loss / 150) for analysed moves.
    pub accuracy: Option<f64>,
    pub error_value: Option<f64>,
    /// Per [`Category::ALL`].
    pub categories: [u32; 8],
}

/// A text for the game: before the first move (node 0) or after a move.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextItem {
    pub node: u32,
    pub comment: String,
}

/// A move in the review timeline. `best_line` is SAN from before the move.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedMove {
    pub node: u32,
    pub ply: u16,
    pub move_number: u32,
    pub san: String,
    pub white: bool,
    pub loss: i32,
    pub category: Option<Category>,
    pub best_line: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CentipawnReport {
    /// The document revision the texts were made for.
    pub revision: String,
    pub cancelled: bool,
    pub positions: u32,
    /// Plies of opening left out.
    pub book_plies: u16,
    pub millis_per_move: u64,
    pub white: SideSummary,
    pub black: SideSummary,
    pub items: Vec<TextItem>,
    pub moves: Vec<ReviewedMove>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CentipawnProgress {
    pub done: u32,
    pub total: u32,
}

/// "−1.2" for a loss in centipawns, "mate" when a mate is involved.
fn pawns(before: i32, after: i32) -> String {
    if before.abs() >= MATE - 1_000 || after.abs() >= MATE - 1_000 {
        "mate".into()
    } else {
        format!("−{:.1}", f64::from((before - after).max(0)) / 100.0)
    }
}

/// The summary text before the first move.
pub fn summary_text(white: &SideSummary, black: &SideSummary, book: u16) -> String {
    let num = |v: Option<f64>, digits: usize| {
        v.map_or_else(|| "–".to_string(), |v| format!("{v:.digits$}"))
    };
    let mut out = format!(
        "Centipawn Analysis after ply {book}: average loss White {}, Black {}; error value White {}, Black {}",
        num(white.average_loss, 0),
        num(black.average_loss, 0),
        num(white.error_value, 2),
        num(black.error_value, 2),
    );
    for (i, c) in Category::ALL.iter().enumerate().rev() {
        let (w, b) = (white.categories[i], black.categories[i]);
        if w + b > 0 {
            out.push_str(&format!("\n{}: White {w}, Black {b}", c.label()));
        }
    }
    out
}

impl Registry {
    /// Runs Centipawn Analysis on document `id`'s main line.
    pub fn centipawn_analysis(
        &self,
        id: &str,
        params: &CentipawnParams,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u32, u32),
    ) -> Result<CentipawnReport, AppError> {
        if params.seconds > MAX_SECONDS {
            return Err(AppError::new(
                "invalidParameters",
                format!("The time per move must be from 0 to {MAX_SECONDS} seconds"),
            ));
        }
        let (game, revision) = {
            let d = self.document(id)?;
            let d = d
                .lock()
                .map_err(|_| AppError::new("state", "Document unavailable"))?;
            (d.game.clone(), d.revision)
        };
        let millis = millis_per_move(params.seconds);
        // Book moves: the reference app skips the opening table's line, at
        // least ten plies; nothing when the table does not know the game.
        let book = chess_core::eco::classify_game(&game).map_or(0, |c| c.ply.max(MIN_BOOK_PLIES));
        let mut nodes = vec![0u32];
        while let Some(&next) = game.nodes[*nodes.last().unwrap() as usize].children.first() {
            if nodes.len() > MAX_PLIES {
                break;
            }
            nodes.push(next);
        }
        let mut line = vec![chess_core::model::parse_position(&game.start_fen)?];
        let mut moves = vec![];
        for &n in &nodes[1..] {
            let Some(uci) = game.nodes[n as usize].uci.clone() else {
                break;
            };
            let Ok(next) = chess_core::model::apply_uci(line.last().unwrap().clone(), &uci) else {
                break;
            };
            moves.push(uci);
            line.push(next);
        }
        // Moves counted: those after the book (ply number above `book`).
        let counted = |i: usize| i + 1 > usize::from(book);
        let needed: Vec<bool> = (0..line.len())
            .map(|i| (i < moves.len() && counted(i)) || (i > 0 && counted(i - 1)))
            .collect();
        let total = needed.iter().filter(|&&n| n).count() as u32;
        let mut report = CentipawnReport {
            revision: revision.to_string(),
            cancelled: false,
            positions: 0,
            book_plies: book,
            millis_per_move: millis,
            white: SideSummary::default(),
            black: SideSummary::default(),
            items: vec![],
            moves: vec![],
        };
        if total == 0 {
            return Err(AppError::new(
                "noMoves",
                "The game has no moves after its opening to analyse",
            ));
        }
        let mut session = EngineSession::spawn(
            EngineSpec {
                executable: params.engine.clone(),
                arguments: vec![],
            },
            Duration::from_secs(5),
        )
        .map_err(|e| AppError::new("engine", e.to_string()))?;
        crate::tablebases::configure_engine(&mut session);
        let result = (|| -> Result<CentipawnReport, AppError> {
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
            progress(0, total);
            let mut done = 0;
            let Some(evals) = evaluate_line(
                &mut session,
                &game.start_fen,
                &moves,
                &line,
                &needed,
                millis,
                None,
                cancel,
                "centipawn",
                || {
                    done += 1;
                    progress(done, total);
                },
            )?
            else {
                report.cancelled = true;
                report.positions = done;
                return Ok(report);
            };
            report.positions = done;
            if evals.iter().all(Option::is_none) {
                return Err(AppError::new(
                    "engine",
                    "The engine could not analyse this game",
                ));
            }
            let mut records: HashMap<bool, Vec<crate::error_report::MoveRecord>> = HashMap::new();
            for i in 0..moves.len() {
                if !counted(i) {
                    continue;
                }
                let Some(m) = move_loss(&line, &moves, &evals, i) else {
                    continue;
                };
                let white = line[i].turn().is_white();
                let side = if white {
                    &mut report.white
                } else {
                    &mut report.black
                };
                side.moves += 1;
                let engine_move = m.best.as_deref() == Some(moves[i].as_str());
                let category = classify(engine_move, m.before, m.after);
                let best_line = evals[i]
                    .as_ref()
                    .map(|(_, pv)| crate::deep_analysis::san_line(&line[i], pv))
                    .unwrap_or_default();
                report.moves.push(ReviewedMove {
                    node: nodes[i + 1],
                    ply: (i + 1) as u16,
                    move_number: line[i].fullmoves().get(),
                    san: game
                        .san_at(nodes[i + 1])
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| moves[i].clone()),
                    white,
                    loss: m.loss,
                    category,
                    best_line,
                });
                records
                    .entry(white)
                    .or_default()
                    .push(record(&m, i, nodes[i + 1], white));
                let Some(category) = category else {
                    continue;
                };
                side.categories[category as usize] += 1;
                if params.every_move || category.is_error() {
                    let mut text = category.label().to_string();
                    if category >= Category::Mistake {
                        text.push_str(&format!(" ({})", pawns(m.before, m.after)));
                    }
                    report.items.push(TextItem {
                        node: nodes[i + 1],
                        comment: text,
                    });
                }
            }
            for (white, side) in [(true, &mut report.white), (false, &mut report.black)] {
                let r = records.remove(&white).unwrap_or_default();
                if !r.is_empty() {
                    side.average_loss =
                        Some(r.iter().map(|m| f64::from(m.loss)).sum::<f64>() / r.len() as f64);
                    side.accuracy = Some(
                        r.iter()
                            .map(|m| 100.0 * (-f64::from(m.loss) / 150.0).exp())
                            .sum::<f64>()
                            / r.len() as f64,
                    );
                    // The reference app gives the weighted value once the
                    // weights pass 4.
                    if r.iter().map(|m| m.weight).sum::<f64>() > 4.0 {
                        side.error_value = error_value(&r);
                    }
                }
            }
            report.items.insert(
                0,
                TextItem {
                    node: 0,
                    comment: summary_text(&report.white, &report.black, book),
                },
            );
            Ok(report)
        })();
        let _ = session.shutdown();
        result
    }
}

/// A move as the Error Report's statistics take it.
fn record(
    m: &crate::error_report::MoveLoss,
    ply: usize,
    node: u32,
    white: bool,
) -> crate::error_report::MoveRecord {
    crate::error_report::MoveRecord {
        ordinal: 0,
        node,
        ply: ply as u16,
        white,
        san: String::new(),
        best: None,
        loss: m.loss,
        weight: m.weight,
        before: m.before,
        after: m.after,
        phase: crate::error_report::Phase::Middlegame,
        opponent: crate::error_report::Strength::Unrated,
    }
}

/// Running analyses, by document id, for cancelling.
#[derive(Default)]
pub struct CentipawnJobs(std::sync::Mutex<HashMap<String, Arc<AtomicBool>>>);

#[tauri::command]
pub async fn centipawn_analysis(
    registry: tauri::State<'_, Arc<Registry>>,
    jobs: tauri::State<'_, Arc<CentipawnJobs>>,
    document_id: String,
    params: CentipawnParams,
    events: tauri::ipc::Channel<CentipawnProgress>,
) -> Result<CentipawnReport, AppError> {
    let registry = registry.inner().clone();
    let jobs = jobs.inner().clone();
    let cancel = Arc::new(AtomicBool::new(false));
    if let Ok(mut j) = jobs.0.lock()
        && let Some(old) = j.insert(document_id.clone(), cancel.clone())
    {
        old.store(true, Ordering::Relaxed);
    }
    let (id, flag) = (document_id.clone(), cancel.clone());
    let result = tauri::async_runtime::spawn_blocking(move || {
        registry.centipawn_analysis(&id, &params, &flag, |done, total| {
            let _ = events.send(CentipawnProgress { done, total });
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()));
    if let Ok(mut j) = jobs.0.lock()
        && j.get(&document_id).is_some_and(|c| Arc::ptr_eq(c, &cancel))
    {
        j.remove(&document_id);
    }
    result?
}

#[tauri::command]
pub fn cancel_centipawn_analysis(jobs: tauri::State<'_, Arc<CentipawnJobs>>, document_id: String) {
    if let Ok(j) = jobs.0.lock()
        && let Some(c) = j.get(&document_id)
    {
        c.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_per_move_follows_the_reference() {
        assert_eq!(millis_per_move(0), 200);
        assert_eq!(millis_per_move(1), 750);
        assert_eq!(millis_per_move(4), 3_000);
    }

    #[test]
    fn classifies_like_the_reference_rule() {
        assert_eq!(classify(true, -300, -300), Some(Category::Best));
        // From +2.50 to +0.50: the win slips away.
        assert_eq!(classify(false, 250, 50), Some(Category::MissedWin));
        // From −0.30 to −2.30: now lost.
        assert_eq!(classify(false, -30, -230), Some(Category::Losing));
        assert_eq!(classify(false, 0, -120), Some(Category::Mistake));
        // 40 cp is an inaccuracy at +0.20, not at +3.00.
        assert_eq!(classify(false, 20, -20), Some(Category::Inaccuracy));
        assert_eq!(classify(false, 300, 260), None);
        assert_eq!(classify(false, 10, 0), None);
    }

    #[test]
    fn the_summary_lists_categories_worst_first() {
        let mut w = SideSummary {
            moves: 20,
            average_loss: Some(18.4),
            error_value: Some(4.1),
            ..Default::default()
        };
        w.categories[Category::Best as usize] = 12;
        w.categories[Category::Mistake as usize] = 1;
        let b = SideSummary::default();
        let text = summary_text(&w, &b, 12);
        assert!(text.starts_with(
            "Centipawn Analysis after ply 12: average loss White 18, Black –; error value White 4.10, Black –"
        ));
        let mistake = text.find("Mistake: White 1, Black 0").unwrap();
        let best = text.find("Engine's move: White 12, Black 0").unwrap();
        assert!(mistake < best);
        assert_eq!(pawns(30, -90), "−1.2");
        assert_eq!(pawns(MATE, 0), "mate");
    }
}
