//! Opening trees built from a database's own games (see docs/opening-tree.md).
//!
//! A tree file holds every position that the main lines of at least two games
//! pass through within the first `plies` half-moves. For each position it
//! stores, per next move, the number of games, their results, ratings and
//! years, and the games themselves. A position reached by a single game is
//! not stored; that game is found by following the line from the last stored
//! position ([`OpeningTree::query`]).
use crate::{CbhError, Database, header::RawEntry};
use chess_core::Game;
use shakmaty::{Board, Chess, Color, File, Move, Position, Role, Square, uci::UciMove};
use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

mod build;
mod journal;
pub use build::{BuildOptions, BuildStage, DEFAULT_MIN_GAMES, DEFAULT_PLIES};
pub use journal::{GameLine, PendingChange};

/// splitmix64 step, used to fill the hash table at compile time.
const fn splitmix(state: u64) -> (u64, u64) {
    let state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (state, z ^ (z >> 31))
}

/// One random key per (role, colour, square), plus one for Black to move.
/// Fixed forever: tree files store these hashes.
const KEYS: [u64; 12 * 64 + 1] = {
    let mut keys = [0u64; 12 * 64 + 1];
    let mut state = 0x4f50_454e_5452_4545; // "OPENTREE"
    let mut i = 0;
    while i < keys.len() {
        let (next, value) = splitmix(state);
        state = next;
        keys[i] = value;
        i += 1;
    }
    keys
};
const BLACK_KEY: u64 = KEYS[12 * 64];

/// Piece kind 0..12: (role - 1) * 2, plus one for Black.
fn kind(role: Role, color: Color) -> u8 {
    (role as u8 - 1) * 2 + u8::from(color.is_black())
}

/// Hash of the board and side to move. Castling rights, en passant and move
/// numbers are ignored, as in Find Position.
pub fn position_hash(board: &Board, turn: Color) -> u64 {
    let mut h = if turn.is_black() { BLACK_KEY } else { 0 };
    for (square, piece) in board.iter() {
        h ^= KEYS[kind(piece.role, piece.color) as usize * 64 + square as usize];
    }
    h
}

/// No move: the game ended in this position.
pub const NO_MOVE: u16 = 0;
/// A null move; from and to square are both h8.
pub const NULL_MOVE: u16 = 0x0fff;

/// Packs a move as from | to << 6 | promotion << 12, with the promotion as
/// the shakmaty role number (2 knight .. 5 queen). Castling is stored as the
/// king's two-square move, as UCI writes it in standard chess.
pub fn pack_move(m: &Move) -> Option<u16> {
    let (from, to, promotion) = match *m {
        Move::Normal {
            from,
            to,
            promotion,
            ..
        } => (from, to, promotion),
        Move::EnPassant { from, to } => (from, to, None),
        Move::Castle { king, rook } => {
            let file = if rook > king { File::G } else { File::C };
            (king, Square::from_coords(file, king.rank()), None)
        }
        Move::Put { .. } => return None,
    };
    Some(from as u16 | (to as u16) << 6 | promotion.map_or(0, |r| r as u16) << 12)
}

/// The packed move in UCI; `None` for [`NO_MOVE`].
pub fn unpack_uci(code: u16) -> Option<String> {
    if code == NULL_MOVE {
        return Some("0000".into());
    }
    let (from, to, promotion) = (code & 63, (code >> 6) & 63, code >> 12);
    if from == to {
        return None;
    }
    let square = |s: u16| format!("{}{}", (b'a' + (s % 8) as u8) as char, s / 8 + 1);
    let promotion = match promotion {
        2 => "n",
        3 => "b",
        4 => "r",
        5 => "q",
        _ => "",
    };
    Some(format!("{}{}{promotion}", square(from), square(to)))
}

/// A board that plays packed moves and keeps [`position_hash`] up to date,
/// without checking legality. Only moves that were legal when packed are
/// replayed.
#[derive(Clone)]
pub struct Replay {
    squares: [u8; 64],
    black: bool,
    hash: u64,
}

impl Replay {
    pub fn new(board: &Board, turn: Color) -> Self {
        let mut squares = [0u8; 64];
        for (square, piece) in board.iter() {
            squares[square as usize] = kind(piece.role, piece.color) + 1;
        }
        Self {
            squares,
            black: turn.is_black(),
            hash: position_hash(board, turn),
        }
    }
    pub fn hash(&self) -> u64 {
        self.hash
    }
    pub fn black_to_move(&self) -> bool {
        self.black
    }
    fn toggle(&mut self, square: usize, piece: u8) {
        self.hash ^= KEYS[(piece as usize - 1) * 64 + square];
    }
    fn take(&mut self, square: usize) -> u8 {
        let piece = std::mem::take(&mut self.squares[square]);
        if piece != 0 {
            self.toggle(square, piece);
        }
        piece
    }
    fn put(&mut self, square: usize, piece: u8) {
        self.squares[square] = piece;
        self.toggle(square, piece);
    }
    /// Plays `code`. Returns whether the move was irreversible (a pawn move,
    /// capture or castling), or `None` if there is no piece to move.
    pub fn play(&mut self, code: u16) -> Option<bool> {
        self.black = !self.black;
        self.hash ^= BLACK_KEY;
        if code == NULL_MOVE {
            return Some(false);
        }
        let (from, to, promotion) = (
            (code & 63) as usize,
            ((code >> 6) & 63) as usize,
            (code >> 12) as u8,
        );
        let piece = self.take(from);
        if piece == 0 {
            return None;
        }
        let role = (piece - 1) / 2 + 1;
        let captured = self.take(to);
        let mut irreversible = captured != 0;
        if role == Role::Pawn as u8 {
            irreversible = true;
            if from % 8 != to % 8 && captured == 0 {
                // En passant: the captured pawn stands beside the mover.
                self.take(from / 8 * 8 + to % 8);
            }
        }
        if role == Role::King as u8 && (from % 8).abs_diff(to % 8) == 2 {
            let rank = from / 8 * 8;
            let (rook_from, rook_to) = if to % 8 > from % 8 {
                (rank + 7, rank + 5)
            } else {
                (rank, rank + 3)
            };
            let rook = self.take(rook_from);
            if rook != 0 {
                self.put(rook_to, rook);
            }
            irreversible = true;
        }
        let placed = if promotion == 0 {
            piece
        } else {
            (promotion - 1) * 2 + (piece - 1) % 2 + 1
        };
        self.put(to, placed);
        Some(irreversible)
    }
}

/// The start position and up to `limit` packed main-line moves of `game`.
/// `None` when the start position is invalid or a main-line move is illegal.
pub fn line_moves(game: &Game, limit: usize) -> Option<(Chess, Vec<u16>)> {
    let start = game.chess_position(0).ok()?;
    let mut pos = start.clone();
    let mut node = 0usize;
    let mut moves = vec![];
    while moves.len() < limit {
        let Some(&child) = game.nodes.get(node)?.children.first() else {
            break;
        };
        let uci = game.nodes.get(child as usize)?.uci.as_deref()?;
        if uci == "0000" {
            pos = chess_core::model::apply_uci(pos, uci).ok()?;
            moves.push(NULL_MOVE);
        } else {
            let m = UciMove::from_ascii(uci.as_bytes())
                .ok()?
                .to_move(&pos)
                .ok()?;
            moves.push(pack_move(&m)?);
            pos.play_unchecked(m);
        }
        node = child as usize;
    }
    Some((start, moves))
}

/// Calls `visit(hash, black_to_move, next_move)` for each position of a
/// line replayed from `replay`: after 0..=`moves.len()` moves, the last with
/// [`NO_MOVE`]. Stops after position `last` or when `visit` returns false. A
/// position repeated within the line is visited once, at its first
/// occurrence.
fn each_position(
    mut replay: Replay,
    moves: &[u16],
    last: usize,
    mut visit: impl FnMut(u64, bool, u16) -> bool,
) {
    // Hashes since the last irreversible move; only those can repeat.
    let mut window: Vec<u64> = Vec::with_capacity(16);
    for i in 0..=moves.len().min(last) {
        let hash = replay.hash();
        let next = moves.get(i).copied().unwrap_or(NO_MOVE);
        if !window.contains(&hash) {
            if !visit(hash, replay.black_to_move(), next) {
                return;
            }
            window.push(hash);
        }
        if i == moves.len() {
            break;
        }
        match replay.play(next) {
            Some(true) => window.clear(),
            Some(false) => {}
            None => return,
        }
    }
}

/// The header fields a tree counts: result, ratings and year.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameStats {
    /// 0 unknown, 1 White won, 2 draw, 3 Black won.
    pub result: u8,
    pub white_elo: u16,
    pub black_elo: u16,
    pub year: u16,
}

impl GameStats {
    /// Forfeits count as their PGN result, as in the search mask.
    pub fn of(raw: &RawEntry) -> Self {
        let result = match raw.result_code.and_then(crate::header::pgn_result) {
            Some("1-0") => 1,
            Some("1/2-1/2") => 2,
            Some("0-1") => 3,
            _ => 0,
        };
        Self {
            result,
            white_elo: raw.white_elo.unwrap_or(0),
            black_elo: raw.black_elo.unwrap_or(0),
            year: raw.year.unwrap_or(0),
        }
    }
}

/// Performance rating differences for scores of 50% to 100% in steps of one
/// percent: FIDE's conversion table (Handbook B.02, 8.1a). The same table
/// is in the reference binary at .data 0x141978ad0.
pub(crate) const DP: [u16; 51] = [
    0, 7, 14, 21, 29, 36, 43, 50, 57, 65, 72, 80, 87, 95, 102, 110, 117, 125, 133, 141, 149, 158,
    166, 175, 184, 193, 202, 211, 220, 230, 240, 251, 262, 273, 284, 296, 309, 322, 336, 351, 366,
    383, 401, 422, 444, 470, 501, 538, 589, 677, 800,
];

/// Games after one move from a position, seen from the side to move there.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStats {
    /// The packed move; [`NO_MOVE`] for games that end in the position.
    pub code: u16,
    pub games: u64,
    pub wins: u64,
    pub draws: u64,
    pub losses: u64,
    /// Sum and count of the mover's ratings, where rated.
    pub elo_sum: u64,
    pub elo_count: u64,
    /// Games with a rated opponent and a known result: the opponents' rating
    /// sum, the number of such games and the points scored, doubled.
    pub opponent_sum: u64,
    pub opponent_count: u64,
    pub points2: u64,
    /// First and last year played; 0 when no game has a year.
    pub year_min: u16,
    pub year_max: u16,
}

impl MoveStats {
    pub fn new(code: u16) -> Self {
        Self {
            code,
            ..Default::default()
        }
    }
    /// Counts one game; `black` is whether Black moves in the position.
    pub fn add(&mut self, game: &GameStats, black: bool) {
        self.games += 1;
        let (mine, theirs) = if black {
            (game.black_elo, game.white_elo)
        } else {
            (game.white_elo, game.black_elo)
        };
        let points2 = match (game.result, black) {
            (0, _) => None,
            (2, _) => Some(1),
            (1, false) | (3, true) => Some(2),
            _ => Some(0),
        };
        match points2 {
            Some(2) => self.wins += 1,
            Some(1) => self.draws += 1,
            Some(_) => self.losses += 1,
            None => {}
        }
        if mine > 0 {
            self.elo_sum += u64::from(mine);
            self.elo_count += 1;
        }
        if let (true, Some(p)) = (theirs > 0, points2) {
            self.opponent_sum += u64::from(theirs);
            self.opponent_count += 1;
            self.points2 += p;
        }
        if game.year > 0 {
            if self.year_min == 0 || game.year < self.year_min {
                self.year_min = game.year;
            }
            self.year_max = self.year_max.max(game.year);
        }
    }
    pub fn uci(&self) -> Option<String> {
        unpack_uci(self.code)
    }
    /// Percentage scored by the mover in games with a known result.
    pub fn score(&self) -> Option<f64> {
        let decided = self.wins + self.draws + self.losses;
        (decided > 0).then(|| (self.wins as f64 + self.draws as f64 / 2.0) * 100.0 / decided as f64)
    }
    /// Percentage of draws among games with a known result.
    pub fn draw_rate(&self) -> Option<f64> {
        let decided = self.wins + self.draws + self.losses;
        (decided > 0).then(|| self.draws as f64 * 100.0 / decided as f64)
    }
    pub fn average_elo(&self) -> Option<u32> {
        (self.elo_count > 0).then(|| ((self.elo_sum + self.elo_count / 2) / self.elo_count) as u32)
    }
    /// The mover's performance: the opponents' average rating plus FIDE's
    /// rating difference for the score.
    pub fn performance(&self) -> Option<u32> {
        if self.opponent_count == 0 {
            return None;
        }
        let average = self.opponent_sum as f64 / self.opponent_count as f64;
        let percent = (self.points2 as f64 * 50.0 / self.opponent_count as f64).round() as i64;
        let dp = if percent >= 50 {
            i64::from(DP[(percent - 50) as usize])
        } else {
            -i64::from(DP[(50 - percent) as usize])
        };
        Some((average.round() as i64 + dp).max(0) as u32)
    }
    /// Takes back one game counted by [`MoveStats::add`]. The years stay.
    pub fn remove(&mut self, game: &GameStats, black: bool) {
        let mut one = Self::new(self.code);
        one.add(game, black);
        self.games = self.games.saturating_sub(one.games);
        self.wins = self.wins.saturating_sub(one.wins);
        self.draws = self.draws.saturating_sub(one.draws);
        self.losses = self.losses.saturating_sub(one.losses);
        self.elo_sum = self.elo_sum.saturating_sub(one.elo_sum);
        self.elo_count = self.elo_count.saturating_sub(one.elo_count);
        self.opponent_sum = self.opponent_sum.saturating_sub(one.opponent_sum);
        self.opponent_count = self.opponent_count.saturating_sub(one.opponent_count);
        self.points2 = self.points2.saturating_sub(one.points2);
    }
    fn merge(&mut self, other: &Self) {
        self.games += other.games;
        self.wins += other.wins;
        self.draws += other.draws;
        self.losses += other.losses;
        self.elo_sum += other.elo_sum;
        self.elo_count += other.elo_count;
        self.opponent_sum += other.opponent_sum;
        self.opponent_count += other.opponent_count;
        self.points2 += other.points2;
        if other.year_min > 0 && (self.year_min == 0 || other.year_min < self.year_min) {
            self.year_min = other.year_min;
        }
        self.year_max = self.year_max.max(other.year_max);
    }
}

/// Counts a game under its move, adding a row for a new move.
pub(crate) fn add_game(moves: &mut Vec<MoveStats>, code: u16, game: &GameStats, black: bool) {
    match moves.iter_mut().find(|s| s.code == code) {
        Some(s) => s.add(game, black),
        None => {
            let mut s = MoveStats::new(code);
            s.add(game, black);
            moves.push(s);
        }
    }
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn get_varint(bytes: &[u8], at: &mut usize) -> Result<u64, CbhError> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *bytes.get(*at).ok_or(CbhError::Truncated)?;
        *at += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(CbhError::Decode("opening tree: varint too long".into()))
}

const MAGIC: &[u8; 8] = b"CWTREE01";
const HEADER: u64 = 128;
/// Directory entries per summary key.
const STRIDE: u64 = 256;
const DIR_ENTRY: u64 = 16;

/// An opening tree file opened for reading.
pub struct OpeningTree {
    file: std::fs::File,
    journal: journal::Journal,
    plies: usize,
    min_games: usize,
    games: u64,
    positions: u64,
    dir_offset: u64,
    summary: Vec<u64>,
}

/// A stored position: per-move statistics and where its games are.
#[derive(Clone, Debug)]
pub struct Entry {
    pub moves: Vec<MoveStats>,
    /// Per move: its only game, or where its games are listed.
    postings: Vec<Postings>,
}

#[derive(Clone, Copy, Debug)]
enum Postings {
    Single(u64),
    Listed { offset: u64, len: u64 },
}

/// Where an answer came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// The position is stored in the tree.
    Tree,
    /// The position is not stored; games were followed from the last stored
    /// position of the line and replayed.
    Line,
    /// Neither the position nor any earlier position of the line is stored.
    None,
}

/// Games as (entry number, packed next move).
pub type Games = Vec<(u64, u16)>;

/// Statistics and games for one position.
#[derive(Clone, Debug)]
pub struct Answer {
    pub source: Source,
    /// Per next move, most played first; [`NO_MOVE`] for games ending here.
    pub moves: Vec<MoveStats>,
    /// Games reaching the position (entry number, next move), in entry order.
    pub games: Vec<(u64, u16)>,
    /// The game list stopped at its limit.
    pub truncated: bool,
}

impl OpeningTree {
    /// Opens `path` if it was built from `db` as it is now.
    pub fn open(path: &Path, db: &Database) -> Result<Option<Self>, CbhError> {
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut head = [0u8; HEADER as usize];
        if file.read_exact(&mut head).is_err() || &head[..8] != MAGIC {
            return Ok(None);
        }
        let u64_at = |i: usize| u64::from_le_bytes(head[i..i + 8].try_into().unwrap());
        let plies = u32::from_le_bytes(head[12..16].try_into().unwrap()) as usize;
        let stamp: Vec<u64> = (0..4).map(|i| u64_at(24 + 8 * i)).collect();
        let (games, positions, dir_offset, summary_offset, length) =
            (u64_at(56), u64_at(64), u64_at(72), u64_at(80), u64_at(88));
        let min_games = u32::from_le_bytes(head[96..100].try_into().unwrap()) as usize;
        let summary_len = positions.div_ceil(STRIDE);
        let base = (u64_at(16), [stamp[0], stamp[1], stamp[2], stamp[3]]);
        let now = journal::stamp(db);
        // Changes the workbench saved since the build are in the journal.
        let journal = if base == now {
            journal::Journal::default()
        } else {
            match journal::Journal::load(&journal::journal_path(path))? {
                Some(j) if j.base == base && j.current == now => j,
                _ => return Ok(None),
            }
        };
        if min_games < 2
            || file.metadata()?.len() != length
            || dir_offset + positions * DIR_ENTRY != summary_offset
            || summary_offset + summary_len * 8 != length
        {
            return Ok(None);
        }
        file.seek(SeekFrom::Start(summary_offset))?;
        let mut bytes = vec![0u8; summary_len as usize * 8];
        file.read_exact(&mut bytes)?;
        let summary = bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| u64::from_le_bytes(*b))
            .collect();
        Ok(Some(Self {
            file,
            journal,
            plies,
            min_games,
            games,
            positions,
            dir_offset,
            summary,
        }))
    }

    pub fn plies(&self) -> usize {
        self.plies
    }
    /// Games a position needs to be stored.
    pub fn min_games(&self) -> usize {
        self.min_games
    }
    /// Games whose main line was read into the tree.
    pub fn games(&self) -> u64 {
        self.games
    }
    pub fn positions(&self) -> u64 {
        self.positions
    }

    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, CbhError> {
        let mut bytes = vec![0u8; len];
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    /// The stored position with this hash.
    pub fn lookup(&mut self, db: &Database, hash: u64) -> Result<Option<Entry>, CbhError> {
        let block = self.summary.partition_point(|&s| s <= hash);
        if block == 0 {
            return Ok(None);
        }
        let first = (block as u64 - 1) * STRIDE;
        let n = STRIDE.min(self.positions - first);
        let dir = self.read_at(
            self.dir_offset + first * DIR_ENTRY,
            (n * DIR_ENTRY) as usize,
        )?;
        let entries: Vec<(u64, u64)> = dir
            .as_chunks::<{ DIR_ENTRY as usize }>()
            .0
            .iter()
            .map(|e| {
                (
                    u64::from_le_bytes(e[..8].try_into().unwrap()),
                    u64::from_le_bytes(e[8..].try_into().unwrap()),
                )
            })
            .collect();
        let Ok(i) = entries.binary_search_by_key(&hash, |e| e.0) else {
            return Ok(None);
        };
        let offset = entries[i].1;
        if offset < HEADER || offset >= self.dir_offset {
            return Err(CbhError::Decode("opening tree: bad entry offset".into()));
        }
        // The head usually fits in the first read.
        let available = (self.dir_offset - offset).min(4096) as usize;
        let mut bytes = self.read_at(offset, available)?;
        let mut at = 0;
        let head_len = get_varint(&bytes, &mut at)? as usize;
        if at + head_len > bytes.len() {
            if offset + (at + head_len) as u64 > self.dir_offset {
                return Err(CbhError::Truncated);
            }
            bytes = self.read_at(offset, at + head_len)?;
        }
        let head = &bytes[at..at + head_len];
        let mut postings_at = offset + (at + head_len) as u64;
        let mut p = 0;
        let black = get_varint(head, &mut p)? & 1 == 1;
        let count = get_varint(head, &mut p)?;
        let mut moves = Vec::with_capacity(count.min(256) as usize);
        let mut postings = Vec::with_capacity(moves.capacity());
        for _ in 0..count {
            let code = u16::try_from(get_varint(head, &mut p)?)
                .map_err(|_| CbhError::Decode("opening tree: bad move".into()))?;
            let games = get_varint(head, &mut p)?;
            if games == 1 {
                let ordinal = get_varint(head, &mut p)?;
                if ordinal == 0 || ordinal > db.info().entry_count {
                    return Err(CbhError::Decode("opening tree: bad entry number".into()));
                }
                let mut s = MoveStats::new(code);
                // A game changed since the build is counted as it was then.
                let stats = match self.journal.changes.get(&ordinal) {
                    Some((Some(old), _)) => old.stats,
                    _ => GameStats::of(&db.raw_chunk(ordinal, 1)?[0]),
                };
                s.add(&stats, black);
                moves.push(s);
                postings.push(Postings::Single(ordinal));
                continue;
            }
            let mut v = [0u64; 11];
            for slot in &mut v {
                *slot = get_varint(head, &mut p)?;
            }
            moves.push(MoveStats {
                code,
                games,
                wins: v[0],
                draws: v[1],
                losses: v[2],
                elo_sum: v[3],
                elo_count: v[4],
                opponent_sum: v[5],
                opponent_count: v[6],
                points2: v[7],
                year_min: v[8] as u16,
                year_max: v[9] as u16,
            });
            postings.push(Postings::Listed {
                offset: postings_at,
                len: v[10],
            });
            postings_at += v[10];
        }
        if postings_at > self.dir_offset {
            return Err(CbhError::Truncated);
        }
        Ok(Some(Entry { moves, postings }))
    }

    /// Games of `entry` in entry order, at most `limit`; only those that
    /// played `only` when given. Also says whether the list stopped early.
    pub fn entry_games(
        &mut self,
        entry: &Entry,
        only: Option<u16>,
        limit: usize,
    ) -> Result<(Vec<(u64, u16)>, bool), CbhError> {
        let mut all = vec![];
        let mut truncated = false;
        for (stats, &postings) in entry.moves.iter().zip(&entry.postings) {
            if only.is_some_and(|m| m != stats.code) {
                continue;
            }
            let (offset, len) = match postings {
                Postings::Single(ordinal) => {
                    if !self.journal.changed(ordinal) {
                        all.push((ordinal, stats.code));
                    }
                    continue;
                }
                Postings::Listed { offset, len } => (offset, len),
            };
            let bytes = self.read_at(offset, len as usize)?;
            let (mut at, mut ordinal) = (0, 0u64);
            let mut taken = 0;
            while at < bytes.len() {
                if taken == limit {
                    truncated = true;
                    break;
                }
                ordinal += get_varint(&bytes, &mut at)?;
                // Changed games are listed from the journal.
                if !self.journal.changed(ordinal) {
                    all.push((ordinal, stats.code));
                    taken += 1;
                }
            }
        }
        all.sort_unstable();
        if all.len() > limit {
            all.truncate(limit);
            truncated = true;
        }
        Ok((all, truncated))
    }

    /// Statistics and games for the last position of a line. `hashes` are the
    /// line's positions from its start, `moves` the packed moves between
    /// them. A position the tree does not store is answered by replaying the
    /// games that left the last stored position of the line with the line's
    /// move, when there are at most `max_follow` of them.
    pub fn query(
        &mut self,
        db: &Database,
        hashes: &[u64],
        moves: &[u16],
        limit: usize,
        max_follow: usize,
        cancel: &AtomicBool,
    ) -> Result<Answer, CbhError> {
        let Some(&target) = hashes.last() else {
            return Err(CbhError::Bounds);
        };
        if let Some(entry) = self.lookup(db, target)? {
            let (mut games, truncated) = self.entry_games(&entry, None, limit)?;
            let mut moves = entry.moves;
            self.journal.apply(target, &mut moves, &mut games, |_| true);
            games.truncate(limit);
            return Ok(Answer {
                source: Source::Tree,
                moves,
                games,
                truncated,
            });
        }
        for i in (0..hashes.len() - 1).rev() {
            let Some(entry) = self.lookup(db, hashes[i])? else {
                continue;
            };
            let (candidates, too_many) = self.entry_games(&entry, Some(moves[i]), max_follow)?;
            if too_many {
                return Err(CbhError::LimitExceeded);
            }
            let ordinals: Vec<u64> = candidates.iter().map(|c| c.0).collect();
            let (mut moves, mut games) = db.line_answer(&ordinals, target, limit, cancel)?;
            self.journal
                .apply(target, &mut moves, &mut games, |_| false);
            let truncated = games.len() >= limit;
            games.truncate(limit);
            return Ok(Answer {
                source: Source::Line,
                moves,
                games,
                truncated,
            });
        }
        Ok(Answer {
            source: Source::None,
            moves: vec![],
            games: vec![],
            truncated: false,
        })
    }
}

impl Database {
    /// The header statistics of entry `ordinal` and the positions of its main
    /// line after at most `plies` half-moves: hash, whether Black is to move,
    /// and the move played there. `None` for text entries and games whose
    /// moves cannot be replayed.
    pub fn main_line(&self, ordinal: u64, plies: usize) -> Result<Option<GameLine>, CbhError> {
        let raw = self.raw_chunk(ordinal, 1)?.remove(0);
        let Some(game) = self.moves_of(&raw)? else {
            return Ok(None);
        };
        let Some((start, moves)) = line_moves(&game, plies.saturating_add(1)) else {
            return Ok(None);
        };
        let mut line = vec![];
        each_position(
            Replay::new(start.board(), start.turn()),
            &moves,
            plies,
            |h, b, m| {
                line.push((h, b, m));
                true
            },
        );
        Ok(Some(GameLine {
            stats: GameStats::of(&raw),
            positions: line,
        }))
    }

    /// Statistics of the games among `ordinals` whose main line reaches the
    /// position `target`, by the move each played there, and those games (at
    /// most `limit`).
    pub fn line_answer(
        &self,
        ordinals: &[u64],
        target: u64,
        limit: usize,
        cancel: &AtomicBool,
    ) -> Result<(Vec<MoveStats>, Games), CbhError> {
        let mut moves: Vec<MoveStats> = vec![];
        let mut games = vec![];
        for &ordinal in ordinals {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let raw = self.raw_chunk(ordinal, 1)?.remove(0);
            let Some(game) = self.moves_of(&raw)? else {
                continue;
            };
            let Some((start, line)) = line_moves(&game, usize::MAX) else {
                continue;
            };
            let mut found = None;
            each_position(
                Replay::new(start.board(), start.turn()),
                &line,
                usize::MAX,
                |h, b, m| {
                    if h == target {
                        found = Some((b, m));
                        return false;
                    }
                    true
                },
            );
            let Some((black, code)) = found else {
                continue;
            };
            add_game(&mut moves, code, &GameStats::of(&raw), black);
            if games.len() < limit {
                games.push((ordinal, code));
            }
        }
        moves.sort_by(|a, b| b.games.cmp(&a.games).then(a.code.cmp(&b.code)));
        Ok((moves, games))
    }

    /// Statistics by next move, from the main lines of the given games that
    /// reach `target`. For results of a position search.
    pub fn stats_of_games(
        &self,
        ordinals: &[u64],
        target: u64,
        cancel: &AtomicBool,
    ) -> Result<Vec<MoveStats>, CbhError> {
        Ok(self.line_answer(ordinals, target, 0, cancel)?.0)
    }
}

/// Totals over the moves of an answer.
pub fn total(moves: &[MoveStats]) -> MoveStats {
    let mut all = MoveStats::new(NO_MOVE);
    for m in moves {
        all.merge(m);
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(moves: &[&str]) -> Chess {
        let mut pos = Chess::default();
        for m in moves {
            pos = chess_core::model::apply_uci(pos, m).unwrap();
        }
        pos
    }

    #[test]
    fn hash_ignores_move_order_but_not_turn() {
        let hash = |moves: &[&str]| {
            let pos = play(moves);
            position_hash(pos.board(), pos.turn())
        };
        assert_eq!(
            hash(&["g1f3", "g8f6", "d2d4"]),
            hash(&["d2d4", "g8f6", "g1f3"])
        );
        assert_eq!(hash(&[]), hash(&["g1f3", "g8f6", "f3g1", "f6g8"]));
        assert_ne!(hash(&[]), hash(&["0000"]));
    }

    #[test]
    fn replay_matches_shakmaty() {
        // Castling both ways, en passant, promotion, null move.
        let line = [
            "e2e4", "d7d5", "e4e5", "f7f5", "e5f6", "g7f6", "g1f3", "b8c6", "f1e2", "c8e6", "e1g1",
            "d8d7", "0000", "e8c8", "b2b4", "f6f5", "b4b5", "f5f4", "b5c6", "d7d6", "c6b7", "c8d7",
            "b7b8q",
        ];
        let mut pos = Chess::default();
        let mut replay = Replay::new(pos.board(), pos.turn());
        for uci in line {
            let code = if uci == "0000" {
                NULL_MOVE
            } else {
                let m = UciMove::from_ascii(uci.as_bytes())
                    .unwrap()
                    .to_move(&pos)
                    .unwrap();
                pack_move(&m).unwrap()
            };
            assert_eq!(unpack_uci(code).as_deref(), Some(uci));
            pos = chess_core::model::apply_uci(pos, uci).unwrap();
            replay.play(code).unwrap();
            assert_eq!(
                replay.hash(),
                position_hash(pos.board(), pos.turn()),
                "{uci}"
            );
        }
    }

    #[test]
    fn performance_follows_the_fide_table() {
        let mut s = MoveStats::new(1);
        for result in [1, 1, 2, 3] {
            s.add(
                &GameStats {
                    result,
                    white_elo: 2500,
                    black_elo: 2400,
                    year: 2000,
                },
                false,
            );
        }
        assert_eq!(s.score(), Some(62.5));
        assert_eq!(s.draw_rate(), Some(25.0));
        assert_eq!(s.average_elo(), Some(2500));
        // 62.5% rounds to 63%: +95.
        assert_eq!(s.performance(), Some(2495));
        assert_eq!((s.year_min, s.year_max), (2000, 2000));
    }

    #[test]
    fn varints_round_trip() {
        let mut out = vec![];
        for v in [0, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX] {
            put_varint(&mut out, v);
        }
        let mut at = 0;
        for v in [0, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX] {
            assert_eq!(get_varint(&out, &mut at).unwrap(), v);
        }
    }
}
