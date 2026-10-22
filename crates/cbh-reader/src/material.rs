//! Piece-combination criteria of the search mask's Material tab: the final
//! position's material and the pieces involved in a mate (the workbench's own
//! design; see docs/search.md).

/// Inclusive bounds on a piece count; `None` leaves that side open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CountRange {
    pub min: Option<u8>,
    pub max: Option<u8>,
}

impl CountRange {
    pub fn is_any(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    pub fn contains(&self, n: u8) -> bool {
        self.min.is_none_or(|min| n >= min) && self.max.is_none_or(|max| n <= max)
    }
}

/// Ranges on one side's pieces, by kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PieceCounts {
    pub kings: CountRange,
    pub queens: CountRange,
    pub rooks: CountRange,
    pub bishops: CountRange,
    pub knights: CountRange,
    /// Bishops and knights together.
    pub minors: CountRange,
    pub pawns: CountRange,
    /// Every kind left at "any" (kings excepted) must be 0.
    pub others_zero: bool,
}

/// The Material tab's criteria (see docs/search.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MaterialQuery {
    /// White's and Black's pieces in the final position of the main line.
    pub white: PieceCounts,
    pub black: PieceCounts,
    /// The sides may also match the other way round.
    pub either_colour: bool,
    /// The game ends in mate, and the mating side's pieces involved in the
    /// mate match `mate_by`.
    pub mate_by_enabled: bool,
    pub mate_by: PieceCounts,
}

/// One side's pieces by kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub kings: u8,
    pub queens: u8,
    pub rooks: u8,
    pub bishops: u8,
    pub knights: u8,
    pub pawns: u8,
}

impl Tally {
    pub fn of(board: &shakmaty::Board, color: shakmaty::Color) -> Tally {
        Tally::of_squares(board, board.by_color(color))
    }

    /// The pieces of `board` standing on `squares`.
    fn of_squares(board: &shakmaty::Board, squares: shakmaty::Bitboard) -> Tally {
        use shakmaty::Role;
        let count = |role: Role| (squares & board.by_role(role)).count() as u8;
        Tally {
            kings: count(Role::King),
            queens: count(Role::Queen),
            rooks: count(Role::Rook),
            bishops: count(Role::Bishop),
            knights: count(Role::Knight),
            pawns: count(Role::Pawn),
        }
    }
}

impl PieceCounts {
    /// No range is set and `others_zero` is off.
    pub fn is_any(&self) -> bool {
        !self.others_zero
            && [
                self.kings,
                self.queens,
                self.rooks,
                self.bishops,
                self.knights,
                self.minors,
                self.pawns,
            ]
            .iter()
            .all(CountRange::is_any)
    }

    /// Every range given holds for `tally`; with `others_zero`, so does 0 for
    /// every other kind (kings excepted). Bishops and knights are not forced
    /// to 0 when `minors` is given, and `minors` is not when either is given.
    pub fn matches(&self, tally: &Tally) -> bool {
        let minors = tally.bishops + tally.knights;
        if !(self.kings.contains(tally.kings)
            && self.queens.contains(tally.queens)
            && self.rooks.contains(tally.rooks)
            && self.bishops.contains(tally.bishops)
            && self.knights.contains(tally.knights)
            && self.minors.contains(minors)
            && self.pawns.contains(tally.pawns))
        {
            return false;
        }
        if !self.others_zero {
            return true;
        }
        let unset = |range: &CountRange, n: u8| range.is_any() && n != 0;
        let minors_given = !self.minors.is_any();
        let single_given = !self.bishops.is_any() || !self.knights.is_any();
        !(unset(&self.queens, tally.queens)
            || unset(&self.rooks, tally.rooks)
            || unset(&self.pawns, tally.pawns)
            || (!minors_given
                && (unset(&self.bishops, tally.bishops) || unset(&self.knights, tally.knights)))
            || (!single_given && unset(&self.minors, minors)))
    }
}

impl MaterialQuery {
    /// The final-material part of the query is set.
    pub fn final_active(&self) -> bool {
        !self.white.is_any() || !self.black.is_any()
    }

    /// Whether the two sides of `board` match `white` and `black`, or with
    /// `either_colour` the other way round.
    pub fn matches_final(&self, board: &shakmaty::Board) -> bool {
        use shakmaty::Color;
        let white = Tally::of(board, Color::White);
        let black = Tally::of(board, Color::Black);
        (self.white.matches(&white) && self.black.matches(&black))
            || (self.either_colour && self.white.matches(&black) && self.black.matches(&white))
    }
}

/// The pieces of the mating side that take part in a mate, by kind; `None`
/// unless `pos` is checkmate. A piece takes part when it gives check or
/// attacks a square the mated king could otherwise flee to (empty, or holding
/// a mating-side piece), with the mated king removed from the occupancy so a
/// slider's attack runs on through its square. Pins, blockers and distant
/// defenders of other attackers are not considered.
pub fn mating_pieces(pos: &shakmaty::Chess) -> Option<Tally> {
    use shakmaty::{
        Bitboard, Position,
        attacks::{attacks, king_attacks},
    };
    if !pos.is_checkmate() {
        return None;
    }
    let board = pos.board();
    let mated = pos.turn();
    let king = board.king_of(mated)?;
    let occupied = board.occupied() ^ Bitboard::from_square(king);
    let escapes = king_attacks(king) & !board.by_color(mated);
    let mut involved = Bitboard::EMPTY;
    for sq in board.by_color(!mated) {
        let reach = attacks(sq, board.piece_at(sq)?, occupied);
        if reach.contains(king) || reach.intersects(escapes) {
            involved.add(sq);
        }
    }
    Some(Tally::of_squares(board, involved))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(min: Option<u8>, max: Option<u8>) -> CountRange {
        CountRange { min, max }
    }
    fn exactly(n: u8) -> CountRange {
        range(Some(n), Some(n))
    }
    fn at_least(n: u8) -> CountRange {
        range(Some(n), None)
    }
    fn board(fen: &str) -> shakmaty::Board {
        use shakmaty::Position;
        chess_core::model::parse_position(fen)
            .unwrap()
            .board()
            .clone()
    }
    fn tally(q: u8, r: u8, b: u8, n: u8, p: u8) -> Tally {
        Tally {
            kings: 1,
            queens: q,
            rooks: r,
            bishops: b,
            knights: n,
            pawns: p,
        }
    }

    #[test]
    fn ranges_any_exact_at_least_and_at_most() {
        let any = CountRange::default();
        assert!(any.is_any() && any.contains(0) && any.contains(9));
        assert!(!exactly(2).is_any());
        assert!(exactly(2).contains(2) && !exactly(2).contains(1) && !exactly(2).contains(3));
        assert!(at_least(1).contains(1) && at_least(1).contains(8) && !at_least(1).contains(0));
        let at_most = range(None, Some(1));
        assert!(at_most.contains(0) && at_most.contains(1) && !at_most.contains(2));
        // No count can satisfy min > max.
        assert!(!range(Some(3), Some(1)).contains(2));
    }

    #[test]
    fn counts_are_taken_per_colour() {
        let b = board("8/p7/4k3/3b4/8/4K3/1PPN4/R7 w - - 0 1");
        assert_eq!(Tally::of(&b, shakmaty::Color::White), tally(0, 1, 0, 1, 2));
        assert_eq!(Tally::of(&b, shakmaty::Color::Black), tally(0, 0, 1, 0, 1));
    }

    #[test]
    fn every_range_given_must_match() {
        let counts = PieceCounts {
            rooks: exactly(1),
            minors: at_least(1),
            ..Default::default()
        };
        assert!(counts.matches(&tally(0, 1, 1, 0, 5)));
        assert!(counts.matches(&tally(3, 1, 0, 2, 0)));
        assert!(!counts.matches(&tally(0, 2, 1, 0, 0)));
        assert!(!counts.matches(&tally(0, 1, 0, 0, 0)));
        // Minors pool bishops and knights.
        let two = PieceCounts {
            minors: exactly(2),
            ..Default::default()
        };
        assert!(two.matches(&tally(0, 0, 1, 1, 0)) && two.matches(&tally(0, 0, 2, 0, 0)));
        assert!(!two.matches(&tally(0, 0, 3, 0, 0)));
        // Kings can be counted too.
        let no_king = PieceCounts {
            kings: exactly(0),
            ..Default::default()
        };
        assert!(!no_king.matches(&tally(0, 0, 0, 0, 0)));
    }

    #[test]
    fn others_zero_forces_unset_kinds_to_zero() {
        let counts = PieceCounts {
            queens: exactly(1),
            others_zero: true,
            ..Default::default()
        };
        assert!(counts.matches(&tally(1, 0, 0, 0, 0)));
        for other in [
            tally(1, 1, 0, 0, 0),
            tally(1, 0, 1, 0, 0),
            tally(1, 0, 0, 1, 0),
            tally(1, 0, 0, 0, 1),
        ] {
            assert!(!counts.matches(&other), "{other:?}");
        }
        // Kings are never forced.
        assert!(
            PieceCounts {
                others_zero: true,
                ..Default::default()
            }
            .matches(&Tally::default())
        );
    }

    #[test]
    fn others_zero_with_minors_leaves_bishops_and_knights_free() {
        let minors = PieceCounts {
            minors: exactly(1),
            others_zero: true,
            ..Default::default()
        };
        assert!(minors.matches(&tally(0, 0, 1, 0, 0)));
        assert!(minors.matches(&tally(0, 0, 0, 1, 0)));
        assert!(!minors.matches(&tally(0, 0, 1, 1, 0)));
        assert!(!minors.matches(&tally(0, 0, 1, 0, 1)));
        // The other way: bishops given leave minors free, knights are forced.
        let bishop = PieceCounts {
            bishops: exactly(1),
            others_zero: true,
            ..Default::default()
        };
        assert!(bishop.matches(&tally(0, 0, 1, 0, 0)));
        assert!(!bishop.matches(&tally(0, 0, 1, 1, 0)));
        // With nothing given everything is zero, minors included.
        let bare = PieceCounts {
            others_zero: true,
            ..Default::default()
        };
        assert!(bare.matches(&tally(0, 0, 0, 0, 0)));
        assert!(!bare.matches(&tally(0, 0, 0, 1, 0)));
    }

    #[test]
    fn is_any_needs_no_range_and_no_others_zero() {
        assert!(PieceCounts::default().is_any());
        assert!(
            !PieceCounts {
                others_zero: true,
                ..Default::default()
            }
            .is_any()
        );
        assert!(
            !PieceCounts {
                kings: exactly(1),
                ..Default::default()
            }
            .is_any()
        );
        assert!(!MaterialQuery::default().final_active());
        // The mate-by part does not make the final material active.
        let mate_by = MaterialQuery {
            mate_by_enabled: true,
            mate_by: PieceCounts {
                queens: exactly(1),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!mate_by.final_active());
        let black = MaterialQuery {
            black: PieceCounts {
                others_zero: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(black.final_active());
    }

    fn rook_and_minor() -> PieceCounts {
        PieceCounts {
            rooks: exactly(1),
            minors: exactly(1),
            others_zero: true,
            ..Default::default()
        }
    }

    #[test]
    fn rook_and_bishop_against_rook_and_knight() {
        let q = MaterialQuery {
            white: rook_and_minor(),
            black: rook_and_minor(),
            ..Default::default()
        };
        assert!(q.matches_final(&board("3rn1k1/8/8/8/8/8/8/3RB1K1 w - - 0 1")));
        // A spare pawn is another piece.
        assert!(!q.matches_final(&board("3rn1k1/p7/8/8/8/8/8/3RB1K1 w - - 0 1")));
        // A second minor piece too.
        assert!(!q.matches_final(&board("3rn1k1/8/8/8/8/8/8/3RBNK1 w - - 0 1")));
    }

    #[test]
    fn either_colour_swaps_whole_sides() {
        let q = MaterialQuery {
            white: PieceCounts {
                queens: exactly(1),
                ..Default::default()
            },
            black: PieceCounts {
                rooks: exactly(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let as_asked = board("rr2k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        let reversed = board("qq2k3/8/8/8/8/8/8/2RR1K2 w - - 0 1");
        let black_queen = board("q3k3/8/8/8/8/8/8/2RRK3 w - - 0 1");
        assert!(q.matches_final(&as_asked));
        assert!(!q.matches_final(&black_queen));
        let either = MaterialQuery {
            either_colour: true,
            ..q
        };
        assert!(either.matches_final(&as_asked));
        assert!(either.matches_final(&black_queen));
        // Whole sides swap: it is not enough that each kind is found somewhere.
        assert!(!either.matches_final(&reversed));
    }

    #[test]
    fn an_empty_query_matches_everything() {
        let q = MaterialQuery::default();
        assert!(q.matches_final(&board("4k3/8/8/8/8/8/8/4K3 w - - 0 1")));
        assert!(q.matches_final(&board(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1"
        )));
    }

    fn involved(fen: &str) -> Option<Tally> {
        mating_pieces(&chess_core::model::parse_position(fen).unwrap())
    }
    fn only(k: u8, q: u8, r: u8, b: u8, n: u8, p: u8) -> Option<Tally> {
        Some(Tally {
            kings: k,
            queens: q,
            rooks: r,
            bishops: b,
            knights: n,
            pawns: p,
        })
    }

    #[test]
    fn back_rank_mate_involves_only_the_rook() {
        assert_eq!(
            involved("R5k1/5ppp/8/8/8/8/8/6K1 b - - 0 1"),
            only(0, 0, 1, 0, 0, 0)
        );
    }

    #[test]
    fn scholars_mate_involves_queen_and_the_bishop_defending_it() {
        assert_eq!(
            involved("r1bqkb1r/pppp1Qpp/2n2n2/4p3/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4"),
            only(0, 1, 0, 1, 0, 0)
        );
    }

    #[test]
    fn smothered_mate_involves_only_the_knight() {
        assert_eq!(
            involved("6rk/5Npp/8/8/8/8/8/6K1 b - - 0 1"),
            only(0, 0, 0, 0, 1, 0)
        );
    }

    #[test]
    fn queen_and_king_mate() {
        assert_eq!(
            involved("k7/1Q6/2K5/8/8/8/8/8 b - - 0 1"),
            only(1, 1, 0, 0, 0, 0)
        );
    }

    #[test]
    fn rook_king_and_pawn_mate() {
        assert_eq!(
            involved("R6k/8/5KP1/8/8/8/8/8 b - - 0 1"),
            only(1, 0, 1, 0, 0, 1)
        );
    }

    #[test]
    fn black_can_be_the_mating_side() {
        // Fool's mate: only the queen takes part, not the pawns or bishops.
        assert_eq!(
            involved("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3"),
            only(0, 1, 0, 0, 0, 0)
        );
    }

    #[test]
    fn positions_that_are_not_mate_have_no_mating_pieces() {
        // Check that can be answered.
        assert_eq!(involved("6k1/5ppp/8/8/8/8/8/R5K1 b - - 0 1"), None);
        assert_eq!(involved("R5k1/5p1p/8/8/8/8/8/6K1 b - - 0 1"), None);
        // Stalemate and the start position.
        assert_eq!(involved("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1"), None);
        assert_eq!(
            involved("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
            None
        );
    }
}
