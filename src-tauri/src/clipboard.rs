//! Copying games between databases (the reference app's Copy, Paste, Delete and
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
    /// Chess960 games left out: 2CBH cannot hold them yet.
    pub chess960: String,
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
    /// Annotations that are not decoded or that the reference app cannot store are
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
        // The tree records the pasted games in its journal.
        let tree = self.tree_before_change(&database_id, None);
        let mut source = cbh_reader::Database::open(&clip.path, cbh_reader::ReadLimits::default())?;
        let total = clip.ordinals.len() as u64;
        let (mut pasted, mut skipped, mut incomplete) = (0u64, 0u64, 0u64);
        let mut chess960 = 0u64;
        let mut cancelled = false;
        let mut done = 0u64;
        let result = (|| -> Result<(), AppError> {
            for chunk in clip.ordinals.chunks(BATCH) {
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    return Ok(());
                }
                let mut games = Vec::with_capacity(chunk.len());
                // Medals travel with the games; deleted marks do not.
                let mut marks = Vec::with_capacity(chunk.len());
                // Games whose original annotations were not all decoded.
                let mut partial = Vec::with_capacity(chunk.len());
                for &o in chunk {
                    let loaded = source.load(o)?;
                    match loaded.game {
                        Some(game) if game.is_chess960() => chess960 += 1,
                        Some(game) => {
                            partial.push(!matches!(
                                loaded.annotations_status,
                                cbh_reader::DataStatus::Available | cbh_reader::DataStatus::Missing
                            ));
                            marks.push(cbh_reader::v2::write::EntryMarks {
                                deleted: false,
                                medals: loaded.summary.medals,
                            });
                            games.push(game);
                        }
                        None => skipped += 1,
                    }
                }
                let written = writer
                    .lock()
                    .map_err(|_| AppError::new("state", "Database writer unavailable"))?
                    .append_marked(&games, &marks)?;
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
        let tree_current = pasted == 0 || self.tree_after_append(&database_id, tree);
        result?;
        Ok(PasteResponse {
            pasted: pasted.to_string(),
            skipped: skipped.to_string(),
            incomplete: incomplete.to_string(),
            chess960: chess960.to_string(),
            cancelled,
            entry_count: count.to_string(),
            tree_stale: had_tree && !tree_current,
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

/// Games changed per write by Mark Games and Edit Game Data; progress and
/// Stop apply between writes.
const EDIT_CHUNK: usize = 2_000;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionEditResponse {
    /// Games changed (written before a Stop).
    pub changed: String,
    /// Mark Games: games marked afterwards; the others were unmarked.
    pub marked: String,
    pub cancelled: bool,
}

impl Registry {
    /// The game entries of `ranges`, at most `MAX_COPY`.
    fn selected_games(
        &self,
        database_id: &str,
        ranges: &[RowRange],
        what: &str,
    ) -> Result<Vec<u64>, AppError> {
        let (ordinals, truncated) = self.ordinals_of_rows(database_id, ranges, MAX_COPY)?;
        if truncated {
            return Err(AppError::new(
                "limitExceeded",
                format!("{what} at most {MAX_COPY} games at once"),
            ));
        }
        let entries = self.entries_of(database_id, &ordinals)?;
        Ok(entries
            .iter()
            .zip(&ordinals)
            .filter(|(e, _)| e.kind == "game")
            .map(|(_, &o)| o)
            .collect())
    }

    /// Writes `games` in chunks through `apply`, with progress and Stop.
    fn edit_selection(
        &self,
        database_id: &str,
        games: &[u64],
        mut apply: impl FnMut(&mut cbh_reader::v2::write::Writer, &[u64]) -> Result<u64, AppError>,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<SelectionEditResponse, AppError> {
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .maintenance_cancels
            .insert(database_id.to_string(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        let (writer, path) = self.writer_of(database_id)?;
        let (mut changed, mut marked, mut cancelled) = (0u64, 0u64, false);
        let total = games.len() as u64;
        progress(0, total);
        let result = (|| -> Result<(), AppError> {
            for chunk in games.chunks(EDIT_CHUNK) {
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    return Ok(());
                }
                let mut w = writer
                    .lock()
                    .map_err(|_| AppError::new("state", "Database writer unavailable"))?;
                marked += apply(&mut w, chunk)?;
                changed += chunk.len() as u64;
                progress(changed, total);
            }
            Ok(())
        })();
        if let Ok(mut state) = self.lock()
            && state
                .maintenance_cancels
                .get(database_id)
                .is_some_and(|c| Arc::ptr_eq(c, &cancel))
        {
            state.maintenance_cancels.remove(database_id);
        }
        if changed > 0 {
            self.refresh_after_write(database_id, &path)?;
        }
        result?;
        Ok(SelectionEditResponse {
            changed: changed.to_string(),
            marked: marked.to_string(),
            cancelled,
        })
    }

    /// Mark Games (`MARKGAME`, +): toggles the user medal of each selected
    /// game, as the reference app does.
    pub fn mark_rows(
        &self,
        database_id: String,
        ranges: Vec<RowRange>,
        progress: impl FnMut(u64, u64),
    ) -> Result<SelectionEditResponse, AppError> {
        let games = self.selected_games(&database_id, &ranges, "Mark")?;
        self.edit_selection(
            &database_id,
            &games,
            |w, chunk| Ok(w.toggle_user_medal(chunk)?),
            progress,
        )
    }

    /// Edit Game Data for a selection (`DBTOOLSEDIT`): sets the tournament,
    /// annotator or source of every selected game.
    pub fn set_names_of_rows(
        &self,
        database_id: String,
        ranges: Vec<RowRange>,
        names: cbh_reader::v2::write::SelectionNames,
        progress: impl FnMut(u64, u64),
    ) -> Result<SelectionEditResponse, AppError> {
        if names.tournament.is_none() && names.annotator.is_none() && names.source.is_none() {
            return Err(AppError::new("invalidParameters", "Choose a field to set"));
        }
        let too_long = |s: &str| s.chars().count() > 255 || s.contains(['\r', '\n']);
        let fields = [
            names
                .tournament
                .as_ref()
                .map(|t| [t.0.as_str(), t.1.as_str()]),
            names.source.as_ref().map(|t| [t.0.as_str(), t.1.as_str()]),
            names.annotator.as_deref().map(|a| [a, ""]),
        ];
        if fields.iter().flatten().flatten().any(|s| too_long(s)) {
            return Err(AppError::new(
                "invalidParameters",
                "Names are one line of at most 255 characters",
            ));
        }
        let games = self.selected_games(&database_id, &ranges, "Edit")?;
        self.edit_selection(
            &database_id,
            &games,
            |w, chunk| {
                w.set_names(chunk, &names)?;
                Ok(0)
            },
            progress,
        )
    }
}
