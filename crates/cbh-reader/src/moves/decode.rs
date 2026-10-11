use super::{
    pieces::{PieceState, square},
    tables,
};
use crate::{CbhError, ReadLimits};
use chess_core::{Game, GameBuilder};
use shakmaty::{Position, Role, uci::UciMove};

pub struct TokenCursor<'a> {
    pub bytes: &'a [u8],
    pub offset: usize,
    pub processed: u8,
}
impl<'a> TokenCursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            processed: 0,
        }
    }
    pub fn next(&mut self) -> Result<u8, CbhError> {
        let b = *self.bytes.get(self.offset).ok_or(CbhError::Truncated)?;
        self.offset += 1;
        Ok(b.wrapping_sub(self.processed))
    }
}
pub fn decode_classic(record: &[u8], limits: &ReadLimits) -> Result<Game, CbhError> {
    if record.len() > limits.max_game_bytes {
        return Err(CbhError::LimitExceeded);
    }
    if record.len() < 5 {
        return Err(CbhError::Truncated);
    }
    let header = u32::from_be_bytes(record[..4].try_into().unwrap());
    if (header & 0xffffff) as usize != record.len() {
        return Err(CbhError::Truncated);
    }
    if header & 0xbf000000 != 0 {
        return Err(CbhError::Unsupported(format!(
            "CBG flags {:#010x}",
            header & 0xff000000
        )));
    }
    let (game, mut pieces, offset) = if header & 0x40000000 != 0 {
        let bytes = record.get(4..32).ok_or(CbhError::Truncated)?;
        let (fen, pieces) = super::setup::decode_setup(bytes)?;
        (Game::from_fen(&fen)?, pieces, 32)
    } else {
        let game = Game::initial();
        let pieces = PieceState::from_position(&game.chess_position(0)?);
        (game, pieces, 4)
    };
    let mut builder = GameBuilder::new(game, limits.game.clone())?;
    let mut cursor = TokenCursor::new(&record[offset..]);
    let mut current = 0;
    let mut frames = Vec::new();
    while cursor.offset < cursor.bytes.len() {
        let token = cursor.next()?;
        if token == 0x0c {
            if let Some((node, state)) = frames.pop() {
                current = node;
                pieces = state;
                continue;
            }
            if cursor.offset != cursor.bytes.len() {
                return Err(CbhError::Decode("Early line terminator".into()));
            }
            return Ok(builder.finish());
        }
        if token == 0xdc {
            if frames.len() >= limits.game.max_variation_depth {
                return Err(CbhError::LimitExceeded);
            }
            frames.push((current, pieces.clone()));
            continue;
        }
        if token == 0x9f {
            continue;
        }
        if token == 0xaa {
            current = builder.append_uci(current, "0000")?;
            cursor.processed = cursor.processed.wrapping_add(1);
            continue;
        }
        let before = builder.position(current)?.clone();
        let uci = if token == 0x29 {
            let hi = tables::DEOBFUSCATE[cursor.next()? as usize];
            let lo = tables::DEOBFUSCATE[cursor.next()? as usize];
            let word = u16::from_be_bytes([hi, lo]);
            if word & 0xc000 != 0 {
                return Err(CbhError::Unsupported("Extra coordinate-move flags".into()));
            }
            let src = (word & 63) as u8;
            let dst = ((word >> 6) & 63) as u8;
            let from = square(src / 8, src % 8);
            let to = square(dst / 8, dst % 8);
            let piece = before
                .board()
                .piece_at(from)
                .ok_or_else(|| CbhError::Decode("Coordinate move has empty source".into()))?;
            let promotion = if piece.role == Role::Pawn && [0, 7].contains(&(to.rank() as u8)) {
                Some(
                    [Role::Queen, Role::Rook, Role::Bishop, Role::Knight]
                        [((word >> 12) & 3) as usize],
                )
            } else {
                None
            };
            UciMove::Normal {
                from,
                to,
                promotion,
            }
            .to_string()
        } else {
            let def = tables::primary(token)
                .ok_or_else(|| CbhError::Decode(format!("Unknown primary token {token:#04x}")))?;
            let from = pieces.source(before.turn(), def.role, def.ordinal)?;
            let flip = if def.role == Role::Pawn && before.turn().is_black() {
                -1
            } else {
                1
            };
            let f = (from.file() as i8 + def.dx * flip).rem_euclid(8) as u8;
            let r = (from.rank() as i8 + def.dy * flip).rem_euclid(8) as u8;
            UciMove::Normal {
                from,
                to: square(f, r),
                promotion: None,
            }
            .to_string()
        };
        let parsed: UciMove = uci
            .parse()
            .map_err(|_| CbhError::Decode("Invalid coordinate move".into()))?;
        let m = parsed
            .to_move(&before)
            .map_err(|_| CbhError::Decode(format!("Illegal decoded move {uci}")))?;
        current = builder.append_uci(current, &uci)?;
        pieces.update(&before, builder.position(current)?, m)?;
        cursor.processed = cursor.processed.wrapping_add(1);
    }
    Err(CbhError::Decode("Game has no line terminator".into()))
}
