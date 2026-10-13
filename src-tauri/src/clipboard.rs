//! Copying games between databases (ChessBase's Copy, Paste, Delete and
//! Select All in the database window). Copy remembers entries of the list;
//! Paste appends them to a .2cbh database opened for editing.
use crate::{
    commands::AppError,
    registry::{Registry, decimal},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Games copied at most at once.
pub const MAX_COPY: usize = 50_000;
/// Games appended per write; each batch is synced once.
const BATCH: usize = 200;

/// A range of list rows, counted from 1, end exclusive.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowRange {
    pub start: String,
    pub end: String,
}

/// Copied games: entries of a database, by its path.
pub struct Clip {
    pub path: std::path::PathBuf,
    pub ordinals: Vec<u64>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResponse {
    pub games: String,
    /// The selection was larger than the copy limit.
    pub truncated: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteProgress {
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteResponse {
    pub pasted: String,
    /// Entries that are not games, or whose moves could not be read.
    pub skipped: String,
    /// Games stored without some annotations (not decoded, or not storable).
    pub incomplete: String,
    pub cancelled: bool,
    pub entry_count: String,
    /// The target's opening tree no longer covers it and needs rebuilding.
    pub tree_stale: bool,
}

impl Registry {
    /// Entry numbers of the given rows of the current list, in row order,
    /// at most `limit`; also says whether there were more.
    pub(crate) fn ordinals_of_rows(
        &self,
        database_id: &str,
        ranges: &[RowRange],
        limit: usize,
    ) -> Result<(Vec<u64>, bool), AppError> {
        let mut out = vec![];
        for range in ranges {
            let (start, end) = (decimal(&range.start)?, decimal(&range.end)?);
            if start == 0 || end < start {
                return Err(AppError::new("bounds", "Row range is out of range"));
            }
            let mut at = start;
            while at < end {
                if out.len() >= limit {
                    return Ok((out, true));
                }
                let n = ((end - at) as usize)
                    .min(crate::lists::MAX_ROWS)
                    .min(limit - out.len());
                let got = self.list_ordinals(database_id, at, n)?;
                if got.is_empty() {
                    break;
                }
                at += got.len() as u64;
                out.extend(got);
            }
        }
        Ok((out, false))
    }

    /// Copy: remembers the games in `ranges` of the current list.
    pub fn copy_rows(
        &self,
        database_id: String,
        ranges: Vec<RowRange>,
    ) -> Result<CopyResponse, AppError> {
        let (ordinals, truncated) = self.ordinals_of_rows(&database_id, &ranges, MAX_COPY)?;
        let path = self
            .lock()?
            .paths
            .get(&database_id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        let games = ordinals.len().to_string();
        self.lock()?.clip = Some(Arc::new(Clip { path, ordinals }));
        Ok(CopyResponse { games, truncated })
    }

    /// Games waiting to be pasted.
    pub fn clip_size(&self) -> Result<String, AppError> {
        Ok(self
            .lock()?
            .clip
            .as_ref()
            .map_or(0, |c| c.ordinals.len())
            .to_string())
    }

    /// Paste: appends the copied games to a database opened for editing.
    /// Annotations that are not decoded or that ChessBase cannot store are
    /// left out and counted; the user agrees to that before pasting.
    pub fn paste_games(
        &self,
        database_id: String,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<PasteResponse, AppError> {
        let clip = self
            .lock()?
            .clip
            .clone()
            .ok_or_else(|| AppError::new("emptyClip", "Copy games first"))?;
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        let (writer, path) = self.writer_of(&database_id)?;
        if crate::save::same_source(&clip.path, &path)? {
            // Reading and appending the same files at once is not supported.
            return Err(AppError::new(
                "sameDatabase",
                "Games cannot be pasted into the database they were copied from",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .paste_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let had_tree = self.has_tree(&database_id).unwrap_or(false);
        let mut source = cbh_reader::Database::open(&clip.path, cbh_reader::ReadLimits::default())?;
        let total = clip.ordinals.len() as u64;
        let (mut pasted, mut skipped, mut incomplete) = (0u64, 0u64, 0u64);
        let mut cancelled = false;
        let mut done = 0u64;
        let result = (|| -> Result<(), AppError> {
            for chunk in clip.ordinals.chunks(BATCH) {
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    return Ok(());
                }
                let mut games = Vec::with_capacity(chunk.len());
                // Games whose original annotations were not all decoded.
                let mut partial = Vec::with_capacity(chunk.len());
                for &o in chunk {
                    let loaded = source.load(o)?;
                    match loaded.game {
                        Some(game) => {
                            partial.push(!matches!(
                                loaded.annotations_status,
                                cbh_reader::DataStatus::Available | cbh_reader::DataStatus::Missing
                            ));
                            games.push(game);
                        }
                        None => skipped += 1,
                    }
                }
                let written = writer
                    .lock()
                    .map_err(|_| AppError::new("state", "Database writer unavailable"))?
                    .append_all(&games)?;
                pasted += written.len() as u64;
                incomplete += written
                    .iter()
                    .zip(&partial)
                    .filter(|((_, omitted), partial)| **partial || !omitted.notes.is_empty())
                    .count() as u64;
                done += chunk.len() as u64;
                progress(done, total);
            }
            Ok(())
        })();
        {
            let mut state = self.lock()?;
            if state
                .paste_cancels
                .get(&database_id)
                .is_some_and(|c| Arc::ptr_eq(c, &cancel))
            {
                state.paste_cancels.remove(&database_id);
            }
        }
        // The database changed even if a later batch failed.
        let count = if pasted > 0 {
            self.refresh_after_write(&database_id, &path)?
        } else {
            self.database(&database_id)?
                .lock()
                .map_err(|_| AppError::new("state", "Database unavailable"))?
                .info()
                .entry_count
        };
        if pasted > 0 {
            self.tree_after_change(&database_id, None, None);
        }
        result?;
        Ok(PasteResponse {
            pasted: pasted.to_string(),
            skipped: skipped.to_string(),
            incomplete: incomplete.to_string(),
            cancelled,
            entry_count: count.to_string(),
            tree_stale: had_tree && pasted > 0,
        })
    }

    pub fn cancel_paste(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.paste_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Delete: marks the games in `ranges` deleted, or undeletes them when
    /// every one of them is deleted already. Returns how many changed.
    pub fn delete_rows(
        &self,
        database_id: String,
        ranges: Vec<RowRange>,
    ) -> Result<String, AppError> {
        let (ordinals, truncated) = self.ordinals_of_rows(&database_id, &ranges, MAX_COPY)?;
        if truncated {
            return Err(AppError::new(
                "limitExceeded",
                format!("Delete at most {MAX_COPY} games at once"),
            ));
        }
        let entries = self.entries_of(&database_id, &ordinals)?;
        let games: Vec<(u64, bool)> = entries
            .iter()
            .zip(&ordinals)
            .filter(|(e, _)| e.kind == "game")
            .map(|(e, &o)| (o, e.deleted))
            .collect();
        let delete = !games.iter().all(|(_, d)| *d);
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        let (writer, path) = self.writer_of(&database_id)?;
        let tree = self.tree_before_change(&database_id, None);
        let mut changed = 0;
        {
            let mut writer = writer
                .lock()
                .map_err(|_| AppError::new("state", "Database writer unavailable"))?;
            for &(o, deleted) in &games {
                if deleted != delete {
                    writer.set_deleted(o, delete)?;
                    changed += 1;
                }
            }
        }
        self.refresh_after_write(&database_id, &path)?;
        self.tree_after_change(&database_id, tree, None);
        Ok(changed.to_string())
    }
}
