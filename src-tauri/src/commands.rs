use serde::Serialize;
use shakmaty::{san::SanPlus, uci::UciMove};
#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub diagnostics: Vec<DiagnosticDto>,
}
impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            diagnostics: vec![],
        }
    }
}
impl From<cbh_reader::CbhError> for AppError {
    fn from(e: cbh_reader::CbhError) -> Self {
        Self::new(e.code(), e.to_string())
    }
}
impl From<chess_core::CoreError> for AppError {
    fn from(e: chess_core::CoreError) -> Self {
        Self::new("invalidGame", e.to_string())
    }
}
#[derive(Debug, Serialize)]
pub struct DiagnosticDto {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<String>,
}
impl From<cbh_reader::Diagnostic> for DiagnosticDto {
    fn from(d: cbh_reader::Diagnostic) -> Self {
        Self {
            code: d.code,
            message: d.message,
            file: d.file,
            offset: d.offset.map(|v| v.to_string()),
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDatabaseResponse {
    pub request_id: String,
    pub database_id: String,
    pub entry_count: String,
    pub companions: Vec<cbh_reader::CompanionInfo>,
    pub diagnostics: Vec<DiagnosticDto>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDto {
    pub ordinal: String,
    pub kind: String,
    pub deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub black: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_elo: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub black_elo: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eco: Option<String>,
    pub diagnostics: Vec<DiagnosticDto>,
}
impl From<cbh_reader::EntrySummary> for EntryDto {
    fn from(e: cbh_reader::EntrySummary) -> Self {
        Self {
            ordinal: e.ordinal.to_string(),
            kind: match e.kind {
                cbh_reader::EntryKind::Game => "game",
                cbh_reader::EntryKind::Text => "text",
                _ => "unknown",
            }
            .into(),
            deleted: e.deleted,
            white: e.white,
            black: e.black,
            event: e.event,
            site: e.site,
            date: e.date,
            result: e.result,
            round: e.round,
            white_elo: e.white_elo,
            black_elo: e.black_elo,
            eco: e.eco,
            diagnostics: e.diagnostics.into_iter().map(Into::into).collect(),
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResponse {
    pub request_id: String,
    pub entries: Vec<EntryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct GameNode {
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub san: Option<String>,
    pub children: Vec<u32>,
    pub comments: Vec<String>,
    pub nags: Vec<u16>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    pub tags: std::collections::BTreeMap<String, String>,
    pub start_fen: String,
    pub nodes: Vec<GameNode>,
}
impl GameView {
    pub fn from_game(game: &chess_core::Game) -> Result<Self, AppError> {
        let mut nodes: Vec<_> = game
            .nodes
            .iter()
            .map(|n| GameNode {
                id: n.id,
                parent: n.parent,
                uci: n.uci.clone(),
                san: None,
                children: n.children.clone(),
                comments: n.comments.clone(),
                nags: n.nags.clone(),
            })
            .collect();
        let mut seen = vec![false; nodes.len()];
        let mut stack = vec![(0u32, chess_core::model::parse_position(&game.start_fen)?)];
        while let Some((id, before)) = stack.pop() {
            let node = nodes
                .get_mut(id as usize)
                .ok_or_else(|| AppError::new("invalidNode", "Unknown node"))?;
            if seen[id as usize] || node.id != id {
                return Err(AppError::new("invalidNode", "Invalid game tree"));
            }
            seen[id as usize] = true;
            let after = if let Some(uci) = &node.uci {
                node.san = Some(if uci == "0000" {
                    "--".into()
                } else {
                    let m = uci
                        .parse::<UciMove>()
                        .map_err(|_| AppError::new("invalidGame", "Invalid UCI move"))?
                        .to_move(&before)
                        .map_err(|_| AppError::new("invalidGame", "Illegal move"))?;
                    SanPlus::from_move(before.clone(), m).to_string()
                });
                chess_core::model::apply_uci(before, uci)?
            } else {
                before
            };
            for &child in node.children.iter().rev() {
                stack.push((child, after.clone()));
            }
        }
        if seen.iter().any(|&v| !v) {
            return Err(AppError::new("invalidNode", "Unreachable game node"));
        }
        Ok(Self {
            tags: game.tags.clone(),
            start_fen: game.start_fen.clone(),
            nodes,
        })
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameResponse {
    pub revision: String,
    pub dirty: bool,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game: Option<GameView>,
    pub summary: EntryDto,
    pub metadata_status: cbh_reader::DataStatus,
    pub moves_status: cbh_reader::DataStatus,
    pub annotations_status: cbh_reader::DataStatus,
    pub diagnostics: Vec<DiagnosticDto>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionResponse {
    pub request_id: String,
    pub document_id: String,
    pub node_id: u32,
    pub fen: String,
    pub turn: String,
    pub legal_uci: Vec<String>,
}

use crate::registry::Registry;
use std::{path::PathBuf, sync::Arc};
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new("worker", e.to_string()))?
}
#[tauri::command]
pub async fn open_database(
    state: tauri::State<'_, Arc<Registry>>,
    path: PathBuf,
    request_id: String,
) -> Result<OpenDatabaseResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.open_database(path, request_id)).await
}
#[tauri::command]
pub async fn read_page(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    start: String,
    limit: usize,
    request_id: String,
) -> Result<PageResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.read_page(database_id, start, limit, request_id)).await
}
#[tauri::command]
pub async fn load_entry(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ordinal: String,
    request_id: String,
) -> Result<GameResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.load_entry(database_id, ordinal, request_id)).await
}
#[tauri::command]
pub async fn position_at(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
    node_id: u32,
    request_id: String,
) -> Result<PositionResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.position_at(document_id, node_id, request_id)).await
}
#[tauri::command]
pub fn close_database(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.close_database(database_id)
}
#[tauri::command]
pub fn close_document(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
) -> Result<(), AppError> {
    state.close_document(document_id)
}

#[tauri::command]
pub async fn open_pgn(
    state: tauri::State<'_, Arc<Registry>>,
    path: PathBuf,
    request_id: String,
) -> Result<crate::documents::PgnCollectionResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.open_pgn(path, request_id)).await
}
#[tauri::command]
pub async fn load_pgn_game(
    state: tauri::State<'_, Arc<Registry>>,
    collection_id: String,
    index: usize,
    request_id: String,
) -> Result<crate::documents::DocumentResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.load_pgn_game(collection_id, index, request_id)).await
}
#[tauri::command]
pub fn close_pgn(
    state: tauri::State<'_, Arc<Registry>>,
    collection_id: String,
) -> Result<(), AppError> {
    state.close_pgn(collection_id)
}
#[tauri::command]
pub async fn edit_document(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
    edit: crate::documents::Edit,
    expected_revision: String,
) -> Result<crate::documents::DocumentResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.edit_document(document_id, edit, expected_revision)).await
}
#[tauri::command]
pub async fn save_document(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
    destination: PathBuf,
    expected_revision: String,
    allow_incomplete: bool,
) -> Result<crate::documents::SaveResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        s.save_document(
            document_id,
            destination,
            expected_revision,
            allow_incomplete,
        )
    })
    .await
}

#[tauri::command]
pub fn finish_close(app: tauri::AppHandle, state: tauri::State<'_, crate::ExitPolicy>) {
    state.0.store(true, std::sync::atomic::Ordering::Release);
    app.exit(0);
}
