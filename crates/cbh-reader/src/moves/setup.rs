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
    if bytes[0] != 0 || bytes[1] & 0xe0 != 0 || bytes[2] & 0xf0 != 0 {
        return Err(CbhError::UnsupportedSetup(
            "Unknown setup control bits".into(),
        ));
    }
    let ep_file = bytes[1] & 15;
    if ep_file > 8 || bytes[3] == 0 {
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
        if bytes[2] & mask != 0 {
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
    fen.push_str(&format!(
        " {} {rights} {ep} 0 {}",
        if black { "b" } else { "w" },
        bytes[3]
    ));
    let game = Game::from_fen(&fen)?;
    let pieces = PieceState::from_position(&game.chess_position(0)?);
    Ok((game.start_fen, pieces))
}
