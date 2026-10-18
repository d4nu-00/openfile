//! Syzygy endgame tablebases from a folder the user chose: the result of a
//! position (win, draw or loss with the 50-move rule, and the distance to
//! the next capture or pawn move) and of each legal move.
use crate::commands::AppError;
use serde::Serialize;
use shakmaty::{Chess, Position, san::SanPlus};
use shakmaty_syzygy::{AmbiguousWdl, MaybeRounded, SyzygyError, Tablebase};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Table files read from one folder at most (a full 7-piece set has 1,022).
const MAX_FILES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TablebaseStatus {
    /// No tablebase folder is chosen, or it holds no tables.
    NoTables,
    /// More pieces than the largest table.
    TooManyPieces,
    /// Syzygy tables hold no positions with castling rights.
    Castling,
    /// The table for this material is not in the folder.
    Missing,
    /// A table could not be read.
    Failed,
    Found,
}

/// A result from the point of view of the side that moves (for a move: of
/// the side that plays it). "Cursed" wins and "blessed" losses are drawn
/// by the 50-move rule; "maybe" results depend on the moves since the last
/// capture or pawn move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Win,
    MaybeWin,
    CursedWin,
    Draw,
    BlessedLoss,
    MaybeLoss,
    Loss,
}

impl Outcome {
    fn of(wdl: AmbiguousWdl) -> Self {
        match wdl {
            AmbiguousWdl::Win => Self::Win,
            AmbiguousWdl::MaybeWin => Self::MaybeWin,
            AmbiguousWdl::CursedWin => Self::CursedWin,
            AmbiguousWdl::Draw => Self::Draw,
            AmbiguousWdl::BlessedLoss => Self::BlessedLoss,
            AmbiguousWdl::MaybeLoss => Self::MaybeLoss,
            AmbiguousWdl::Loss => Self::Loss,
        }
    }
    fn flip(self) -> Self {
        match self {
            Self::Win => Self::Loss,
            Self::MaybeWin => Self::MaybeLoss,
            Self::CursedWin => Self::BlessedLoss,
            Self::Draw => Self::Draw,
            Self::BlessedLoss => Self::CursedWin,
            Self::MaybeLoss => Self::MaybeWin,
            Self::Loss => Self::Win,
        }
    }
    /// Best first for the side choosing.
    fn rank(self) -> i32 {
        match self {
            Self::Win => 6,
            Self::MaybeWin => 5,
            Self::CursedWin => 4,
            Self::Draw => 3,
            Self::BlessedLoss => 2,
            Self::MaybeLoss => 1,
            Self::Loss => 0,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablebaseMove {
    pub uci: String,
    pub san: String,
    pub outcome: Outcome,
    /// Plies until the next capture or pawn move after this move, with the
    /// best play, when the tables give it; 0 for such a move itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtz: Option<i32>,
    /// The tables round this distance by one ply.
    pub rounded: bool,
    /// The move captures or moves a pawn.
    pub zeroing: bool,
    /// The move mates or stalemates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ends: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablebaseResponse {
    pub request_id: String,
    pub status: TablebaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub pieces: u32,
    pub max_pieces: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtz: Option<i32>,
    pub rounded: bool,
    pub moves: Vec<TablebaseMove>,
}

/// The chosen folder, for engines that read Syzygy tables themselves.
static FOLDER: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Points an engine at the chosen tablebase folder through its `SyzygyPath`
/// option, as the reference app does (`setoption name SyzygyPath value …`). Engines
/// without the option, or no chosen folder, are left as they are.
pub fn configure_engine(session: &mut engine::EngineSession) {
    let Some(dir) = FOLDER.lock().ok().and_then(|f| f.clone()) else {
        return;
    };
    let Some(name) = session
        .options()
        .iter()
        .find(|o| o.name.eq_ignore_ascii_case("SyzygyPath"))
        .map(|o| o.name.clone())
    else {
        return;
    };
    if let Some(dir) = dir.to_str() {
        // A path the engine cannot take is left out; analysis still runs.
        let _ = session.set_option(&name, dir);
    }
}

/// The folder, its tables, and how many table files it had.
type OpenTables = (PathBuf, Arc<Tablebase<Chess>>, usize);
/// The tablebases of the chosen folder, opened once.
#[derive(Default)]
pub struct Tablebases {
    open: Mutex<Option<OpenTables>>,
}

fn status_of(e: &SyzygyError) -> TablebaseStatus {
    match e {
        SyzygyError::Castling => TablebaseStatus::Castling,
        SyzygyError::TooManyPieces => TablebaseStatus::TooManyPieces,
        SyzygyError::MissingTable { .. } => TablebaseStatus::Missing,
        SyzygyError::ProbeFailed { .. } => TablebaseStatus::Failed,
    }
}

impl Tablebases {
    /// Uses the tables in `dir`, or none. Returns the number of tables.
    pub fn set_dir(&self, dir: Option<&Path>) -> Result<usize, AppError> {
        let mut open = self
            .open
            .lock()
            .map_err(|_| AppError::new("state", "Tablebases unavailable"))?;
        if let Ok(mut f) = FOLDER.lock() {
            *f = dir.map(Path::to_path_buf);
        }
        let Some(dir) = dir else {
            *open = None;
            return Ok(0);
        };
        if let Some((d, _, n)) = open.as_ref()
            && d == dir
        {
            return Ok(*n);
        }
        let files = std::fs::read_dir(dir)
            .map_err(|e| AppError::new("io", format!("Tablebase folder: {e}")))?
            .take(MAX_FILES + 1)
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x == "rtbw" || x == "rtbz")
            })
            .count();
        if files > MAX_FILES {
            return Err(AppError::new(
                "limitExceeded",
                "The tablebase folder holds too many files",
            ));
        }
        let mut tables = Tablebase::new();
        let added = tables
            .add_directory(dir)
            .map_err(|e| AppError::new("io", format!("Tablebase folder: {e}")))?;
        *open = Some((dir.to_path_buf(), Arc::new(tables), added));
        Ok(added)
    }

    /// The position `fen` and each of its moves in the tables.
    pub fn probe(&self, fen: &str, request_id: String) -> Result<TablebaseResponse, AppError> {
        let pos = chess_core::model::parse_position(fen)?;
        let tables = self
            .open
            .lock()
            .map_err(|_| AppError::new("state", "Tablebases unavailable"))?
            .as_ref()
            .map(|(_, t, _)| t.clone());
        let mut response = TablebaseResponse {
            request_id,
            status: TablebaseStatus::NoTables,
            message: None,
            pieces: pos.board().occupied().count() as u32,
            max_pieces: 0,
            outcome: None,
            dtz: None,
            rounded: false,
            moves: vec![],
        };
        let Some(tables) = tables.filter(|t| t.max_pieces() > 0) else {
            return Ok(response);
        };
        response.max_pieces = tables.max_pieces() as u32;
        let fail = |mut r: TablebaseResponse, e: SyzygyError| {
            r.status = status_of(&e);
            r.message = Some(e.to_string());
            r
        };
        let wdl = match tables.probe_wdl(&pos) {
            Ok(w) => w,
            Err(e) => return Ok(fail(response, e)),
        };
        response.outcome = Some(Outcome::of(wdl));
        if let Ok(dtz) = tables.probe_dtz(&pos) {
            (response.dtz, response.rounded) = split(dtz);
        }
        for m in pos.legal_moves() {
            let san = SanPlus::from_move(pos.clone(), m).to_string();
            let zeroing = m.is_zeroing();
            let uci = chess_core::model::uci_of(&pos, m);
            let mut after = pos.clone();
            after.play_unchecked(m);
            let ends = if after.is_checkmate() {
                Some("mate")
            } else if after.is_stalemate() {
                Some("stalemate")
            } else {
                None
            };
            let outcome = match tables.probe_wdl(&after) {
                Ok(w) => Outcome::of(w).flip(),
                Err(e) => return Ok(fail(response, e)),
            };
            let (dtz, rounded) = if zeroing {
                (Some(0), false)
            } else {
                match tables.probe_dtz(&after) {
                    // The opponent's distance, seen from the mover.
                    Ok(d) => {
                        let (d, r) = split(d);
                        (d.map(|d| -d), r)
                    }
                    Err(_) => (None, false),
                }
            };
            response.moves.push(TablebaseMove {
                uci,
                san,
                outcome,
                dtz,
                rounded,
                zeroing,
                ends,
            });
        }
        // Best first: the outcome, then winning fast and losing slowly.
        response.moves.sort_by_key(|m| {
            let mate = i32::from(m.ends == Some("mate"));
            let d = m.dtz.map_or(0, |d| d.abs());
            let speed = match m.outcome.rank() {
                r if r >= 4 => -d,
                r if r <= 2 => d,
                _ => 0,
            };
            (-m.outcome.rank(), -mate, -speed)
        });
        response.status = TablebaseStatus::Found;
        Ok(response)
    }
}

fn split(dtz: MaybeRounded<shakmaty_syzygy::Dtz>) -> (Option<i32>, bool) {
    match dtz {
        MaybeRounded::Precise(d) => (Some(d.0), false),
        MaybeRounded::Rounded(d) => (Some(d.0), true),
    }
}

#[tauri::command]
pub async fn probe_tablebase(
    state: tauri::State<'_, Arc<Tablebases>>,
    fen: String,
    request_id: String,
) -> Result<TablebaseResponse, AppError> {
    let tables = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || tables.probe(&fen, request_id))
        .await
        .map_err(|e| AppError::new("state", e.to_string()))?
}

/// Main-line moves annotated at most.
const MAX_PLIES: usize = 1000;

/// One annotation for the game, as `Edit::AddAnalysis` takes it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablebaseItem {
    pub node: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nag: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub line: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_comment: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablebaseAnnotation {
    /// The document revision the items were made for.
    pub revision: String,
    pub items: Vec<TablebaseItem>,
    /// Main-line positions the tables held.
    pub probed: u32,
    /// The move that brought the game into the tables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entered: Option<u32>,
    /// Moves that gave away a win or a draw.
    pub mistakes: u32,
    pub status: TablebaseStatus,
}

/// Win 2, draw 1 (including the 50-move rule's cursed wins and blessed
/// losses), loss 0. "Maybe" results count as their side.
fn class(o: Outcome) -> u8 {
    match o {
        Outcome::Win | Outcome::MaybeWin => 2,
        Outcome::CursedWin | Outcome::Draw | Outcome::BlessedLoss => 1,
        Outcome::MaybeLoss | Outcome::Loss => 0,
    }
}

/// A result from White's side, as notation text.
fn white_text(o: Outcome, white_to_move: bool) -> String {
    let o = if white_to_move { o } else { o.flip() };
    match o {
        Outcome::Win => "White wins".into(),
        Outcome::Loss => "Black wins".into(),
        Outcome::Draw => "draw".into(),
        Outcome::MaybeWin => "White wins (unless the 50-move rule draws)".into(),
        Outcome::MaybeLoss => "Black wins (unless the 50-move rule draws)".into(),
        Outcome::CursedWin => "White wins, but the 50-move rule draws".into(),
        Outcome::BlessedLoss => "Black wins, but the 50-move rule draws".into(),
    }
}

impl Tablebases {
    /// Annotates the main line of `game` from the tables: the move that
    /// enters them gets the result, and each move that gives away a win or
    /// a draw gets "?" ("??" from a win to a loss), the result after it and
    /// the tables' best move as a variation.
    pub fn annotate(&self, game: &chess_core::Game) -> Result<TablebaseAnnotation, AppError> {
        let tables = self
            .open
            .lock()
            .map_err(|_| AppError::new("state", "Tablebases unavailable"))?
            .as_ref()
            .map(|(_, t, _)| t.clone());
        let mut out = TablebaseAnnotation {
            revision: String::new(),
            items: vec![],
            probed: 0,
            entered: None,
            mistakes: 0,
            status: TablebaseStatus::NoTables,
        };
        let Some(tables) = tables.filter(|t| t.max_pieces() > 0) else {
            return Ok(out);
        };
        let mut nodes = vec![0u32];
        while let Some(&next) = game.nodes[*nodes.last().unwrap() as usize].children.first() {
            if nodes.len() > MAX_PLIES {
                break;
            }
            nodes.push(next);
        }
        let mut positions = vec![chess_core::model::parse_position(&game.start_fen)?];
        for &n in &nodes[1..] {
            let uci = game.nodes[n as usize]
                .uci
                .as_deref()
                .ok_or_else(|| AppError::new("invalidNode", "A move is missing"))?;
            let next = chess_core::model::apply_uci(positions.last().unwrap().clone(), uci)?;
            positions.push(next);
        }
        // Each position's result for the side to move, when the tables hold it.
        let outcomes: Vec<Option<Outcome>> = positions
            .iter()
            .map(|p| {
                if p.board().occupied().count() > tables.max_pieces() {
                    return None;
                }
                tables.probe_wdl(p).ok().map(Outcome::of)
            })
            .collect();
        out.probed = outcomes.iter().filter(|o| o.is_some()).count() as u32;
        out.status = if out.probed > 0 {
            TablebaseStatus::Found
        } else {
            TablebaseStatus::TooManyPieces
        };
        for i in 1..positions.len() {
            let Some(now) = outcomes[i] else { continue };
            let white = positions[i].turn().is_white();
            if out.entered.is_none() {
                out.entered = Some(nodes[i]);
                if outcomes[i - 1].is_none() {
                    out.items.push(TablebaseItem {
                        node: nodes[i],
                        nag: None,
                        comment: Some(format!("Tablebase: {}", white_text(now, white))),
                        line: vec![],
                        line_comment: None,
                    });
                }
            }
            let Some(before) = outcomes[i - 1] else {
                continue;
            };
            // Both from the mover's side.
            let after = now.flip();
            if class(after) >= class(before) {
                continue;
            }
            out.mistakes += 1;
            let mover_white = positions[i - 1].turn().is_white();
            let fen = shakmaty::fen::Fen::from_position(
                &positions[i - 1],
                shakmaty::EnPassantMode::Legal,
            )
            .to_string();
            let best = self
                .probe(&fen, String::new())?
                .moves
                .into_iter()
                .next()
                .filter(|m| Some(m.uci.as_str()) != game.nodes[nodes[i] as usize].uci.as_deref());
            out.items.push(TablebaseItem {
                node: nodes[i],
                nag: Some(if class(before) == 2 && class(after) == 0 {
                    4
                } else {
                    2
                }),
                comment: Some(format!("Tablebase: {}", white_text(after, mover_white))),
                line_comment: best
                    .as_ref()
                    .map(|m| format!("Tablebase: {}", white_text(m.outcome, mover_white))),
                line: best.map(|m| vec![m.uci]).unwrap_or_default(),
            });
            if out.items.len() >= 400 {
                break;
            }
        }
        Ok(out)
    }
}

#[tauri::command]
pub async fn tablebase_annotation(
    tables: tauri::State<'_, Arc<Tablebases>>,
    registry: tauri::State<'_, Arc<crate::registry::Registry>>,
    document_id: String,
) -> Result<TablebaseAnnotation, AppError> {
    let tables = tables.inner().clone();
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (game, revision) = {
            let d = registry.document(&document_id)?;
            let d = d
                .lock()
                .map_err(|_| AppError::new("state", "Document unavailable"))?;
            (d.game.clone(), d.revision)
        };
        let mut out = tables.annotate(&game)?;
        out.revision = revision.to_string();
        Ok(out)
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}
