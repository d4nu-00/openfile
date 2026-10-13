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

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub request_id: String,
    pub hits: String,
    pub scanned: String,
    pub truncated: bool,
    pub cancelled: bool,
}

impl Registry {
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
        if !result.cancelled && state.databases.contains_key(&database_id) {
            state
                .searches
                .insert(database_id, Arc::new(result.ordinals.clone()));
        }
        Ok(SearchResponse {
            request_id,
            hits: result.ordinals.len().to_string(),
            scanned: result.scanned.to_string(),
            truncated: result.truncated,
            cancelled: result.cancelled,
        })
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
        if start == 0 || limit == 0 || limit > 500 || start > hits.len() as u64 + 1 {
            return Err(AppError::new("bounds", "Search page is out of range"));
        }
        let from = (start - 1) as usize;
        let to = (from + limit).min(hits.len());
        let db = self.database(&database_id)?;
        let mut db = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?;
        let mut entries = Vec::with_capacity(to - from);
        for &ordinal in &hits[from..to] {
            // Entries are read one at a time; a search result is sparse.
            let mut page = db.page(ordinal, 1)?;
            entries.extend(page.entries.drain(..).map(Into::into));
        }
        Ok(PageResponse {
            request_id,
            entries,
            next: (to < hits.len()).then(|| (to + 1).to_string()),
        })
    }
}
