//! Opening classification: Classify Opening for the shown game and Set ECO
//! Codes for a database opened for editing (see docs/openings.md).
use crate::{commands::AppError, registry::Registry};
use std::sync::Arc;

/// Entries whose codes are written per sync.
const ECO_CHUNK: usize = 50_000;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    /// "B90", or none for a set-up or a game the table does not know.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eco: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The ply of the main line whose position the table holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ply: Option<u16>,
    /// The game's own ECO tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EcoResponse {
    pub games: String,
    /// Codes written.
    pub changed: String,
    pub unchanged: String,
    /// Games with another code, left alone ("only games without a code").
    pub kept: String,
    pub unclassified: String,
    pub unreadable: String,
    /// Coded games that already had the table's code.
    pub agreed: String,
    pub had_code: String,
    pub cancelled: bool,
}

impl Registry {
    /// Classify Opening (`CLASSIFYGAME`): the document's main line.
    pub fn classify_game(&self, document_id: &str) -> Result<Classification, AppError> {
        let d = self.document(document_id)?;
        let d = d
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        let found = chess_core::eco::classify_game(&d.game);
        Ok(Classification {
            eco: found.as_ref().map(|c| c.opening.eco.clone()),
            name: found.as_ref().map(|c| c.opening.name.clone()),
            ply: found.as_ref().map(|c| c.ply),
            current: d.game.tags.get("ECO").filter(|e| !e.is_empty()).cloned(),
        })
    }

    /// Set ECO Codes (`DBTOOLSSETECOCODE`): classifies every game of a 2CBH
    /// opened for editing and writes the codes that change, in chunks.
    /// Progress counts the scan, then the writing.
    pub fn set_eco_codes(
        &self,
        database_id: String,
        only_missing: bool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<EcoResponse, AppError> {
        // Refused before the long scan when the database is read-only.
        self.writer_of(&database_id)?;
        let db = self.own_handle(&database_id)?;
        let count = db.info().entry_count;
        let cancel = self.maintenance_cancel(&database_id)?;
        let found = db.classify_eco(
            cbh_reader::eco::EcoOptions { only_missing },
            &cancel,
            &mut progress,
        );
        let result = (|| {
            let found = found?;
            let mut response = EcoResponse {
                games: found.games.to_string(),
                changed: "0".into(),
                unchanged: found.unchanged.to_string(),
                kept: found.kept.to_string(),
                unclassified: found.unclassified.to_string(),
                unreadable: found.unreadable.to_string(),
                agreed: found.agreed.to_string(),
                had_code: found.had_code.to_string(),
                cancelled: found.cancelled,
            };
            if found.cancelled || found.changes.is_empty() {
                return Ok(response);
            }
            let _save = self
                .save_lock
                .lock()
                .map_err(|_| AppError::new("state", "Save service unavailable"))?;
            let (writer, path) = self.writer_of(&database_id)?;
            if self
                .database(&database_id)?
                .lock()
                .map_err(|_| AppError::new("state", "Database unavailable"))?
                .info()
                .entry_count
                != count
            {
                return Err(AppError::new(
                    "sourceChanged",
                    "The database changed during the classification; run it again",
                ));
            }
            let tree = self.tree_before_change(&database_id, None);
            let total = found.changes.len() as u64;
            let mut written = 0u64;
            let outcome = (|| -> Result<(), AppError> {
                for chunk in found.changes.chunks(ECO_CHUNK) {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        response.cancelled = true;
                        return Ok(());
                    }
                    writer
                        .lock()
                        .map_err(|_| AppError::new("state", "Database writer unavailable"))?
                        .set_eco_many(chunk)?;
                    written += chunk.len() as u64;
                    progress(written, total);
                }
                Ok(())
            })();
            if written > 0 {
                self.refresh_after_write(&database_id, &path)?;
                self.tree_after_change(&database_id, tree, None);
            }
            outcome?;
            response.changed = written.to_string();
            Ok(response)
        })();
        self.maintenance_done(&database_id, &cancel);
        result
    }
}

#[tauri::command]
pub fn classify_game(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
) -> Result<Classification, AppError> {
    state.classify_game(&document_id)
}

#[tauri::command]
pub async fn set_eco_codes(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    only_missing: bool,
    events: tauri::ipc::Channel<crate::maintenance::MaintenanceProgress>,
) -> Result<EcoResponse, AppError> {
    let s = state.inner().clone();
    let mut last = std::time::Instant::now();
    tauri::async_runtime::spawn_blocking(move || {
        s.set_eco_codes(database_id, only_missing, |done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::maintenance::MaintenanceProgress {
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}
