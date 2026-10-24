//! Opening trees built from the open database, and the reference search
//! (see docs/opening-tree.md).
use crate::{
    commands::{AppError, PageResponse},
    registry::Registry,
    searches::Hits,
};
use cbh_reader::{
    CbhError,
    tree::{self, BuildStage, OpeningTree, PendingChange, ReferenceFilter, Source},
};
use shakmaty::{Chess, Color, EnPassantMode, Position, fen::Fen, san::SanPlus, uci::UciMove};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Games a reference search lists, as for position searches.
const MAX_REFERENCE_HITS: usize = 100_000;
/// Games replayed to follow a line past the stored positions.
const MAX_FOLLOW: usize = 5_000;
/// Without a search booster, Find Position reads every game; the reference
/// search does that by itself only for databases up to this size. (The
/// classic test database, 11.15 million games, takes 23 minutes.)
const AUTO_SEARCH_ENTRIES: u64 = 200_000;
/// Moves accepted in a line.
const MAX_LINE: usize = 1_000;
/// Keep filtered lookups responsive even when the search has millions of hits.
const MAX_FILTERED_TREE_HITS: usize = 50_000;

/// An opening tree with its own read handle on the database.
pub struct OpenTree {
    tree: OpeningTree,
    db: cbh_reader::Database,
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;

    #[test]
    fn old_request_cannot_cancel_new_filtered_query() {
        let registry = Registry::default();
        let cancel = Arc::new(AtomicBool::new(false));
        registry
            .lock()
            .unwrap()
            .filtered_tree_cancels
            .insert("db".into(), ("new".into(), cancel.clone()));
        registry
            .cancel_filtered_opening_tree("db".into(), "old".into())
            .unwrap();
        assert!(!cancel.load(Ordering::Relaxed));
        registry
            .cancel_filtered_opening_tree("db".into(), "new".into())
            .unwrap();
        assert!(cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn closing_database_cancels_filtered_query() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::default();
        let id = registry
            .create_database(dir.path().join("Q.2cbh"), "t".into())
            .unwrap()
            .database_id;
        let cancel = Arc::new(AtomicBool::new(false));
        registry
            .lock()
            .unwrap()
            .filtered_tree_cancels
            .insert(id.clone(), ("query".into(), cancel.clone()));
        registry.close_database(id).unwrap();
        assert!(cancel.load(Ordering::Relaxed));
    }
}

/// A position given as a line: the start position and the moves from it.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub start_fen: String,
    pub moves: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TreeStatus {
    /// The tree stores the position.
    Tree,
    /// Answered by following the line's games past the stored positions.
    Line,
    /// No position of the line is in the tree.
    NotInTree,
    /// Too many games to follow past the stored positions.
    TooMany,
    /// The database has no current opening tree.
    NoTree,
    /// Statistics from the current search result's main lines.
    Filtered,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeMove {
    pub uci: String,
    pub san: String,
    pub games: u64,
    /// Percentage of the position's games.
    pub share: f64,
    /// Score of the side to move, in percent.
    pub score: Option<f64>,
    /// Draws, in percent.
    pub draws: Option<f64>,
    pub average_elo: Option<u32>,
    pub performance: Option<u32>,
    pub first_year: Option<u16>,
    pub last_year: Option<u16>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeResponse {
    pub request_id: String,
    pub status: TreeStatus,
    /// Games reaching the position.
    pub games: u64,
    /// Of those, games that end in it.
    pub ended: u64,
    pub moves: Vec<TreeMove>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampled_hits: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_hits: Option<u64>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeProgress {
    pub stage: BuildStage,
    pub done: String,
    pub total: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReferenceSource {
    Tree,
    Line,
    /// Find Position, with the search booster when `indexed`.
    Search,
    /// The tree cannot answer, and Find Position would read every game of a
    /// large database without a search booster; it runs only when asked.
    NotSearched,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceResponse {
    pub request_id: String,
    pub hits: String,
    pub truncated: bool,
    pub cancelled: bool,
    pub source: ReferenceSource,
    pub indexed: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceChoice {
    pub san: String,
    pub full: u64,
    pub filtered: u64,
    pub player: u64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceStatistics {
    pub request_id: String,
    pub full_games: u64,
    pub filtered_games: u64,
    pub player_games: u64,
    pub scanned_games: u64,
    pub after_elo: u64,
    pub after_year: u64,
    pub missing_elo: u64,
    pub missing_year: u64,
    pub complete: bool,
    pub full_complete: bool,
    pub moves: Vec<ReferenceChoice>,
}

/// The line's last position, and the hashes and packed moves leading there.
pub(crate) fn replay(line: &Line) -> Result<(Chess, Vec<u64>, Vec<u16>), AppError> {
    if line.moves.len() > MAX_LINE {
        return Err(AppError::new("bounds", "The line is too long"));
    }
    let mut pos = chess_core::model::parse_position(&line.start_fen)?;
    let mut hashes = vec![tree::position_hash(pos.board(), pos.turn())];
    let mut moves = Vec::with_capacity(line.moves.len());
    for uci in &line.moves {
        if uci == "0000" {
            pos = chess_core::model::apply_uci(pos, uci)?;
            moves.push(tree::NULL_MOVE);
        } else {
            let illegal = || AppError::new("illegalMove", format!("Illegal move {uci}"));
            let m = UciMove::from_ascii(uci.as_bytes())
                .map_err(|_| illegal())?
                .to_move(&pos)
                .map_err(|_| illegal())?;
            moves.push(tree::pack_move(&m).ok_or_else(illegal)?);
            pos.play_unchecked(m);
        }
        hashes.push(tree::position_hash(pos.board(), pos.turn()));
    }
    Ok((pos, hashes, moves))
}

pub(crate) fn san(pos: &Chess, uci: &str) -> String {
    if uci == "0000" {
        return "--".into();
    }
    UciMove::from_ascii(uci.as_bytes())
        .ok()
        .and_then(|u| u.to_move(pos).ok())
        .map_or_else(
            || uci.into(),
            |m| SanPlus::from_move(pos.clone(), m).to_string(),
        )
}

/// Puts the rows of `moves`, played in `pos`, into `response`; games that
/// end in the position are counted apart.
pub(crate) fn fill_moves(response: &mut TreeResponse, pos: &Chess, moves: &[tree::MoveStats]) {
    response.games = moves.iter().map(|m| m.games).sum();
    for m in moves {
        let Some(uci) = m.uci() else {
            response.ended += m.games;
            continue;
        };
        response.moves.push(TreeMove {
            san: san(pos, &uci),
            uci,
            games: m.games,
            share: m.games as f64 * 100.0 / response.games.max(1) as f64,
            score: m.score(),
            draws: m.draw_rate(),
            average_elo: m.average_elo(),
            performance: m.performance(),
            first_year: (m.year_min > 0).then_some(m.year_min),
            last_year: (m.year_max > 0).then_some(m.year_max),
        });
    }
}

impl Registry {
    fn tree_path(&self, database_id: &str) -> Result<std::path::PathBuf, AppError> {
        self.data_file(database_id, Some("opening-trees"), "cwtree")
    }

    /// The database's current opening tree, opened once and kept.
    fn open_tree(&self, database_id: &str) -> Result<Option<Arc<Mutex<OpenTree>>>, AppError> {
        if let Some(open) = self.lock()?.trees.get(database_id) {
            return Ok(Some(open.clone()));
        }
        let path = self.tree_path(database_id)?;
        let db = self.own_handle(database_id)?;
        let Some(tree) = OpeningTree::open(&path, &db)? else {
            return Ok(None);
        };
        let open = Arc::new(Mutex::new(OpenTree { tree, db }));
        let mut state = self.lock()?;
        if !state.databases.contains_key(database_id) {
            return Err(AppError::new(
                "invalidHandle",
                "Database is closed or unknown",
            ));
        }
        state.trees.insert(database_id.into(), open.clone());
        Ok(Some(open))
    }

    pub fn has_tree(&self, database_id: &str) -> Result<bool, AppError> {
        Ok(self.open_tree(database_id)?.is_some())
    }

    /// Builds the opening tree; `progress` gets the stage and its progress.
    pub fn create_opening_tree(
        &self,
        database_id: String,
        mut progress: impl FnMut(BuildStage, u64, u64),
    ) -> Result<bool, AppError> {
        let path = self.tree_path(&database_id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .tree_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let reader = self.own_handle(&database_id)?;
        let built = OpeningTree::build(
            &reader,
            &path,
            &tree::BuildOptions::default(),
            &cancel,
            &mut progress,
        );
        let mut state = self.lock()?;
        if state
            .tree_cancels
            .get(&database_id)
            .is_some_and(|c| Arc::ptr_eq(c, &cancel))
        {
            state.tree_cancels.remove(&database_id);
        }
        state.trees.remove(&database_id);
        Ok(built?)
    }

    pub fn cancel_opening_tree(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.tree_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn delete_opening_tree(&self, database_id: String) -> Result<(), AppError> {
        let path = self.tree_path(&database_id)?;
        self.lock()?.trees.remove(&database_id);
        for file in [path.with_extension("cwtj"), path] {
            match std::fs::remove_file(file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(AppError::new("io", e.to_string()));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The moves played from the line's last position, with statistics.
    pub fn opening_tree(
        &self,
        database_id: String,
        line: Line,
        request_id: String,
    ) -> Result<TreeResponse, AppError> {
        let (pos, hashes, moves) = replay(&line)?;
        let mut response = TreeResponse {
            request_id,
            status: TreeStatus::NoTree,
            games: 0,
            ended: 0,
            moves: vec![],
            sampled_hits: None,
            total_hits: None,
        };
        let Some(open) = self.open_tree(&database_id)? else {
            return Ok(response);
        };
        let mut open = open
            .lock()
            .map_err(|_| AppError::new("state", "Opening tree unavailable"))?;
        let OpenTree { tree, db } = &mut *open;
        // Only statistics are needed, so no games are listed.
        let answer = match tree.query(db, &hashes, &moves, 0, MAX_FOLLOW, &AtomicBool::new(false)) {
            Ok(answer) => answer,
            Err(CbhError::LimitExceeded) => {
                response.status = TreeStatus::TooMany;
                return Ok(response);
            }
            Err(e) => return Err(e.into()),
        };
        response.status = match answer.source {
            Source::Tree => TreeStatus::Tree,
            Source::Line => TreeStatus::Line,
            Source::None => TreeStatus::NotInTree,
        };
        fill_moves(&mut response, &pos, &answer.moves);
        Ok(response)
    }

    /// Move statistics for the current search result at the board position.
    /// The first 50,000 hits are scanned; the response makes that bound visible.
    pub fn filtered_opening_tree(
        &self,
        database_id: String,
        line: Line,
        request_id: String,
    ) -> Result<TreeResponse, AppError> {
        let (pos, hashes, _) = replay(&line)?;
        let target = *hashes
            .last()
            .ok_or_else(|| AppError::new("bounds", "The line has no position"))?;
        let (hits, cancel) = {
            let mut state = self.lock()?;
            let hits = state
                .searches
                .get(&database_id)
                .cloned()
                .ok_or_else(|| AppError::new("noSearch", "No current search result"))?;
            let cancel = Arc::new(AtomicBool::new(false));
            if let Some((_, old)) = state
                .filtered_tree_cancels
                .insert(database_id.clone(), (request_id.clone(), cancel.clone()))
            {
                old.store(true, Ordering::Relaxed);
            }
            (hits, cancel)
        };
        let total = hits.ordinals.len();
        let sampled = total.min(MAX_FILTERED_TREE_HITS);
        let db = match self.own_handle(&database_id) {
            Ok(db) => db,
            Err(error) => {
                let mut state = self.lock()?;
                if state
                    .filtered_tree_cancels
                    .get(&database_id)
                    .is_some_and(|(_, active)| Arc::ptr_eq(active, &cancel))
                {
                    state.filtered_tree_cancels.remove(&database_id);
                }
                return Err(error);
            }
        };
        let stats = db.stats_of_games(&hits.ordinals[..sampled], target, &cancel);
        let mut state = self.lock()?;
        let current = state
            .filtered_tree_cancels
            .get(&database_id)
            .is_some_and(|(_, active)| Arc::ptr_eq(active, &cancel));
        if current {
            state.filtered_tree_cancels.remove(&database_id);
        }
        if cancel.load(Ordering::Relaxed)
            || !current
            || !state
                .searches
                .get(&database_id)
                .is_some_and(|active| Arc::ptr_eq(active, &hits))
        {
            return Err(AppError::new(
                "cancelled",
                "Filtered tree query was superseded",
            ));
        }
        drop(state);
        let mut response = TreeResponse {
            request_id,
            status: TreeStatus::Filtered,
            games: 0,
            ended: 0,
            moves: vec![],
            sampled_hits: Some(sampled as u64),
            total_hits: Some(total as u64),
        };
        fill_moves(&mut response, &pos, &stats?);
        Ok(response)
    }

    pub fn cancel_filtered_opening_tree(
        &self,
        database_id: String,
        request_id: String,
    ) -> Result<(), AppError> {
        if let Some((active_id, cancel)) = self.lock()?.filtered_tree_cancels.get(&database_id)
            && *active_id == request_id
        {
            cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Lists the games reaching the line's last position: from the opening
    /// tree when it can answer, otherwise by Find Position. A new request
    /// cancels the previous one.
    pub fn reference_search(
        &self,
        database_id: String,
        line: Line,
        request_id: String,
    ) -> Result<ReferenceResponse, AppError> {
        self.reference_search_with(database_id, line, request_id, false)
    }

    /// The reference search; `search_anyway` allows a Find Position without
    /// a search booster on a large database.
    pub fn reference_search_with(
        &self,
        database_id: String,
        line: Line,
        request_id: String,
        search_anyway: bool,
    ) -> Result<ReferenceResponse, AppError> {
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .reference_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let result = self.reference_hits(&database_id, &line, &cancel, search_anyway);
        let mut state = self.lock()?;
        let current = state
            .reference_cancels
            .get(&database_id)
            .is_some_and(|c| Arc::ptr_eq(c, &cancel));
        if current {
            state.reference_cancels.remove(&database_id);
        }
        let (hits, truncated, source, indexed) = result?;
        let cancelled = cancel.load(Ordering::Relaxed);
        let response = ReferenceResponse {
            request_id,
            hits: hits.ordinals.len().to_string(),
            truncated,
            cancelled,
            source,
            indexed,
        };
        if current && !cancelled && state.databases.contains_key(&database_id) {
            state.references.insert(database_id, Arc::new(hits));
        }
        Ok(response)
    }

    pub(crate) fn reference_hits(
        &self,
        database_id: &str,
        line: &Line,
        cancel: &AtomicBool,
        search_anyway: bool,
    ) -> Result<(Hits, bool, ReferenceSource, bool), AppError> {
        let (pos, hashes, moves) = replay(line)?;
        if let Some(open) = self.open_tree(database_id)? {
            let mut open = open
                .lock()
                .map_err(|_| AppError::new("state", "Opening tree unavailable"))?;
            let OpenTree { tree, db } = &mut *open;
            match tree.query(db, &hashes, &moves, MAX_REFERENCE_HITS, MAX_FOLLOW, cancel) {
                Ok(answer) if answer.source != Source::None => {
                    let source = match answer.source {
                        Source::Tree => ReferenceSource::Tree,
                        _ => ReferenceSource::Line,
                    };
                    let fen = Fen::from_position(&pos, EnPassantMode::Legal).to_string();
                    let hits = Hits {
                        ordinals: answer.games.iter().map(|g| g.0).collect(),
                        nodes: None,
                        fen: Some(fen),
                        truncated: false,
                    };
                    return Ok((hits, answer.truncated, source, false));
                }
                Ok(_) | Err(CbhError::LimitExceeded) => {}
                Err(e) => return Err(e.into()),
            }
        }
        let limit = match self.reference_auto_limit.load(Ordering::Relaxed) {
            0 => AUTO_SEARCH_ENTRIES,
            n => n,
        };
        let large = self
            .database(database_id)?
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?
            .info()
            .entry_count
            > limit;
        if large && !search_anyway && !self.has_booster(database_id)? {
            let hits = Hits {
                ordinals: vec![],
                nodes: None,
                fen: None,
                truncated: false,
            };
            return Ok((hits, false, ReferenceSource::NotSearched, false));
        }
        let target = cbh_reader::position::Target::new(&pos);
        let result = self.position_search(
            database_id,
            &cbh_reader::pattern::Compiled::Exact(vec![target]),
            cancel,
            |_, _| {},
        )?;
        let (ordinals, nodes) = result.hits.into_iter().unzip();
        let hits = Hits {
            ordinals,
            nodes: Some(nodes),
            fen: None,
            truncated: false,
        };
        Ok((
            hits,
            result.truncated,
            ReferenceSource::Search,
            result.indexed,
        ))
    }

    #[doc(hidden)]
    pub fn set_reference_auto_limit(&self, entries: u64) {
        self.reference_auto_limit.store(entries, Ordering::Relaxed);
    }

    pub fn cancel_reference(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.reference_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Compare the indexed full position with a filtered cohort and one
    /// player's choices. The same Stop action cancels this work and a pending
    /// reference search. No registry mutex is held while reading headers.
    pub fn reference_statistics(
        &self,
        database_id: String,
        line: Line,
        request_id: String,
        filter: ReferenceFilter,
    ) -> Result<ReferenceStatistics, AppError> {
        if filter
            .elo_min
            .zip(filter.elo_max)
            .is_some_and(|(a, b)| a > b)
            || filter
                .year_min
                .zip(filter.year_max)
                .is_some_and(|(a, b)| a > b)
        {
            return Err(AppError::new(
                "bounds",
                "Reference filter minimum exceeds maximum",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .reference_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let result = (|| {
            let (pos, hashes, moves) = replay(&line)?;
            let target = *hashes
                .last()
                .ok_or_else(|| AppError::new("bounds", "Empty line"))?;
            let mut indexed = None;
            if let Some(open) = self.open_tree(&database_id)? {
                let mut open = open
                    .lock()
                    .map_err(|_| AppError::new("state", "Opening tree unavailable"))?;
                let OpenTree { tree, db } = &mut *open;
                match tree.query(db, &hashes, &moves, MAX_REFERENCE_HITS, MAX_FOLLOW, &cancel) {
                    Ok(answer) if answer.source != Source::None => indexed = Some(answer),
                    Ok(_) | Err(CbhError::LimitExceeded) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            let db = self.own_handle(&database_id)?;
            let (full, games, complete, full_complete) = if let Some(answer) = indexed {
                let exact = answer.source == Source::Tree || !answer.truncated;
                (answer.moves, answer.games, !answer.truncated, exact)
            } else {
                let hits = self
                    .lock()?
                    .references
                    .get(&database_id)
                    .cloned()
                    .ok_or_else(|| AppError::new("noSearch", "Search the position first"))?;
                let (stats, games) =
                    db.line_answer(&hits.ordinals, target, MAX_REFERENCE_HITS, &cancel)?;
                let exact = hits.ordinals.len() < MAX_REFERENCE_HITS;
                (stats, games, exact, exact)
            };
            let selected =
                db.reference_choice_stats(&games, pos.turn() == Color::Black, &filter, &cancel)?;
            if cancel.load(Ordering::Relaxed) {
                return Err(AppError::new("cancelled", "Reference statistics stopped"));
            }
            let full_games = tree::total(&full).games;
            let filtered_games = tree::total(&selected.filtered).games;
            let player_games = tree::total(&selected.player).games;
            let mut rows = Vec::new();
            for entry in full.iter().filter(|entry| entry.uci().is_some()) {
                let uci = entry.uci().unwrap_or_default();
                let count = |moves: &[tree::MoveStats]| {
                    moves
                        .iter()
                        .find(|m| m.code == entry.code)
                        .map_or(0, |m| m.games)
                };
                rows.push(ReferenceChoice {
                    san: san(&pos, &uci),
                    full: entry.games,
                    filtered: count(&selected.filtered),
                    player: count(&selected.player),
                });
            }
            Ok(ReferenceStatistics {
                request_id,
                full_games,
                filtered_games,
                player_games,
                scanned_games: selected.scanned,
                after_elo: selected.after_elo,
                after_year: selected.after_year,
                missing_elo: selected.missing_elo,
                missing_year: selected.missing_year,
                complete: complete && selected.scanned == games.len() as u64,
                full_complete,
                moves: rows,
            })
        })();
        let mut state = self.lock()?;
        if state
            .reference_cancels
            .get(&database_id)
            .is_some_and(|active| Arc::ptr_eq(active, &cancel))
        {
            state.reference_cancels.remove(&database_id);
        }
        result
    }

    /// Entries `start .. start + limit` of the reference search result.
    pub fn read_reference_page(
        &self,
        database_id: String,
        start: String,
        limit: usize,
        request_id: String,
    ) -> Result<PageResponse, AppError> {
        let hits = self
            .lock()?
            .references
            .get(&database_id)
            .cloned()
            .ok_or_else(|| AppError::new("noSearch", "No reference search for this database"))?;
        self.read_hits_page(&database_id, &hits, &start, limit, request_id)
    }

    /// Before the workbench writes to a database: what keeps its tree
    /// current. Problems only make the tree stale, so they are not errors.
    pub(crate) fn tree_before_change(
        &self,
        database_id: &str,
        replaced: Option<u64>,
    ) -> Option<PendingChange> {
        let path = self.tree_path(database_id).ok()?;
        let db = self.database(database_id).ok()?;
        let db = db.lock().ok()?;
        OpeningTree::before_change(&path, &db, replaced).ok()?
    }

    /// After games were appended: records every new entry in the tree's
    /// journal. True when the tree is current.
    pub(crate) fn tree_after_append(
        &self,
        database_id: &str,
        pending: Option<PendingChange>,
    ) -> bool {
        if let Ok(mut state) = self.lock() {
            state.trees.remove(database_id);
            state.references.remove(database_id);
        }
        let (Some(pending), Ok(db)) = (pending, self.database(database_id)) else {
            return false;
        };
        db.lock()
            .ok()
            .and_then(|db| pending.commit_appended(&db).ok())
            .unwrap_or(false)
    }

    /// After a write: records it in the tree's journal.
    pub(crate) fn tree_after_change(
        &self,
        database_id: &str,
        pending: Option<PendingChange>,
        changed: Option<u64>,
    ) {
        if let Ok(mut state) = self.lock() {
            state.trees.remove(database_id);
            state.references.remove(database_id);
        }
        let (Some(pending), Ok(db)) = (pending, self.database(database_id)) else {
            return;
        };
        if let Ok(db) = db.lock() {
            let _ = pending.commit(&db, changed);
        }
    }
}
