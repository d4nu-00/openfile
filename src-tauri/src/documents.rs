use crate::{
    commands::{AppError, GameView},
    registry::{MAX_DOCUMENTS, Registry, decimal},
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
    /// The database entry this game was loaded from: database id, ordinal.
    pub origin: Option<(String, u64)>,
    /// Earlier states of the game, for undo, and undone ones, for redo.
    pub history: Vec<chess_core::Game>,
    pub future: Vec<chess_core::Game>,
}
impl Document {
    pub fn new(game: chess_core::Game) -> Self {
        Self {
            game,
            revision: 0,
            dirty: false,
            incomplete_annotations: false,
            source_paths: vec![],
            origin: None,
            history: vec![],
            future: vec![],
        }
    }
}
/// Undo steps kept per document, and the nodes they may hold in all.
const MAX_UNDO: usize = 100;
const MAX_UNDO_NODES: usize = 200_000;
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Edit {
    AppendMove {
        parent: u32,
        uci: String,
    },
    SetComments {
        node: u32,
        comments: Vec<String>,
    },
    SetNags {
        node: u32,
        nags: Vec<u16>,
    },
    SetPreComments {
        node: u32,
        comments: Vec<String>,
    },
    DeleteVariation {
        node: u32,
    },
    PromoteVariation {
        node: u32,
    },
    DeleteRemaining {
        node: u32,
    },
    DeletePrevious {
        node: u32,
    },
    SetTags {
        tags: std::collections::BTreeMap<String, String>,
    },
    SetMarks {
        node: u32,
        squares: Vec<chess_core::SquareMark>,
        arrows: Vec<chess_core::ArrowMark>,
    },
    ClearMarks,
    /// The reference app's Exchange Moves on the first move of a variation.
    ExchangeMoves {
        node: u32,
    },
    /// The reference app's Reorder Variations from `node` along its line.
    ReorderVariations {
        node: u32,
        #[serde(default, rename = "mainLine")]
        main_line: bool,
    },
    /// The reference app's Clean up variations of the whole game.
    CleanUpVariations,
    /// The reference app's Insert Into Game for the variation containing `node`.
    InsertVariation {
        node: u32,
    },
    /// The reference app's Delete Commentary commands; `node` is the selected move.
    DeleteCommentary {
        commentary: chess_core::edit::Commentary,
        node: u32,
    },
    /// Sets or removes the training question on a move; `moves` gives the
    /// answers' moves in SAN or UCI (see `Game::set_training`).
    SetTraining {
        node: u32,
        training: Option<Box<chess_core::training::Training>>,
        #[serde(default)]
        moves: Vec<Option<String>>,
    },
    /// The reference app's Delete Training Commentary: every question.
    ClearTraining,
    /// Engine annotations of several moves at once (Blunder Check).
    AddAnalysis {
        items: Vec<AnalysisItem>,
    },
    /// Lines from `parent` (Deep Analysis), each ending with an evaluation
    /// symbol and a comment.
    AddLines {
        parent: u32,
        lines: Vec<crate::deep_analysis::LineItem>,
    },
    /// Moves from `parent` (an engine line), with a comment after the last.
    AppendLine {
        parent: u32,
        moves: Vec<String>,
        #[serde(default)]
        comment: Option<String>,
    },
    Undo,
    Redo,
}
/// One annotated move: a symbol and a comment for the move played, and the
/// engine's line from the position before it as a variation.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisItem {
    pub node: u32,
    /// A move symbol, added when the move has none.
    #[serde(default)]
    pub nag: Option<u16>,
    /// An evaluation symbol; it replaces the move's evaluation, but not
    /// another position symbol such as "with attack".
    #[serde(default)]
    pub evaluation: Option<u16>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub line: Vec<String>,
    #[serde(default)]
    pub line_comment: Option<String>,
}

/// Adds `items` to `game`; nothing changes unless all of them apply.
fn add_analysis(game: &mut chess_core::Game, items: Vec<AnalysisItem>) -> Result<(), AppError> {
    if items.len() > 2_000 {
        return Err(AppError::new(
            "limitExceeded",
            "Too many annotations at once",
        ));
    }
    let mut next = game.clone();
    let push = |g: &mut chess_core::Game, node: u32, text: Option<String>| {
        let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
            return Ok(());
        };
        let mut comments = g
            .nodes
            .get(node as usize)
            .ok_or_else(|| AppError::new("invalidNode", "Unknown node"))?
            .comments
            .clone();
        if !comments.contains(&text) {
            comments.push(text);
            g.set_comments(node, comments)?;
        }
        Ok::<_, AppError>(())
    };
    for item in items {
        let n = next
            .nodes
            .get(item.node as usize)
            .ok_or_else(|| AppError::new("invalidNode", "Unknown node"))?;
        // The start takes a comment only (a summary before the first move).
        let parent = n.parent;
        if parent.is_none()
            && (item.nag.is_some() || item.evaluation.is_some() || !item.line.is_empty())
        {
            return Err(AppError::new("invalidNode", "The start has no move"));
        }
        if let Some(nag) = item.nag
            && !n.nags.iter().any(|g| (1..=6).contains(g))
        {
            let mut nags = n.nags.clone();
            nags.insert(0, nag);
            next.set_nags(item.node, nags)?;
        }
        if let Some(nag) = item.evaluation {
            set_evaluation(&mut next, item.node, nag)?;
        }
        push(&mut next, item.node, item.comment)?;
        if let (Some(parent), false) = (parent, item.line.is_empty()) {
            let last = next.append_line(parent, &item.line, &chess_core::GameLimits::default())?;
            push(&mut next, last, item.line_comment)?;
        }
    }
    *game = next;
    Ok(())
}
/// Sets the reference app's evaluation symbol `nag` on `node`, replacing an
/// evaluation it has but not another position symbol such as "with attack".
fn set_evaluation(game: &mut chess_core::Game, node: u32, nag: u16) -> Result<(), AppError> {
    let nags = &game.nodes[node as usize].nags;
    let other = nags.iter().any(|g| {
        !crate::blunder_check::EVALUATION_NAGS.contains(g)
            && !(1..=9).contains(g)
            && *g != 22
            && !(140..=145).contains(g)
    });
    if !other {
        let mut nags: Vec<u16> = nags
            .iter()
            .copied()
            .filter(|g| !crate::blunder_check::EVALUATION_NAGS.contains(g))
            .collect();
        nags.push(nag);
        game.set_nags(node, nags)?;
    }
    Ok(())
}

/// Adds `lines` from `parent`, following moves already there; nothing
/// changes unless all of them apply.
pub fn add_lines(
    game: &mut chess_core::Game,
    parent: u32,
    lines: &[crate::deep_analysis::LineItem],
) -> Result<(), AppError> {
    if lines.len() > 1_000 {
        return Err(AppError::new("limitExceeded", "Too many lines at once"));
    }
    if game.nodes.get(parent as usize).is_none() {
        return Err(AppError::new("invalidNode", "Unknown node"));
    }
    let mut next = game.clone();
    for line in lines {
        let last = next.append_line(parent, &line.moves, &chess_core::GameLimits::default())?;
        if let Some(nag) = line.evaluation {
            set_evaluation(&mut next, last, nag)?;
        }
        if let Some(text) = line.comment.as_ref().filter(|t| !t.trim().is_empty()) {
            let mut comments = next.nodes[last as usize].comments.clone();
            if !comments.contains(text) {
                comments.push(text.clone());
                next.set_comments(last, comments)?;
            }
        }
    }
    *game = next;
    Ok(())
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
    /// The node to select after a structural edit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<u32>,
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
        if state.documents.len() >= MAX_DOCUMENTS {
            return Err(AppError::new("limitExceeded", "Document budget exhausted"));
        }
        let document_id = state.id("doc")?;
        state.documents.insert(
            document_id.clone(),
            Arc::new(Mutex::new(Document::new(game))),
        );
        Ok(DocumentResponse {
            document_id,
            request_id: Some(request_id),
            game: view,
            revision: "0".into(),
            dirty: false,
            selected: None,
        })
    }
    /// A new, empty game from the initial position or `fen` (the reference app "New
    /// Game" and "Position Setup").
    pub fn new_document(
        &self,
        fen: Option<String>,
        request_id: String,
    ) -> Result<DocumentResponse, AppError> {
        let game = match fen {
            Some(fen) => chess_core::Game::from_fen(fen.trim())?,
            None => chess_core::Game::initial(),
        };
        self.insert_document(game, request_id)
    }
    /// A new document from the first game of PGN text ("Paste Game").
    pub fn paste_game(
        &self,
        text: String,
        request_id: String,
    ) -> Result<DocumentResponse, AppError> {
        let limits = chess_core::GameLimits::default();
        if text.len() > limits.max_input_bytes {
            return Err(AppError::new(
                "limitExceeded",
                "The pasted text is too long",
            ));
        }
        let game = chess_core::parse_pgn(text.as_bytes(), &limits)?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::new("invalidPgn", "The clipboard holds no PGN game"))?;
        // Plain text parses as an empty game; a game needs tags or moves.
        if game.nodes.len() == 1 && !text.trim_start().starts_with('[') {
            return Err(AppError::new(
                "invalidPgn",
                "The clipboard holds no PGN game",
            ));
        }
        self.insert_document(game, request_id)
    }
    fn insert_document(
        &self,
        game: chess_core::Game,
        request_id: String,
    ) -> Result<DocumentResponse, AppError> {
        let view = GameView::from_game(&game)?;
        let mut state = self.lock()?;
        if state.documents.len() >= MAX_DOCUMENTS {
            return Err(AppError::new("limitExceeded", "Document budget exhausted"));
        }
        let document_id = state.id("doc")?;
        state.documents.insert(
            document_id.clone(),
            Arc::new(Mutex::new(Document::new(game))),
        );
        Ok(DocumentResponse {
            document_id,
            request_id: Some(request_id),
            game: view,
            revision: "0".into(),
            dirty: false,
            selected: None,
        })
    }
    /// The document's game as PGN ("Copy Game").
    pub fn document_pgn(&self, id: String) -> Result<String, AppError> {
        let document = self.document(&id)?;
        let d = document
            .lock()
            .map_err(|_| AppError::new("state", "Document unavailable"))?;
        Ok(chess_core::write_pgn(&d.game)?)
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
        let before = d.game.clone();
        let mut selected = None;
        match edit {
            Edit::AppendMove { parent, uci } => {
                d.game
                    .append_uci(parent, &uci, &chess_core::GameLimits::default())?;
            }
            Edit::SetComments { node, comments } => d.game.set_comments(node, comments)?,
            Edit::SetNags { node, nags } => d.game.set_nags(node, nags)?,
            Edit::SetPreComments { node, comments } => d.game.set_pre_comments(node, comments)?,
            Edit::DeleteVariation { node } => selected = Some(d.game.delete_variation(node)?),
            Edit::PromoteVariation { node } => selected = Some(d.game.promote_variation(node)?),
            Edit::ExchangeMoves { node } => selected = Some(d.game.exchange_moves(node)?),
            Edit::ReorderVariations { node, main_line } => {
                selected = Some(d.game.reorder_variations(node, main_line)?)
            }
            Edit::CleanUpVariations => {
                d.game.clean_up_variations()?;
                selected = Some(0);
            }
            Edit::DeleteRemaining { node } => selected = Some(d.game.delete_remaining(node)?),
            Edit::DeletePrevious { node } => selected = Some(d.game.delete_previous(node)?),
            Edit::SetTags { tags } => d.game.set_tags(tags)?,
            Edit::SetMarks {
                node,
                squares,
                arrows,
            } => d.game.set_marks(node, squares, arrows)?,
            Edit::ClearMarks => d.game.clear_marks(),
            Edit::InsertVariation { node } => selected = Some(d.game.insert_variation(node)?),
            Edit::DeleteCommentary { commentary, node } => {
                selected = Some(d.game.delete_commentary(commentary, node)?)
            }
            Edit::SetTraining {
                node,
                training,
                moves,
            } => d.game.set_training(node, training.map(|t| *t), &moves)?,
            Edit::ClearTraining => {
                d.game.clear_training();
            }
            Edit::AddAnalysis { items } => add_analysis(&mut d.game, items)?,
            Edit::AddLines { parent, lines } => add_lines(&mut d.game, parent, &lines)?,
            Edit::AppendLine {
                parent,
                moves,
                comment,
            } => {
                let last =
                    d.game
                        .append_line(parent, &moves, &chess_core::GameLimits::default())?;
                if let Some(text) = comment.filter(|t| !t.trim().is_empty()) {
                    let mut comments = d.game.nodes[last as usize].comments.clone();
                    if !comments.contains(&text) {
                        comments.push(text);
                        if let Err(e) = d.game.set_comments(last, comments) {
                            d.game = before.clone();
                            return Err(e.into());
                        }
                    }
                }
                selected = Some(last);
            }
            Edit::Undo | Edit::Redo => {
                let undo = matches!(edit, Edit::Undo);
                let d = &mut *d;
                let (from, to) = if undo {
                    (&mut d.history, &mut d.future)
                } else {
                    (&mut d.future, &mut d.history)
                };
                if let Some(game) = from.pop() {
                    to.push(std::mem::replace(&mut d.game, game));
                    d.revision += 1;
                    d.dirty = true;
                }
                return Ok(DocumentResponse {
                    document_id: id,
                    request_id: None,
                    game: GameView::from_game(&d.game)?,
                    revision: d.revision.to_string(),
                    dirty: d.dirty,
                    selected: None,
                });
            }
        }
        if before != d.game {
            d.revision += 1;
            d.dirty = true;
            d.future.clear();
            d.history.push(before);
            // Bound the undo history by steps and by stored nodes.
            while d.history.len() > MAX_UNDO
                || d.history.iter().map(|g| g.nodes.len()).sum::<usize>() > MAX_UNDO_NODES
            {
                d.history.remove(0);
            }
        }
        Ok(DocumentResponse {
            document_id: id,
            request_id: None,
            game: GameView::from_game(&d.game)?,
            revision: d.revision.to_string(),
            dirty: d.dirty,
            selected,
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
        let _save = self
            .save_lock
            .lock()
            .map_err(|_| AppError::new("state", "Save service unavailable"))?;
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
                    "Some original database annotations are not decoded; acknowledge their omission before exporting",
                ));
            }
            if !allow_incomplete
                && d.game
                    .nodes
                    .iter()
                    .any(|n| n.critical.is_some() || n.training.is_some())
            {
                return Err(AppError::new(
                    "incompleteAnnotations",
                    "PGN cannot hold critical position marks or training questions; acknowledge their omission before exporting",
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
