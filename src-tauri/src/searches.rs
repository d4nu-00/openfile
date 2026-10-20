//! Whole-database game searches. One result list is kept per open database;
//! the game list pages through it (see docs/search.md).
use crate::{
    commands::{AppError, PageResponse},
    registry::{Registry, decimal},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Hits kept per search; the response says when a search stopped there.
const MAX_HITS: usize = 2_000_000;
/// Position searches replay games, so they keep fewer hits.
const MAX_POSITION_HITS: usize = 100_000;

/// A search result: entry numbers, and for a position search the node at
/// which each game first reaches the position.
pub struct Hits {
    pub ordinals: Vec<u64>,
    pub nodes: Option<Vec<u32>>,
    /// The position searched, when nodes are found page by page instead.
    pub fen: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoosterProgress {
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub request_id: String,
    pub hits: String,
    pub scanned: String,
    pub truncated: bool,
    pub cancelled: bool,
    /// A position search used the search booster.
    pub indexed: bool,
}

impl Registry {
    /// Sets the folder for search boosters.
    pub fn set_index_dir(&self, dir: std::path::PathBuf) {
        if let Ok(mut d) = self.index_dir.lock() {
            *d = Some(dir);
        }
    }
    pub fn search_games(
        &self,
        database_id: String,
        filter: cbh_reader::search::GameFilter,
        request_id: String,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        self.search_games_scoped(database_id, filter, SearchScope::All, request_id, progress)
    }

    pub fn search_games_scoped(
        &self,
        database_id: String,
        filter: cbh_reader::search::GameFilter,
        scope: SearchScope,
        request_id: String,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        let job = self.begin_search(&database_id, scope)?;
        let selection = job.selection(scope);
        let result = (|| {
            let mut db = self.own_handle(&database_id)?;
            let limit = if filter.material.is_some() {
                MAX_POSITION_HITS
            } else {
                MAX_HITS
            };
            let result =
                db.search_selected(&filter, selection.as_deref(), limit, &job.cancel, progress)?;
            Ok(Outcome {
                hits: Hits {
                    ordinals: result.ordinals,
                    nodes: result.nodes,
                    fen: None,
                    truncated: false,
                },
                scanned: result.scanned,
                truncated: result.truncated,
                indexed: false,
                cancelled: result.cancelled,
            })
        })();
        self.finish_search(database_id, request_id, scope, job, result)
    }

    fn begin_search(&self, database_id: &str, scope: SearchScope) -> Result<SearchJob, AppError> {
        let mut state = self.lock()?;
        if !state.databases.contains_key(database_id) {
            return Err(AppError::new(
                "invalidHandle",
                "Database is closed or unknown",
            ));
        }
        let base = state.searches.get(database_id).cloned();
        if scope != SearchScope::All && base.is_none() {
            return Err(AppError::new(
                "noSearch",
                "No current results; search all games first",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = state
            .search_cancels
            .insert(database_id.into(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        Ok(SearchJob { cancel, base })
    }

    fn finish_search(
        &self,
        database_id: String,
        request_id: String,
        scope: SearchScope,
        job: SearchJob,
        result: Result<Outcome, AppError>,
    ) -> Result<SearchResponse, AppError> {
        let mut state = self.lock()?;
        let owned = state
            .search_cancels
            .get(&database_id)
            .is_some_and(|c| Arc::ptr_eq(c, &job.cancel));
        if owned {
            state.search_cancels.remove(&database_id);
        }
        let result = result?;
        let same_base = match (&job.base, state.searches.get(&database_id)) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        let cancelled = result.cancelled
            || job.cancel.load(Ordering::Relaxed)
            || !owned
            || !same_base
            || !state.databases.contains_key(&database_id);
        let (mut hits, overflow) = combine(job.base.as_deref(), result.hits, scope, MAX_HITS);
        hits.truncated = result.truncated
            || overflow
            || (scope != SearchScope::All && job.base.as_ref().is_some_and(|h| h.truncated));
        let response = SearchResponse {
            request_id,
            hits: hits.ordinals.len().to_string(),
            scanned: result.scanned.to_string(),
            truncated: hits.truncated,
            cancelled,
            indexed: result.indexed,
        };
        if !cancelled {
            state.searches.insert(database_id, Arc::new(hits));
        }
        Ok(response)
    }

    /// Where the search booster of a database lives.
    fn booster_path(&self, database_id: &str) -> Result<std::path::PathBuf, AppError> {
        self.data_file(database_id, None, "cwpi")
    }

    /// The app's own file for a database: in the search booster folder, or
    /// in `sibling` beside it, named by a hash of the database's path.
    pub(crate) fn data_file(
        &self,
        database_id: &str,
        sibling: Option<&str>,
        extension: &str,
    ) -> Result<std::path::PathBuf, AppError> {
        let mut dir = self
            .index_dir
            .lock()
            .map_err(|_| AppError::new("state", "Registry unavailable"))?
            .clone()
            .ok_or_else(|| AppError::new("noIndexFolder", "No folder for search boosters"))?;
        if let Some(sibling) = sibling {
            dir = dir.with_file_name(sibling);
        }
        let path = self
            .lock()?
            .paths
            .get(database_id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        // FNV-1a of the path: one file per database.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in canonical.to_string_lossy().bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
        Ok(dir.join(format!("{h:016x}.{extension}")))
    }

    /// A separate read-only handle on an open database.
    pub(crate) fn own_handle(&self, database_id: &str) -> Result<cbh_reader::Database, AppError> {
        let path = self
            .lock()?
            .paths
            .get(database_id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        Ok(cbh_reader::Database::open(
            &path,
            cbh_reader::ReadLimits::default(),
        )?)
    }

    /// Whether the database has a current search booster.
    pub fn has_booster(&self, database_id: &str) -> Result<bool, AppError> {
        let Ok(path) = self.booster_path(database_id) else {
            return Ok(false);
        };
        let db = self.database(database_id)?;
        let db = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?;
        Ok(cbh_reader::position::PositionIndex::open(&path, &db)?.is_some())
    }

    /// Builds the search booster; `progress` gets (done, total) entries.
    pub fn create_search_booster(
        &self,
        database_id: String,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<bool, AppError> {
        let path = self.booster_path(&database_id)?;
        let db = self.database(&database_id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .search_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        drop(db);
        // Long jobs read through their own handle, so the list stays usable.
        let result = (|| {
            let reader = self.own_handle(&database_id)?;
            Ok(cbh_reader::position::PositionIndex::build(
                &reader,
                &path,
                &cancel,
                &mut progress,
            )?)
        })();
        let mut state = self.lock()?;
        if state
            .search_cancels
            .get(&database_id)
            .is_some_and(|current| Arc::ptr_eq(current, &cancel))
        {
            state.search_cancels.remove(&database_id);
        }
        result
    }

    pub fn delete_search_booster(&self, database_id: String) -> Result<(), AppError> {
        let path = self.booster_path(&database_id)?;
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(AppError::new("io", e.to_string()))
            }
            _ => Ok(()),
        }
    }

    /// Finds games that reach the position `fen` (board and side to move),
    /// with the search booster when it is current.
    pub fn find_position(
        &self,
        database_id: String,
        fen: String,
        request_id: String,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        let target = cbh_reader::position::Target::from_fen(&fen)?;
        let query = cbh_reader::pattern::Compiled::Exact(vec![target]);
        self.run_position_search(database_id, &query, request_id, progress)
    }

    /// Runs the search mask's position tab: an exact position or a pattern.
    pub fn search_position(
        &self,
        database_id: String,
        query: cbh_reader::pattern::PositionQuery,
        request_id: String,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        let query = query
            .compile()
            .map_err(|e| AppError::new("invalidQuery", e.to_string()))?;
        self.run_position_search(database_id, &query, request_id, progress)
    }

    pub fn search_position_scoped(
        &self,
        database_id: String,
        query: cbh_reader::pattern::PositionQuery,
        scope: SearchScope,
        request_id: String,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        let query = query
            .compile()
            .map_err(|e| AppError::new("invalidQuery", e.to_string()))?;
        self.run_position_search_scoped(database_id, &query, scope, request_id, progress)
    }

    fn run_position_search(
        &self,
        database_id: String,
        query: &cbh_reader::pattern::Compiled,
        request_id: String,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        self.run_position_search_scoped(database_id, query, SearchScope::All, request_id, progress)
    }

    fn run_position_search_scoped(
        &self,
        database_id: String,
        query: &cbh_reader::pattern::Compiled,
        scope: SearchScope,
        request_id: String,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        let job = self.begin_search(&database_id, scope)?;
        let selection = job.selection(scope);
        let result = (|| {
            let booster = self.booster_path(&database_id).ok();
            let mut db = self.own_handle(&database_id)?;
            let mut index = booster
                .map(|p| cbh_reader::position::PositionIndex::open(&p, &db))
                .transpose()?
                .flatten();
            let r = db.find_query_selected(
                query,
                index.as_mut(),
                selection.as_deref(),
                MAX_POSITION_HITS,
                &job.cancel,
                progress,
            )?;
            let (ordinals, nodes) = r.hits.into_iter().unzip();
            Ok(Outcome {
                hits: Hits {
                    ordinals,
                    nodes: Some(nodes),
                    fen: None,
                    truncated: false,
                },
                scanned: r.scanned,
                truncated: r.truncated,
                cancelled: r.cancelled,
                indexed: r.indexed,
            })
        })();
        self.finish_search(database_id, request_id, scope, job, result)
    }

    /// Finds games matching `query` through a separate read handle, with
    /// the search booster when it is current and the query is exact.
    pub(crate) fn position_search(
        &self,
        database_id: &str,
        query: &cbh_reader::pattern::Compiled,
        cancel: &AtomicBool,
        progress: impl FnMut(u64, u64),
    ) -> Result<cbh_reader::position::PositionResult, AppError> {
        let booster = self.booster_path(database_id).ok();
        let mut db = self.own_handle(database_id)?;
        let mut index = match &booster {
            Some(path) => cbh_reader::position::PositionIndex::open(path, &db)?,
            None => None,
        };
        Ok(db.find_query(query, index.as_mut(), MAX_POSITION_HITS, cancel, progress)?)
    }

    pub fn cancel_search(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.search_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Shows the given entries as the database's search result, in the
    /// order given (at most 100,000).
    pub fn show_entries(
        &self,
        database_id: String,
        ordinals: Vec<String>,
        request_id: String,
    ) -> Result<SearchResponse, AppError> {
        if ordinals.len() > 100_000 {
            return Err(AppError::new("bounds", "Too many entries"));
        }
        let count = self
            .database(&database_id)?
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?
            .info()
            .entry_count;
        let ordinals = ordinals
            .iter()
            .map(|o| match o.parse::<u64>() {
                Ok(n) if (1..=count).contains(&n) => Ok(n),
                _ => Err(AppError::new("bounds", format!("No entry {o}"))),
            })
            .collect::<Result<Vec<u64>, _>>()?;
        let response = SearchResponse {
            request_id,
            hits: ordinals.len().to_string(),
            scanned: ordinals.len().to_string(),
            truncated: false,
            cancelled: false,
            indexed: false,
        };
        self.cancel_search(database_id.clone())?;
        self.lock()?.searches.insert(
            database_id,
            Arc::new(Hits {
                ordinals,
                nodes: None,
                fen: None,
                truncated: false,
            }),
        );
        Ok(response)
    }

    pub fn clear_search(&self, database_id: String) -> Result<(), AppError> {
        let mut state = self.lock()?;
        if let Some(cancel) = state.search_cancels.remove(&database_id) {
            cancel.store(true, Ordering::Relaxed);
        }
        state.searches.remove(&database_id);
        Ok(())
    }

    /// Entries `start .. start + limit` of the current search result, where
    /// `start` counts hits from 1.
    pub fn read_search_page(
        &self,
        database_id: String,
        start: String,
        limit: usize,
        request_id: String,
    ) -> Result<PageResponse, AppError> {
        let hits = self
            .lock()?
            .searches
            .get(&database_id)
            .cloned()
            .ok_or_else(|| AppError::new("noSearch", "No search result for this database"))?;
        self.read_hits_page(&database_id, &hits, &start, limit, request_id)
    }

    /// Entries `start .. start + limit` of `hits`, where `start` counts hits
    /// from 1.
    pub(crate) fn read_hits_page(
        &self,
        database_id: &str,
        hits: &Hits,
        start: &str,
        limit: usize,
        request_id: String,
    ) -> Result<PageResponse, AppError> {
        let start = decimal(start)?;
        if start == 0 || limit == 0 || limit > 500 || start > hits.ordinals.len() as u64 + 1 {
            return Err(AppError::new("bounds", "Search page is out of range"));
        }
        let from = (start - 1) as usize;
        let to = (from + limit).min(hits.ordinals.len());
        let target = match &hits.fen {
            Some(fen) => Some(cbh_reader::position::Target::from_fen(fen)?),
            None => None,
        };
        let db = self.database(database_id)?;
        let mut db = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?;
        let mut entries = Vec::with_capacity(to - from);
        for i in from..to {
            // Entries are read one at a time; a search result is sparse.
            let mut page = db.page(hits.ordinals[i], 1)?;
            let node = match (&hits.nodes, &target) {
                (Some(nodes), Some(target)) if nodes[i] == u32::MAX => {
                    db.find_node(hits.ordinals[i], target)?
                }
                (Some(nodes), _) => Some(nodes[i]),
                (None, Some(target)) => db.find_node(hits.ordinals[i], target)?,
                (None, None) => None,
            };
            entries.extend(page.entries.drain(..).map(|e| {
                let mut dto: crate::commands::EntryDto = e.into();
                dto.found_node = node;
                dto
            }));
        }
        Ok(PageResponse {
            request_id,
            entries,
            next: (to < hits.ordinals.len()).then(|| (to + 1).to_string()),
        })
    }
}

/// The result combination applies to entry identities, independent of list sorting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchScope {
    #[default]
    All,
    Within,
    Add,
}

struct SearchJob {
    cancel: Arc<AtomicBool>,
    base: Option<Arc<Hits>>,
}
impl SearchJob {
    fn selection(&self, scope: SearchScope) -> Option<Vec<u64>> {
        (scope == SearchScope::Within).then(|| {
            let mut picked = self
                .base
                .as_ref()
                .map(|h| h.ordinals.clone())
                .unwrap_or_default();
            picked.sort_unstable();
            picked.dedup();
            picked
        })
    }
}
struct Outcome {
    hits: Hits,
    scanned: u64,
    truncated: bool,
    cancelled: bool,
    indexed: bool,
}

/// Existing entries win ties on Add; a refinement's new position wins when present.
/// Keep all existing entries at capacity, then the earliest new entries that fit.
fn combine(base: Option<&Hits>, new: Hits, scope: SearchScope, limit: usize) -> (Hits, bool) {
    let pairs = |h: &Hits| {
        h.ordinals
            .iter()
            .enumerate()
            .map(|(i, &o)| {
                (
                    o,
                    h.nodes
                        .as_ref()
                        .map_or(if h.fen.is_some() { u32::MAX } else { 0 }, |n| n[i]),
                )
            })
            .collect::<Vec<_>>()
    };
    let mut old = base.map(pairs).unwrap_or_default();
    old.sort_by_key(|p| p.0);
    old.dedup_by_key(|p| p.0);
    let mut fresh = pairs(&new);
    fresh.sort_by_key(|p| p.0);
    fresh.dedup_by_key(|p| p.0);
    let nodes = new.nodes.is_some()
        || (scope != SearchScope::All
            && base.is_some_and(|h| h.nodes.is_some() || h.fen.is_some()));
    let mut overflow = false;
    let mut kept = match scope {
        SearchScope::All => fresh,
        SearchScope::Within => {
            if new.nodes.is_none() {
                for p in &mut fresh {
                    if let Ok(i) = old.binary_search_by_key(&p.0, |p| p.0) {
                        p.1 = old[i].1;
                    }
                }
            }
            fresh
        }
        SearchScope::Add => {
            fresh.retain(|p| old.binary_search_by_key(&p.0, |p| p.0).is_err());
            let capacity = limit.saturating_sub(old.len());
            overflow = fresh.len() > capacity;
            fresh.truncate(capacity);
            old.extend(fresh);
            old.sort_by_key(|p| p.0);
            old
        }
    };
    if kept.len() > limit {
        kept.truncate(limit);
        overflow = true;
    }
    let (ordinals, found) = kept.into_iter().unzip();
    (
        Hits {
            ordinals,
            nodes: nodes.then_some(found),
            fen: if scope == SearchScope::All {
                new.fen
            } else {
                base.and_then(|h| h.fen.clone())
            },
            truncated: false,
        },
        overflow,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hits(ordinals: &[u64], nodes: Option<Vec<u32>>) -> Hits {
        Hits {
            ordinals: ordinals.to_vec(),
            nodes,
            fen: None,
            truncated: false,
        }
    }
    #[test]
    fn union_at_capacity_retains_old_and_earliest_unique_new_nodes() {
        let old = hits(&[8, 9], Some(vec![80, 90]));
        let new = hits(&[7, 1, 7, 2], Some(vec![70, 10, 71, 20]));
        let (kept, full) = combine(Some(&old), new, SearchScope::Add, 4);
        assert!(full);
        assert_eq!(kept.ordinals, [1, 2, 8, 9]);
        assert_eq!(kept.nodes, Some(vec![10, 20, 80, 90]));
        let (kept, full) = combine(Some(&old), hits(&[9, 8], None), SearchScope::Within, 4);
        assert!(!full);
        assert_eq!(kept.nodes, Some(vec![80, 90]));
    }
}
