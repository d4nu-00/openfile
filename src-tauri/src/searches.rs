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
    ) -> Result<SearchResponse, AppError> {
        let db = self.database(&database_id)?;
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
            .search(&filter, MAX_HITS, &cancel, |_| {})?;
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
                }),
            );
        }
        Ok(response)
    }

    /// Where the search booster of the database at `path` lives.
    fn booster_path(&self, database_id: &str) -> Result<std::path::PathBuf, AppError> {
        let dir = self
            .index_dir
            .lock()
            .map_err(|_| AppError::new("state", "Registry unavailable"))?
            .clone()
            .ok_or_else(|| AppError::new("noIndexFolder", "No folder for search boosters"))?;
        let path = self
            .lock()?
            .paths
            .get(database_id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        // FNV-1a of the path: one booster per database file.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in canonical.to_string_lossy().bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
        Ok(dir.join(format!("{h:016x}.cwpi")))
    }

    /// A separate read-only handle on an open database.
    fn own_handle(&self, database_id: &str) -> Result<cbh_reader::Database, AppError> {
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
    ) -> Result<SearchResponse, AppError> {
        let target = cbh_reader::position::Target::from_fen(&fen)?;
        let booster = self.booster_path(&database_id).ok();
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
        let result = {
            let mut db = self.own_handle(&database_id)?;
            let mut index = match &booster {
                Some(path) => cbh_reader::position::PositionIndex::open(path, &db)?,
                None => None,
            };
            db.find_position(
                &target,
                index.as_mut(),
                MAX_POSITION_HITS,
                &cancel,
                |_, _| {},
            )?
        };
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
                }),
            );
        }
        Ok(response)
    }

    pub fn cancel_search(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.search_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
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
        let start = decimal(&start)?;
        let hits = self
            .lock()?
            .searches
            .get(&database_id)
            .cloned()
            .ok_or_else(|| AppError::new("noSearch", "No search result for this database"))?;
        if start == 0 || limit == 0 || limit > 500 || start > hits.ordinals.len() as u64 + 1 {
            return Err(AppError::new("bounds", "Search page is out of range"));
        }
        let from = (start - 1) as usize;
        let to = (from + limit).min(hits.ordinals.len());
        let db = self.database(&database_id)?;
        let mut db = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?;
        let mut entries = Vec::with_capacity(to - from);
        for i in from..to {
            // Entries are read one at a time; a search result is sparse.
            let mut page = db.page(hits.ordinals[i], 1)?;
            entries.extend(page.entries.drain(..).map(|e| {
                let mut dto: crate::commands::EntryDto = e.into();
                dto.found_node = hits.nodes.as_ref().map(|n| n[i]);
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
