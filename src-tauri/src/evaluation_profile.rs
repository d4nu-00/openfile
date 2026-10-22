//! Evaluation profiles (CBA type 0x26) and the reference app's Instant
//! Analysis, which fills one for the shown game. See
//! docs/evaluation-profile.md.
use crate::{
    blunder_check::{Eval, MATE, evaluate_to},
    commands::AppError,
    registry::Registry,
};
use engine::{AnalysisRequest, EngineSession, EngineSpec};
use serde::{Deserialize, Serialize};
use shakmaty::{Chess, EnPassantMode, Position, fen::Fen};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Plies a profile covers at most: entries 0..=2000 (`EvaluationArr`).
pub const MAX_PLIES: usize = 2000;
/// The reference app's step: `V_IDEPTH` × 35 ms per position.
pub const MILLIS_PER_LEVEL: u64 = 35;
/// Endless analysis stops once the time per position passes this.
pub const ENDLESS_LIMIT_MILLIS: u64 = 89_999;
/// Progress reaches the window about this often, as in the reference app.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// One evaluation of a profile, from White's side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePoint {
    /// Plies from the start; 0 is the start position.
    pub ply: u32,
    /// Centipawns, when the evaluation is a score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cp: Option<i32>,
    /// Moves to mate, positive when White mates; 0 for a mated position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mate: Option<i32>,
    /// Instant analysis's pass, or the depth another tool stored.
    pub depth: u8,
}

/// The evaluations of a packed profile; empty entries and kinds other than
/// score (0) and mate (1) are left out.
pub fn decode(profile: &[u32]) -> Vec<ProfilePoint> {
    profile
        .iter()
        .enumerate()
        .filter_map(|(ply, &v)| {
            let kind = (v >> 24) as u8 as i8;
            let value = i32::from(v as u16 as i16);
            let (cp, mate) = match kind {
                0 => (Some(value), None),
                1 => (None, Some(value)),
                _ => return None,
            };
            Some(ProfilePoint {
                ply: ply as u32,
                cp,
                mate,
                depth: (v >> 16) as u8,
            })
        })
        .collect()
}

/// An empty profile entry (kind −1).
pub const EMPTY: u32 = 0xff00_0000;

/// Packs an evaluation from the side to move (mates as [`MATE`] less the
/// distance) as the reference app stores it: from White's side.
pub fn pack(cp: i32, white_to_move: bool, depth: u8) -> u32 {
    let white = if white_to_move { cp } else { -cp };
    let (kind, value) = if white.abs() > MATE - 1_000 {
        let moves = MATE - white.abs();
        (1u32, if white > 0 { moves } else { -moves })
    } else {
        (0u32, white.clamp(i16::MIN as i32, i16::MAX as i32))
    };
    (kind << 24) | (u32::from(depth) << 16) | u32::from(value as i16 as u16)
}

/// Positions an evaluation cache keeps.
pub const CACHE_LIMIT: usize = 50_000;

/// Evaluations of this session by engine and position, with the time they
/// had, so a position is not sent to the engine again for the same time.
#[derive(Default)]
pub struct EvalCache {
    map: HashMap<(String, String), (i32, Vec<String>, u64)>,
    order: VecDeque<(String, String)>,
}
impl EvalCache {
    pub fn global() -> &'static Mutex<EvalCache> {
        static CACHE: OnceLock<Mutex<EvalCache>> = OnceLock::new();
        CACHE.get_or_init(Default::default)
    }
    fn key(engine: &str, position: &Chess) -> (String, String) {
        (
            engine.to_string(),
            Fen::from_position(position, EnPassantMode::Legal).to_string(),
        )
    }
    /// The evaluation for the side to move and the line, when one of at
    /// least `millis` is kept.
    pub fn get(&self, engine: &str, position: &Chess, millis: u64) -> Option<(i32, Vec<String>)> {
        self.map
            .get(&Self::key(engine, position))
            .filter(|(_, _, t)| *t >= millis)
            .map(|(cp, pv, _)| (*cp, pv.clone()))
    }
    pub fn put(&mut self, engine: &str, position: &Chess, cp: i32, pv: Vec<String>, millis: u64) {
        let key = Self::key(engine, position);
        if let Some(old) = self.map.get(&key)
            && old.2 > millis
        {
            return;
        }
        if self.map.insert(key.clone(), (cp, pv, millis)).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > CACHE_LIMIT {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// The engine's name, which keys the cache.
pub(crate) fn engine_key(session: &EngineSession) -> String {
    session
        .identity()
        .get("name")
        .cloned()
        .unwrap_or_else(|| "UCI engine".into())
}

/// [`evaluate_to`] for `millis`, through the cache.
pub(crate) fn cached_evaluate(
    session: &mut EngineSession,
    request: AnalysisRequest,
    position: &Chess,
    millis: u64,
    cancel: &AtomicBool,
) -> Result<Option<Eval>, AppError> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let engine = engine_key(session);
    if let Some((cp, pv)) = EvalCache::global()
        .lock()
        .ok()
        .and_then(|c| c.get(&engine, position, millis))
    {
        return Ok(Some(Eval {
            cp,
            depth: None,
            pv,
        }));
    }
    let eval = evaluate_to(session, request, position, millis, None, cancel)?;
    if let (Some(e), Ok(mut cache)) = (&eval, EvalCache::global().lock()) {
        cache.put(&engine, position, e.cp, e.pv.clone(), millis);
    }
    Ok(eval)
}

/// Instant Analysis's settings.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstantParams {
    pub engine: PathBuf,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// The reference app's `V_IDEPTH`, 1 to 6: × 35 ms per position.
    pub level: u32,
    /// `V_ENDLESS`: repeat with twice the time until it passes 90 s.
    #[serde(default)]
    pub endless: bool,
}

/// Instant Analysis's state, sent while it runs and at the end.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstantProgress {
    /// The document revision analysed.
    pub revision: String,
    pub pass: u32,
    pub millis_per_position: u64,
    /// Positions done in this pass, and in all.
    pub done: u32,
    pub total: u32,
    pub profile: Vec<ProfilePoint>,
    pub finished: bool,
    pub cancelled: bool,
}

impl Registry {
    /// Instant Analysis of document `id`'s main line: each position in turn,
    /// pass after pass when endless.
    pub fn instant_analysis(
        &self,
        id: &str,
        params: &InstantParams,
        cancel: &AtomicBool,
        mut progress: impl FnMut(&InstantProgress),
    ) -> Result<InstantProgress, AppError> {
        if !(1..=6).contains(&params.level) {
            return Err(AppError::new(
                "invalidParameters",
                "The instant analysis level must be from 1 to 6",
            ));
        }
        let (game, revision) = {
            let d = self.document(id)?;
            let d = d
                .lock()
                .map_err(|_| AppError::new("state", "Document unavailable"))?;
            (d.game.clone(), d.revision)
        };
        let mut line = vec![chess_core::model::parse_position(&game.start_fen)?];
        let mut moves = vec![];
        let mut at = 0usize;
        while let Some(&next) = game.nodes[at].children.first() {
            if moves.len() >= MAX_PLIES {
                break;
            }
            let Some(uci) = game.nodes[next as usize].uci.clone() else {
                break;
            };
            let Ok(position) = chess_core::model::apply_uci(line.last().unwrap().clone(), &uci)
            else {
                break;
            };
            moves.push(uci);
            line.push(position);
            at = next as usize;
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
        let err = |e: engine::EngineError| AppError::new("engine", e.to_string());
        let result = (|| -> Result<InstantProgress, AppError> {
            for (name, value) in &params.options {
                crate::engine_commands::standard_option(name, value)?;
                if session.options().iter().any(|o| &o.name == name) {
                    session.set_option(name, value).map_err(err)?;
                }
            }
            if session.options().iter().any(|o| o.name == "MultiPV") {
                session.set_option("MultiPV", "1").map_err(err)?;
            }
            let mut packed = vec![EMPTY; line.len()];
            let mut state = InstantProgress {
                revision: revision.to_string(),
                pass: 1,
                millis_per_position: u64::from(params.level) * MILLIS_PER_LEVEL,
                done: 0,
                total: line.len() as u32,
                profile: vec![],
                finished: false,
                cancelled: false,
            };
            let mut sent = Instant::now();
            loop {
                for (i, position) in line.iter().enumerate() {
                    let request = AnalysisRequest {
                        request_id: format!("instant-{}-{i}", state.pass),
                        search_moves: Vec::new(),
                        start_fen: game.start_fen.clone(),
                        history: moves[..i].to_vec(),
                        root_turn: if position.turn().is_white() {
                            "white"
                        } else {
                            "black"
                        }
                        .into(),
                    };
                    let eval = match cached_evaluate(
                        &mut session,
                        request,
                        position,
                        state.millis_per_position,
                        cancel,
                    ) {
                        Ok(Some(e)) => Some(e),
                        Ok(None) => {
                            state.cancelled = true;
                            state.profile = decode(&packed);
                            return Ok(state);
                        }
                        Err(e) => return Err(e),
                    };
                    if let Some(e) = eval {
                        let depth = state.pass.min(255) as u8;
                        packed[i] = pack(e.cp, position.turn().is_white(), depth);
                    }
                    state.done = i as u32 + 1;
                    if sent.elapsed() >= PROGRESS_EVERY {
                        sent = Instant::now();
                        state.profile = decode(&packed);
                        progress(&state);
                    }
                }
                state.profile = decode(&packed);
                let next = state.millis_per_position * 2;
                if !params.endless || next > ENDLESS_LIMIT_MILLIS || cancel.load(Ordering::Relaxed)
                {
                    state.finished = !cancel.load(Ordering::Relaxed);
                    state.cancelled = !state.finished;
                    return Ok(state);
                }
                progress(&state);
                state.pass += 1;
                state.millis_per_position = next;
                state.done = 0;
            }
        })();
        let _ = session.shutdown();
        result
    }
}

/// The running instant analysis: starting another cancels it.
#[derive(Default)]
pub struct InstantJobs(Mutex<Option<Arc<AtomicBool>>>);
impl InstantJobs {
    fn begin(&self) -> Result<Arc<AtomicBool>, AppError> {
        let mut current = self
            .0
            .lock()
            .map_err(|_| AppError::new("state", "Instant analysis unavailable"))?;
        if let Some(old) = current.take() {
            old.store(true, Ordering::Relaxed);
        }
        let flag = Arc::new(AtomicBool::new(false));
        *current = Some(flag.clone());
        Ok(flag)
    }
    pub fn cancel(&self) {
        if let Ok(mut current) = self.0.lock()
            && let Some(old) = current.take()
        {
            old.store(true, Ordering::Relaxed);
        }
    }
}

#[tauri::command]
pub async fn instant_analysis(
    registry: tauri::State<'_, Arc<Registry>>,
    jobs: tauri::State<'_, Arc<InstantJobs>>,
    document_id: String,
    params: InstantParams,
    events: tauri::ipc::Channel<InstantProgress>,
) -> Result<InstantProgress, AppError> {
    let registry = registry.inner().clone();
    let cancel = jobs.begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        registry.instant_analysis(&document_id, &params, &cancel, |p| {
            let _ = events.send(p.clone());
        })
    })
    .await
    .map_err(|e| AppError::new("internal", e.to_string()))?
}

#[tauri::command]
pub fn cancel_instant_analysis(jobs: tauri::State<'_, Arc<InstantJobs>>) {
    jobs.cancel();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packing_matches_the_stored_layout() {
        // +0.35 for White, Black to move: −35 for the side to move.
        assert_eq!(pack(-35, false, 1), 0x0001_0023);
        // White mates in 2, depth 20.
        assert_eq!(pack(MATE - 2, true, 20), 0x0114_0002);
        // Black mates in 3, White to move.
        let black_mates = pack(-(MATE - 3), true, 1);
        assert_eq!(decode(&[black_mates])[0].mate, Some(-3));
        let points = decode(&[0x0001_0023, EMPTY, 0x0114_0002, 0x2018_0022]);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].cp, Some(35));
        assert_eq!(points[1].ply, 2);
        assert_eq!(points[1].mate, Some(2));
    }
    #[test]
    fn the_cache_keeps_the_longest_evaluation_within_its_limit() {
        let mut c = EvalCache::default();
        let start = Chess::default();
        c.put("e", &start, 20, vec!["e2e4".into()], 100);
        assert_eq!(c.get("e", &start, 100).unwrap().0, 20);
        assert!(c.get("e", &start, 200).is_none());
        assert!(c.get("other", &start, 50).is_none());
        c.put("e", &start, 25, vec![], 50);
        assert_eq!(c.get("e", &start, 100).unwrap().0, 20);
        for i in 0..CACHE_LIMIT + 10 {
            c.put(&i.to_string(), &start, 0, vec![], 1);
        }
        assert_eq!(c.len(), CACHE_LIMIT);
    }
}
