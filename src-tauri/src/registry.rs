use crate::{commands::*, documents::Document};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};
#[derive(Default)]
pub struct Registry {
    pub(crate) state: Mutex<State>,
}
#[derive(Default)]
pub(crate) struct State {
    next: u64,
    databases: HashMap<String, Arc<Mutex<cbh_reader::ClassicDatabase>>>,
    pub documents: HashMap<String, Arc<Mutex<Document>>>,
}
impl State {
    fn id(&mut self, prefix: &str) -> Result<String, AppError> {
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
    fn database(&self, id: &str) -> Result<Arc<Mutex<cbh_reader::ClassicDatabase>>, AppError> {
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
        if self.lock()?.databases.len() >= 4 {
            return Err(AppError::new(
                "limitExceeded",
                "Close a database before opening another",
            ));
        }
        let db = cbh_reader::ClassicDatabase::open(&path, cbh_reader::ReadLimits::default())?;
        let mut state = self.lock()?;
        if state.databases.len() >= 4 {
            return Err(AppError::new("limitExceeded", "Database budget exhausted"));
        }
        let database_id = state.id("db")?;
        let info = db.info();
        let response = OpenDatabaseResponse {
            request_id,
            database_id: database_id.clone(),
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
        };
        state
            .databases
            .insert(database_id, Arc::new(Mutex::new(db)));
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
        let db = self.database(&id)?;
        let loaded = db
            .lock()
            .map_err(|_| AppError::new("state", "Database unavailable"))?
            .load(ordinal)?;
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
                    incomplete_annotations: loaded.annotations_status
                        == cbh_reader::DataStatus::NotDecoded,
                })),
            );
            (Some(id), Some(view))
        } else {
            (None, None)
        };
        Ok(GameResponse {
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
    pub fn close_database(&self, id: String) -> Result<(), AppError> {
        self.lock()?
            .databases
            .remove(&id)
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
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
