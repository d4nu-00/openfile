//! Writing the selected games of a database list to a PGN file (the database
//! window's Export PGN).
use crate::{
    clipboard::RowRange,
    commands::AppError,
    registry::Registry,
    save::{atomic_pgn_with_writer, reject_source_destination},
};
use std::{
    io::{self, BufWriter, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Games exported at most at once.
pub const MAX_EXPORT: usize = 1_000_000;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    pub written: String,
    /// Entries that are not games, or whose moves could not be read.
    pub skipped: String,
    /// Games written without critical-position marks or training questions.
    pub stripped: String,
    pub cancelled: bool,
    pub destination: String,
}

impl Registry {
    /// Export PGN: writes the games in `ranges` of the current list, in row
    /// order, to `path`. The file appears only when the export completes.
    pub fn export_rows_pgn(
        &self,
        database_id: String,
        ranges: Vec<RowRange>,
        path: PathBuf,
        progress: impl FnMut(u64, u64),
    ) -> Result<ExportResponse, AppError> {
        let (ordinals, truncated) = self.ordinals_of_rows(&database_id, &ranges, MAX_EXPORT)?;
        if truncated {
            return Err(AppError::new(
                "limitExceeded",
                format!("Export at most {MAX_EXPORT} games at once"),
            ));
        }
        let source_path = self
            .lock()?
            .paths
            .get(&database_id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        // A separate handle keeps the list usable during the export.
        let mut source =
            cbh_reader::Database::open(&source_path, cbh_reader::ReadLimits::default())?;
        reject_source_destination(&path, source.source_paths())?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .export_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        // The write closure reports through here, so that a cancelled or
        // failed export drops the temporary file instead of renaming it.
        let mut outcome = None;
        let written = atomic_pgn_with_writer(&path, |file| {
            let mut out = BufWriter::new(file);
            let result = cbh_reader::pgn_export::write_games(
                &mut source,
                &ordinals,
                &mut out,
                &cancel,
                progress,
            );
            let ok = matches!(&result, Ok(s) if !s.cancelled);
            outcome = Some(result);
            if ok {
                out.flush()
            } else {
                Err(io::Error::other("export stopped"))
            }
        });
        if let Ok(mut state) = self.lock()
            && state
                .export_cancels
                .get(&database_id)
                .is_some_and(|c| Arc::ptr_eq(c, &cancel))
        {
            state.export_cancels.remove(&database_id);
        }
        let Some(result) = outcome else {
            // The file could not be created, so nothing was written.
            return written.and(Err(AppError::new("state", "Export did not run")));
        };
        let summary = result?;
        if !summary.cancelled {
            written?;
        }
        Ok(ExportResponse {
            written: summary.written.to_string(),
            skipped: summary.skipped.to_string(),
            stripped: summary.stripped.to_string(),
            cancelled: summary.cancelled,
            destination: path.to_string_lossy().into_owned(),
        })
    }

    pub fn cancel_export(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.export_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}
