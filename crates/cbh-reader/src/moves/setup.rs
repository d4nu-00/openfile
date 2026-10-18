use super::pieces::PieceState;
use crate::CbhError;
use chess_core::Game;
use shakmaty::{CastlingMode, Chess, fen::Fen};

/// Classic 28-byte setup: file-major, MSB-first occupancy/piece stream.
/// Control layout corroborated against libcbh's PositionStack::setup:
/// https://github.com/rolandlo/libcbh/blob/main/src/cbh_position.cpp
/// In particular the en-passant file is a four-bit value, 0 absent, 1..8 a..h.
/// `chess960` is set for records flagged as Chess960.
pub fn decode_setup(bytes: &[u8], chess960: bool) -> Result<(String, PieceState), CbhError> {
    if bytes.len() != 28 {
        return Err(CbhError::Truncated);
    }
    // Byte 0 is 1 in every the test database setup and is skipped by both
    // cbh2pgn and libcbh; only the two observed values are accepted.
    if bytes[0] > 1 || bytes[1] & 0xe0 != 0 || bytes[2] & 0xf0 != 0 {
        return Err(CbhError::UnsupportedSetup(
            "Unknown setup control bits".into(),
        ));
    }
    let ep_file = bytes[1] & 15;
    if ep_file > 8 {
        return Err(CbhError::Decode("Invalid setup controls".into()));
    }
    let black = bytes[1] & 0x10 != 0;
    let mut bit = 32usize;
    let mut read = || -> Result<u8, CbhError> {
        let value = bytes.get(bit / 8).ok_or(CbhError::Truncated)? >> (7 - bit % 8) & 1;
        bit += 1;
        Ok(value)
    };
    let mut board = [' '; 64];
    for file in 0..8 {
        for rank in 0..8 {
            if read()? != 0 {
                let mut code = 0;
                for _ in 0..4 {
                    code = code * 2 + read()?;
                }
                board[rank * 8 + file] = match code {
                    1 => 'K',
                    2 => 'Q',
                    3 => 'N',
                    4 => 'B',
                    5 => 'R',
                    6 => 'P',
                    9 => 'k',
                    10 => 'q',
                    11 => 'n',
                    12 => 'b',
                    13 => 'r',
                    14 => 'p',
                    _ => return Err(CbhError::Decode("Invalid setup piece".into())),
                };
            }
        }
    }
    let game = setup_game(&board, black, bytes[2], ep_file, bytes[3].into(), chess960)?;
    let pieces = PieceState::from_position(&game.chess_position(0)?);
    Ok((game.start_fen, pieces))
}

/// The game of a decoded set-up. An en-passant file that is impossible on
/// this board (no pawn that could just have double-pushed) carries no
/// information; the reference app's own 2CBG conversion drops it (the test database
/// entry 13409), and so does this reader. Castling rights whose king or rook
/// is off its standard square are dropped the same way, one right at a time
/// (classic test database entries 64687, 64740 and ten others; see
/// docs/compatibility.md). The set-up as stored is tried first, so nothing
/// that decodes as stored changes.
///
/// A standard game is read as standard chess first, so that a stored right
/// is not taken for a Chess960 one (king g8 and rook h8 with "k"). A Chess960
/// record keeps its rights; `chess960_castling` replaces them.
pub(crate) fn setup_game(
    board: &[char; 64],
    black: bool,
    rights: u8,
    ep_file: u8,
    number: u16,
    chess960: bool,
) -> Result<Game, CbhError> {
    let castling = possible_castling(board, rights);
    let mut tries = vec![(rights, ep_file)];
    for candidate in [(rights, 0), (castling, ep_file), (castling, 0)] {
        if !tries.contains(&candidate) {
            tries.push(candidate);
        }
    }
    let standard = |f: &str| {
        f.parse::<Fen>()
            .is_ok_and(|f| f.into_position::<Chess>(CastlingMode::Standard).is_ok())
    };
    if !chess960
        && let Some(f) = tries
            .iter()
            .map(|&(rights, ep)| fen(board, black, rights, ep, number))
            .find(|f| standard(f))
    {
        return Ok(Game::from_fen(&f)?);
    }
    let mut result = Err(CbhError::Decode("Invalid setup".into()));
    for (rights, ep) in tries {
        result = Game::from_fen(&fen(board, black, rights, ep, number)).map_err(CbhError::from);
        if result.is_ok() {
            break;
        }
    }
    result
}

/// The castling rights of `rights` (bits 1/Q, 2/K, 4/q, 8/k) whose king and
/// rook stand on their standard squares on `board` (indexed `rank * 8 +
/// file`).
pub(crate) fn possible_castling(board: &[char; 64], rights: u8) -> u8 {
    let mut kept = 0;
    for (mask, king, rook, k, r) in [
        (1, 4, 0, 'K', 'R'),
        (2, 4, 7, 'K', 'R'),
        (4, 60, 56, 'k', 'r'),
        (8, 60, 63, 'k', 'r'),
    ] {
        if rights & mask != 0 && board[king] == k && board[rook] == r {
            kept |= mask;
        }
    }
    kept
}

/// FEN for a decoded setup. `board` is indexed `rank * 8 + file`; castling uses
/// bits 1/Q, 2/K, 4/q, 8/k; `ep_file` is 0 or 1..=8. Move number 0, which
/// The reference app stores for some setups, is written as 1.
pub(crate) fn fen(
    board: &[char; 64],
    black: bool,
    castling: u8,
    ep_file: u8,
    number: u16,
) -> String {
    let mut fen = String::new();
    for rank in (0..8).rev() {
        if rank != 7 {
            fen.push('/');
        }
        let mut empty = 0u8;
        for file in 0..8 {
            let p = board[rank * 8 + file];
            if p == ' ' {
                empty += 1;
            } else {
                if empty != 0 {
                    fen.push(char::from(b'0' + empty));
                    empty = 0;
                }
                fen.push(p);
            }
        }
        if empty != 0 {
            fen.push(char::from(b'0' + empty));
        }
    }
    let mut rights = String::new();
    for (mask, ch) in [(2, 'K'), (1, 'Q'), (8, 'k'), (4, 'q')] {
        if castling & mask != 0 {
            rights.push(ch);
        }
    }
    if rights.is_empty() {
        rights.push('-');
    }
    let ep = if ep_file == 0 {
        "-".into()
    } else {
        format!(
            "{}{}",
            char::from(b'a' + ep_file - 1),
            if black { 3 } else { 6 }
        )
    };
    format!(
        "{fen} {} {rights} {ep} 0 {}",
        if black { "b" } else { "w" },
        number.max(1)
    )
}

/// The castling field of a Chess960 set-up. After the 28 set-up bytes a
/// Chess960 record holds six squares (file-major, `8 * file + rank`): the
/// white and black kings, then the rooks White castles with on the king's
/// and the queen's side, then Black's. They name the rook files of the
/// castling rights (control bits 2/K, 1/Q, 8/k, 4/q), written as
/// Shredder-FEN letters. Observed in every Chess960 game of test database
/// 2025; a square that does not hold the king or rook it names is refused.
pub fn chess960_castling(fen: &str, rights: u8, squares: &[u8]) -> Result<String, CbhError> {
    if squares.len() != 6 || squares.iter().any(|&s| s > 63) {
        return Err(CbhError::Decode("Invalid Chess960 squares".into()));
    }
    let placement = fen.split(' ').next().unwrap_or("");
    let mut board = [' '; 64];
    for (i, row) in placement.split('/').enumerate() {
        let rank = 7 - i;
        let mut file = 0;
        for c in row.chars() {
            match c.to_digit(10) {
                Some(n) => file += n as usize,
                None => {
                    if file < 8 && rank < 8 {
                        board[rank * 8 + file] = c;
                    }
                    file += 1;
                }
            }
        }
    }
    // File-major square to (file, piece on it).
    let at = |sq: u8| {
        let (file, rank) = ((sq / 8) as usize, (sq % 8) as usize);
        (file, board[rank * 8 + file])
    };
    let (_, wk) = at(squares[0]);
    let (_, bk) = at(squares[1]);
    if wk != 'K' || bk != 'k' {
        return Err(CbhError::Decode(
            "Chess960 king squares do not match".into(),
        ));
    }
    let mut field = String::new();
    for (bit, sq, rook, upper) in [
        (2, squares[2], 'R', true),
        (1, squares[3], 'R', true),
        (8, squares[4], 'r', false),
        (4, squares[5], 'r', false),
    ] {
        if rights & bit == 0 {
            continue;
        }
        let (file, piece) = at(sq);
        if piece != rook {
            return Err(CbhError::Decode("Chess960 castling rook is missing".into()));
        }
        let letter = char::from(b'a' + file as u8);
        field.push(if upper {
            letter.to_ascii_uppercase()
        } else {
            letter
        });
    }
    if field.is_empty() {
        field.push('-');
    }
    let mut parts: Vec<&str> = fen.split(' ').collect();
    if parts.len() < 3 {
        return Err(CbhError::Decode("Invalid set-up FEN".into()));
    }
    parts[2] = &field;
    Ok(parts.join(" "))
}
