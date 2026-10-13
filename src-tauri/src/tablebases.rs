//! Syzygy endgame tablebases from a folder the user chose: the result of a
//! position (win, draw or loss with the 50-move rule, and the distance to
//! the next capture or pawn move) and of each legal move.
use crate::commands::AppError;
use serde::Serialize;
use shakmaty::{Chess, Position, san::SanPlus, uci::UciMove};
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
            let uci = UciMove::from_standard(m).to_string();
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
