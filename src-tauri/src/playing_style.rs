//! Source-bound sampling for independently implemented recovered STYLE_REPORT calculations.
pub use crate::style_dimensions::NativeEvidence as StyleEvidence;
use crate::{
    commands::AppError,
    registry::Registry,
    style_dimensions::{NativeDimension, NativeInput, NativeResults, dimensions},
};
pub use cbh_reader::style_selection::StyleCriteria as StyleParams;
use cbh_reader::{DataStatus, source::SourceFile};
use chess_core::Game;
use serde::Serialize;
use shakmaty::{Chess, Position, san::SanPlus, uci::UciMove};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub const MAX_PLIES: usize = 400;
const MAX_EVIDENCE: usize = 7;
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
    pub stored_moves: u8,
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
    pub too_short: u64,
    pub unknown_date: u64,
    pub unknown_opponent_elo: u64,
    pub unknown_result: u64,
    pub unknown_category: u64,
    pub skipped: u32,
    pub skipped_setup: u32,
    pub skipped_unreadable: u32,
    pub skipped_unsupported: u32,
    pub skipped_partial: u32,
    pub cut_games: u32,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
    pub dimensions: Vec<NativeDimension>,
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
struct Replay {
    plies: u32,
    cut: bool,
    last_node: u32,
    last_san: String,
}

/// Validate only legal main-line plies and retain original node IDs. Result
/// dimensions describe the whole recorded game, so their supporting link opens
/// the last observed position; a capped game is explicitly marked as cut.
fn replay(game: &Game, cancel: &AtomicBool) -> Result<Option<Replay>, ()> {
    let mut pos = Chess::default();
    let mut node = 0u32;
    let mut last_san = String::new();
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
        let m = UciMove::from_ascii(child.uci.as_deref().ok_or(())?.as_bytes())
            .map_err(|_| ())?
            .to_move(&pos)
            .map_err(|_| ())?;
        last_san = SanPlus::from_move(pos.clone(), m).to_string();
        pos.play_unchecked(m);
        node = next;
        plies = ply as u32;
    }
    let cut = !game.nodes.get(node as usize).ok_or(())?.children.is_empty();
    Ok(Some(Replay {
        plies,
        cut,
        last_node: node,
        last_san,
    }))
}

fn record_result(results: &mut NativeResults, code: u8, white: bool) {
    results.games += 1;
    match code {
        1 | 5 => results.draws += 1,
        0 | 4 if !white => results.wins += 1,
        2 | 6 if white => results.wins += 1,
        0 | 2 | 4 | 6 => {}
        _ => results.unknown += 1,
    }
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
            too_short: found.too_short,
            unknown_date: found.unknown_date,
            unknown_opponent_elo: found.unknown_opponent_elo,
            unknown_result: found.unknown_result,
            unknown_category: found.unknown_category,
            skipped: 0,
            skipped_setup: 0,
            skipped_unreadable: 0,
            skipped_unsupported: 0,
            skipped_partial: 0,
            cut_games: 0,
            first_date: None,
            last_date: None,
            dimensions: vec![],
            games: vec![],
            elapsed_ms: 0,
        };
        let total = found.games.len();
        let mut white_results = NativeResults::default();
        let mut black_results = NativeResults::default();
        let mut result_evidence = Vec::new();
        let mut short_draws = 0_u64;
        let mut draw_length = 0_u64;
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
            let line = match replay(&game, &cancel) {
                Ok(Some(l)) => l,
                Ok(None) => return Ok(None),
                Err(()) => {
                    report.skipped_unreadable += 1;
                    continue;
                }
            };
            record_result(
                if g.white {
                    &mut white_results
                } else {
                    &mut black_results
                },
                g.result,
                g.white,
            );
            if result_evidence.len() < MAX_EVIDENCE
                && line.last_node > 0
                && matches!(g.result, 0..=2 | 4..=6)
            {
                result_evidence.push(StyleEvidence {
                    ordinal: g.ordinal.to_string(),
                    node: line.last_node,
                    ply: line.plies,
                    san: line.last_san,
                });
            }
            if matches!(g.result, 1 | 5) {
                short_draws += u64::from(g.stored_moves < 26);
                draw_length += u64::from(g.stored_moves);
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
                stored_moves: g.stored_moves,
            });
        }
        report.dimensions = dimensions(white_results, black_results);
        // These verified Tenacity components are useful evidence, but cannot
        // replace its unresolved exchange-opportunity terms.
        let draw_inputs = report.dimensions[2].inputs.clone();
        let tenacity = &mut report.dimensions[3];
        tenacity.inputs = draw_inputs;
        let draws = white_results.draws + black_results.draws;
        for (label, numerator, denominator, reference) in [
            (
                "Short draws (stored length below 26)",
                short_draws,
                u64::from(report.analysed),
                0.13,
            ),
            ("Mean stored draw length", draw_length, draws, 40.1),
        ] {
            tenacity.inputs.push(NativeInput {
                label: label.into(),
                numerator: Some(numerator as f64),
                denominator: Some(denominator),
                value: (denominator > 0).then(|| numerator as f64 / denominator as f64),
                reference: Some(reference),
            });
        }
        for dimension in &mut report.dimensions {
            if dimension.id == "score" || dimension.id == "decidedGames" {
                dimension.evidence = result_evidence.clone();
            }
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
    fn promotions_are_validated_without_substituting_a_style_property() {
        let game = parse_pgn(
            b"1. a4 h5 2. a5 h4 3. a6 h3 4. axb7 hxg2 5. bxa8=Q gxh1=Q *",
            &GameLimits::default(),
        )
        .unwrap()
        .remove(0);
        let line = replay(&game, &AtomicBool::new(false)).unwrap().unwrap();
        assert_eq!((line.plies, line.last_node, line.cut), (10, 10, false));
        assert_eq!(Some(line.last_san), game.san_at(10).unwrap());
    }
    #[test]
    fn null_and_illegal_moves_are_rejected_instead_of_counting_partial_lines() {
        let game = parse_pgn(b"1. e4 e5 *", &GameLimits::default())
            .unwrap()
            .remove(0);
        for uci in ["0000", "e7e4"] {
            let mut invalid = game.clone();
            invalid.nodes[2].uci = Some(uci.into());
            assert!(replay(&invalid, &AtomicBool::new(false)).is_err());
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
