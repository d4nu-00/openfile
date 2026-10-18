//! The search mask's position tab (the reference app's dialog 314). See
//! docs/position-search.md.
//!
//! Three boards say where pieces must stand ('Look for'), where at least one
//! of them must stand ('Or') and where they must not ('Exclude'). Squares not
//! on a board may hold anything, so the search finds a pattern. With `exact`,
//! the 'Look for' board is instead a whole position, found as Find Position
//! finds one.
use crate::{CbhError, position::Target};
use chess_core::{Game, NodeId};
use shakmaty::{Bitboard, Board, Chess, Color, Piece, Position, Role, Square};

/// The piece kinds of a board, in the reference app's code order 1..=7: K, Q, N, B,
/// R, P, and any piece.
const ROLES: [Option<Role>; 7] = [
    Some(Role::King),
    Some(Role::Queen),
    Some(Role::Knight),
    Some(Role::Bishop),
    Some(Role::Rook),
    Some(Role::Pawn),
    None,
];

/// What the position tab asks for.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PositionQuery {
    /// Pieces as (square, letter): "KQRBNP" White, "kqrbnp" Black, "X" any
    /// white piece, "x" any black piece. A square may appear more than once.
    pub look_for: Vec<(String, char)>,
    pub or: Vec<(String, char)>,
    pub exclude: Vec<(String, char)>,
    /// The 'Look for' board is a whole position with `white_to_move`.
    pub exact: bool,
    /// Side to move; None for either (patterns only).
    pub white_to_move: Option<bool>,
    /// Also ranks 1↔8 with the colours swapped.
    pub horizontal: bool,
    /// Also files a↔h.
    pub vertical: bool,
    /// Move numbers: the first and last move tried, and how many consecutive
    /// positions must match.
    pub first: Option<u16>,
    pub last: Option<u16>,
    pub length: Option<u16>,
    /// Patterns: search variations too, not only the main line.
    pub lines: bool,
}

/// One board: a bitboard per piece kind and colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Boards {
    white: [Bitboard; 7],
    black: [Bitboard; 7],
}

impl Boards {
    fn parse(pieces: &[(String, char)]) -> Result<Self, CbhError> {
        let mut b = Self::default();
        for (square, letter) in pieces {
            let sq: Square = square
                .parse()
                .map_err(|_| CbhError::Decode(format!("Invalid square {square}")))?;
            let kind = match letter.to_ascii_lowercase() {
                'k' => 0,
                'q' => 1,
                'n' => 2,
                'b' => 3,
                'r' => 4,
                'p' => 5,
                'x' => 6,
                _ => return Err(CbhError::Decode(format!("Invalid piece {letter}"))),
            };
            let side = if letter.is_ascii_uppercase() {
                &mut b.white
            } else {
                &mut b.black
            };
            side[kind].add(sq);
        }
        Ok(b)
    }
    fn map(&self, f: impl Fn(Bitboard) -> Bitboard, swap: bool) -> Self {
        let (white, black) = if swap {
            (self.black, self.white)
        } else {
            (self.white, self.black)
        };
        Self {
            white: white.map(&f),
            black: black.map(&f),
        }
    }
    fn is_empty(&self) -> bool {
        self.white.iter().chain(&self.black).all(|b| b.is_empty())
    }
}

/// The three boards of one orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Set {
    look_for: Boards,
    or: Boards,
    exclude: Boards,
}

impl Set {
    fn map(&self, f: impl Fn(Bitboard) -> Bitboard + Copy, swap: bool) -> Self {
        Self {
            look_for: self.look_for.map(f, swap),
            or: self.or.map(f, swap),
            exclude: self.exclude.map(f, swap),
        }
    }
    /// The reference app's test (`0x14018afb0`), for every piece kind of both sides.
    fn matches(&self, board: &Board) -> bool {
        [Color::White, Color::Black].into_iter().all(|color| {
            let pick = |b: &Boards| if color.is_white() { b.white } else { b.black };
            let (look, or, exclude) = (pick(&self.look_for), pick(&self.or), pick(&self.exclude));
            let mine = board.by_color(color);
            (0..7).all(|i| {
                let here = match ROLES[i] {
                    Some(role) => mine & board.by_role(role),
                    None => mine,
                };
                ((!exclude[i] & here) | look[i]) == here
                    && (or[i].is_empty() || !(here & or[i]).is_empty())
            })
        })
    }
}

/// A position tab query ready to run.
#[derive(Clone, Debug)]
pub enum Compiled {
    /// Whole positions: the board and its mirrors.
    Exact(Vec<Target>),
    Pattern(Pattern),
}

#[derive(Clone, Debug)]
pub struct Pattern {
    sets: Vec<Set>,
    side: Option<Color>,
    /// Plies tried: `first_ply ..= last_ply`.
    first_ply: u32,
    last_ply: u32,
    length: u32,
    lines: bool,
}

impl PositionQuery {
    pub fn compile(&self) -> Result<Compiled, CbhError> {
        let first = self.first.unwrap_or(1).max(1);
        let last = self.last.map(|l| l.max(first));
        if self.exact {
            let board = Boards::parse(&self.look_for)?;
            let mut setup = Board::empty();
            for (i, role) in ROLES.iter().enumerate() {
                let Some(role) = role else {
                    if !board.white[i].is_empty() || !board.black[i].is_empty() {
                        return Err(CbhError::Decode(
                            "An exact position cannot hold \"any piece\"".into(),
                        ));
                    }
                    continue;
                };
                for (color, squares) in [
                    (Color::White, board.white[i]),
                    (Color::Black, board.black[i]),
                ] {
                    for sq in squares {
                        if setup.piece_at(sq).is_some() {
                            return Err(CbhError::Decode(format!(
                                "An exact position has one piece on {sq}"
                            )));
                        }
                        setup.set_piece_at(sq, Piece { color, role: *role });
                    }
                }
            }
            let turn = Color::from_white(self.white_to_move.unwrap_or(true));
            let mut targets = vec![];
            if setup.occupied().is_empty() {
                return Err(CbhError::Decode("The board is empty".into()));
            }
            let mut add = |board: Board, turn: Color| -> Result<(), CbhError> {
                let target = Target::from_board(board, turn);
                if !targets.iter().any(|t: &Target| t.same(&target)) {
                    targets.push(target);
                }
                Ok(())
            };
            add(setup.clone(), turn)?;
            if self.horizontal {
                let mut b = setup.clone();
                b.flip_vertical();
                b.swap_colors();
                add(b, !turn)?;
            }
            if self.vertical {
                let mut b = setup.clone();
                b.flip_horizontal();
                add(b, turn)?;
            }
            if self.horizontal && self.vertical {
                let mut b = setup.clone();
                b.flip_vertical();
                b.flip_horizontal();
                b.swap_colors();
                add(b, !turn)?;
            }
            return Ok(Compiled::Exact(targets));
        }
        let set = Set {
            look_for: Boards::parse(&self.look_for)?,
            or: Boards::parse(&self.or)?,
            exclude: Boards::parse(&self.exclude)?,
        };
        if set.look_for.is_empty() && set.or.is_empty() && set.exclude.is_empty() {
            return Err(CbhError::Decode("The boards are empty".into()));
        }
        let mut sets = vec![set];
        if self.horizontal {
            sets.push(set.map(Bitboard::flip_vertical, true));
        }
        if self.vertical {
            sets.push(set.map(Bitboard::flip_horizontal, false));
        }
        if self.horizontal && self.vertical {
            sets.push(set.map(|b| b.flip_vertical().flip_horizontal(), true));
        }
        Ok(Compiled::Pattern(Pattern {
            sets,
            side: self.white_to_move.map(Color::from_white),
            first_ply: 2 * u32::from(first) - 2,
            last_ply: last.map_or(u32::MAX, |l| 2 * u32::from(l) - 1),
            length: u32::from(self.length.unwrap_or(1).max(1)),
            lines: self.lines,
        }))
    }
}

impl Pattern {
    fn matches(&self, pos: &Chess) -> bool {
        self.sets.iter().any(|s| s.matches(pos.board()))
    }

    /// The first position of the first run of `length` matching positions:
    /// on the main line, then (with `lines`) in variations, depth first.
    pub fn find_in(&self, game: &Game) -> Result<Option<NodeId>, CbhError> {
        // (node, position, ply, run length, run start)
        let mut stack: Vec<(NodeId, Chess, u32, u32, NodeId)> =
            vec![(0, game.chess_position(0)?, 0, 0, 0)];
        while let Some((id, pos, ply, run, start)) = stack.pop() {
            if ply > self.last_ply {
                continue;
            }
            let (mut run, mut start) = (run, start);
            if ply >= self.first_ply && self.side.is_none_or(|s| s == pos.turn()) {
                if self.matches(&pos) {
                    if run == 0 {
                        start = id;
                    }
                    run += 1;
                    if run >= self.length {
                        return Ok(Some(start));
                    }
                } else {
                    run = 0;
                }
            }
            let children = &game.nodes[id as usize].children;
            let followed = if self.lines {
                children.len()
            } else {
                children.len().min(1)
            };
            for &child in children[..followed].iter().rev() {
                let uci = game.nodes[child as usize].uci.as_deref().unwrap_or("");
                if let Ok(next) = chess_core::model::apply_uci(pos.clone(), uci) {
                    stack.push((child, next, ply + 1, run, start));
                }
            }
        }
        Ok(None)
    }
}
