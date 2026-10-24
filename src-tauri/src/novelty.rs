//! Find Novelty (`FINDNOVELTYBOARD`, Shift+F6): the first move of the shown
//! game that no other game of the reference database reaches, with the games
//! that reached the position before it. See docs/novelty.md.
use crate::{
    commands::{AppError, EntryDto},
    registry::Registry,
    trees::{Line, ReferenceSource},
};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Plies of the main line Find Novelty looks at.
pub const MAX_PLIES: usize = 300;
/// Games listed at most, as in the reference app.
pub const MAX_GAMES: usize = 400;

/// The novelty, when the game has one.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Novelty {
    /// Plies from the start: the novelty is the move reaching this ply.
    pub ply: u32,
    pub node: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoveltyReport {
    pub novelty: Option<Novelty>,
    /// The deepest ply whose position other games reach.
    pub known_ply: u32,
    /// How many other games reach it, and the first [`MAX_GAMES`].
    pub total: u32,
    pub games: Vec<EntryDto>,
    /// Positions asked of the database.
    pub queries: u32,
    /// The database could not be searched for some position: without an
    /// opening tree or a search booster, a large database is not scanned.
    pub not_searched: bool,
    pub cancelled: bool,
}

impl Registry {
    /// Find Novelty for document `id` against database `database_id`. The
    /// game's own entry, when it comes from that database, does not count.
    pub fn find_novelty(
        &self,
        id: &str,
        database_id: &str,
        cancel: &AtomicBool,
    ) -> Result<NoveltyReport, AppError> {
        let (game, origin) = {
            let d = self.document(id)?;
            let d = d
                .lock()
                .map_err(|_| AppError::new("state", "Document unavailable"))?;
            (d.game.clone(), d.origin.clone())
        };
        let own = origin.filter(|(db, _)| db == database_id).map(|(_, o)| o);
        let mut nodes = vec![0u32];
        let mut moves = vec![];
        while let Some(&next) = game.nodes[*nodes.last().unwrap() as usize].children.first() {
            if moves.len() >= MAX_PLIES {
                break;
            }
            let Some(uci) = game.nodes[next as usize].uci.clone() else {
                break;
            };
            moves.push(uci);
            nodes.push(next);
        }
        let mut report = NoveltyReport {
            novelty: None,
            known_ply: 0,
            total: 0,
            games: vec![],
            queries: 0,
            not_searched: false,
            cancelled: false,
        };
        // Other games reaching the position after `ply` plies.
        let mut known: Vec<Option<Vec<u64>>> = vec![None; moves.len() + 1];
        let mut ask = |ply: usize, report: &mut NoveltyReport| -> Result<Option<usize>, AppError> {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                return Ok(None);
            }
            let line = Line {
                start_fen: game.start_fen.clone(),
                moves: moves[..ply].to_vec(),
            };
            report.queries += 1;
            let (hits, _, source, _) = self.reference_hits(database_id, &line, cancel, false)?;
            if source == ReferenceSource::NotSearched {
                report.not_searched = true;
                return Ok(None);
            }
            let others: Vec<u64> = hits
                .ordinals
                .into_iter()
                .filter(|&o| Some(o) != own)
                .collect();
            let n = others.len();
            known[ply] = Some(others);
            Ok(Some(n))
        };
        // The first ply no other game reaches, walking the line from the
        // start (halving would skip a position left and later regained by a
        // transposition).
        let mut lo = 0usize;
        let mut first_new = None;
        for ply in 0..=moves.len() {
            match ask(ply, &mut report)? {
                None => return Ok(report),
                Some(0) => {
                    first_new = Some(ply);
                    break;
                }
                Some(_) => lo = ply,
            }
        }
        match first_new {
            // Not even the start position: every move is new.
            Some(0) => {
                report.novelty = (!moves.is_empty()).then_some(Novelty {
                    ply: 1,
                    node: nodes[1],
                });
                return Ok(report);
            }
            Some(ply) => {
                report.novelty = Some(Novelty {
                    ply: ply as u32,
                    node: nodes[ply],
                })
            }
            None => {}
        }
        report.known_ply = lo as u32;
        let games = known[lo].take().unwrap_or_default();
        report.total = games.len() as u32;
        report.games = self.entries_of(database_id, &games[..games.len().min(MAX_GAMES)])?;
        Ok(report)
    }
}

/// The running Find Novelty, for Stop.
#[derive(Default)]
pub struct NoveltyJob(Mutex<Option<Arc<AtomicBool>>>);

#[tauri::command]
pub async fn find_novelty(
    registry: tauri::State<'_, Arc<Registry>>,
    job: tauri::State<'_, Arc<NoveltyJob>>,
    document_id: String,
    database_id: String,
) -> Result<NoveltyReport, AppError> {
    let registry = registry.inner().clone();
    let cancel = Arc::new(AtomicBool::new(false));
    if let Ok(mut current) = job.0.lock()
        && let Some(old) = current.replace(cancel.clone())
    {
        old.store(true, Ordering::Relaxed);
    }
    tauri::async_runtime::spawn_blocking(move || {
        registry.find_novelty(&document_id, &database_id, &cancel)
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}

#[tauri::command]
pub fn cancel_find_novelty(job: tauri::State<'_, Arc<NoveltyJob>>) {
    if let Ok(current) = job.0.lock()
        && let Some(c) = current.as_ref()
    {
        c.store(true, Ordering::Relaxed);
    }
}
