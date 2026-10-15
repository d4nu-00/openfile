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
    /// Chess960 and flag-0x04 records use their own byte order (see
    /// `tables`).
    pub order: Option<tables::Order>,
}
impl<'a> TokenCursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            processed: 0,
            order: None,
        }
    }
    pub fn next(&mut self) -> Result<u8, CbhError> {
        let b = *self.bytes.get(self.offset).ok_or(CbhError::Truncated)?;
        self.offset += 1;
        Ok(match self.order {
            Some(order) => tables::remapped_token(order, b, self.processed),
            None => b.wrapping_sub(self.processed),
        })
    }
}
/// CBG header bits of a Chess960 game.
const CHESS960: u32 = 0x0a000000;

pub fn decode_classic(record: &[u8], limits: &ReadLimits) -> Result<Game, CbhError> {
    if record.len() > limits.max_game_bytes {
        return Err(CbhError::LimitExceeded);
    }
    if record.len() < 4 {
        return Err(CbhError::Truncated);
    }
    let header = u32::from_be_bytes(record[..4].try_into().unwrap());
    if (header & 0xffffff) as usize != record.len() {
        return Err(CbhError::Truncated);
    }
    // Kinds 1 and 5 write every move as a two-byte word; a record of just
    // the header holds no moves.
    let kind = (header >> 24) & 0x3f;
    if header & 0x80000000 == 0 && (kind == 1 || kind == 5) {
        return decode_words(record, header, kind, limits);
    }
    if record.len() < 5 {
        return Err(CbhError::Truncated);
    }
    // Chess960 games carry both bits of 0x0a000000 (cbh2pgn's
    // MASK_IS_960); every one in Mega Database 2025 that has moves also has
    // a set-up position.
    let chess960 = header & CHESS960 == CHESS960;
    // Flag 0x04000000 alone: standard games written in another byte order.
    let kind4 = header & 0x3f000000 == 0x04000000;
    let known = if chess960 {
        CHESS960
    } else if kind4 {
        0x04000000
    } else {
        0
    };
    if header & 0xbf000000 & !known != 0 {
        return Err(CbhError::Unsupported(format!(
            "CBG flags {:#010x}",
            header & 0xff000000
        )));
    }
    if chess960 && header & 0x40000000 == 0 {
        return Err(CbhError::Unsupported(
            "Chess960 game without its start position".into(),
        ));
    }
    let (game, mut pieces, offset) = if header & 0x40000000 != 0 {
        let bytes = record.get(4..32).ok_or(CbhError::Truncated)?;
        let (mut fen, pieces) = super::setup::decode_setup(bytes, chess960)?;
        let mut offset = 32;
        if chess960 {
            let squares = record.get(32..38).ok_or(CbhError::Truncated)?;
            fen = super::setup::chess960_castling(&fen, bytes[2], squares)?;
            // Then two bytes holding 0..=959 (the Chess960 start position's
            // number, by their range; not used here).
            offset = 40;
        }
        let mut game = Game::from_fen(&fen)?;
        if chess960 {
            game.tags
                .insert("Variant".into(), chess_core::model::CHESS960.into());
        }
        (game, pieces, offset)
    } else {
        let game = Game::initial();
        let pieces = PieceState::from_position(&game.chess_position(0)?);
        (game, pieces, 4)
    };
    let mut builder = GameBuilder::new(game, limits.game.clone())?;
    let mut cursor = TokenCursor::new(&record[offset..]);
    cursor.order = if chess960 {
        Some(tables::Order::Chess960)
    } else if kind4 {
        Some(tables::Order::Kind4)
    } else {
        None
    };
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
            // Chess960 records write castling as a two-byte move whose source
            // and target are both the king's castling square: g1 or c1 (g8,
            // c8 for Black). It castles with that side's rook.
            if chess960 && src == dst && [2, 6].contains(&(src / 8)) {
                let side = if src / 8 == 6 {
                    shakmaty::CastlingSide::KingSide
                } else {
                    shakmaty::CastlingSide::QueenSide
                };
                let king = before
                    .board()
                    .king_of(before.turn())
                    .ok_or_else(|| CbhError::Decode("Castling without a king".into()))?;
                let rook = before
                    .castles()
                    .rook(before.turn(), side)
                    .ok_or_else(|| CbhError::Decode("Castling without the right".into()))?;
                if to.rank() != king.rank() {
                    return Err(CbhError::Decode(
                        "Castling square off the king's rank".into(),
                    ));
                }
                let uci = UciMove::Normal {
                    from: king,
                    to: rook,
                    promotion: None,
                }
                .to_string();
                let m = shakmaty::Move::Castle { king, rook };
                current = builder.append_uci(current, &uci)?;
                pieces.update(&before, builder.position(current)?, m)?;
                cursor.processed = cursor.processed.wrapping_add(1);
                continue;
            }
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
            // In Chess960 the castling tokens (king two files right or left)
            // castle with that side's rook, written king to rook.
            if chess960 && def.role == Role::King && def.dx.abs() == 2 && def.dy == 0 {
                let side = if def.dx > 0 {
                    shakmaty::CastlingSide::KingSide
                } else {
                    shakmaty::CastlingSide::QueenSide
                };
                let rook = before
                    .castles()
                    .rook(before.turn(), side)
                    .ok_or_else(|| CbhError::Decode("Castling without the right".into()))?;
                let uci = UciMove::Normal {
                    from,
                    to: rook,
                    promotion: None,
                }
                .to_string();
                let m = shakmaty::Move::Castle { king: from, rook };
                current = builder.append_uci(current, &uci)?;
                pieces.update(&before, builder.position(current)?, m)?;
                cursor.processed = cursor.processed.wrapping_add(1);
                continue;
            }
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

/// Records of encoding kinds 1 and 5 (CBG flags `0x01000000` and
/// `0x05000000`): every move is a 16-bit word, written by `0x1401fc8b0` in
/// CBase18.exe 18.1. Bits 0–5 hold the source and 6–11 the target square
/// (file-major), 12–14 a promotion piece, 0x4000 marks a line's last move
/// and 0x8000 a move with variations or a line followed by another. Both
/// bytes are mapped through the kind's table with the move counter: kind 1
/// reads the position of `byte - counter` in `KIND1_ORDER`, kind 5 the
/// position of the byte in `KIND5_ORDER`, less the counter. Variations are not read: every such
/// record in Mega Database 2025 is a single line, and the 0x8000 bit's two
/// meanings were not separated. In kind 1 the upper four bits are not read
/// at all (see below).
fn decode_words(
    record: &[u8],
    header: u32,
    kind: u32,
    limits: &ReadLimits,
) -> Result<Game, CbhError> {
    let (game, offset) = if header & 0x40000000 != 0 {
        let bytes = record.get(4..32).ok_or(CbhError::Truncated)?;
        let (fen, _) = super::setup::decode_setup(bytes, false)?;
        (Game::from_fen(&fen)?, 32)
    } else {
        (Game::initial(), 4)
    };
    let order = if kind == 1 {
        &tables::KIND1_ORDER
    } else {
        &tables::KIND5_ORDER
    };
    let mut inv = [0u8; 256];
    for (i, &b) in order.iter().enumerate() {
        inv[b as usize] = i as u8;
    }
    let unmap = |b: u8, counter: u8| -> u8 {
        if kind == 1 {
            inv[b.wrapping_sub(counter) as usize]
        } else {
            inv[b as usize].wrapping_sub(counter)
        }
    };
    let body = &record[offset..];
    if !body.len().is_multiple_of(2) {
        return Err(CbhError::Decode(
            "Odd length of a two-byte move record".into(),
        ));
    }
    let mut builder = GameBuilder::new(game, limits.game.clone())?;
    let mut current = 0;
    let mut counter = 0u8;
    let mut ended = body.is_empty();
    for (i, pair) in body.as_chunks::<2>().0.iter().enumerate() {
        if ended {
            return Err(CbhError::Decode("Moves after the last move".into()));
        }
        let mut word = u16::from_be_bytes([unmap(pair[0], counter), unmap(pair[1], counter)]);
        // Kind 1's upper bits follow no pattern in its only Mega record (a
        // single line whose every move is legal); only the squares are read
        // and the line ends with the record.
        if kind == 1 {
            word &= 0x0fff;
            if i + 1 == body.len() / 2 {
                word |= 0x4000;
            }
        }
        if word & 0x8000 != 0 {
            return Err(CbhError::Unsupported(
                "Variations in two-byte move records".into(),
            ));
        }
        let (src, dst) = ((word & 63) as u8, ((word >> 6) & 63) as u8);
        let from = square(src / 8, src % 8);
        let to = square(dst / 8, dst % 8);
        let before = builder.position(current)?.clone();
        let piece = before
            .board()
            .piece_at(from)
            .ok_or_else(|| CbhError::Decode("Move from an empty square".into()))?;
        let promotion = if piece.role == Role::Pawn && [0, 7].contains(&(to.rank() as u8)) {
            Some(match (word >> 12) & 7 {
                0 | 2 => Role::Queen,
                3 => Role::Knight,
                4 => Role::Bishop,
                5 => Role::Rook,
                code => {
                    return Err(CbhError::Decode(format!("Promotion code {code}")));
                }
            })
        } else {
            None
        };
        let uci = UciMove::Normal {
            from,
            to,
            promotion,
        }
        .to_string();
        current = builder.append_uci(current, &uci)?;
        counter = counter.wrapping_add(1);
        ended = word & 0x4000 != 0;
        let _ = i;
    }
    if !ended && !body.is_empty() {
        return Err(CbhError::Decode("Line without its last move".into()));
    }
    Ok(builder.finish())
}
