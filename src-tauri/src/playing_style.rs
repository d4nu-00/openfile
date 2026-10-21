//! Independent descriptions of recorded board behaviour, not hidden reference scores.
use crate::{commands::AppError, registry::Registry};
pub use cbh_reader::style_selection::StyleCriteria as StyleParams;
use cbh_reader::{DataStatus, source::SourceFile};
use chess_core::Game;
use serde::Serialize;
use shakmaty::{Chess, Color, Move, Position, Role, san::SanPlus, uci::UciMove};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub const MAX_PLIES: usize = 400;
const MAX_EVIDENCE: usize = 8;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleProgress {
    pub stage: String,
    pub done: String,
    pub total: String,
}
#[derive(Default)]
struct ProgressThrottle {
    stage: Option<String>,
    at: u128,
}
impl ProgressThrottle {
    fn allow(&mut self, p: &StyleProgress, now: u128) -> bool {
        if self.stage.as_deref() != Some(p.stage.as_str())
            || p.done == p.total
            || now.saturating_sub(self.at) >= 100
        {
            self.stage = Some(p.stage.clone());
            self.at = now;
            true
        } else {
            false
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StylePlayer {
    pub id: u32,
    pub name: Option<String>,
}
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleRate {
    pub numerator: u64,
    pub denominator: u64,
    pub value: Option<f64>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleEvidence {
    pub ordinal: String,
    pub node: u32,
    pub ply: u32,
    pub san: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleMetric {
    pub id: String,
    pub label: String,
    pub description: String,
    pub numerator: u64,
    pub denominator: u64,
    pub value: Option<f64>,
    pub minimum: u64,
    pub status: String,
    pub white: StyleRate,
    pub black: StyleRate,
    pub evidence: Vec<StyleEvidence>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleGame {
    pub ordinal: String,
    pub white: bool,
    pub opponent: Option<String>,
    pub date: Option<String>,
    pub result: Option<String>,
    pub plies: u32,
    pub cut: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleReport {
    pub job_id: String,
    pub players: Vec<StylePlayer>,
    pub criteria: StyleParams,
    pub database: String,
    pub database_entries: String,
    pub eligible: u64,
    pub selected: u32,
    pub analysed: u32,
    pub white_games: u32,
    pub black_games: u32,
    pub deleted: u64,
    pub filtered: u64,
    pub unknown_date: u64,
    pub unknown_opponent_elo: u64,
    pub unknown_result: u64,
    pub skipped: u32,
    pub skipped_setup: u32,
    pub skipped_unreadable: u32,
    pub skipped_unsupported: u32,
    pub skipped_partial: u32,
    pub cut_games: u32,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
    pub metrics: Vec<StyleMetric>,
    pub games: Vec<StyleGame>,
    pub elapsed_ms: u64,
}
/// Only the watermark and source signatures are retained, never the report.
pub(crate) struct StyleJob {
    id: String,
    cancel: Arc<AtomicBool>,
    published: Option<Arc<Vec<SourceFile>>>,
}
impl StyleJob {
    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn percent(n: u64, d: u64) -> Option<f64> {
    (d > 0).then(|| 100.0 * n as f64 / d as f64)
}
impl StyleRate {
    fn add(&mut self, n: u64, d: u64) {
        self.numerator += n;
        self.denominator += d;
        self.value = percent(self.numerator, self.denominator);
    }
}
impl StyleMetric {
    fn add(&mut self, white: bool, o: &Observation, ordinal: u64) {
        self.numerator += o.n;
        self.denominator += o.d;
        self.value = percent(self.numerator, self.denominator);
        if white {
            &mut self.white
        } else {
            &mut self.black
        }
        .add(o.n, o.d);
        self.status = if self.denominator == 0 {
            "none"
        } else if self.denominator < self.minimum {
            "small"
        } else {
            "ok"
        }
        .into();
        if self.evidence.len() < MAX_EVIDENCE
            && let Some((node, ply, san)) = &o.evidence
        {
            self.evidence.push(StyleEvidence {
                ordinal: ordinal.to_string(),
                node: *node,
                ply: *ply,
                san: san.clone(),
            });
        }
    }
}
fn metrics() -> Vec<StyleMetric> {
    [
        ("checks","Checking moves","Checking moves / all observed player moves (including mate).",100),
        ("captures","Capturing moves","Capturing moves / all observed player moves, including en passant. Captures do not establish exchanges or aggression.",100),
        ("earlyQueen","Early queen move","Games with a queen move in the first 10 own moves / games reaching 10 own moves or already showing the event. Short unobserved windows are excluded.",20),
        ("kingsideCastle","Kingside castling by move 20","Games castled kingside within 20 own moves / games reaching 20 own moves or already castled within that window.",20),
        ("queensideCastle","Queenside castling by move 20","Games castled queenside within 20 own moves / games reaching 20 own moves or already castled within that window.",20),
        ("oppositeCastle","Opposite-side castling","Games castled on opposite sides / games where both players' castling is observed.",20),
        ("queenless30","Queenless by move 30","Games with neither queen present by ply 60 / games reaching ply 60 or showing the event earlier. Promoted queens count.",20),
        ("imbalance20","Material imbalance at move 20","Games with different White/Black non-pawn piece inventories at ply 40 / games reaching ply 40. Counts piece types, including promoted pieces.",20),
        ("sustainedDeficit","Sustained material deficit","Games with a material deficit of at least 3 points after an own move and still at least 3 before the next own move / complete observed games or games showing the event before the replay cap. P=1,N=B=3,R=5,Q=9. This does not establish an intentional or sound sacrifice.",20),
        ("endgame","Endgame reached","Games where both sides have at most 13 non-pawn material points / complete observed games or games showing the event before the replay cap. N=B=3,R=5,Q=9; kings excluded.",20),
    ].into_iter().map(|(id,label,description,minimum)|StyleMetric{id:id.into(),label:label.into(),description:description.into(),numerator:0,denominator:0,value:None,minimum,status:"none".into(),white:StyleRate::default(),black:StyleRate::default(),evidence:vec![]}).collect()
}
#[derive(Default, Clone)]
struct Observation {
    n: u64,
    d: u64,
    evidence: Option<(u32, u32, String)>,
}
impl Observation {
    fn event(&mut self, node: u32, ply: u32, san: &str) {
        self.n += 1;
        if self.evidence.is_none() {
            self.evidence = Some((node, ply, san.into()));
        }
    }
}
struct Replay {
    obs: Vec<Observation>,
    plies: u32,
    cut: bool,
}
fn inventory(p: &Chess, c: Color) -> [u32; 4] {
    [Role::Knight, Role::Bishop, Role::Rook, Role::Queen]
        .map(|r| (p.board().by_color(c) & p.board().by_role(r)).count() as u32)
}
fn points(p: &Chess, c: Color, pawns: bool) -> i32 {
    let a = inventory(p, c);
    (a[0] * 3 + a[1] * 3 + a[2] * 5 + a[3] * 9) as i32
        + if pawns {
            (p.board().by_color(c) & p.board().by_role(Role::Pawn)).count() as i32
        } else {
            0
        }
}
/// Replay only legal main-line plies. Original IDs navigate in the game viewer.
fn replay(game: &Game, white: bool, cancel: &AtomicBool) -> Result<Option<Replay>, ()> {
    let mut pos = Chess::default();
    let mut node = 0u32;
    let mut own = 0u32;
    let mut obs = vec![Observation::default(); 10];
    let mut castles: [Option<bool>; 2] = [None, None];
    let mut early_castle = None;
    let mut prev_deficit = false;
    let mut plies = 0;
    for ply in 1..=MAX_PLIES {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let current = game.nodes.get(node as usize).ok_or(())?;
        let Some(&next) = current.children.first() else {
            break;
        };
        let child = game.nodes.get(next as usize).ok_or(())?;
        if child.id != next || child.parent != Some(node) || next == 0 {
            return Err(());
        }
        let uci = child.uci.as_deref().ok_or(())?;
        let m = UciMove::from_ascii(uci.as_bytes())
            .map_err(|_| ())?
            .to_move(&pos)
            .map_err(|_| ())?;
        let mover = pos.turn();
        let is_own = mover.is_white() == white;
        let san = SanPlus::from_move(pos.clone(), m).to_string();
        if let Move::Castle { king, rook } = m {
            let kingside = rook.file() > king.file();
            castles[usize::from(!mover.is_white())] = Some(kingside);
            if is_own && own < 20 {
                early_castle = Some(kingside);
                obs[if kingside { 3 } else { 4 }].event(next, ply as u32, &san);
            }
            if let [Some(w), Some(b)] = castles
                && w != b
                && obs[5].n == 0
            {
                obs[5].event(next, ply as u32, &san);
            }
        }
        pos.play_unchecked(m);
        if !is_own
            && prev_deficit
            && obs[8].n == 0
            && points(&pos, !mover, true) - points(&pos, mover, true) <= -3
        {
            obs[8].event(next, ply as u32, &san);
        }
        if is_own {
            own += 1;
            obs[0].d += 1;
            obs[1].d += 1;
            if pos.is_check() {
                obs[0].event(next, ply as u32, &san);
            }
            if m.is_capture() {
                obs[1].event(next, ply as u32, &san);
            }
            if own <= 10 && m.role() == Role::Queen && obs[2].n == 0 {
                obs[2].event(next, ply as u32, &san);
            }
            prev_deficit = points(&pos, mover, true) - points(&pos, !mover, true) <= -3;
        }
        if ply <= 60 && pos.board().by_role(Role::Queen).is_empty() && obs[6].n == 0 {
            obs[6].event(next, ply as u32, &san);
        }
        if ply == 40 {
            obs[7].d = 1;
            if inventory(&pos, Color::White) != inventory(&pos, Color::Black) {
                obs[7].event(next, ply as u32, &san);
            }
        }
        if points(&pos, Color::White, false) <= 13
            && points(&pos, Color::Black, false) <= 13
            && obs[9].n == 0
        {
            obs[9].event(next, ply as u32, &san);
        }
        plies = ply as u32;
        node = next;
    }
    if plies == 0 {
        return Err(());
    }
    let cut = !game.nodes.get(node as usize).ok_or(())?.children.is_empty();
    obs[2].d = u64::from(own >= 10 || obs[2].n > 0);
    obs[3].d = u64::from(own >= 20 || early_castle.is_some());
    obs[4].d = obs[3].d;
    obs[5].d = u64::from(castles.iter().all(Option::is_some));
    obs[6].d = u64::from(plies >= 60 || obs[6].n > 0);
    for i in [8, 9] {
        obs[i].d = u64::from(!cut || obs[i].n > 0);
    }
    Ok(Some(Replay { obs, plies, cut }))
}
impl Registry {
    fn begin_style_job(
        &self,
        database_id: &str,
        job_id: &str,
        params: &StyleParams,
    ) -> Result<Arc<AtomicBool>, AppError> {
        params.validate()?;
        if job_id.is_empty() || job_id.len() > 128 {
            return Err(AppError::new(
                "invalidParameters",
                "Expected a unique job ID (1–128 characters)",
            ));
        }
        let mut s = self.lock()?;
        if !s.databases.contains_key(database_id) {
            return Err(AppError::new(
                "invalidHandle",
                "Database is closed or unknown",
            ));
        }
        if s.style_jobs
            .get(database_id)
            .is_some_and(|j| j.id == job_id)
        {
            return Err(AppError::new(
                "invalidParameters",
                "Use a new job ID for each report",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = s.style_jobs.insert(
            database_id.into(),
            StyleJob {
                id: job_id.into(),
                cancel: cancel.clone(),
                published: None,
            },
        ) {
            old.cancel.store(true, Ordering::Relaxed)
        }
        Ok(cancel)
    }
    /// None means cancelled or replaced. Registration precedes worker dispatch.
    pub fn playing_style(
        &self,
        database_id: String,
        job_id: String,
        params: &StyleParams,
        progress: impl FnMut(StyleProgress),
    ) -> Result<Option<StyleReport>, AppError> {
        let cancel = self.begin_style_job(&database_id, &job_id, params)?;
        self.run_style_job(database_id, job_id, params, cancel, progress)
    }
    fn run_style_job(
        &self,
        database_id: String,
        job_id: String,
        params: &StyleParams,
        cancel: Arc<AtomicBool>,
        mut progress: impl FnMut(StyleProgress),
    ) -> Result<Option<StyleReport>, AppError> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let started = Instant::now();
        let mut throttle = ProgressThrottle::default();
        let mut progress = |p: StyleProgress| {
            if throttle.allow(&p, started.elapsed().as_millis()) {
                progress(p)
            }
        };
        let mut db = self.own_handle(&database_id)?;
        let sources = Arc::new(
            db.source_paths()
                .iter()
                .map(|p| SourceFile::open(p))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let Some(found) = db.select_style_games(params, &cancel, |done, total| {
            progress(StyleProgress {
                stage: "scan".into(),
                done: done.to_string(),
                total: total.to_string(),
            })
        })?
        else {
            return Ok(None);
        };
        let mut criteria = params.clone();
        let mut seen = std::collections::HashSet::new();
        criteria.player_ids.retain(|id| seen.insert(*id));
        let players = criteria
            .player_ids
            .iter()
            .map(|&id| {
                Ok(StylePlayer {
                    id,
                    name: db.player_name(id)?,
                })
            })
            .collect::<Result<_, AppError>>()?;
        let database = self
            .lock()?
            .paths
            .get(&database_id)
            .and_then(|p| p.file_stem())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut report = StyleReport {
            job_id: job_id.clone(),
            players,
            criteria,
            database,
            database_entries: db.info().entry_count.to_string(),
            eligible: found.eligible,
            selected: found.games.len() as u32,
            analysed: 0,
            white_games: 0,
            black_games: 0,
            deleted: found.deleted,
            filtered: found.filtered,
            unknown_date: found.unknown_date,
            unknown_opponent_elo: found.unknown_opponent_elo,
            unknown_result: found.unknown_result,
            skipped: 0,
            skipped_setup: 0,
            skipped_unreadable: 0,
            skipped_unsupported: 0,
            skipped_partial: 0,
            cut_games: 0,
            first_date: None,
            last_date: None,
            metrics: metrics(),
            games: vec![],
            elapsed_ms: 0,
        };
        let total = found.games.len();
        for (done, g) in found.games.iter().enumerate() {
            progress(StyleProgress {
                stage: "replay".into(),
                done: done.to_string(),
                total: total.to_string(),
            });
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let loaded = match db.load(g.ordinal) {
                Ok(l) => l,
                Err(cbh_reader::CbhError::SourceChanged) => {
                    return Err(cbh_reader::CbhError::SourceChanged.into());
                }
                Err(_) => {
                    report.skipped_unreadable += 1;
                    continue;
                }
            };
            if loaded.moves_status != DataStatus::Available {
                match loaded.moves_status {
                    DataStatus::Unsupported => report.skipped_unsupported += 1,
                    DataStatus::Partial => report.skipped_partial += 1,
                    _ => report.skipped_unreadable += 1,
                };
                continue;
            }
            // The decoder currently leaves this diagnostic's status Available.
            if loaded
                .diagnostics
                .iter()
                .any(|d| d.code == "uninterpretedMoveFlag")
            {
                report.skipped_partial += 1;
                continue;
            }
            let Some(game) = loaded.game else {
                report.skipped_unreadable += 1;
                continue;
            };
            if game.is_chess960()
                || game.start_fen != Game::initial().start_fen
                || game.tags.get("SetUp").is_some_and(|s| s == "1")
            {
                report.skipped_setup += 1;
                continue;
            }
            let line = match replay(&game, g.white, &cancel) {
                Ok(Some(l)) => l,
                Ok(None) => return Ok(None),
                Err(()) => {
                    report.skipped_unreadable += 1;
                    continue;
                }
            };
            for (m, o) in report.metrics.iter_mut().zip(&line.obs) {
                m.add(g.white, o, g.ordinal);
            }
            report.analysed += 1;
            if g.white {
                report.white_games += 1
            } else {
                report.black_games += 1
            }
            report.cut_games += u32::from(line.cut);
            let date = loaded.summary.date.filter(|_| g.date > 0);
            // Selection is newest first, including partial dates whose unknown
            // month/day sort as zero. Display '?' strings cannot be compared.
            if date.is_some() {
                report.first_date = date.clone();
                if report.last_date.is_none() {
                    report.last_date = date.clone();
                }
            }
            report.games.push(StyleGame {
                ordinal: g.ordinal.to_string(),
                white: g.white,
                opponent: if g.white {
                    loaded.summary.black
                } else {
                    loaded.summary.white
                },
                date,
                result: loaded.summary.result,
                plies: line.plies,
                cut: line.cut,
            });
        }
        report.skipped = report.selected - report.analysed;
        report.elapsed_ms = started.elapsed().as_millis() as u64;
        progress(StyleProgress {
            stage: "replay".into(),
            done: total.to_string(),
            total: total.to_string(),
        });
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        db.verify_sources()?;
        for source in sources.iter() {
            source.verify_unchanged()?;
        }
        let mut s = self.lock()?;
        if cancel.load(Ordering::Relaxed) || !s.databases.contains_key(&database_id) {
            return Ok(None);
        }
        let Some(job) = s
            .style_jobs
            .get_mut(&database_id)
            .filter(|j| j.id == job_id && Arc::ptr_eq(&j.cancel, &cancel))
        else {
            return Ok(None);
        };
        job.published = Some(sources);
        Ok(Some(report))
    }
    /// Keep the report snapshot valid across the ordinary reader's refresh
    /// retry. A changed source/job must never publish a different game's nodes.
    pub fn load_style_entry(
        &self,
        database_id: String,
        job_id: String,
        ordinal: String,
        request_id: String,
    ) -> Result<crate::commands::GameResponse, AppError> {
        self.validate_playing_style(database_id.clone(), job_id.clone())?;
        let loaded = self.load_entry(database_id.clone(), ordinal, request_id)?;
        self.finish_style_entry(&database_id, &job_id, loaded)
    }

    fn finish_style_entry(
        &self,
        database_id: &str,
        job_id: &str,
        loaded: crate::commands::GameResponse,
    ) -> Result<crate::commands::GameResponse, AppError> {
        if let Err(error) = self.validate_playing_style(database_id.into(), job_id.into()) {
            if let Some(document) = &loaded.document_id {
                let _ = self.close_document(document.clone());
            }
            return Err(error);
        }
        Ok(loaded)
    }

    pub fn cancel_playing_style(
        &self,
        database_id: String,
        job_id: String,
    ) -> Result<(), AppError> {
        if let Some(j) = self
            .lock()?
            .style_jobs
            .get_mut(&database_id)
            .filter(|j| j.id == job_id)
        {
            j.cancel.store(true, Ordering::Relaxed);
            j.published = None;
        }
        Ok(())
    }
    pub fn validate_playing_style(
        &self,
        database_id: String,
        job_id: String,
    ) -> Result<(), AppError> {
        let sources = {
            let s = self.lock()?;
            s.style_jobs
                .get(&database_id)
                .filter(|j| j.id == job_id && !j.cancel.load(Ordering::Relaxed))
                .and_then(|j| j.published.clone())
        }
        .ok_or_else(|| {
            AppError::new(
                "staleReport",
                "Generate a current playing-style report before opening its evidence",
            )
        })?;
        for source in sources.iter() {
            source.verify_unchanged()?;
        }
        let s = self.lock()?;
        if !s.databases.contains_key(&database_id)
            || !s.style_jobs.get(&database_id).is_some_and(|j| {
                j.id == job_id
                    && !j.cancel.load(Ordering::Relaxed)
                    && j.published
                        .as_ref()
                        .is_some_and(|p| Arc::ptr_eq(p, &sources))
            })
        {
            return Err(AppError::new(
                "staleReport",
                "The playing-style report is no longer current",
            ));
        }
        Ok(())
    }
}
#[tauri::command]
pub async fn playing_style(
    registry: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    job_id: String,
    params: StyleParams,
    events: tauri::ipc::Channel<StyleProgress>,
) -> Result<Option<StyleReport>, AppError> {
    let cancel = registry.begin_style_job(&database_id, &job_id, &params)?;
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        registry.run_style_job(database_id, job_id, &params, cancel, |p| {
            let _ = events.send(p);
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}
#[tauri::command]
pub fn cancel_playing_style(
    registry: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    job_id: String,
) -> Result<(), AppError> {
    registry.cancel_playing_style(database_id, job_id)
}
#[tauri::command]
pub fn validate_playing_style(
    registry: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    job_id: String,
) -> Result<(), AppError> {
    registry.validate_playing_style(database_id, job_id)
}

#[tauri::command]
pub async fn load_style_entry(
    registry: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    job_id: String,
    ordinal: String,
    request_id: String,
) -> Result<crate::commands::GameResponse, AppError> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        registry.load_style_entry(database_id, job_id, ordinal, request_id)
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_core::{GameLimits, parse_pgn};
    #[test]
    fn progress_emits_stage_boundaries_final_and_at_most_ten_updates_per_second() {
        let mut throttle = ProgressThrottle::default();
        let mut emitted = 0;
        for done in 0..1000 {
            let p = StyleProgress {
                stage: "scan".into(),
                done: done.to_string(),
                total: "1000".into(),
            };
            emitted += usize::from(throttle.allow(&p, done as u128));
        }
        assert_eq!(emitted, 10);
        assert!(throttle.allow(
            &StyleProgress {
                stage: "scan".into(),
                done: "1000".into(),
                total: "1000".into()
            },
            1000
        ));
        assert!(throttle.allow(
            &StyleProgress {
                stage: "replay".into(),
                done: "0".into(),
                total: "2000".into()
            },
            1000
        ));
    }
    #[test]
    fn promotions_are_pieces_and_promotion_moves_are_not_queen_moves() {
        let game = parse_pgn(
            b"1. a4 h5 2. a5 h4 3. a6 h3 4. axb7 hxg2 5. bxa8=Q gxh1=Q *",
            &GameLimits::default(),
        )
        .unwrap()
        .remove(0);
        let after = game.chess_position(10).unwrap();
        assert_eq!(inventory(&after, Color::White), [2, 2, 1, 2]);
        assert_eq!(inventory(&after, Color::Black), [2, 2, 1, 2]);
        assert_eq!(points(&after, Color::White, false), 35);
        for white in [true, false] {
            let r = replay(&game, white, &AtomicBool::new(false))
                .unwrap()
                .unwrap();
            assert_eq!((r.obs[1].n, r.obs[1].d), (2, 5));
            assert_eq!((r.obs[2].n, r.obs[2].d), (0, 0));
        }
    }
    #[test]
    fn material_points_include_promoted_queens_and_exclude_kings_and_pawns_for_endgames() {
        let p = chess_core::model::parse_position("4k3/pppp4/8/8/8/8/PPPP4/1NB1K1Q1 w - - 0 1")
            .unwrap();
        assert_eq!(points(&p, Color::White, false), 15);
        assert_eq!(points(&p, Color::White, true), 19);
        assert_eq!(points(&p, Color::Black, false), 0);
    }
    #[test]
    fn null_and_illegal_moves_are_rejected_instead_of_counting_partial_lines() {
        let game = parse_pgn(b"1. e4 e5 *", &GameLimits::default())
            .unwrap()
            .remove(0);
        for uci in ["0000", "e7e4"] {
            let mut invalid = game.clone();
            invalid.nodes[2].uci = Some(uci.into());
            assert!(replay(&invalid, true, &AtomicBool::new(false)).is_err());
        }
    }
    #[test]
    fn changed_source_between_entry_load_and_style_publication_rejects_and_closes_document() {
        use std::io::Write;
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("Atomic.2cbh");
        let game = parse_pgn(
            b"[White \"Alpha\"]\n[Black \"Other\"]\n\n1. e4 e5 *",
            &GameLimits::default(),
        )
        .unwrap();
        let mut w = cbh_reader::v2::write::Writer::create(&path).unwrap();
        w.append_all(&game).unwrap();
        drop(w);
        let r = Registry::default();
        let db = r
            .open_database(path.clone(), "open".into())
            .unwrap()
            .database_id;
        let p = StyleParams {
            player_ids: vec![0],
            ..Default::default()
        };
        r.playing_style(db.clone(), "snapshot".into(), &p, |_| {})
            .unwrap()
            .unwrap();
        let loaded = r.load_entry(db.clone(), "1".into(), "load".into()).unwrap();
        let document = loaded.document_id.clone().unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(&[0])
            .unwrap();
        assert_eq!(
            r.finish_style_entry(&db, "snapshot", loaded)
                .unwrap_err()
                .code,
            "sourceChanged"
        );
        assert!(r.document(&document).is_err());
    }
    #[test]
    fn replaced_job_between_entry_load_and_style_publication_also_closes_document() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("Replaced.2cbh");
        let game = parse_pgn(
            b"[White \"Alpha\"]\n[Black \"Other\"]\n\n1. e4 e5 *",
            &GameLimits::default(),
        )
        .unwrap();
        let mut w = cbh_reader::v2::write::Writer::create(&path).unwrap();
        w.append_all(&game).unwrap();
        drop(w);
        let r = Registry::default();
        let db = r.open_database(path, "open".into()).unwrap().database_id;
        let p = StyleParams {
            player_ids: vec![0],
            ..Default::default()
        };
        r.playing_style(db.clone(), "old".into(), &p, |_| {})
            .unwrap()
            .unwrap();
        let loaded = r.load_entry(db.clone(), "1".into(), "load".into()).unwrap();
        let document = loaded.document_id.clone().unwrap();
        r.playing_style(db.clone(), "new".into(), &p, |_| {})
            .unwrap()
            .unwrap();
        assert_eq!(
            r.finish_style_entry(&db, "old", loaded).unwrap_err().code,
            "staleReport"
        );
        assert!(r.document(&document).is_err());
        let loaded = r
            .load_style_entry(db, "new".into(), "1".into(), "good".into())
            .unwrap();
        assert!(loaded.document_id.is_some());
    }
}
