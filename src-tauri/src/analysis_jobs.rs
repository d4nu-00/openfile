//! The analysis jobs queue (ChessBase's analysis jobs, see
//! docs/analysis.md): positions of games in a 2CBH database opened for
//! editing, analysed one after another by one engine. Each result is merged
//! into its game, which replaces the entry ("Merge result with original
//! game").
use crate::{
    blunder_check::white_text,
    commands::AppError,
    deep_analysis::{Analyst, EngineAnalyst, LineItem, Prefixed, Search},
    documents::add_lines,
    registry::{Registry, decimal},
};
use engine::{EngineSession, EngineSpec};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Jobs in the queue at most.
pub const MAX_JOBS: usize = 1_000;
/// Moves of an Analysis line written at most.
const LINE_MOVES: usize = 20;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JobKind {
    /// ChessBase's type 1: the engine's lines after the time.
    Analysis { lines: u32, seconds: u64 },
    /// ChessBase's type 2: Deep Analysis for the time.
    Deep { seconds: u64 },
}

impl JobKind {
    fn check(&self) -> Result<(), AppError> {
        let bad = |m: &str| Err(AppError::new("invalidParameters", m));
        match *self {
            // ChessBase's minimums: 15 s for Analysis, 30 s for Deep Analysis.
            Self::Analysis { lines, seconds } => {
                if !(1..=10).contains(&lines) {
                    return bad("Analysis takes 1 to 10 lines");
                }
                if !(15..=3_600).contains(&seconds) {
                    return bad("Analysis takes 15 s to 1 hour");
                }
            }
            Self::Deep { seconds } => {
                if !(30..=86_400).contains(&seconds) {
                    return bad("Deep Analysis takes 30 s to 24 hours");
                }
            }
        }
        Ok(())
    }
    fn seconds(&self) -> u64 {
        match *self {
            Self::Analysis { seconds, .. } | Self::Deep { seconds } => seconds,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    Waiting,
    Running,
    Done,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: u64,
    pub database_id: String,
    pub ordinal: String,
    /// "White – Black" of the game.
    pub label: String,
    /// The moves leading to the position, from the game's start.
    pub moves: Vec<String>,
    pub kind: JobKind,
    pub status: JobStatus,
    /// Why it was skipped or failed, or what it added.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// A hash of the game when the job was added.
    #[serde(skip)]
    fingerprint: u64,
}

#[derive(Default)]
pub struct Queue {
    jobs: Vec<Job>,
    next: u64,
    /// The running queue's stop flag.
    running: Option<Arc<AtomicBool>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobsView {
    pub jobs: Vec<Job>,
    pub running: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobsProgress {
    /// The job running, and its share done in thousandths.
    pub job: Option<u64>,
    pub done: u32,
    pub total: u32,
    pub jobs: Vec<Job>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunParams {
    pub engine: PathBuf,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// Milliseconds per second of job time, 1,000 normally; tests run
    /// faster.
    #[serde(default)]
    pub millis_per_second: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    pub stopped: bool,
    pub done: u32,
    pub skipped: u32,
    pub failed: u32,
    pub jobs: Vec<Job>,
}

/// A game's fingerprint: its PGN, which covers moves, tags and comments.
fn fingerprint(game: &chess_core::Game) -> Result<u64, AppError> {
    let text = chess_core::write_pgn(game)?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    Ok(h.finish())
}

fn label(white: Option<&str>, black: Option<&str>) -> String {
    format!("{} – {}", white.unwrap_or("?"), black.unwrap_or("?"))
}

/// The node reached from the start by `moves`, following any line.
fn find_node(game: &chess_core::Game, moves: &[String]) -> Option<u32> {
    let mut at = 0u32;
    for uci in moves {
        at = *game.nodes[at as usize]
            .children
            .iter()
            .find(|&&c| game.nodes[c as usize].uci.as_deref() == Some(uci.as_str()))?;
    }
    Some(at)
}

/// The moves leading to `node`.
fn moves_to(game: &chess_core::Game, node: u32) -> Result<Vec<String>, AppError> {
    Ok(game
        .path_to(node)?
        .iter()
        .filter_map(|&n| game.nodes[n as usize].uci.clone())
        .collect())
}

/// The legal moves at the start of `pv`, at most `LINE_MOVES`.
fn legal_prefix(position: &shakmaty::Chess, pv: &[String]) -> Vec<String> {
    let mut p = position.clone();
    let mut out = vec![];
    for uci in pv.iter().take(LINE_MOVES) {
        match chess_core::model::apply_uci(p.clone(), uci) {
            Ok(next) => {
                p = next;
                out.push(uci.clone());
            }
            Err(_) => break,
        }
    }
    out
}

/// The last node of the main line.
fn main_line_end(game: &chess_core::Game) -> u32 {
    let mut at = 0u32;
    while let Some(&next) = game.nodes[at as usize].children.first() {
        at = next;
    }
    at
}

impl Registry {
    fn queue(&self) -> Result<std::sync::MutexGuard<'_, Queue>, AppError> {
        self.analysis_jobs
            .lock()
            .map_err(|_| AppError::new("state", "Analysis jobs unavailable"))
    }

    pub fn analysis_jobs(&self) -> Result<JobsView, AppError> {
        let q = self.queue()?;
        Ok(JobsView {
            jobs: q.jobs.clone(),
            running: q.running.is_some(),
        })
    }

    fn push_job(
        &self,
        database_id: &str,
        ordinal: u64,
        label: String,
        moves: Vec<String>,
        kind: &JobKind,
        fingerprint: u64,
    ) -> Result<(), AppError> {
        let mut q = self.queue()?;
        if q.jobs.len() >= MAX_JOBS {
            return Err(AppError::new(
                "limitExceeded",
                format!("The queue holds at most {MAX_JOBS} jobs"),
            ));
        }
        q.next += 1;
        let id = q.next;
        q.jobs.push(Job {
            id,
            database_id: database_id.into(),
            ordinal: ordinal.to_string(),
            label,
            moves,
            kind: kind.clone(),
            status: JobStatus::Waiting,
            message: None,
            fingerprint,
        });
        Ok(())
    }

    /// ChessBase's Create analysis jobs: the final position of each game's
    /// main line, for the games in `ranges` of the current list. Text
    /// entries are left out; games that cannot be read are left out and
    /// counted.
    pub fn add_analysis_jobs(
        &self,
        database_id: String,
        ranges: Vec<crate::clipboard::RowRange>,
        kind: JobKind,
    ) -> Result<(JobsView, u32), AppError> {
        kind.check()?;
        self.writer_of(&database_id)?;
        let free = MAX_JOBS.saturating_sub(self.queue()?.jobs.len());
        let (ordinals, truncated) = self.ordinals_of_rows(&database_id, &ranges, free)?;
        if truncated {
            return Err(AppError::new(
                "limitExceeded",
                format!("The queue has room for {free} more jobs"),
            ));
        }
        let entries = self.entries_of(&database_id, &ordinals)?;
        let ordinals: Vec<u64> = entries
            .iter()
            .zip(&ordinals)
            .filter(|(e, _)| e.kind == "game")
            .map(|(_, &o)| o)
            .collect();
        let reader = self.game_reader(&database_id)?;
        let mut left_out = 0;
        for &ordinal in &ordinals {
            let loaded = reader
                .lock()
                .map_err(|_| AppError::new("state", "Database unavailable"))?
                .load(ordinal);
            let Ok(loaded) = loaded else {
                left_out += 1;
                continue;
            };
            let Some(game) = loaded.game else {
                left_out += 1;
                continue;
            };
            let moves = moves_to(&game, main_line_end(&game))?;
            self.push_job(
                &database_id,
                ordinal,
                label(
                    loaded.summary.white.as_deref(),
                    loaded.summary.black.as_deref(),
                ),
                moves,
                &kind,
                fingerprint(&game)?,
            )?;
        }
        Ok((self.analysis_jobs()?, left_out))
    }

    /// ChessBase's Add Analysis Job: the board position of a game loaded
    /// from a database opened for editing, without unsaved changes.
    pub fn add_analysis_job(
        &self,
        document_id: String,
        node: u32,
        kind: JobKind,
    ) -> Result<JobsView, AppError> {
        kind.check()?;
        let (game, origin, dirty) = {
            let d = self.document(&document_id)?;
            let d = d
                .lock()
                .map_err(|_| AppError::new("state", "Document unavailable"))?;
            (d.game.clone(), d.origin.clone(), d.dirty)
        };
        let (database_id, ordinal) = origin
            .ok_or_else(|| AppError::new("noOrigin", "This game was not loaded from a database"))?;
        if dirty {
            return Err(AppError::new(
                "unsaved",
                "Save or replace the game first: the job analyses the game in the database",
            ));
        }
        self.writer_of(&database_id)?;
        if game.nodes.get(node as usize).is_none() {
            return Err(AppError::new("invalidNode", "Unknown node"));
        }
        let moves = moves_to(&game, node)?;
        let tag = |k: &str| game.tags.get(k).map(String::as_str);
        self.push_job(
            &database_id,
            ordinal,
            label(tag("White"), tag("Black")),
            moves,
            &kind,
            fingerprint(&game)?,
        )?;
        self.analysis_jobs()
    }

    /// Restart, Skip or Remove a job; not the running one.
    pub fn edit_analysis_job(&self, id: u64, action: &str) -> Result<JobsView, AppError> {
        {
            let mut q = self.queue()?;
            let i = q
                .jobs
                .iter()
                .position(|j| j.id == id)
                .ok_or_else(|| AppError::new("invalidHandle", "Unknown job"))?;
            if q.jobs[i].status == JobStatus::Running {
                return Err(AppError::new("busy", "The job is running"));
            }
            match action {
                "restart" => {
                    q.jobs[i].status = JobStatus::Waiting;
                    q.jobs[i].message = None;
                }
                "skip" => {
                    q.jobs[i].status = JobStatus::Skipped;
                    q.jobs[i].message = Some("Skipped by the user".into());
                }
                "remove" => {
                    q.jobs.remove(i);
                }
                _ => return Err(AppError::new("invalidParameters", "Unknown action")),
            }
        }
        self.analysis_jobs()
    }

    /// New analysis jobs: an empty queue.
    pub fn clear_analysis_jobs(&self) -> Result<JobsView, AppError> {
        let mut q = self.queue()?;
        if q.running.is_some() {
            return Err(AppError::new("busy", "Stop the jobs first"));
        }
        q.jobs.clear();
        drop(q);
        self.analysis_jobs()
    }

    pub fn stop_analysis_jobs(&self) -> Result<(), AppError> {
        if let Some(stop) = &self.queue()?.running {
            stop.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    fn set_job(&self, id: u64, status: JobStatus, message: Option<String>) -> Vec<Job> {
        let Ok(mut q) = self.queue() else {
            return vec![];
        };
        if let Some(j) = q.jobs.iter_mut().find(|j| j.id == id) {
            j.status = status;
            j.message = message;
        }
        q.jobs.clone()
    }

    /// Runs the waiting jobs in order until done or stopped.
    pub fn run_analysis_jobs(
        &self,
        params: &RunParams,
        mut progress: impl FnMut(JobsProgress),
    ) -> Result<RunReport, AppError> {
        let scale = params.millis_per_second.unwrap_or(1_000);
        if !(1..=1_000).contains(&scale) {
            return Err(AppError::new("invalidParameters", "Invalid time scale"));
        }
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut q = self.queue()?;
            if q.running.is_some() {
                return Err(AppError::new("busy", "The jobs are already running"));
            }
            if !q.jobs.iter().any(|j| j.status == JobStatus::Waiting) {
                return Err(AppError::new("noJobs", "There are no open positions"));
            }
            q.running = Some(stop.clone());
        }
        let result = self.run_jobs(params, scale, &stop, &mut progress);
        let mut q = self.queue()?;
        q.running = None;
        // A job interrupted by an error goes back to waiting.
        for j in &mut q.jobs {
            if j.status == JobStatus::Running {
                j.status = JobStatus::Waiting;
            }
        }
        let jobs = q.jobs.clone();
        drop(q);
        let (stopped, counts) = result?;
        Ok(RunReport {
            stopped,
            done: counts[0],
            skipped: counts[1],
            failed: counts[2],
            jobs,
        })
    }

    fn run_jobs(
        &self,
        params: &RunParams,
        scale: u64,
        stop: &AtomicBool,
        progress: &mut impl FnMut(JobsProgress),
    ) -> Result<(bool, [u32; 3]), AppError> {
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
                if name != "MultiPV" && session.options().iter().any(|o| &o.name == name) {
                    session
                        .set_option(name, value)
                        .map_err(|e| AppError::new("engine", e.to_string()))?;
                }
            }
            let mut counts = [0u32; 3];
            loop {
                if stop.load(Ordering::Relaxed) {
                    return Ok((true, counts));
                }
                let next = {
                    let q = self.queue()?;
                    let total = q.jobs.len() as u32;
                    let done = q
                        .jobs
                        .iter()
                        .filter(|j| j.status != JobStatus::Waiting)
                        .count() as u32;
                    q.jobs
                        .iter()
                        .find(|j| j.status == JobStatus::Waiting)
                        .cloned()
                        .map(|j| (j, done, total))
                };
                let Some((job, done, total)) = next else {
                    return Ok((false, counts));
                };
                let jobs = self.set_job(job.id, JobStatus::Running, None);
                progress(JobsProgress {
                    job: Some(job.id),
                    done,
                    total,
                    jobs,
                });
                match self.run_job(&job, &mut session, scale, stop) {
                    Ok(Outcome::Done(message)) => {
                        counts[0] += 1;
                        self.set_job(job.id, JobStatus::Done, Some(message));
                    }
                    Ok(Outcome::Skipped(message)) => {
                        counts[1] += 1;
                        self.set_job(job.id, JobStatus::Skipped, Some(message));
                    }
                    Ok(Outcome::Stopped) => {
                        self.set_job(job.id, JobStatus::Waiting, None);
                        return Ok((true, counts));
                    }
                    Err(e) if e.code == "engine" => {
                        // The engine is gone: the job waits for another run.
                        self.set_job(job.id, JobStatus::Waiting, None);
                        return Err(e);
                    }
                    Err(e) => {
                        counts[2] += 1;
                        self.set_job(job.id, JobStatus::Failed, Some(e.message.clone()));
                    }
                }
                let q = self.queue()?;
                let jobs = q.jobs.clone();
                let done = jobs
                    .iter()
                    .filter(|j| j.status != JobStatus::Waiting)
                    .count() as u32;
                drop(q);
                progress(JobsProgress {
                    job: None,
                    done,
                    total: jobs.len() as u32,
                    jobs,
                });
            }
        })();
        let _ = session.shutdown();
        result
    }

    /// Loads a job's game, checking it can be replaced without loss.
    fn job_game(&self, job: &Job) -> Result<Result<chess_core::Game, String>, AppError> {
        let ordinal = decimal(&job.ordinal)?;
        let reader = self.game_reader(&job.database_id)?;
        let mut reader = reader
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?;
        let mut loaded = reader.load(ordinal);
        if matches!(loaded, Err(cbh_reader::CbhError::SourceChanged)) {
            drop(reader);
            self.lock()?.game_readers.remove(&job.database_id);
            let fresh = self.game_reader(&job.database_id)?;
            loaded = fresh
                .lock()
                .map_err(|_| AppError::new("state", "Database unavailable"))?
                .load(ordinal);
        }
        let loaded = loaded?;
        let Some(game) = loaded.game else {
            return Ok(Err("The entry has no game".into()));
        };
        use cbh_reader::DataStatus;
        if matches!(
            loaded.annotations_status,
            DataStatus::NotDecoded
                | DataStatus::Partial
                | DataStatus::Corrupt
                | DataStatus::Unsupported
        ) || !cbh_reader::v2::encode::encode_annotations(&game)
            .1
            .notes
            .is_empty()
        {
            return Ok(Err(
                "Its annotations are not all decoded; replacing it would lose them".into(),
            ));
        }
        if game.is_chess960() {
            return Ok(Err("Chess960 games cannot be saved to 2CBH yet".into()));
        }
        Ok(Ok(game))
    }

    fn run_job(
        &self,
        job: &Job,
        session: &mut EngineSession,
        scale: u64,
        stop: &AtomicBool,
    ) -> Result<Outcome, AppError> {
        let ordinal = decimal(&job.ordinal)?;
        let documents: Vec<_> = self.lock()?.documents.values().cloned().collect();
        let open = documents.iter().any(|d| {
            d.lock()
                .is_ok_and(|d| d.origin.as_ref() == Some(&(job.database_id.clone(), ordinal)))
        });
        if open {
            return Ok(Outcome::Skipped("The game is open in a tab".into()));
        }
        let mut game = match self.job_game(job)? {
            Ok(g) => g,
            Err(reason) => return Ok(Outcome::Skipped(reason)),
        };
        if fingerprint(&game)? != job.fingerprint {
            return Ok(Outcome::Skipped(
                "The game changed since the job was added".into(),
            ));
        }
        let Some(node) = find_node(&game, &job.moves) else {
            return Ok(Outcome::Skipped(
                "The position is no longer in the game".into(),
            ));
        };
        let original = game.clone();
        let root = game.chess_position(node)?;
        let millis = job.kind.seconds() * scale;
        // Cancelling a job means stopping it; nothing is written.
        let lines = match &job.kind {
            JobKind::Analysis { lines, .. } => {
                let mut analyst = EngineAnalyst::new(session, game.start_fen.clone(), stop);
                let Some(found) = analyst.lines(&job.moves, &root, *lines as usize, millis)? else {
                    return Ok(Outcome::Stopped);
                };
                let white = shakmaty::Position::turn(&root).is_white();
                found
                    .into_iter()
                    .map(|l| LineItem {
                        comment: Some(match l.depth {
                            Some(d) => format!("{}/{d}", white_text(l.cp, white)),
                            None => white_text(l.cp, white),
                        }),
                        moves: legal_prefix(&root, &l.pv),
                        evaluation: None,
                    })
                    .collect::<Vec<_>>()
            }
            JobKind::Deep { seconds } => {
                let mut analyst = Prefixed {
                    inner: EngineAnalyst::new(session, game.start_fen.clone(), stop),
                    prefix: job.moves.clone(),
                };
                let never = AtomicBool::new(false);
                let refuted_game = original.clone();
                let mut search = Search::new(
                    &mut analyst,
                    root.clone(),
                    crate::deep_analysis::BASE_MILLIS * scale / 1_000,
                    Some(Duration::from_millis(seconds * scale)),
                    &never,
                    Box::new(move |path| {
                        crate::deep_analysis::game_refuted(&refuted_game, node, path)
                    }),
                );
                let (_, cancelled) = search.run(|_, _| {})?;
                if cancelled || stop.load(Ordering::Relaxed) {
                    return Ok(Outcome::Stopped);
                }
                search.line_items()
            }
        };
        if lines.is_empty() {
            return Ok(Outcome::Done("Nothing to add".into()));
        }
        let lines: Vec<LineItem> = lines.into_iter().filter(|l| !l.moves.is_empty()).collect();
        let count = lines.len();
        add_lines(&mut game, node, &lines)?;
        // The entry must still be the game analysed.
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        match self.job_game(job)? {
            Ok(now) if fingerprint(&now)? == fingerprint(&original)? => {}
            _ => {
                return Err(AppError::new(
                    "staleGame",
                    "The game changed during the analysis; nothing was written",
                ));
            }
        }
        let (writer, path) = self.writer_of(&job.database_id)?;
        let tree = self.tree_before_change(&job.database_id, Some(ordinal));
        writer
            .lock()
            .map_err(|_| AppError::new("state", "Database writer unavailable"))?
            .replace(ordinal, &game)?;
        self.refresh_after_write(&job.database_id, &path)?;
        self.tree_after_change(&job.database_id, tree, Some(ordinal));
        Ok(Outcome::Done(format!(
            "{count} {} added",
            if count == 1 { "line" } else { "lines" }
        )))
    }
}

enum Outcome {
    Done(String),
    Skipped(String),
    Stopped,
}

#[tauri::command]
pub fn analysis_jobs(registry: tauri::State<'_, Arc<Registry>>) -> Result<JobsView, AppError> {
    registry.analysis_jobs()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddJobsResponse {
    pub view: JobsView,
    pub left_out: u32,
}

#[tauri::command]
pub async fn add_analysis_jobs(
    registry: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ranges: Vec<crate::clipboard::RowRange>,
    kind: JobKind,
) -> Result<AddJobsResponse, AppError> {
    let r = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (view, left_out) = r.add_analysis_jobs(database_id, ranges, kind)?;
        Ok(AddJobsResponse { view, left_out })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}

#[tauri::command]
pub fn add_analysis_job(
    registry: tauri::State<'_, Arc<Registry>>,
    document_id: String,
    node: u32,
    kind: JobKind,
) -> Result<JobsView, AppError> {
    registry.add_analysis_job(document_id, node, kind)
}

#[tauri::command]
pub fn edit_analysis_job(
    registry: tauri::State<'_, Arc<Registry>>,
    id: String,
    action: String,
) -> Result<JobsView, AppError> {
    registry.edit_analysis_job(decimal(&id)?, &action)
}

#[tauri::command]
pub fn clear_analysis_jobs(
    registry: tauri::State<'_, Arc<Registry>>,
) -> Result<JobsView, AppError> {
    registry.clear_analysis_jobs()
}

#[tauri::command]
pub async fn run_analysis_jobs(
    registry: tauri::State<'_, Arc<Registry>>,
    params: RunParams,
    events: tauri::ipc::Channel<JobsProgress>,
) -> Result<RunReport, AppError> {
    let r = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        r.run_analysis_jobs(&params, |p| {
            let _ = events.send(p);
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}

#[tauri::command]
pub fn stop_analysis_jobs(registry: tauri::State<'_, Arc<Registry>>) -> Result<(), AppError> {
    registry.stop_analysis_jobs()
}
