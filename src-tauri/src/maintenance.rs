//! Database maintenance: Check Integrity (any database, read only), Find
//! Double Games and Remove Deleted Games (2CBH databases opened for
//! editing). Each runs as a cancellable job with progress; see
//! docs/maintenance.md.
use crate::{commands::AppError, registry::Registry};
use cbh_reader::{
    maintain::{DoubleOptions, IntegrityReport},
    v2::compact,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Games Find Double Games marks at most at once.
pub const MAX_MARKED: usize = 200_000;
/// Pairs listed in the response; all are counted.
const MAX_LISTED: usize = 200;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceProgress {
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoublesResponse {
    pub checked: String,
    /// Games marked deleted, one of each pair of doubles.
    pub marked: String,
    /// Pairs of which both games were kept.
    pub kept_both: String,
    /// The first pairs, as (marked, kept) entry numbers.
    pub pairs: Vec<(String, String)>,
    pub cancelled: bool,
    pub tree_stale: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactResponse {
    pub kept: String,
    pub removed: String,
    pub bytes_before: String,
    pub bytes_after: String,
    pub cancelled: bool,
    pub entry_count: String,
    /// Open games from this database whose entry number changed, as
    /// (old, new); new is 0 for a removed entry.
    pub moved: Vec<(String, String)>,
    /// The workbench's opening tree of this database was deleted.
    pub tree_deleted: bool,
}

impl Registry {
    pub(crate) fn maintenance_cancel(
        &self,
        database_id: &str,
    ) -> Result<Arc<AtomicBool>, AppError> {
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .maintenance_cancels
            .insert(database_id.to_string(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        Ok(cancel)
    }

    pub(crate) fn maintenance_done(&self, database_id: &str, cancel: &Arc<AtomicBool>) {
        if let Ok(mut state) = self.lock()
            && state
                .maintenance_cancels
                .get(database_id)
                .is_some_and(|c| Arc::ptr_eq(c, cancel))
        {
            state.maintenance_cancels.remove(database_id);
        }
    }

    pub fn cancel_maintenance(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.maintenance_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Check Integrity: reads every entry through its own handles and
    /// reports what cannot be read. Nothing is written.
    pub fn check_integrity(
        &self,
        database_id: String,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<IntegrityReport, AppError> {
        let db = self.own_handle(&database_id)?;
        let total = db.info().entry_count;
        let cancel = self.maintenance_cancel(&database_id)?;
        let result = db.check_integrity(&cancel, |done| progress(done, total));
        self.maintenance_done(&database_id, &cancel);
        Ok(result?)
    }

    /// Find Double Games: finds the doubles and marks one of each pair
    /// deleted, as ChessBase's decision chooses it.
    pub fn find_double_games(
        &self,
        database_id: String,
        options: DoubleOptions,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<DoublesResponse, AppError> {
        // Refused before the long scan when the database is read-only.
        self.writer_of(&database_id)?;
        let db = self.own_handle(&database_id)?;
        let cancel = self.maintenance_cancel(&database_id)?;
        let found = db.find_doubles(&options, &cancel, &mut progress);
        self.maintenance_done(&database_id, &cancel);
        let found = found?;
        let mut response = DoublesResponse {
            checked: found.checked.to_string(),
            marked: "0".into(),
            kept_both: found.kept_both.to_string(),
            pairs: found
                .pairs
                .iter()
                .take(MAX_LISTED)
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            cancelled: found.cancelled,
            tree_stale: false,
        };
        if found.cancelled || found.pairs.is_empty() {
            return Ok(response);
        }
        if found.pairs.len() > MAX_MARKED {
            return Err(AppError::new(
                "limitExceeded",
                format!(
                    "{} doubles were found; at most {MAX_MARKED} are marked at once",
                    found.pairs.len()
                ),
            ));
        }
        let marks: Vec<u64> = found.pairs.iter().map(|p| p.0).collect();
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        let (writer, path) = self.writer_of(&database_id)?;
        let had_tree = self.has_tree(&database_id).unwrap_or(false);
        let tree = self.tree_before_change(&database_id, None);
        writer
            .lock()
            .map_err(|_| AppError::new("state", "Database writer unavailable"))?
            .set_deleted_many(&marks)?;
        self.refresh_after_write(&database_id, &path)?;
        self.tree_after_change(&database_id, tree, None);
        response.marked = marks.len().to_string();
        response.tree_stale = had_tree && !self.has_tree(&database_id).unwrap_or(false);
        Ok(response)
    }

    /// Remove Deleted Games: rewrites the database without its deleted
    /// entries, then swaps the new files in.
    pub fn remove_deleted_games(
        &self,
        database_id: String,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<CompactResponse, AppError> {
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        let (writer, path) = self.writer_of(&database_id)?;
        let needed = compact::family_bytes(&path);
        let dir = path
            .parent()
            .ok_or_else(|| AppError::new("invalidPath", "The database has no folder"))?;
        match crate::convert::free_space(dir) {
            Some(free) if free >= needed.saturating_add(crate::convert::RESERVE) => {}
            free => {
                return Err(AppError::new(
                    "diskSpace",
                    format!(
                        "Removing deleted games needs up to {:.2} GB beside the database, and 256 MB must stay free; {} free",
                        needed as f64 / 1e9,
                        free.map_or("the free space cannot be read".into(), |f| format!(
                            "only {:.2} GB are",
                            f as f64 / 1e9
                        ))
                    ),
                ));
            }
        }
        let cancel = self.maintenance_cancel(&database_id)?;
        // The writer stays locked: nothing else writes while the files are
        // rewritten and swapped.
        let mut w = writer
            .lock()
            .map_err(|_| AppError::new("state", "Database writer unavailable"))?;
        let result = compact::compact(&path, &cancel, &mut progress);
        self.maintenance_done(&database_id, &cancel);
        let report = result?;
        if report.cancelled {
            let count = w.entry_count();
            return Ok(CompactResponse {
                kept: report.kept.to_string(),
                removed: "0".into(),
                bytes_before: report.bytes_before.to_string(),
                bytes_after: report.bytes_before.to_string(),
                cancelled: true,
                entry_count: count.to_string(),
                moved: vec![],
                tree_deleted: false,
            });
        }
        *w = cbh_reader::v2::write::Writer::open(&path)?;
        drop(w);
        let count = self.refresh_after_write(&database_id, &path)?;
        // Entry numbers changed: the tree, searches and open games follow.
        let tree_deleted = self.has_tree(&database_id).unwrap_or(false);
        self.delete_opening_tree(database_id.clone())?;
        let mut moved = vec![];
        {
            let mut state = self.lock()?;
            state.searches.remove(&database_id);
            state.references.remove(&database_id);
            for document in state.documents.values() {
                let Ok(mut d) = document.lock() else { continue };
                if let Some((db, old)) = d.origin.clone()
                    && db == database_id
                {
                    let new = report.renumbered.get(old as usize).copied().unwrap_or(0);
                    if new != old {
                        moved.push((old.to_string(), new.to_string()));
                        d.origin = (new != 0).then_some((db, new));
                    }
                }
            }
        }
        Ok(CompactResponse {
            kept: report.kept.to_string(),
            removed: report.removed.to_string(),
            bytes_before: report.bytes_before.to_string(),
            bytes_after: report.bytes_after.to_string(),
            cancelled: false,
            entry_count: count.to_string(),
            moved,
            tree_deleted,
        })
    }
}
