//! Blunder Check: an engine of the user's evaluates every position of a
//! game's main line for a fixed time; a move that loses at least the
//! threshold against the engine's best line is a finding. The findings are
//! applied to the game as one edit (`Edit::AddAnalysis`).
use crate::{commands::AppError, registry::Registry};
use engine::{AnalysisRequest, EnginePayload, EngineSession, EngineSpec, Score};
use serde::{Deserialize, Serialize};
use shakmaty::{Chess, Position};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// A mate in `n` counts as this many centipawns, less `n`.
const MATE: i32 = 30_000;
/// Moves of the main line checked at most.
const MAX_PLIES: usize = 1_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlunderParams {
    pub engine: PathBuf,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// Engine time per position, 100 ms to 60 s.
    pub millis_per_move: u64,
    /// The smallest loss, in centipawns, that counts (10 to 1,000).
    pub threshold: i32,
    /// "white", "black" or "both".
    pub side: String,
}

/// A move that lost at least the threshold.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// The node of the move played.
    pub node: u32,
    /// The mover's loss in centipawns against the engine's best line.
    pub loss: i32,
    /// The scores before (best line) and after the move, from White's side,
    /// as text ("+0.35", "-M3").
    pub best: String,
    pub played: String,
    pub depth: Option<u32>,
    /// The engine's best line from the position before the move.
    pub line: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlunderReport {
    pub cancelled: bool,
    pub checked: u32,
    pub findings: Vec<Finding>,
    /// The game's revision the findings belong to.
    pub revision: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlunderProgress {
    pub done: u32,
    pub total: u32,
}

impl BlunderParams {
    fn check(&self) -> Result<(), AppError> {
        let bad = |m: &str| Err(AppError::new("invalidParameters", m));
        if !(100..=60_000).contains(&self.millis_per_move) {
            return bad("The time per move must be from 0.1 to 60 seconds");
        }
        if !(10..=1_000).contains(&self.threshold) {
            return bad("The threshold must be from 0.10 to 10.00 pawns");
        }
        if !["white", "black", "both"].contains(&self.side.as_str()) {
            return bad("Choose White, Black or both");
        }
        Ok(())
    }
}

/// A score from the side to move, as centipawns.
fn centipawns(score: &Score) -> i32 {
    match score {
        Score::Cp { value, .. } => *value,
        Score::Mate { moves, .. } if *moves > 0 => MATE - moves,
        Score::Mate { moves, .. } => -MATE - moves,
    }
}

/// `cp` from the side to move as text from White's side.
pub fn white_text(cp: i32, white_to_move: bool) -> String {
    let v = if white_to_move { cp } else { -cp };
    if v.abs() > MATE - 1_000 {
        let n = MATE - v.abs();
        format!("{}M{n}", if v > 0 { "+" } else { "-" })
    } else {
        format!(
            "{}{:.2}",
            if v >= 0 { "+" } else { "-" },
            v.abs() as f64 / 100.0
        )
    }
}

struct Eval {
    cp: i32,
    depth: Option<u32>,
    pv: Vec<String>,
}

/// The engine's view of one position after `millis`; `None` when cancelled.
fn evaluate(
    session: &mut EngineSession,
    request: AnalysisRequest,
    position: &Chess,
    millis: u64,
    cancel: &AtomicBool,
) -> Result<Option<Eval>, AppError> {
    if position.is_checkmate() {
        return Ok(Some(Eval {
            cp: -MATE,
            depth: None,
            pv: vec![],
        }));
    }
    if position.is_stalemate() || position.is_insufficient_material() {
        return Ok(Some(Eval {
            cp: 0,
            depth: None,
            pv: vec![],
        }));
    }
    let id = request.request_id.clone();
    let err = |e: engine::EngineError| AppError::new("engine", e.to_string());
    session.analyse(request).map_err(err)?;
    let deadline = Instant::now() + Duration::from_millis(millis);
    let mut best: Option<Eval> = None;
    let take = |info: engine::AnalysisInfo, best: &mut Option<Eval>| {
        if info.multipv != 1 {
            return;
        }
        if let (Some(score), Some(pv)) = (info.score, info.pv)
            && !pv.is_empty()
        {
            *best = Some(Eval {
                cp: centipawns(&score),
                depth: info.depth,
                pv,
            });
        }
    };
    while Instant::now() < deadline {
        if cancel.load(Ordering::Relaxed) {
            session.stop().map_err(err)?;
            return Ok(None);
        }
        let wait = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(50));
        match session.recv_event(wait).map_err(err)? {
            Some(e) if e.request_id == id => match e.payload {
                EnginePayload::Info(info) => take(info, &mut best),
                EnginePayload::Error { message } => {
                    return Err(AppError::new("engine", message));
                }
                _ => {}
            },
            _ => {}
        }
    }
    session.stop().map_err(err)?;
    // Lines that arrived with the stop still count.
    while let Some(e) = session.recv_event(Duration::ZERO).map_err(err)? {
        if e.request_id == id
            && let EnginePayload::Info(info) = e.payload
        {
            take(info, &mut best);
        }
    }
    best.map(Some)
        .ok_or_else(|| AppError::new("engine", "The engine gave no evaluation"))
}

impl Registry {
    /// Runs Blunder Check on document `id`'s main line.
    pub fn blunder_check(
        &self,
        id: &str,
        params: &BlunderParams,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u32, u32),
    ) -> Result<BlunderReport, AppError> {
        params.check()?;
        let (game, revision) = {
            let d = self.document(id)?;
            let d = d
                .lock()
                .map_err(|_| AppError::new("state", "Document unavailable"))?;
            (d.game.clone(), d.revision)
        };
        // The main line: node ids and the moves leading to each.
        let mut nodes = vec![0u32];
        while let Some(&next) = game.nodes[*nodes.last().unwrap() as usize].children.first() {
            if nodes.len() > MAX_PLIES {
                break;
            }
            nodes.push(next);
        }
        let mut positions = vec![chess_core::model::parse_position(&game.start_fen)?];
        let mut moves = vec![];
        for &n in &nodes[1..] {
            let uci = game.nodes[n as usize]
                .uci
                .clone()
                .ok_or_else(|| AppError::new("invalidNode", "A move is missing"))?;
            let next = chess_core::model::apply_uci(positions.last().unwrap().clone(), &uci)?;
            moves.push(uci);
            positions.push(next);
        }
        let checked_side = |white_moved: bool| match params.side.as_str() {
            "white" => white_moved,
            "black" => !white_moved,
            _ => true,
        };
        // Positions needed: before and after each checked move.
        let needed: Vec<bool> = (0..positions.len())
            .map(|i| {
                let before = i < moves.len() && checked_side(positions[i].turn().is_white());
                let after = i > 0 && checked_side(positions[i - 1].turn().is_white());
                before || after
            })
            .collect();
        let total = needed.iter().filter(|&&n| n).count() as u32;
        let mut session = EngineSession::spawn(
            EngineSpec {
                executable: params.engine.clone(),
                arguments: vec![],
            },
            Duration::from_secs(5),
        )
        .map_err(|e| AppError::new("engine", e.to_string()))?;
        crate::tablebases::configure_engine(&mut session);
        let result = (|| {
            for (name, value) in &params.options {
                crate::engine_commands::standard_option(name, value)?;
                if session.options().iter().any(|o| &o.name == name) {
                    session
                        .set_option(name, value)
                        .map_err(|e| AppError::new("engine", e.to_string()))?;
                }
            }
            let mut evals: Vec<Option<Eval>> = (0..positions.len()).map(|_| None).collect();
            let mut done = 0;
            progress(0, total);
            for i in 0..positions.len() {
                if !needed[i] {
                    continue;
                }
                let request = AnalysisRequest {
                    request_id: format!("blunder-{i}"),
                    start_fen: game.start_fen.clone(),
                    history: moves[..i].to_vec(),
                    root_turn: if positions[i].turn().is_white() {
                        "white"
                    } else {
                        "black"
                    }
                    .into(),
                };
                match evaluate(
                    &mut session,
                    request,
                    &positions[i],
                    params.millis_per_move,
                    cancel,
                )? {
                    Some(e) => evals[i] = Some(e),
                    None => {
                        return Ok(BlunderReport {
                            cancelled: true,
                            checked: done,
                            findings: vec![],
                            revision: revision.to_string(),
                        });
                    }
                }
                done += 1;
                progress(done, total);
            }
            let mut findings = vec![];
            for i in 0..moves.len() {
                let white = positions[i].turn().is_white();
                if !checked_side(white) {
                    continue;
                }
                let (Some(before), Some(after)) = (&evals[i], &evals[i + 1]) else {
                    continue;
                };
                // Both from the mover's side.
                let best = before.cp;
                let played = -after.cp;
                let loss = best - played;
                if loss < params.threshold || before.pv.first() == Some(&moves[i]) {
                    continue;
                }
                findings.push(Finding {
                    node: nodes[i + 1],
                    loss,
                    best: white_text(best, white),
                    played: white_text(played, white),
                    depth: before.depth,
                    line: before.pv.iter().take(16).cloned().collect(),
                });
            }
            Ok(BlunderReport {
                cancelled: false,
                checked: done,
                findings,
                revision: revision.to_string(),
            })
        })();
        let _ = session.shutdown();
        result
    }
}

/// Running Blunder Checks, by document id, for cancelling.
#[derive(Default)]
pub struct BlunderJobs(
    std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<AtomicBool>>>,
);

#[tauri::command]
pub async fn blunder_check(
    registry: tauri::State<'_, std::sync::Arc<Registry>>,
    jobs: tauri::State<'_, std::sync::Arc<BlunderJobs>>,
    document_id: String,
    params: BlunderParams,
    events: tauri::ipc::Channel<BlunderProgress>,
) -> Result<BlunderReport, AppError> {
    let registry = registry.inner().clone();
    let jobs = jobs.inner().clone();
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    if let Ok(mut j) = jobs.0.lock()
        && let Some(old) = j.insert(document_id.clone(), cancel.clone())
    {
        old.store(true, Ordering::Relaxed);
    }
    let id = document_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        registry.blunder_check(&id, &params, &cancel, |done, total| {
            let _ = events.send(BlunderProgress { done, total });
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?;
    if let Ok(mut j) = jobs.0.lock() {
        j.remove(&document_id);
    }
    result
}

#[tauri::command]
pub fn cancel_blunder_check(
    jobs: tauri::State<'_, std::sync::Arc<BlunderJobs>>,
    document_id: String,
) {
    if let Ok(j) = jobs.0.lock()
        && let Some(c) = j.get(&document_id)
    {
        c.store(true, Ordering::Relaxed);
    }
}
