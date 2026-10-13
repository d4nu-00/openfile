use super::pieces::PieceState;
use crate::CbhError;
use chess_core::Game;

/// Classic 28-byte setup: file-major, MSB-first occupancy/piece stream.
/// Control layout corroborated against libcbh's PositionStack::setup:
/// https://github.com/rolandlo/libcbh/blob/main/src/cbh_position.cpp
/// In particular the en-passant file is a four-bit value, 0 absent, 1..8 a..h.
pub fn decode_setup(bytes: &[u8]) -> Result<(String, PieceState), CbhError> {
    if bytes.len() != 28 {
        return Err(CbhError::Truncated);
    }
    // Byte 0 is 1 in every Mega Database 2025 setup and is skipped by both
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
    // An en-passant file that is impossible on this board (no pawn that could
    // just have double-pushed) carries no information; ChessBase's own 2CBG
    // conversion drops it (Mega Database 2025 entry 13409), and so does this
    // reader.
    let game = match Game::from_fen(&fen(&board, black, bytes[2], ep_file, bytes[3].into())) {
        Err(_) if ep_file != 0 => {
            Game::from_fen(&fen(&board, black, bytes[2], 0, bytes[3].into()))?
        }
        other => other?,
    };
    let pieces = PieceState::from_position(&game.chess_position(0)?);
    Ok((game.start_fen, pieces))
}

/// FEN for a decoded setup. `board` is indexed `rank * 8 + file`; castling uses
/// bits 1/Q, 2/K, 4/q, 8/k; `ep_file` is 0 or 1..=8. Move number 0, which
/// ChessBase stores for some setups, is written as 1.
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
