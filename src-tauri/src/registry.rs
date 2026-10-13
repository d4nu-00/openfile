use crate::{commands::*, documents::Document};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};
#[derive(Default)]
pub struct Registry {
    pub(crate) state: Mutex<State>,
    /// Folder for search boosters (position indexes); the app's data folder.
    pub(crate) index_dir: Mutex<Option<PathBuf>>,
    // Persistence is serialized without holding registry/document state locks.
    pub(crate) save_lock: Mutex<()>,
}
#[derive(Default)]
pub(crate) struct State {
    next: u64,
    pub collections: HashMap<String, Arc<Vec<chess_core::Game>>>,
    pub(crate) databases: HashMap<String, Arc<Mutex<cbh_reader::Database>>>,
    /// Writers of databases opened for editing, by database id.
    pub(crate) writers: HashMap<String, Arc<Mutex<cbh_reader::v2::write::Writer>>>,
    /// The path each database was opened from.
    pub(crate) paths: HashMap<String, PathBuf>,
    /// The current search result of each database.
    pub(crate) searches: HashMap<String, Arc<crate::searches::Hits>>,
    pub(crate) search_cancels: HashMap<String, Arc<std::sync::atomic::AtomicBool>>,
    /// Open opening trees, each with its own read handle.
    pub(crate) trees: HashMap<String, Arc<Mutex<crate::trees::OpenTree>>>,
    /// Running opening tree builds.
    pub(crate) tree_cancels: HashMap<String, Arc<std::sync::atomic::AtomicBool>>,
    /// The reference search result of each database, and its running search.
    pub(crate) references: HashMap<String, Arc<crate::searches::Hits>>,
    pub(crate) reference_cancels: HashMap<String, Arc<std::sync::atomic::AtomicBool>>,
    /// The column each database's list is sorted by, and running sorts.
    pub(crate) sorts: HashMap<String, Arc<crate::lists::ListSort>>,
    pub(crate) sort_cancels: HashMap<String, Arc<std::sync::atomic::AtomicBool>>,
    /// Open player and tournament lists, and running builds and crosstables.
    pub(crate) name_lists: HashMap<
        (String, cbh_reader::name_list::NameKind),
        Arc<Mutex<cbh_reader::name_list::NameList>>,
    >,
    pub(crate) name_cancels: HashMap<String, Arc<std::sync::atomic::AtomicBool>>,
    pub(crate) crosstable_cancels: HashMap<String, Arc<std::sync::atomic::AtomicBool>>,
    /// Games copied in a database window, and running pastes.
    pub(crate) clip: Option<Arc<crate::clipboard::Clip>>,
    pub(crate) paste_cancels: HashMap<String, Arc<std::sync::atomic::AtomicBool>>,
    pub documents: HashMap<String, Arc<Mutex<Document>>>,
}
impl State {
    pub(crate) fn id(&mut self, prefix: &str) -> Result<String, AppError> {
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| AppError::new("limitExceeded", "Handle space exhausted"))?;
        Ok(format!("{prefix}-{}", self.next))
    }
}
impl Registry {
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, State>, AppError> {
        self.state
            .lock()
            .map_err(|_| AppError::new("state", "Registry unavailable"))
    }
    pub(crate) fn database(&self, id: &str) -> Result<Arc<Mutex<cbh_reader::Database>>, AppError> {
        self.lock()?
            .databases
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))
    }
    pub(crate) fn document(&self, id: &str) -> Result<Arc<Mutex<Document>>, AppError> {
        self.lock()?
            .documents
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Document is closed or unknown"))
    }
    pub fn open_database(
        &self,
        path: PathBuf,
        request_id: String,
    ) -> Result<OpenDatabaseResponse, AppError> {
        self.open_database_with(path, request_id, false)
    }
    /// Opens a database; `writable` also opens its writer (2CBH only).
    pub fn open_database_with(
        &self,
        path: PathBuf,
        request_id: String,
        writable: bool,
    ) -> Result<OpenDatabaseResponse, AppError> {
        if self.lock()?.databases.len() >= 4 {
            return Err(AppError::new(
                "limitExceeded",
                "Close a database before opening another",
            ));
        }
        let db = cbh_reader::Database::open(&path, cbh_reader::ReadLimits::default())?;
        self.reject_open_alias(db.source_paths(), writable)?;
        let writer = if writable {
            Some(cbh_reader::v2::write::Writer::open(&path)?)
        } else {
            None
        };
        let mut state = self.lock()?;
        if state.databases.len() >= 4 {
            return Err(AppError::new("limitExceeded", "Database budget exhausted"));
        }
        let database_id = state.id("db")?;
        let info = db.info();
        let response = OpenDatabaseResponse {
            request_id,
            database_id: database_id.clone(),
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            entry_count: info.entry_count.to_string(),
            companions: info
                .companions
                .iter()
                .map(|v| cbh_reader::CompanionInfo {
                    extension: v.extension.clone(),
                    status: v.status.clone(),
                })
                .collect(),
            diagnostics: info.diagnostics.iter().cloned().map(Into::into).collect(),
            writable: writer.is_some(),
            booster: false,
            tree: false,
        };
        if let Some(writer) = writer {
            state
                .writers
                .insert(database_id.clone(), Arc::new(Mutex::new(writer)));
        }
        state.paths.insert(database_id.clone(), path);
        state
            .databases
            .insert(database_id.clone(), Arc::new(Mutex::new(db)));
        drop(state);
        let mut response = response;
        response.booster = self.has_booster(&database_id).unwrap_or(false);
        response.tree = self.has_tree(&database_id).unwrap_or(false);
        Ok(response)
    }
    pub fn read_page(
        &self,
        id: String,
        start: String,
        limit: usize,
        request_id: String,
    ) -> Result<PageResponse, AppError> {
        let start = decimal(&start)?;
        let db = self.database(&id)?;
        let page = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?
            .page(start, limit)?;
        Ok(PageResponse {
            request_id,
            entries: page.entries.into_iter().map(Into::into).collect(),
            next: page.next.map(|v| v.to_string()),
        })
    }
    pub fn load_entry(
        &self,
        id: String,
        ordinal: String,
        request_id: String,
    ) -> Result<GameResponse, AppError> {
        let ordinal = decimal(&ordinal)?;
        let id_for_origin = id.clone();
        let db = self.database(&id)?;
        let loaded = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?
            .load(ordinal)?;
        let source_paths = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?
            .source_paths()
            .to_vec();
        let (document_id, game) = if let Some(game) = loaded.game {
            let view = GameView::from_game(&game)?;
            let mut state = self.lock()?;
            if state.documents.len() >= 10 {
                return Err(AppError::new(
                    "limitExceeded",
                    "Close a document before loading another",
                ));
            }
            let id = state.id("doc")?;
            state.documents.insert(
                id.clone(),
                Arc::new(Mutex::new(Document {
                    game,
                    revision: 0,
                    dirty: false,
                    source_paths,
                    origin: Some((id_for_origin.clone(), ordinal)),
                    incomplete_annotations: matches!(
                        loaded.annotations_status,
                        cbh_reader::DataStatus::NotDecoded
                            | cbh_reader::DataStatus::Partial
                            | cbh_reader::DataStatus::Corrupt
                            | cbh_reader::DataStatus::Unsupported
                    ),
                    history: vec![],
                    future: vec![],
                })),
            );
            (Some(id), Some(view))
        } else {
            (None, None)
        };
        Ok(GameResponse {
            revision: "0".into(),
            dirty: false,
            request_id,
            document_id,
            game,
            summary: loaded.summary.into(),
            metadata_status: loaded.metadata_status,
            moves_status: loaded.moves_status,
            annotations_status: loaded.annotations_status,
            diagnostics: loaded.diagnostics.into_iter().map(Into::into).collect(),
        })
    }
    pub fn position_at(
        &self,
        id: String,
        node_id: u32,
        request_id: String,
    ) -> Result<PositionResponse, AppError> {
        let document = self.document(&id)?;
        let d = document
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        let p = d.game.position_at(node_id)?;
        Ok(PositionResponse {
            request_id,
            document_id: id,
            node_id,
            fen: p.fen,
            turn: p.turn,
            legal_uci: p.legal_uci,
        })
    }
    pub fn analysis_position(
        &self,
        id: &str,
        node: u32,
    ) -> Result<(String, Vec<String>, String, String), AppError> {
        let doc = self.document(id)?;
        let d = doc
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        let history = d
            .game
            .path_to(node)?
            .iter()
            .filter_map(|&id| d.game.nodes[id as usize].uci.clone())
            .collect();
        let position = d.game.position_at(node)?;
        Ok((
            d.game.start_fen.clone(),
            history,
            position.fen,
            position.turn,
        ))
    }
    pub fn has_dirty_documents(&self) -> bool {
        let Ok(state) = self.lock() else {
            return true;
        };
        state
            .documents
            .values()
            .any(|doc| doc.lock().map(|d| d.dirty).unwrap_or(true))
    }
    pub fn close_database(&self, id: String) -> Result<(), AppError> {
        let mut state = self.lock()?;
        state
            .databases
            .remove(&id)
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        state.writers.remove(&id);
        state.paths.remove(&id);
        let state = &mut *state;
        state.searches.remove(&id);
        state.trees.remove(&id);
        state.references.remove(&id);
        state.sorts.remove(&id);
        state.name_lists.retain(|(db, _), _| *db != id);
        for cancels in [
            &mut state.sort_cancels,
            &mut state.name_cancels,
            &mut state.crosstable_cancels,
            &mut state.paste_cancels,
            &mut state.search_cancels,
            &mut state.tree_cancels,
            &mut state.reference_cancels,
        ] {
            if let Some(c) = cancels.remove(&id) {
                c.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        Ok(())
    }
    pub fn close_document(&self, id: String) -> Result<(), AppError> {
        self.lock()?
            .documents
            .remove(&id)
            .ok_or_else(|| AppError::new("invalidHandle", "Document is closed or unknown"))?;
        Ok(())
    }
}
pub(crate) fn decimal(value: &str) -> Result<u64, AppError> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AppError::new(
            "bounds",
            "Expected an unsigned decimal integer",
        ));
    }
    value
        .parse()
        .map_err(|_| AppError::new("bounds", "Decimal value exceeds u64"))
}
