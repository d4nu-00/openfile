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
        mut progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        let db = self.database(&database_id)?;
        let total = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?
            .info()
            .entry_count;
        let cancel = Arc::new(AtomicBool::new(false));
        // A new search cancels the previous one on this database.
        if let Some(old) = self
            .lock()?
            .search_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let result = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?
            .search(&filter, MAX_HITS, &cancel, |done| progress(done, total))?;
        let mut state = self.lock()?;
        if state
            .search_cancels
            .get(&database_id)
            .is_some_and(|c| Arc::ptr_eq(c, &cancel))
        {
            state.search_cancels.remove(&database_id);
        }
        let response = SearchResponse {
            request_id,
            hits: result.ordinals.len().to_string(),
            scanned: result.scanned.to_string(),
            truncated: result.truncated,
            cancelled: result.cancelled,
            indexed: false,
        };
        if !result.cancelled && state.databases.contains_key(&database_id) {
            state.searches.insert(
                database_id,
                Arc::new(Hits {
                    ordinals: result.ordinals,
                    nodes: None,
                    fen: None,
                }),
            );
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
        let reader = self.own_handle(&database_id)?;
        let built =
            cbh_reader::position::PositionIndex::build(&reader, &path, &cancel, &mut progress)?;
        self.lock()?.search_cancels.remove(&database_id);
        Ok(built)
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

    fn run_position_search(
        &self,
        database_id: String,
        query: &cbh_reader::pattern::Compiled,
        request_id: String,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResponse, AppError> {
        self.database(&database_id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .search_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let result = self.position_search(&database_id, query, &cancel, progress);
        let mut state = self.lock()?;
        if state
            .search_cancels
            .get(&database_id)
            .is_some_and(|c| Arc::ptr_eq(c, &cancel))
        {
            state.search_cancels.remove(&database_id);
        }
        let result = result?;
        let response = SearchResponse {
            request_id,
            hits: result.hits.len().to_string(),
            scanned: result.scanned.to_string(),
            truncated: result.truncated,
            cancelled: result.cancelled,
            indexed: result.indexed,
        };
        if !result.cancelled && state.databases.contains_key(&database_id) {
            let (ordinals, nodes) = result.hits.into_iter().unzip();
            state.searches.insert(
                database_id,
                Arc::new(Hits {
                    ordinals,
                    nodes: Some(nodes),
                    fen: None,
                }),
            );
        }
        Ok(response)
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
        self.lock()?.searches.insert(
            database_id,
            Arc::new(Hits {
                ordinals,
                nodes: None,
                fen: None,
            }),
        );
        Ok(response)
    }

    pub fn clear_search(&self, database_id: String) -> Result<(), AppError> {
        self.lock()?.searches.remove(&database_id);
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
