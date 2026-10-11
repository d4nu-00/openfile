use crate::CbhError;
use shakmaty::{Chess, Color, File, Move, Position, Rank, Role, Square};
#[derive(Clone)]
pub struct PieceState {
    slots: [Vec<Option<Square>>; 12],
}
fn index(color: Color, role: Role) -> usize {
    (if color.is_white() { 0 } else { 6 }) + role as usize - 1
}
pub fn square(file: u8, rank: u8) -> Square {
    Square::from_coords(File::new(file as u32), Rank::new(rank as u32))
}
impl PieceState {
    pub fn from_position(pos: &Chess) -> Self {
        let mut slots: [Vec<Option<Square>>; 12] = std::array::from_fn(|_| vec![]);
        for file in 0..8 {
            for rank in 0..8 {
                let sq = square(file, rank);
                if let Some(piece) = pos.board().piece_at(sq) {
                    slots[index(piece.color, piece.role)].push(Some(sq));
                }
            }
        }
        for row in &mut slots {
            row.resize(16, None);
        }
        Self { slots }
    }
    pub fn source(&self, color: Color, role: Role, ordinal: usize) -> Result<Square, CbhError> {
        self.slots[index(color, role)]
            .get(ordinal)
            .copied()
            .flatten()
            .ok_or_else(|| CbhError::Decode("Move references an absent piece ordinal".into()))
    }
    pub fn update(&mut self, before: &Chess, after: &Chess, m: Move) -> Result<(), CbhError> {
        let from = m
            .from()
            .ok_or_else(|| CbhError::Unsupported("Nonstandard move".into()))?;
        let rook = if let Move::Castle { rook, .. } = m {
            Some(rook)
        } else {
            None
        };
        // Remove disappearing captured pieces, including the off-target e.p. pawn.
        for row in &mut self.slots {
            let removed: Vec<_> = row
                .iter()
                .enumerate()
                .filter_map(|(i, &sq)| {
                    let sq = sq?;
                    if sq == from || Some(sq) == rook {
                        return None;
                    }
                    let piece = before.board().piece_at(sq)?;
                    if after.board().piece_at(sq) == Some(piece) {
                        None
                    } else {
                        Some((i, piece.role))
                    }
                })
                .collect();
            for (i, role) in removed.into_iter().rev() {
                if role == Role::Pawn {
                    row[i] = None;
                } else {
                    row.remove(i);
                    row.push(None);
                }
            }
        }
        let color = before.turn();
        let move_slot =
            |row: &mut Vec<Option<Square>>, src: Square, dst: Square| -> Result<(), CbhError> {
                let i = row.iter().position(|&s| s == Some(src)).ok_or_else(|| {
                    CbhError::Decode("Piece ordinal state does not match source".into())
                })?;
                row[i] = Some(dst);
                Ok(())
            };
        match m {
            Move::Castle { king, rook } => {
                let short = rook.file() > king.file();
                let rank = king.rank() as u8;
                move_slot(
                    &mut self.slots[index(color, Role::King)],
                    king,
                    square(if short { 6 } else { 2 }, rank),
                )?;
                move_slot(
                    &mut self.slots[index(color, Role::Rook)],
                    rook,
                    square(if short { 5 } else { 3 }, rank),
                )?;
            }
            Move::Normal {
                role,
                from,
                to,
                promotion,
                ..
            } => {
                let row = &mut self.slots[index(color, role)];
                if let Some(promoted) = promotion {
                    let i = row
                        .iter()
                        .position(|&s| s == Some(from))
                        .ok_or_else(|| CbhError::Decode("Promotion source is absent".into()))?;
                    row[i] = None;
                    let row = &mut self.slots[index(color, promoted)];
                    let i = row
                        .iter()
                        .position(Option::is_none)
                        .ok_or(CbhError::LimitExceeded)?;
                    row[i] = Some(to);
                } else {
                    move_slot(row, from, to)?;
                }
            }
            Move::EnPassant { from, to } => {
                move_slot(&mut self.slots[index(color, Role::Pawn)], from, to)?
            }
            Move::Put { .. } => return Err(CbhError::Unsupported("Piece drops".into())),
        }
        Ok(())
    }
}
