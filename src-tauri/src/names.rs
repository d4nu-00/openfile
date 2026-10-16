//! Players and tournaments panes and crosstables (see docs/game-list.md).
use crate::{
    commands::AppError,
    registry::{Registry, decimal},
};
use cbh_reader::name_list::{NameKind, NameList};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Names read at most per request.
pub const MAX_NAMES: usize = 200;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameProgress {
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameListResponse {
    /// Names in the list; absent when cancelled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameRowDto {
    pub id: u32,
    pub name: Option<String>,
    /// Tournaments: the site.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    pub games: u32,
    pub first_year: u16,
    pub last_year: u16,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamePage {
    pub request_id: String,
    pub rows: Vec<NameRowDto>,
}

impl Registry {
    fn name_list_handle(
        &self,
        database_id: &str,
        kind: NameKind,
    ) -> Result<Arc<Mutex<NameList>>, AppError> {
        self.lock()?
            .name_lists
            .get(&(database_id.to_string(), kind))
            .cloned()
            .ok_or_else(|| AppError::new("noNameList", "The list has not been prepared"))
    }

    /// Opens the player or tournament list, building it first when there is
    /// no current one.
    pub fn name_list(
        &self,
        database_id: String,
        kind: NameKind,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<NameListResponse, AppError> {
        let path = self.data_file(
            &database_id,
            Some("name-lists"),
            &format!("{}.cwnames", kind.name()),
        )?;
        let reader = self.own_handle(&database_id)?;
        let list = match NameList::open(&path, &reader, kind)? {
            Some(list) => list,
            None => {
                let cancel = Arc::new(AtomicBool::new(false));
                if let Some(old) = self
                    .lock()?
                    .name_cancels
                    .insert(database_id.clone(), cancel.clone())
                {
                    old.store(true, Ordering::Relaxed);
                }
                let workers = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
                let built = NameList::build(&reader, kind, &path, workers, &cancel, &mut progress);
                {
                    let mut state = self.lock()?;
                    if state
                        .name_cancels
                        .get(&database_id)
                        .is_some_and(|c| Arc::ptr_eq(c, &cancel))
                    {
                        state.name_cancels.remove(&database_id);
                    }
                }
                if !built? {
                    return Ok(NameListResponse { rows: None });
                }
                NameList::open(&path, &reader, kind)?.ok_or_else(|| {
                    AppError::new("sourceChanged", "The database changed while listing")
                })?
            }
        };
        let rows = list.len();
        let mut state = self.lock()?;
        if !state.databases.contains_key(&database_id) {
            return Err(AppError::new(
                "invalidHandle",
                "Database is closed or unknown",
            ));
        }
        state
            .name_lists
            .insert((database_id, kind), Arc::new(Mutex::new(list)));
        Ok(NameListResponse {
            rows: Some(rows.to_string()),
        })
    }

    pub fn cancel_name_list(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.name_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Forgets the lists of a database whose files changed.
    pub(crate) fn drop_name_lists(&self, database_id: &str) {
        if let Ok(mut state) = self.lock() {
            state.name_lists.retain(|(id, _), _| id != database_id);
            state.player_games.remove(database_id);
            state.preparations.remove(database_id);
        }
    }

    /// Names `start .. start + limit` of a list, counted from 1.
    pub fn read_names(
        &self,
        database_id: String,
        kind: NameKind,
        start: String,
        limit: usize,
        request_id: String,
    ) -> Result<NamePage, AppError> {
        let start = decimal(&start)?;
        if start == 0 || limit == 0 || limit > MAX_NAMES {
            return Err(AppError::new("bounds", "Name range is out of range"));
        }
        let list = self.name_list_handle(&database_id, kind)?;
        let rows = list
            .lock()
            .map_err(|_| AppError::new("state", "List unavailable"))?
            .read(start - 1, limit)?;
        let db = self.database(&database_id)?;
        let mut db = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let (name, site) = match kind {
                NameKind::Players => (db.player_name(r.id)?, None),
                NameKind::Tournaments => db.tournament_name(r.id)?,
            };
            out.push(NameRowDto {
                id: r.id,
                name,
                site,
                games: r.games,
                first_year: r.first_year,
                last_year: r.last_year,
            });
        }
        Ok(NamePage {
            request_id,
            rows: out,
        })
    }

    /// The row (from 1) of the first name at or after `prefix`.
    pub fn find_name(
        &self,
        database_id: String,
        kind: NameKind,
        prefix: String,
    ) -> Result<String, AppError> {
        let list = self.name_list_handle(&database_id, kind)?;
        let db = self.database(&database_id)?;
        let mut db = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?;
        let mut list = list
            .lock()
            .map_err(|_| AppError::new("state", "List unavailable"))?;
        let at = list.find(&mut db, kind, &prefix)?;
        Ok((at.min(list.len().saturating_sub(1)) + 1).to_string())
    }

    /// The crosstable of a tournament, read through a separate handle.
    pub fn crosstable(
        &self,
        database_id: String,
        tournament_id: u32,
        layout: Option<cbh_reader::crosstable::Layout>,
    ) -> Result<cbh_reader::crosstable::Crosstable, AppError> {
        let mut reader = self.own_handle(&database_id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .crosstable_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        // A current sort order by tournament finds the games without reading
        // the whole entry index.
        let mut order = self
            .data_file(&database_id, Some("sort-orders"), "tournament.cwsort")
            .ok()
            .and_then(|path| {
                cbh_reader::sort::SortOrder::open(
                    &path,
                    &reader,
                    cbh_reader::sort::SortColumn::Tournament,
                )
                .ok()
                .flatten()
            });
        let table = reader.crosstable_with(tournament_id, layout, order.as_mut(), &cancel);
        let mut state = self.lock()?;
        if state
            .crosstable_cancels
            .get(&database_id)
            .is_some_and(|c| Arc::ptr_eq(c, &cancel))
        {
            state.crosstable_cancels.remove(&database_id);
        }
        table?.ok_or_else(|| AppError::new("cancelled", "Crosstable cancelled"))
    }
}
