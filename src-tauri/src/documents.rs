use crate::{
    commands::{AppError, GameView},
    registry::{Registry, decimal},
    save,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
pub struct Document {
    pub game: chess_core::Game,
    pub revision: u64,
    pub dirty: bool,
    pub incomplete_annotations: bool,
    pub source_paths: Vec<PathBuf>,
}
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Edit {
    AppendMove { parent: u32, uci: String },
    SetComments { node: u32, comments: Vec<String> },
    SetNags { node: u32, nags: Vec<u16> },
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResponse {
    pub document_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub game: GameView,
    pub revision: String,
    pub dirty: bool,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PgnCollectionResponse {
    pub request_id: String,
    pub collection_id: String,
    pub game_count: usize,
    pub games: Vec<std::collections::BTreeMap<String, String>>,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResponse {
    pub saved_revision: String,
    pub destination: String,
}
impl Registry {
    pub fn open_pgn(
        &self,
        path: PathBuf,
        request_id: String,
    ) -> Result<PgnCollectionResponse, AppError> {
        if self.lock()?.collections.len() >= 4 {
            return Err(AppError::new(
                "limitExceeded",
                "Close a PGN collection first",
            ));
        }
        let max = chess_core::GameLimits::default().max_input_bytes;
        let file = std::fs::File::open(path).map_err(|e| AppError::new("io", e.to_string()))?;
        if file
            .metadata()
            .map_err(|e| AppError::new("io", e.to_string()))?
            .len()
            > max as u64
        {
            return Err(AppError::new(
                "limitExceeded",
                "PGN file exceeds the import byte limit",
            ));
        }
        let mut bytes = Vec::new();
        file.take(max as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::new("io", e.to_string()))?;
        let games = chess_core::parse_pgn(&bytes, &chess_core::GameLimits::default())?;
        let tags = games.iter().map(|g| g.tags.clone()).collect();
        let mut state = self.lock()?;
        if state.collections.len() >= 4 {
            return Err(AppError::new(
                "limitExceeded",
                "PGN collection budget exhausted",
            ));
        }
        let collection_id = state.id("pgn")?;
        let game_count = games.len();
        state
            .collections
            .insert(collection_id.clone(), Arc::new(games));
        Ok(PgnCollectionResponse {
            request_id,
            collection_id,
            game_count,
            games: tags,
        })
    }
    pub fn load_pgn_game(
        &self,
        id: String,
        index: usize,
        request_id: String,
    ) -> Result<DocumentResponse, AppError> {
        let collection =
            self.lock()?.collections.get(&id).cloned().ok_or_else(|| {
                AppError::new("invalidHandle", "PGN collection is closed or unknown")
            })?;
        let game = collection
            .get(index)
            .ok_or_else(|| AppError::new("bounds", "PGN game index is outside the collection"))?
            .clone();
        let view = GameView::from_game(&game)?;
        let mut state = self.lock()?;
        if state.documents.len() >= 10 {
            return Err(AppError::new("limitExceeded", "Document budget exhausted"));
        }
        let document_id = state.id("doc")?;
        state.documents.insert(
            document_id.clone(),
            Arc::new(Mutex::new(Document {
                game,
                revision: 0,
                dirty: false,
                incomplete_annotations: false,
                source_paths: vec![],
            })),
        );
        Ok(DocumentResponse {
            document_id,
            request_id: Some(request_id),
            game: view,
            revision: "0".into(),
            dirty: false,
        })
    }
    pub fn close_pgn(&self, id: String) -> Result<(), AppError> {
        self.lock()?
            .collections
            .remove(&id)
            .ok_or_else(|| AppError::new("invalidHandle", "PGN collection is closed or unknown"))?;
        Ok(())
    }
    pub fn edit_document(
        &self,
        id: String,
        edit: Edit,
        expected_revision: String,
    ) -> Result<DocumentResponse, AppError> {
        let expected = decimal(&expected_revision)?;
        let document = self.document(&id)?;
        let mut d = document
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        if expected != d.revision {
            return Err(AppError::new(
                "staleRevision",
                "Document changed before this edit",
            ));
        }
        if d.revision == u64::MAX {
            return Err(AppError::new(
                "limitExceeded",
                "Document revision exhausted",
            ));
        }
        let changed = match edit {
            Edit::AppendMove { parent, uci } => {
                let before = d.game.nodes.len();
                d.game
                    .append_uci(parent, &uci, &chess_core::GameLimits::default())?;
                before != d.game.nodes.len()
            }
            Edit::SetComments { node, comments } => {
                let before = d
                    .game
                    .nodes
                    .get(node as usize)
                    .ok_or_else(|| AppError::new("invalidNode", "Unknown node"))?
                    .comments
                    .clone();
                d.game.set_comments(node, comments)?;
                before != d.game.nodes[node as usize].comments
            }
            Edit::SetNags { node, nags } => {
                let before = d
                    .game
                    .nodes
                    .get(node as usize)
                    .ok_or_else(|| AppError::new("invalidNode", "Unknown node"))?
                    .nags
                    .clone();
                d.game.set_nags(node, nags)?;
                before != d.game.nodes[node as usize].nags
            }
        };
        if changed {
            d.revision += 1;
            d.dirty = true;
        }
        Ok(DocumentResponse {
            document_id: id,
            request_id: None,
            game: GameView::from_game(&d.game)?,
            revision: d.revision.to_string(),
            dirty: d.dirty,
        })
    }
    pub fn save_document(
        &self,
        id: String,
        path: PathBuf,
        expected_revision: String,
        allow_incomplete: bool,
    ) -> Result<SaveResponse, AppError> {
        self.save_document_with_writer(
            id,
            path,
            expected_revision,
            allow_incomplete,
            save::atomic_pgn,
        )
    }
    #[doc(hidden)]
    pub fn save_document_with_writer(
        &self,
        id: String,
        path: PathBuf,
        expected_revision: String,
        allow_incomplete: bool,
        writer: impl FnOnce(&Path, &str) -> Result<(), AppError>,
    ) -> Result<SaveResponse, AppError> {
        let expected = decimal(&expected_revision)?;
        let document = self.document(&id)?;
        let (game, revision, sources) = {
            let d = document
                .lock()
                .map_err(|_| AppError::new("state", "Document unavailable"))?;
            if d.revision != expected {
                return Err(AppError::new(
                    "staleRevision",
                    "Document changed before this save",
                ));
            }
            if d.incomplete_annotations && !allow_incomplete {
                return Err(AppError::new(
                    "incompleteAnnotations",
                    "Original CBH annotations are not decoded; acknowledge their omission before exporting",
                ));
            }
            (d.game.clone(), d.revision, d.source_paths.clone())
        };
        save::reject_source_destination(&path, &sources)?;
        let text = chess_core::write_pgn(&game)?;
        writer(&path, &text)?;
        let mut d = document
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        if d.revision == revision {
            d.dirty = false;
        }
        Ok(SaveResponse {
            saved_revision: revision.to_string(),
            destination: path.to_string_lossy().into_owned(),
        })
    }
}
