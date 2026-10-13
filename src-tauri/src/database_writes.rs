//! Creating databases and saving, replacing and deleting games in them.
//! Only 2CBH databases that the user created or opened for editing have a
//! writer (see docs/writing.md).
use crate::{
    commands::{AppError, OpenDatabaseResponse},
    registry::{Registry, decimal},
};
use cbh_reader::v2::write::Writer;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveGameResponse {
    pub database_id: String,
    pub ordinal: String,
    pub entry_count: String,
    pub saved_revision: String,
}

/// A database id and entry number.
type Origin = (String, u64);

impl Registry {
    /// Refuses to open a family for editing while it is open here, or to
    /// open a family that is open for editing, so a database is never read
    /// through one handle while another writes it. Read-only handles may
    /// share files.
    pub(crate) fn reject_open_alias(
        &self,
        files: &[PathBuf],
        writable: bool,
    ) -> Result<(), AppError> {
        let open: Vec<PathBuf> = {
            let state = self.lock()?;
            state
                .databases
                .iter()
                .filter(|(id, _)| writable || state.writers.contains_key(*id))
                .filter_map(|(_, db)| db.lock().ok().map(|d| d.source_paths().to_vec()))
                .flatten()
                .collect()
        };
        for file in files {
            for other in &open {
                if crate::save::same_source(file, other)? {
                    return Err(AppError::new(
                        "alreadyOpen",
                        "This database is already open; close it first",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn create_database(
        &self,
        path: PathBuf,
        request_id: String,
    ) -> Result<OpenDatabaseResponse, AppError> {
        if crate::save::database_extension(&path)
            && !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("2cbh"))
        {
            return Err(AppError::new(
                "unsupported",
                "New databases use the .2cbh format",
            ));
        }
        let path = path.with_extension("2cbh");
        drop(Writer::create(&path)?);
        self.open_database_with(path, request_id, true)
    }

    fn writer(&self, id: &str) -> Result<(Arc<Mutex<Writer>>, PathBuf), AppError> {
        let state = self.lock()?;
        let writer = state.writers.get(id).cloned().ok_or_else(|| {
            AppError::new(
                "readOnly",
                "This database is open read-only; open it for editing first",
            )
        })?;
        Ok((writer, state.paths[id].clone()))
    }

    /// Re-opens the reader after a write, so pages show the new state.
    fn refresh(&self, id: &str, path: &Path) -> Result<u64, AppError> {
        let fresh = cbh_reader::Database::open(path, cbh_reader::ReadLimits::default())?;
        let count = fresh.info().entry_count;
        let db = self.database(id)?;
        *db.lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))? = fresh;
        Ok(count)
    }

    /// The document's game, after the revision and annotation checks that
    /// PGN saving also makes.
    fn game_for_saving(
        &self,
        document_id: &str,
        expected_revision: &str,
        allow_incomplete: bool,
    ) -> Result<(chess_core::Game, u64, Option<Origin>), AppError> {
        let expected = decimal(expected_revision)?;
        let document = self.document(document_id)?;
        let d = document
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        if d.revision != expected {
            return Err(AppError::new(
                "staleRevision",
                "Document changed before this save",
            ));
        }
        if !allow_incomplete {
            if d.incomplete_annotations {
                return Err(AppError::new(
                    "incompleteAnnotations",
                    "Some original database annotations are not decoded; acknowledge their omission before saving",
                ));
            }
            let (_, omitted) = cbh_reader::v2::encode::encode_annotations(&d.game);
            if let Some(note) = omitted.notes.first() {
                return Err(AppError::new(
                    "incompleteAnnotations",
                    format!(
                        "ChessBase cannot store every annotation ({note}); acknowledge their omission before saving"
                    ),
                ));
            }
        }
        Ok((d.game.clone(), d.revision, d.origin.clone()))
    }

    fn saved(&self, document_id: &str, revision: u64, origin: Origin) -> Result<(), AppError> {
        let document = self.document(document_id)?;
        let mut d = document
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        if d.revision == revision {
            d.dirty = false;
        }
        d.incomplete_annotations = false;
        d.origin = Some(origin);
        Ok(())
    }

    /// Appends the document's game to `database_id` (ChessBase "Save").
    pub fn save_game(
        &self,
        document_id: String,
        database_id: String,
        expected_revision: String,
        allow_incomplete: bool,
    ) -> Result<SaveGameResponse, AppError> {
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        let (game, revision, _) =
            self.game_for_saving(&document_id, &expected_revision, allow_incomplete)?;
        let (writer, path) = self.writer(&database_id)?;
        let tree = self.tree_before_change(&database_id, None);
        let (ordinal, _) = writer
            .lock()
            .map_err(|_| AppError::new("state", "Database writer unavailable"))?
            .append(&game)?;
        let count = self.refresh(&database_id, &path)?;
        self.tree_after_change(&database_id, tree, Some(ordinal));
        self.saved(&document_id, revision, (database_id.clone(), ordinal))?;
        Ok(SaveGameResponse {
            database_id,
            ordinal: ordinal.to_string(),
            entry_count: count.to_string(),
            saved_revision: revision.to_string(),
        })
    }

    /// Replaces the entry the document was loaded from (ChessBase "Replace").
    pub fn replace_game(
        &self,
        document_id: String,
        expected_revision: String,
        allow_incomplete: bool,
    ) -> Result<SaveGameResponse, AppError> {
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        let (game, revision, origin) =
            self.game_for_saving(&document_id, &expected_revision, allow_incomplete)?;
        let (database_id, ordinal) = origin
            .ok_or_else(|| AppError::new("noOrigin", "This game was not loaded from a database"))?;
        let (writer, path) = self.writer(&database_id)?;
        let tree = self.tree_before_change(&database_id, Some(ordinal));
        writer
            .lock()
            .map_err(|_| AppError::new("state", "Database writer unavailable"))?
            .replace(ordinal, &game)?;
        let count = self.refresh(&database_id, &path)?;
        self.tree_after_change(&database_id, tree, Some(ordinal));
        self.saved(&document_id, revision, (database_id.clone(), ordinal))?;
        Ok(SaveGameResponse {
            database_id,
            ordinal: ordinal.to_string(),
            entry_count: count.to_string(),
            saved_revision: revision.to_string(),
        })
    }

    /// Marks an entry deleted or not deleted.
    pub fn set_deleted(
        &self,
        database_id: String,
        ordinal: String,
        deleted: bool,
    ) -> Result<(), AppError> {
        let ordinal = decimal(&ordinal)?;
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
        let (writer, path) = self.writer(&database_id)?;
        let tree = self.tree_before_change(&database_id, None);
        writer
            .lock()
            .map_err(|_| AppError::new("state", "Database writer unavailable"))?
            .set_deleted(ordinal, deleted)?;
        self.refresh(&database_id, &path)?;
        self.tree_after_change(&database_id, tree, None);
        Ok(())
    }
}
