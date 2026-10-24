//! Independently implemented material criteria; reference evidence in docs/search.md.
use crate::CbhError;
use chess_core::{Game, NodeId};
use shakmaty::{Board, Color, Position, Role};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Range {
    pub min: Option<i16>,
    pub max: Option<i16>,
}
impl Range {
    fn contains(self, n: i16) -> bool {
        self.min.is_none_or(|m| n >= m) && self.max.is_none_or(|m| n <= m)
    }
    fn validate(self, low: i16, high: i16) -> bool {
        self.min.is_none_or(|n| (low..=high).contains(&n))
            && self.max.is_none_or(|n| (low..=high).contains(&n))
            && self.min.zip(self.max).is_none_or(|(a, b)| a <= b)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Side {
    pub queens: Range,
    pub rooks: Range,
    pub bishops: Range,
    pub knights: Range,
    pub pawns: Range,
    pub minors: Range,
    /// All men except the king, including pawns.
    pub total: Range,
}
impl Side {
    fn ranges(&self) -> [Range; 7] {
        [
            self.queens,
            self.rooks,
            self.bishops,
            self.knights,
            self.pawns,
            self.minors,
            self.total,
        ]
    }
    fn matches(&self, counts: &[i16; 7]) -> bool {
        self.ranges()
            .into_iter()
            .zip(counts)
            .all(|(r, &n)| r.contains(n))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct MaterialQuery {
    pub white: Side,
    pub black: Side,
    /// White minus Black, using Q=9, R=5, B=N=3, P=1.
    pub balance: Range,
    pub ignore_colors: bool,
    pub first: Option<u16>,
    pub last: Option<u16>,
    /// Consecutive positions (plies), not full moves.
    pub length: Option<u16>,
    pub lines: bool,
}

impl MaterialQuery {
    pub fn validate(&self) -> Result<(), CbhError> {
        if !self
            .white
            .ranges()
            .into_iter()
            .chain(self.black.ranges())
            .all(|r| r.validate(0, 16))
            || !self.balance.validate(-144, 144)
            || self.first == Some(0)
            || self.last == Some(0)
            || self.length == Some(0)
            || self.first.zip(self.last).is_some_and(|(a, b)| a > b)
        {
            return Err(CbhError::InvalidQuery(
                "Invalid material range or move window".into(),
            ));
        }
        Ok(())
    }
    fn matches(&self, board: &Board, swapped: bool) -> bool {
        let counts = |color| {
            let side = board.by_color(color);
            let [q, r, b, n, p] = [
                Role::Queen,
                Role::Rook,
                Role::Bishop,
                Role::Knight,
                Role::Pawn,
            ]
            .map(|role| (side & board.by_role(role)).count() as i16);
            [q, r, b, n, p, b + n, q + r + b + n + p]
        };
        let w = counts(if swapped { Color::Black } else { Color::White });
        let b = counts(if swapped { Color::White } else { Color::Black });
        let score = |c: &[i16; 7]| 9 * c[0] + 5 * c[1] + 3 * (c[2] + c[3]) + c[4];
        self.white.matches(&w)
            && self.black.matches(&b)
            && self.balance.contains(score(&w) - score(&b))
    }
    /// First run in depth-first order; each branch inherits only its actual ancestors.
    /// Orientations have separate run counters, preventing runs that switch colours.
    pub fn find_in(&self, game: &Game, cancel: &AtomicBool) -> Result<Option<NodeId>, CbhError> {
        let first = 2 * u32::from(self.first.unwrap_or(1)) - 2;
        let last = self.last.map_or(u32::MAX, |m| 2 * u32::from(m) - 1);
        let length = self.length.unwrap_or(1);
        let mut stack = vec![(0, game.chess_position(0)?, 0, [(0u16, 0u32); 2])];
        while let Some((id, pos, ply, mut runs)) = stack.pop() {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            if ply > last {
                continue;
            }
            if ply >= first {
                for (i, run) in
                    runs.iter_mut()
                        .enumerate()
                        .take(if self.ignore_colors { 2 } else { 1 })
                {
                    if self.matches(pos.board(), i == 1) {
                        if run.0 == 0 {
                            run.1 = id;
                        }
                        run.0 += 1;
                        if run.0 >= length {
                            return Ok(Some(run.1));
                        }
                    } else {
                        run.0 = 0;
                    }
                }
            }
            let children = &game.nodes[id as usize].children;
            for &child in children[..if self.lines {
                children.len()
            } else {
                children.len().min(1)
            }]
                .iter()
                .rev()
            {
                let next = chess_core::model::apply_uci(
                    pos.clone(),
                    game.nodes[child as usize].uci.as_deref().unwrap_or(""),
                )?;
                stack.push((child, next, ply + 1, runs));
            }
        }
        Ok(None)
    }
}
