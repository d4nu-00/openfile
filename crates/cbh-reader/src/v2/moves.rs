//! 2CBG standard move trees.
//!
//! The token table is rebuilt from the order of the native generators called
//! by `0x140203bb0`: pieces by kind and colour, source squares file-major
//! (`8 * file + rank`), directions in generator order and one token per
//! captured-piece alternative. Only the primary standard-chess section
//! (45,356 tokens) is reconstructed; extended sections are unsupported.
use crate::{CbhError, Diagnostic, ReadLimits};
use chess_core::{Game, GameBuilder, NodeId};
use shakmaty::{Color, File, Position, Rank, Role, Square, uci::UciMove};
use std::sync::OnceLock;

pub const START_STANDARD: u16 = 0xfffc;
pub const START_SETUP: u16 = 0xfffb;
/// First placement token of the extended table: ten nonpawn kinds (white
/// K Q N B R, then black) on 64 squares, then white and black pawns on the 48
/// squares of ranks 2-7, file-major.
pub const PLACEMENT_BASE: u16 = 0xc02d;
const PLACEMENTS: u16 = 10 * 64 + 2 * 48;
pub const NULL_MOVE: u16 = 0xfffa;
pub const VARIATION: u16 = 0xfffd;
pub const MOVE_FLAG: u16 = 0xfffe;
pub const END_LINE: u16 = 0xffff;

#[derive(Clone, Copy, Debug)]
pub struct TokenMove {
    pub from: u8,
    pub to: u8,
    pub color: Color,
    pub role: Role,
    pub promotion: Option<Role>,
}

const KING: &[(i8, i8)] = &[
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];
const QUEEN: &[(i8, i8)] = &[
    (-1, -1),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, 0),
    (0, -1),
    (1, 0),
    (0, 1),
];
const KNIGHT: &[(i8, i8)] = &[
    (-2, -1),
    (-2, 1),
    (2, -1),
    (2, 1),
    (-1, -2),
    (-1, 2),
    (1, -2),
    (1, 2),
];
const BISHOP: &[(i8, i8)] = &[(-1, -1), (1, -1), (1, 1), (-1, 1)];
const ROOK: &[(i8, i8)] = &[(-1, 0), (0, -1), (1, 0), (0, 1)];
/// Nonpawn captures: none, queen, knight, bishop, rook, pawn.
const NONPAWN_CAPTURES: usize = 6;
const PROMOTIONS: [Role; 4] = [Role::Queen, Role::Knight, Role::Bishop, Role::Rook];

pub fn table() -> &'static [Option<TokenMove>] {
    static TABLE: OnceLock<Vec<Option<TokenMove>>> = OnceLock::new();
    TABLE.get_or_init(build)
}

fn build() -> Vec<Option<TokenMove>> {
    let mut table = vec![None];
    let add = |table: &mut Vec<_>, from: u8, to: u8, color, role, promotion| {
        table.push(Some(TokenMove {
            from,
            to,
            color,
            role,
            promotion,
        }))
    };
    let pieces = [
        (Role::King, KING, false),
        (Role::Queen, QUEEN, true),
        (Role::Knight, KNIGHT, false),
        (Role::Bishop, BISHOP, true),
        (Role::Rook, ROOK, true),
    ];
    for color in [Color::White, Color::Black] {
        for (role, directions, slides) in pieces {
            for from in 0..64u8 {
                let (file, rank) = ((from / 8) as i8, (from % 8) as i8);
                for &(df, dr) in directions {
                    let (mut f, mut r) = (file + df, rank + dr);
                    while (0..8).contains(&f) && (0..8).contains(&r) {
                        for _ in 0..NONPAWN_CAPTURES {
                            add(&mut table, from, (f * 8 + r) as u8, color, role, None);
                        }
                        if !slides {
                            break;
                        }
                        f += df;
                        r += dr;
                    }
                }
            }
        }
    }
    debug_assert_eq!(table.len() - 1, 44_016);
    for color in [Color::White, Color::Black] {
        let step: i8 = if color.is_white() { 1 } else { -1 };
        let (start, last, passant) = if color.is_white() {
            (1, 6, 4)
        } else {
            (6, 1, 3)
        };
        for from in 0..64u8 {
            let (file, rank) = ((from / 8) as i8, (from % 8) as i8);
            if rank == 0 || rank == 7 {
                continue;
            }
            let mut targets = vec![];
            if rank == start {
                targets.push((file, rank + 2 * step, false));
            }
            targets.push((file, rank + step, false));
            if file > 0 {
                targets.push((file - 1, rank + step, true));
            }
            if file < 7 {
                targets.push((file + 1, rank + step, true));
            }
            let promote = rank == last;
            for (f, r, capture) in targets {
                let to = (f * 8 + r) as u8;
                // Captured-piece alternatives: queen, knight, bishop, rook, pawn.
                let captures = match (capture, promote) {
                    (false, _) => 1,
                    (true, true) => 4,
                    (true, false) => 5,
                };
                for captured in 0..captures {
                    if promote {
                        for role in PROMOTIONS {
                            add(&mut table, from, to, color, Role::Pawn, Some(role));
                        }
                    } else {
                        add(&mut table, from, to, color, Role::Pawn, None);
                    }
                    if capture && !promote && captured == 4 && rank == passant {
                        add(&mut table, from, to, color, Role::Pawn, None);
                    }
                }
            }
        }
    }
    debug_assert_eq!(table.len() - 1, 45_352);
    for (from, to, color) in [
        (32, 16, Color::White),
        (32, 48, Color::White),
        (39, 23, Color::Black),
        (39, 55, Color::Black),
    ] {
        add(&mut table, from, to, color, Role::King, None);
    }
    table
}

fn square(cb: u8) -> Square {
    Square::from_coords(File::new((cb / 8) as u32), Rank::new((cb % 8) as u32))
}

#[derive(Debug)]
pub struct Decoded {
    pub game: Game,
    pub diagnostics: Vec<Diagnostic>,
    /// Node id for each decoded move, in stream order (for annotation lookup).
    pub stream_nodes: Vec<NodeId>,
}

struct Reader<'a> {
    words: &'a [u16],
    index: usize,
    builder: GameBuilder,
    depth_limit: usize,
    flags: usize,
    stream_nodes: Vec<NodeId>,
}

impl Reader<'_> {
    fn line(&mut self, parent: NodeId, depth: usize) -> Result<(), CbhError> {
        if depth > self.depth_limit {
            return Err(CbhError::LimitExceeded);
        }
        let mut node = parent;
        let mut attachments = vec![];
        while let Some(&token) = self.words.get(self.index) {
            self.index += 1;
            match token {
                END_LINE => {
                    for &attached in attachments.iter().rev() {
                        let from = self.builder.game().nodes[attached as usize]
                            .parent
                            .ok_or_else(|| CbhError::Decode("Variation at root".into()))?;
                        self.line(from, depth + 1)?;
                    }
                    return Ok(());
                }
                VARIATION | MOVE_FLAG if node == parent => {
                    return Err(CbhError::Decode(format!(
                        "Marker {token:#06x} without a preceding move"
                    )));
                }
                VARIATION => attachments.push(node),
                MOVE_FLAG => self.flags += 1,
                NULL_MOVE => {
                    node = self.builder.append_uci(node, "0000")?;
                    self.stream_nodes.push(node);
                }
                _ => {
                    let entry =
                        table()
                            .get(token as usize)
                            .copied()
                            .flatten()
                            .ok_or_else(|| {
                                CbhError::Unsupported(format!("2CBG move token {token:#06x}"))
                            })?;
                    let pos = self.builder.position(node)?;
                    let (from, to) = (square(entry.from), square(entry.to));
                    let piece = pos.board().piece_at(from);
                    if piece.map(|p| (p.color, p.role)) != Some((entry.color, entry.role))
                        || pos.turn() != entry.color
                    {
                        return Err(CbhError::Decode(format!(
                            "Token {token:#06x} does not match the board"
                        )));
                    }
                    let uci = UciMove::Normal {
                        from,
                        to,
                        promotion: entry.promotion,
                    };
                    uci.to_move(pos)
                        .map_err(|_| CbhError::Decode(format!("Illegal decoded move {uci}")))?;
                    node = self.builder.append_uci(node, &uci.to_string())?;
                    self.stream_nodes.push(node);
                }
            }
        }
        Err(CbhError::Decode("Missing line terminator".into()))
    }
}

/// Set-up start: `0xfffb`, move number, side to move (low byte, 1 = Black)
/// with castling rights (high byte, classic bit order), a word that is zero in
/// every sample, one placement token per piece, then `0xfffc` and the tree.
/// Matches the classic 28-byte setups of all 597 shared Mega Database games.
fn setup(words: &[u16]) -> Result<(Game, usize), CbhError> {
    let controls = words.get(1..4).ok_or(CbhError::Truncated)?;
    let (number, turn, extra) = (controls[0], controls[1], controls[2]);
    if turn & 0xff > 1 || turn >> 12 != 0 || extra != 0 {
        return Err(CbhError::UnsupportedSetup(format!(
            "2CBG setup controls {turn:#06x} {extra:#06x}"
        )));
    }
    let mut board = [' '; 64];
    let mut index = 4;
    loop {
        let token = *words.get(index).ok_or(CbhError::Truncated)?;
        index += 1;
        if token == START_STANDARD {
            break;
        }
        let code = token
            .checked_sub(PLACEMENT_BASE)
            .filter(|&c| c < PLACEMENTS)
            .ok_or_else(|| {
                CbhError::UnsupportedSetup(format!("2CBG placement token {token:#06x}"))
            })?;
        let (piece, file, rank) = if code < 640 {
            let (kind, square) = (code / 64, code % 64);
            (b"KQNBRkqnbr"[kind as usize], square / 8, square % 8)
        } else {
            let (kind, index) = ((code - 640) / 48, (code - 640) % 48);
            (b"Pp"[kind as usize], index / 6, index % 6 + 1)
        };
        let square = &mut board[(rank * 8 + file) as usize];
        if *square != ' ' {
            return Err(CbhError::Decode("Two pieces placed on one square".into()));
        }
        *square = piece as char;
    }
    let fen = crate::moves::setup::fen(&board, turn & 1 == 1, (turn >> 8) as u8, 0, number);
    Ok((Game::from_fen(&fen)?, index))
}

pub fn decode_moves(payload: &[u8], limits: &ReadLimits) -> Result<Decoded, CbhError> {
    if payload.len() > limits.max_game_bytes {
        return Err(CbhError::LimitExceeded);
    }
    if !payload.len().is_multiple_of(2) || payload.len() < 4 {
        return Err(CbhError::Truncated);
    }
    let words: Vec<u16> = payload
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&w| u16::from_le_bytes(w))
        .collect();
    let (start, index) = match words[0] {
        START_STANDARD => (Game::initial(), 1),
        START_SETUP => setup(&words)?,
        other => {
            return Err(CbhError::Unsupported(format!(
                "2CBG start marker {other:#06x}"
            )));
        }
    };
    let mut reader = Reader {
        words: &words,
        index,
        builder: GameBuilder::new(start, limits.game.clone())?,
        depth_limit: limits.game.max_variation_depth,
        flags: 0,
        stream_nodes: vec![],
    };
    reader.line(0, 0)?;
    if reader.index != words.len() {
        return Err(CbhError::Decode(format!(
            "{} words after the move tree",
            words.len() - reader.index
        )));
    }
    let mut diagnostics = vec![];
    if reader.flags > 0 {
        diagnostics.push(Diagnostic::new(
            "uninterpretedMoveFlag",
            format!(
                "{} move flag marker(s) 0xfffe are not interpreted",
                reader.flags
            ),
        ));
    }
    Ok(Decoded {
        game: reader.builder.finish(),
        diagnostics,
        stream_nodes: reader.stream_nodes,
    })
}
