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
    /// The file name without its extension, for the database's tab.
    pub name: String,
    pub entry_count: String,
    pub companions: Vec<cbh_reader::CompanionInfo>,
    pub diagnostics: Vec<DiagnosticDto>,
    /// Opened for editing: games can be saved, replaced and marked deleted.
    pub writable: bool,
    /// A current search booster exists for fast position search.
    pub booster: bool,
    /// A current opening tree exists.
    pub tree: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_team: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub black_team: Option<String>,
    /// In a position search: the node where the game reaches the position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found_node: Option<u32>,
    /// For the crosstable of the game's tournament.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tournament_id: Option<u32>,
    /// For the statistics of the game's players.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub black_id: Option<u32>,
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
            annotator: e.annotator,
            source: e.source,
            white_team: e.white_team,
            black_team: e.black_team,
            found_node: None,
            tournament_id: e.tournament_id,
            white_id: e.white_id,
            black_id: e.black_id,
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
    #[serde(rename = "preComments", skip_serializing_if = "Vec::is_empty")]
    pub pre_comments: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub squares: Vec<chess_core::SquareMark>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arrows: Vec<chess_core::ArrowMark>,
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
                pre_comments: n.pre_comments.clone(),
                squares: n.squares.clone(),
                arrows: n.arrows.clone(),
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
    settings: tauri::State<'_, crate::settings::Settings>,
    path: PathBuf,
    request_id: String,
    writable: Option<bool>,
) -> Result<OpenDatabaseResponse, AppError> {
    let s = state.inner().clone();
    let opened_path = path.clone();
    let opened =
        blocking(move || s.open_database_with(path, request_id, writable.unwrap_or(false))).await?;
    // The start screen's lists; a settings problem does not stop the open.
    let _ = settings.record_opened(&opened_path);
    let s = state.inner().clone();
    let id = opened.database_id.clone();
    tauri::async_runtime::spawn_blocking(move || s.warm_up(&id));
    Ok(opened)
}
#[tauri::command]
pub async fn create_database(
    state: tauri::State<'_, Arc<Registry>>,
    settings: tauri::State<'_, crate::settings::Settings>,
    path: PathBuf,
    request_id: String,
) -> Result<OpenDatabaseResponse, AppError> {
    let s = state.inner().clone();
    let created_path = path.clone();
    let created = blocking(move || s.create_database(path, request_id)).await?;
    let _ = settings.record_opened(&created_path);
    Ok(created)
}
#[tauri::command]
pub async fn save_game(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
    database_id: String,
    expected_revision: String,
    allow_incomplete: bool,
) -> Result<crate::database_writes::SaveGameResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        s.save_game(
            document_id,
            database_id,
            expected_revision,
            allow_incomplete,
        )
    })
    .await
}
#[tauri::command]
pub async fn replace_game(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
    expected_revision: String,
    allow_incomplete: bool,
) -> Result<crate::database_writes::SaveGameResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.replace_game(document_id, expected_revision, allow_incomplete)).await
}
#[tauri::command]
pub async fn set_deleted(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ordinal: String,
    deleted: bool,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    blocking(move || s.set_deleted(database_id, ordinal, deleted)).await
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
pub async fn close_document(
    state: tauri::State<'_, Arc<Registry>>,
    engines: tauri::State<'_, Arc<crate::engine_commands::EngineManager>>,
    document_id: String,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    let engines = engines.inner().clone();
    blocking(move || {
        let _ = engines.stop_document(&document_id);
        s.close_document(document_id)
    })
    .await
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
#[tauri::command]
pub fn startup_request(state: tauri::State<'_, crate::StartupRequest>) -> Option<crate::Startup> {
    state.0.lock().ok()?.take()
}
/// The frontend has shown the startup request. In macOS debug builds this
/// takes the `--snapshot` picture after a short settling delay.
#[tauri::command]
pub async fn startup_shown(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, crate::StartupRequest>,
) -> Result<(), AppError> {
    let destination = state.1.lock().ok().and_then(|mut s| s.take());
    #[cfg(all(debug_assertions, target_os = "macos"))]
    if let Some(destination) = destination {
        std::thread::sleep(std::time::Duration::from_millis(800));
        crate::snapshot::take(&window, destination).map_err(|e| AppError::new("snapshot", e))?;
    }
    #[cfg(not(all(debug_assertions, target_os = "macos")))]
    let _ = (window, destination);
    Ok(())
}
#[tauri::command]
pub async fn search_games(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    filter: cbh_reader::search::GameFilter,
    request_id: String,
    events: tauri::ipc::Channel<crate::searches::BoosterProgress>,
) -> Result<crate::searches::SearchResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.search_games(database_id, filter, request_id, progress_to(events))).await
}
#[tauri::command]
pub fn cancel_search(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.cancel_search(database_id)
}
#[tauri::command]
pub fn clear_search(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.clear_search(database_id)
}
#[tauri::command]
pub async fn read_search_page(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    start: String,
    limit: usize,
    request_id: String,
) -> Result<PageResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.read_search_page(database_id, start, limit, request_id)).await
}
#[tauri::command]
pub async fn find_position(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    fen: String,
    request_id: String,
    events: tauri::ipc::Channel<crate::searches::BoosterProgress>,
) -> Result<crate::searches::SearchResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.find_position(database_id, fen, request_id, progress_to(events))).await
}
#[tauri::command]
pub async fn search_position(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    query: cbh_reader::pattern::PositionQuery,
    request_id: String,
    events: tauri::ipc::Channel<crate::searches::BoosterProgress>,
) -> Result<crate::searches::SearchResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.search_position(database_id, query, request_id, progress_to(events))).await
}
/// Sends (done, total) to the frontend at most four times a second.
fn progress_to(
    events: tauri::ipc::Channel<crate::searches::BoosterProgress>,
) -> impl FnMut(u64, u64) {
    let mut last = std::time::Instant::now();
    move |done, total| {
        if last.elapsed().as_millis() >= 250 || done == total {
            last = std::time::Instant::now();
            let _ = events.send(crate::searches::BoosterProgress {
                done: done.to_string(),
                total: total.to_string(),
            });
        }
    }
}
#[tauri::command]
pub async fn create_search_booster(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    events: tauri::ipc::Channel<crate::searches::BoosterProgress>,
) -> Result<bool, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        let mut last = std::time::Instant::now();
        s.create_search_booster(database_id, |done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::searches::BoosterProgress {
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
}
#[tauri::command]
pub async fn delete_search_booster(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    blocking(move || s.delete_search_booster(database_id)).await
}
#[tauri::command]
pub async fn create_opening_tree(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    events: tauri::ipc::Channel<crate::trees::TreeProgress>,
) -> Result<bool, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        let mut last = std::time::Instant::now();
        s.create_opening_tree(database_id, |stage, done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::trees::TreeProgress {
                    stage,
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
}
#[tauri::command]
pub fn cancel_opening_tree(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.cancel_opening_tree(database_id)
}
#[tauri::command]
pub async fn delete_opening_tree(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    blocking(move || s.delete_opening_tree(database_id)).await
}
#[tauri::command]
pub async fn opening_tree(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    line: crate::trees::Line,
    request_id: String,
) -> Result<crate::trees::TreeResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.opening_tree(database_id, line, request_id)).await
}
#[tauri::command]
pub async fn reference_search(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    line: crate::trees::Line,
    request_id: String,
    search_anyway: Option<bool>,
) -> Result<crate::trees::ReferenceResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        s.reference_search_with(
            database_id,
            line,
            request_id,
            search_anyway.unwrap_or(false),
        )
    })
    .await
}
#[tauri::command]
pub fn cancel_reference(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.cancel_reference(database_id)
}
#[tauri::command]
pub async fn read_reference_page(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    start: String,
    limit: usize,
    request_id: String,
) -> Result<PageResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.read_reference_page(database_id, start, limit, request_id)).await
}
#[tauri::command]
pub async fn new_document(
    state: tauri::State<'_, Arc<Registry>>,
    fen: Option<String>,
    request_id: String,
) -> Result<crate::documents::DocumentResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.new_document(fen, request_id)).await
}
#[tauri::command]
pub async fn paste_game(
    state: tauri::State<'_, Arc<Registry>>,
    text: String,
    request_id: String,
) -> Result<crate::documents::DocumentResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.paste_game(text, request_id)).await
}
#[tauri::command]
pub async fn document_pgn(
    state: tauri::State<'_, Arc<Registry>>,
    document_id: String,
) -> Result<String, AppError> {
    let s = state.inner().clone();
    blocking(move || s.document_pgn(document_id)).await
}
#[tauri::command]
pub async fn read_rows(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    start: String,
    limit: usize,
    request_id: String,
) -> Result<PageResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.read_rows(database_id, start, limit, request_id)).await
}
#[tauri::command]
pub async fn find_row(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ordinal: String,
) -> Result<Option<String>, AppError> {
    let s = state.inner().clone();
    blocking(move || s.find_row(database_id, ordinal)).await
}
#[tauri::command]
pub async fn sort_list(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    column: Option<cbh_reader::sort::SortColumn>,
    descending: bool,
    events: tauri::ipc::Channel<crate::lists::SortProgress>,
) -> Result<crate::lists::SortResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        let mut last = std::time::Instant::now();
        s.sort_list(database_id, column, descending, |stage, done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::lists::SortProgress {
                    stage,
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
}
#[tauri::command]
pub fn cancel_sort(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.cancel_sort(database_id)
}
#[tauri::command]
pub async fn name_list(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    kind: cbh_reader::name_list::NameKind,
    events: tauri::ipc::Channel<crate::names::NameProgress>,
) -> Result<crate::names::NameListResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        let mut last = std::time::Instant::now();
        s.name_list(database_id, kind, |done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::names::NameProgress {
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
}
#[tauri::command]
pub fn cancel_name_list(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.cancel_name_list(database_id)
}
#[tauri::command]
pub async fn read_names(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    kind: cbh_reader::name_list::NameKind,
    start: String,
    limit: usize,
    request_id: String,
) -> Result<crate::names::NamePage, AppError> {
    let s = state.inner().clone();
    blocking(move || s.read_names(database_id, kind, start, limit, request_id)).await
}
#[tauri::command]
pub async fn find_name(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    kind: cbh_reader::name_list::NameKind,
    prefix: String,
) -> Result<String, AppError> {
    let s = state.inner().clone();
    blocking(move || s.find_name(database_id, kind, prefix)).await
}
#[tauri::command]
pub async fn player_statistics(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    player_id: u32,
    events: tauri::ipc::Channel<crate::statistics::StatisticsProgress>,
) -> Result<Option<cbh_reader::player_stats::PlayerStatistics>, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        let mut last = std::time::Instant::now();
        s.player_statistics(database_id, player_id, |done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::statistics::StatisticsProgress {
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
}
#[tauri::command]
pub fn cancel_player_statistics(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.cancel_player_statistics(database_id)
}
#[tauri::command]
pub async fn statistics_games(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    player_id: u32,
    group: cbh_reader::player_stats::Group,
    request_id: String,
) -> Result<crate::searches::SearchResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.statistics_games(database_id, player_id, group, request_id)).await
}
#[tauri::command]
pub async fn crosstable(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    tournament_id: u32,
    layout: Option<cbh_reader::crosstable::Layout>,
) -> Result<cbh_reader::crosstable::Crosstable, AppError> {
    let s = state.inner().clone();
    blocking(move || s.crosstable(database_id, tournament_id, layout)).await
}
#[tauri::command]
pub async fn copy_rows(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ranges: Vec<crate::clipboard::RowRange>,
) -> Result<crate::clipboard::CopyResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || s.copy_rows(database_id, ranges)).await
}
#[tauri::command]
pub fn clip_size(state: tauri::State<'_, Arc<Registry>>) -> Result<String, AppError> {
    state.clip_size()
}
#[tauri::command]
pub async fn paste_games(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    events: tauri::ipc::Channel<crate::clipboard::PasteProgress>,
) -> Result<crate::clipboard::PasteResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        let mut last = std::time::Instant::now();
        s.paste_games(database_id, |done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::clipboard::PasteProgress {
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
}
#[tauri::command]
pub async fn convert_estimate(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ranges: Option<Vec<crate::clipboard::RowRange>>,
    target: PathBuf,
) -> Result<crate::convert::ConvertEstimate, AppError> {
    let s = state.inner().clone();
    blocking(move || s.convert_estimate(database_id, ranges, target)).await
}
#[tauri::command]
pub async fn convert_database(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ranges: Option<Vec<crate::clipboard::RowRange>>,
    target: PathBuf,
    events: tauri::ipc::Channel<crate::convert::ConvertProgress>,
) -> Result<crate::convert::ConvertResponse, AppError> {
    let s = state.inner().clone();
    blocking(move || {
        let mut last = std::time::Instant::now();
        s.convert_database(database_id, ranges, target, |done, total| {
            if last.elapsed().as_millis() >= 250 || done == total {
                last = std::time::Instant::now();
                let _ = events.send(crate::convert::ConvertProgress {
                    done: done.to_string(),
                    total: total.to_string(),
                });
            }
        })
    })
    .await
}
#[tauri::command]
pub fn cancel_convert(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.cancel_convert(database_id)
}
#[tauri::command]
pub fn cancel_paste(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
) -> Result<(), AppError> {
    state.cancel_paste(database_id)
}
#[tauri::command]
pub async fn delete_rows(
    state: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ranges: Vec<crate::clipboard::RowRange>,
) -> Result<String, AppError> {
    let s = state.inner().clone();
    blocking(move || s.delete_rows(database_id, ranges)).await
}
