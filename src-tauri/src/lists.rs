//! The game list: the entries of an open database or of its search result,
//! in entry order or arranged by a column. The frontend reads it a window of
//! rows at a time (see docs/game-list.md).
use crate::{
    commands::{AppError, EntryDto, PageResponse},
    registry::{Registry, decimal},
    searches::Hits,
};
use cbh_reader::sort::{SortColumn, SortOptions, SortOrder, SortStage};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

/// Rows read at most per request.
pub const MAX_ROWS: usize = 200;
/// Sort order files kept in the app's data folder, across databases; the
/// least recently built are deleted first.
const MAX_SORT_FILES: usize = 12;

/// The column a database's list is sorted by.
pub struct ListSort {
    pub column: SortColumn,
    pub descending: bool,
    order: Mutex<SortOrder>,
    /// The current search result in this order, and the result it came from.
    arranged: Mutex<Option<(Weak<Hits>, Arc<Hits>)>>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortProgress {
    pub stage: SortStage,
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortResponse {
    /// Rows in the list now.
    pub rows: String,
    pub cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<SortColumn>,
    pub descending: bool,
}

/// What the list shows.
enum Rows {
    /// Entries 1 ..= n in entry order.
    All(u64),
    Hits(Arc<Hits>),
    Sorted(Arc<ListSort>),
}

impl Registry {
    fn sort_path(
        &self,
        database_id: &str,
        column: SortColumn,
    ) -> Result<std::path::PathBuf, AppError> {
        self.data_file(
            database_id,
            Some("sort-orders"),
            &format!("{}.cwsort", column.name()),
        )
    }

    /// Sorts the list by `column`, building its sort order first when there
    /// is no current one; `None` returns to entry order.
    pub fn sort_list(
        &self,
        database_id: String,
        column: Option<SortColumn>,
        descending: bool,
        mut progress: impl FnMut(SortStage, u64, u64),
    ) -> Result<SortResponse, AppError> {
        let Some(column) = column else {
            self.lock()?.sorts.remove(&database_id);
            return Ok(SortResponse {
                rows: self.row_count(&database_id)?.to_string(),
                cancelled: false,
                column: None,
                descending: false,
            });
        };
        let path = self.sort_path(&database_id, column)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .sort_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let finish = |registry: &Self| {
            if let Ok(mut state) = registry.lock()
                && state
                    .sort_cancels
                    .get(&database_id)
                    .is_some_and(|c| Arc::ptr_eq(c, &cancel))
            {
                state.sort_cancels.remove(&database_id);
            }
        };
        let reader = self.own_handle(&database_id)?;
        let order = match SortOrder::open(&path, &reader, column)? {
            Some(order) => order,
            None => {
                let built = SortOrder::build(
                    &reader,
                    column,
                    &path,
                    &SortOptions::default(),
                    &cancel,
                    &mut progress,
                );
                finish(self);
                if !built? {
                    let current = self.lock()?.sorts.get(&database_id).cloned();
                    return Ok(SortResponse {
                        rows: self.row_count(&database_id)?.to_string(),
                        cancelled: true,
                        column: current.as_ref().map(|s| s.column),
                        descending: current.is_some_and(|s| s.descending),
                    });
                }
                evict_sort_files(&path);
                SortOrder::open(&path, &reader, column)?.ok_or_else(|| {
                    AppError::new("sourceChanged", "The database changed while sorting")
                })?
            }
        };
        finish(self);
        {
            let mut state = self.lock()?;
            if !state.databases.contains_key(&database_id) {
                return Err(AppError::new(
                    "invalidHandle",
                    "Database is closed or unknown",
                ));
            }
            state.sorts.insert(
                database_id.clone(),
                Arc::new(ListSort {
                    column,
                    descending,
                    order: Mutex::new(order),
                    arranged: Mutex::new(None),
                }),
            );
        }
        Ok(SortResponse {
            rows: self.row_count(&database_id)?.to_string(),
            cancelled: false,
            column: Some(column),
            descending,
        })
    }

    pub fn cancel_sort(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.sort_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Forgets the sort of a database whose files changed.
    pub(crate) fn drop_sort(&self, database_id: &str) {
        if let Ok(mut state) = self.lock() {
            state.sorts.remove(database_id);
        }
    }

    fn rows(&self, database_id: &str) -> Result<Rows, AppError> {
        let (hits, sort, db) = {
            let state = self.lock()?;
            (
                state.searches.get(database_id).cloned(),
                state.sorts.get(database_id).cloned(),
                state.databases.get(database_id).cloned(),
            )
        };
        let db =
            db.ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        Ok(match (hits, sort) {
            (None, None) => Rows::All(
                db.lock()
                    .map_err(|_| AppError::new("state", "Database unavailable"))?
                    .info()
                    .entry_count,
            ),
            (Some(hits), None) => Rows::Hits(hits),
            (None, Some(sort)) => Rows::Sorted(sort),
            (Some(hits), Some(sort)) => Rows::Hits(arranged(&sort, &hits)?),
        })
    }

    fn row_count(&self, database_id: &str) -> Result<u64, AppError> {
        Ok(match self.rows(database_id)? {
            Rows::All(n) => n,
            Rows::Hits(hits) => hits.ordinals.len() as u64,
            Rows::Sorted(sort) => sort
                .order
                .lock()
                .map_err(|_| AppError::new("state", "Sort order unavailable"))?
                .len(),
        })
    }

    /// Rows `start .. start + limit` of the list, counted from 1.
    pub fn read_rows(
        &self,
        database_id: String,
        start: String,
        limit: usize,
        request_id: String,
    ) -> Result<PageResponse, AppError> {
        let start = decimal(&start)?;
        if start == 0 || limit == 0 || limit > MAX_ROWS {
            return Err(AppError::new("bounds", "Row range is out of range"));
        }
        match self.rows(&database_id)? {
            Rows::All(_) => self.read_page(database_id, start.to_string(), limit, request_id),
            Rows::Hits(hits) => {
                self.read_hits_page(&database_id, &hits, &start.to_string(), limit, request_id)
            }
            Rows::Sorted(sort) => {
                let (ordinals, total) = {
                    let mut order = sort
                        .order
                        .lock()
                        .map_err(|_| AppError::new("state", "Sort order unavailable"))?;
                    (order.read(start - 1, limit, sort.descending)?, order.len())
                };
                if ordinals.is_empty() {
                    return Err(AppError::new("bounds", "Row range is out of range"));
                }
                let entries = self.entries_of(&database_id, &ordinals)?;
                let end = start - 1 + ordinals.len() as u64;
                Ok(PageResponse {
                    request_id,
                    entries,
                    next: (end < total).then(|| (end + 1).to_string()),
                })
            }
        }
    }

    /// The row (from 1) that shows entry `ordinal`, if the list has it.
    pub fn find_row(
        &self,
        database_id: String,
        ordinal: String,
    ) -> Result<Option<String>, AppError> {
        let ordinal = decimal(&ordinal)?;
        let row = match self.rows(&database_id)? {
            Rows::All(n) => (ordinal >= 1 && ordinal <= n).then_some(ordinal),
            Rows::Hits(hits) => hits
                .ordinals
                .iter()
                .position(|&o| o == ordinal)
                .map(|i| i as u64 + 1),
            Rows::Sorted(sort) => sort
                .order
                .lock()
                .map_err(|_| AppError::new("state", "Sort order unavailable"))?
                .place_of(ordinal, sort.descending)?
                .map(|i| i + 1),
        };
        Ok(row.map(|r| r.to_string()))
    }

    /// Summaries of the given entries, in that order.
    pub(crate) fn entries_of(
        &self,
        database_id: &str,
        ordinals: &[u64],
    ) -> Result<Vec<EntryDto>, AppError> {
        let db = self.database(database_id)?;
        let mut db = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?;
        let mut entries = Vec::with_capacity(ordinals.len());
        for &o in ordinals {
            entries.extend(db.page(o, 1)?.entries.into_iter().map(EntryDto::from));
        }
        Ok(entries)
    }
}

/// The search result `hits` in the order of `sort`, arranged once per result.
fn arranged(sort: &ListSort, hits: &Arc<Hits>) -> Result<Arc<Hits>, AppError> {
    let mut cached = sort
        .arranged
        .lock()
        .map_err(|_| AppError::new("state", "Sort order unavailable"))?;
    if let Some((source, result)) = cached.as_ref()
        && source.upgrade().is_some_and(|s| Arc::ptr_eq(&s, hits))
    {
        return Ok(result.clone());
    }
    let order = sort
        .order
        .lock()
        .map_err(|_| AppError::new("state", "Sort order unavailable"))?
        .arrange(&hits.ordinals, sort.descending, &AtomicBool::new(false))?
        .unwrap_or_default();
    let result = Arc::new(Hits {
        ordinals: order.iter().map(|&i| hits.ordinals[i as usize]).collect(),
        nodes: hits
            .nodes
            .as_ref()
            .map(|nodes| order.iter().map(|&i| nodes[i as usize]).collect()),
        fen: hits.fen.clone(),
    });
    *cached = Some((Arc::downgrade(hits), result.clone()));
    Ok(result)
}

/// Keeps at most `MAX_SORT_FILES` sort orders beside `kept`, deleting the
/// oldest.
fn evict_sort_files(kept: &std::path::Path) {
    let Some(dir) = kept.parent() else { return };
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cwsort") && p != kept)
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    if files.len() < MAX_SORT_FILES {
        return;
    }
    files.sort();
    for (_, p) in files.iter().take(files.len() + 1 - MAX_SORT_FILES) {
        let _ = std::fs::remove_file(p);
    }
}
