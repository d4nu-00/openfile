//! 2CBG move-tree encoding, the inverse of [`super::moves::decode_moves`].
//!
//! Tokens come from the same table. The captured-piece alternative is chosen
//! from the board, set-up placements are written in square order and
//! variations follow the reader's attachment order (see docs/writing.md).
use super::moves::{
    Capture, END_LINE, NULL_MOVE, PLACEMENT_BASE, START_SETUP, START_STANDARD, VARIATION, table,
};
use crate::CbhError;
use chess_core::{Game, NodeId};
use shakmaty::{CastlingSide, Chess, Color, EnPassantMode, Position, Role, Square, uci::UciMove};
use std::{collections::HashMap, sync::OnceLock};

/// Moving piece, from, to, promotion and captured-piece alternative.
type Key = (Color, Role, u8, u8, Option<Role>, Capture);

fn index() -> &'static HashMap<Key, u16> {
    static INDEX: OnceLock<HashMap<Key, u16>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut map = HashMap::new();
        for (token, entry) in table().iter().enumerate() {
            if let Some(e) = entry {
                map.entry((e.color, e.role, e.from, e.to, e.promotion, e.capture))
                    .or_insert(token as u16);
            }
        }
        map
    })
}

/// ChessBase square index: `8 * file + rank`.
fn cb(square: Square) -> u8 {
    (u32::from(square.file()) * 8 + u32::from(square.rank())) as u8
}

fn token(pos: &Chess, uci: &str) -> Result<u16, CbhError> {
    if uci == "0000" {
        return Ok(NULL_MOVE);
    }
    let parsed: UciMove = uci
        .parse()
        .map_err(|_| CbhError::Decode(format!("Bad move {uci}")))?;
    let m = parsed
        .to_move(pos)
        .map_err(|_| CbhError::Decode(format!("Illegal move {uci}")))?;
    let (color, role) = (pos.turn(), m.role());
    let (from, to, promotion, capture) = match m {
        shakmaty::Move::Castle { king, rook } => {
            let side = CastlingSide::from_queen_side(rook < king);
            let to = Square::from_coords(side.king_to_file(), king.rank());
            (king, to, None, Capture::None)
        }
        shakmaty::Move::EnPassant { from, to } => (from, to, None, Capture::EnPassant),
        shakmaty::Move::Normal {
            from,
            to,
            capture,
            promotion,
            ..
        } => (
            from,
            to,
            promotion,
            capture.map_or(Capture::None, Capture::Piece),
        ),
        shakmaty::Move::Put { .. } => {
            return Err(CbhError::Unsupported("Piece drops".into()));
        }
    };
    index()
        .get(&(color, role, cb(from), cb(to), promotion, capture))
        .copied()
        .ok_or_else(|| CbhError::Unsupported(format!("No 2CBG token for {uci}")))
}

/// Set-up header: move number, side to move with castling rights, a zero
/// word, then one placement token per piece in square order.
fn setup(words: &mut Vec<u16>, game: &Game) -> Result<(), CbhError> {
    let pos = game.chess_position(0)?;
    let mut rights = 0u16;
    for (color, side, bit) in [
        (Color::White, CastlingSide::QueenSide, 1),
        (Color::White, CastlingSide::KingSide, 2),
        (Color::Black, CastlingSide::QueenSide, 4),
        (Color::Black, CastlingSide::KingSide, 8),
    ] {
        if pos.castles().has(color, side) {
            rights |= bit;
        }
    }
    if pos.ep_square(EnPassantMode::Legal).is_some() {
        // The observed set-up controls have no en-passant field.
        return Err(CbhError::Unsupported(
            "2CBG set-ups with an en-passant square".into(),
        ));
    }
    words.push(START_SETUP);
    words.push(u16::try_from(u32::from(pos.fullmoves())).unwrap_or(u16::MAX));
    words.push(u16::from(pos.turn().is_black()) | rights << 8);
    words.push(0);
    let mut placed: Vec<(u8, u16)> = vec![];
    for (square, piece) in pos.board().clone() {
        let square_cb = cb(square);
        let code = if piece.role == Role::Pawn {
            let rank = u32::from(square.rank());
            if !(1..=6).contains(&rank) {
                return Err(CbhError::UnsupportedSetup(
                    "Pawn on the first or last rank".into(),
                ));
            }
            let index = u32::from(square.file()) * 6 + rank - 1;
            640 + u16::from(piece.color.is_black()) * 48 + index as u16
        } else {
            let kind = match piece.role {
                Role::King => 0,
                Role::Queen => 1,
                Role::Knight => 2,
                Role::Bishop => 3,
                _ => 4,
            } + if piece.color.is_black() { 5 } else { 0 };
            kind * 64 + u16::from(square_cb)
        };
        placed.push((square_cb, PLACEMENT_BASE + code));
    }
    placed.sort();
    words.extend(placed.into_iter().map(|(_, t)| t));
    Ok(())
}

/// Encodes a game's move tree as a 2CBG payload (LE16 words).
pub fn encode_moves(game: &Game) -> Result<Vec<u8>, CbhError> {
    let mut words = vec![];
    if game.start_fen != Game::initial().start_fen {
        setup(&mut words, game)?;
    }
    words.push(START_STANDARD);
    line(game, &mut words, game.chess_position(0)?, 0, None)?;
    Ok(words.iter().flat_map(|w| w.to_le_bytes()).collect())
}

/// Writes the line that starts at `parent` (whose position is `pos`).
/// `first` is the index of the child that opens a variation, or `None` for the
/// main continuation.
///
/// ChessBase chains alternatives: a move with alternatives gets one marker,
/// and the first move of each alternative carries the marker for the next
/// one. The reader reads the attached variations in reverse order once the
/// line ends.
fn line(
    game: &Game,
    words: &mut Vec<u16>,
    mut pos: Chess,
    parent: NodeId,
    first: Option<usize>,
) -> Result<(), CbhError> {
    // (parent, position before the move, child index of the next alternative)
    let mut attached: Vec<(NodeId, Chess, usize)> = vec![];
    let mut at = parent;
    let mut child = first.unwrap_or(0);
    while let Some(&id) = game.nodes[at as usize].children.get(child) {
        let node = &game.nodes[id as usize];
        let uci = node.uci.as_deref().unwrap_or("");
        words.push(token(&pos, uci)?);
        if game.nodes[at as usize].children.len() > child + 1 {
            words.push(VARIATION);
            attached.push((at, pos.clone(), child + 1));
        }
        pos = chess_core::model::apply_uci(pos, uci)?;
        at = id;
        child = 0;
    }
    words.push(END_LINE);
    for (parent, before, next) in attached.into_iter().rev() {
        line(game, words, before, parent, Some(next))?;
    }
    Ok(())
}

/// What a 2CBA payload could not hold; empty when nothing was dropped.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Omitted {
    pub notes: Vec<String>,
}

/// Mark colour letters to stored codes (the reader's inverse).
fn color_code(letter: char) -> Option<u8> {
    Some(match letter {
        'G' => 2,
        'Y' => 3,
        'R' => 4,
        'B' => 7,
        'C' => 8,
        'O' => 9,
        _ => return None,
    })
}

fn square_code(name: &str) -> Option<u8> {
    let b = name.as_bytes();
    (b.len() == 2 && (b'a'..=b'h').contains(&b[0]) && (b'1'..=b'8').contains(&b[1]))
        .then(|| (b[0] - b'a') * 8 + (b[1] - b'1') + 1)
}

/// Symbol slot of a NAG: 0 move (1-9), 2 prefix (140-145), 1 position.
fn nag_slot(nag: u16) -> usize {
    match nag {
        1..=9 => 0,
        140..=145 => 2,
        _ => 1,
    }
}

const MAX_TEXT: usize = 0xfff0;

/// Encodes a game's annotations as a 2CBA payload in line order: per node,
/// text before, symbols, text after, squares, arrows. Text is UTF-8 with no
/// language (index 7).
pub fn encode_annotations(game: &Game) -> (Vec<u8>, Omitted) {
    use crate::annotations::{ARROWS, NEUTRAL_LANGUAGE, Order, SQUARES, SYMBOLS, TEXT_AFTER};
    use crate::annotations::{TEXT_BEFORE, positions};
    let mut omitted = Omitted::default();
    let mut out = vec![];
    let order = std::iter::once((-1i32, 0 as NodeId)).chain(
        positions(game, Order::Line)
            .into_iter()
            .enumerate()
            .map(|(k, id)| (k as i32, id)),
    );
    for (position, id) in order {
        let node = &game.nodes[id as usize];
        let mut entries: Vec<u8> = vec![];
        let mut count = 0u32;
        let text = |ty: u16,
                    value: &str,
                    entries: &mut Vec<u8>,
                    count: &mut u32,
                    omitted: &mut Omitted| {
            if value.len() > MAX_TEXT {
                omitted
                    .notes
                    .push(format!("A text of {} bytes exceeds 0xfff0", value.len()));
                return;
            }
            entries.extend(ty.to_le_bytes());
            entries.extend(0u16.to_le_bytes());
            entries.extend(NEUTRAL_LANGUAGE.to_le_bytes());
            entries.extend((value.len() as u32).to_le_bytes());
            entries.extend(value.as_bytes());
            *count += 1;
        };
        if id != 0 {
            for value in &node.pre_comments {
                text(TEXT_BEFORE, value, &mut entries, &mut count, &mut omitted);
            }
        }
        let mut symbols = [0u8; 3];
        for &nag in &node.nags {
            // The class slot first; a second symbol of one class goes to a
            // free slot, as ChessBase stores $8 beside a move symbol.
            let slot = std::iter::once(nag_slot(nag))
                .chain(0..3)
                .find(|&s| symbols[s] == 0);
            match (u8::try_from(nag), slot) {
                (Ok(byte), Some(slot)) if byte != 0 => symbols[slot] = byte,
                _ => omitted.notes.push(format!(
                    "NAG ${nag} does not fit ChessBase's three symbol slots"
                )),
            }
        }
        if symbols != [0; 3] {
            entries.extend(SYMBOLS.to_le_bytes());
            entries.extend(symbols);
            count += 1;
        }
        for value in &node.comments {
            text(TEXT_AFTER, value, &mut entries, &mut count, &mut omitted);
        }
        let squares: Vec<u8> = node
            .squares
            .iter()
            .filter_map(|m| match (color_code(m.color), square_code(&m.square)) {
                (Some(c), Some(s)) => Some([c, s]),
                _ => {
                    omitted.notes.push(format!(
                        "Square mark {}{} is not storable",
                        m.color, m.square
                    ));
                    None
                }
            })
            .flatten()
            .collect();
        let arrows: Vec<u8> = node
            .arrows
            .iter()
            .filter_map(|m| {
                match (
                    color_code(m.color),
                    square_code(&m.from),
                    square_code(&m.to),
                ) {
                    (Some(c), Some(f), Some(t)) => Some([c, f, t]),
                    _ => {
                        omitted.notes.push(format!(
                            "Arrow {}{}{} is not storable",
                            m.color, m.from, m.to
                        ));
                        None
                    }
                }
            })
            .flatten()
            .collect();
        for (ty, data) in [(SQUARES, squares), (ARROWS, arrows)] {
            if !data.is_empty() {
                entries.extend(ty.to_le_bytes());
                entries.extend((data.len() as u32).to_le_bytes());
                entries.extend(data);
                count += 1;
            }
        }
        if count > 0 {
            out.extend(position.to_le_bytes());
            out.extend(count.to_le_bytes());
            out.extend(entries);
        }
    }
    out.extend(i32::MAX.to_le_bytes());
    (out, omitted)
}
