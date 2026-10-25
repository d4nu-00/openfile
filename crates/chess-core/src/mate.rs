//! Exact search for forced mates in one and two moves, without an engine.
//! Draw rules (repetition, the fifty-move rule) are not considered.
use shakmaty::{Chess, Move, Position};

/// The legal moves after which the opponent is checkmated.
pub fn mates_in_one(pos: &Chess) -> Vec<Move> {
    pos.legal_moves()
        .into_iter()
        .filter(|m| {
            let mut after = pos.clone();
            after.play_unchecked(*m);
            after.is_checkmate()
        })
        .collect()
}

/// The moves that force mate on the next move whatever the reply, when `pos`
/// has no mate in one. A move that stalemates, or leaves a reply that
/// escapes, is not one of them.
pub fn mates_in_two(pos: &Chess) -> Vec<Move> {
    if !mates_in_one(pos).is_empty() {
        return vec![];
    }
    pos.legal_moves()
        .into_iter()
        .filter(|m| {
            let mut after = pos.clone();
            after.play_unchecked(*m);
            let replies = after.legal_moves();
            // No reply means mate or stalemate; neither is a mate in two.
            !replies.is_empty()
                && replies.into_iter().all(|reply| {
                    let mut next = after.clone();
                    next.play_unchecked(reply);
                    !mates_in_one(&next).is_empty()
                })
        })
        .collect()
}

/// The smallest `n <= max` for which `pos` has a forced mate in `n` moves,
/// with the first moves that force it; `max` is at most 2.
pub fn shortest_mates(pos: &Chess, max: u8) -> Option<(u8, Vec<Move>)> {
    debug_assert!(max <= 2);
    let one = mates_in_one(pos);
    if max >= 1 && !one.is_empty() {
        return Some((1, one));
    }
    if max >= 2 {
        let two = mates_in_two(pos);
        if !two.is_empty() {
            return Some((2, two));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::uci::UciMove;

    fn pos(fen: &str) -> Chess {
        crate::model::parse_position(fen).unwrap()
    }
    fn ucis(moves: &[Move]) -> Vec<String> {
        let mut v: Vec<String> = moves
            .iter()
            .map(|m| UciMove::from_standard(*m).to_string())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn a_back_rank_mate_in_one_is_unique() {
        let p = pos("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1");
        assert_eq!(ucis(&mates_in_one(&p)), ["a1a8"]);
    }

    #[test]
    fn two_mates_in_one_are_both_found() {
        let p = pos("6k1/5ppp/8/8/8/8/1Q6/R5K1 w - - 0 1");
        assert_eq!(ucis(&mates_in_one(&p)), ["a1a8", "b2b8"]);
    }

    #[test]
    fn a_position_without_a_mate_in_one_has_none() {
        assert!(mates_in_one(&pos("5rk1/5ppp/8/8/8/8/5PPP/3QR1K1 w - - 0 1")).is_empty());
        // Nothing is left to play in a mated or stalemated position.
        assert!(mates_in_one(&pos("R5k1/5ppp/8/8/8/8/8/6K1 b - - 0 1")).is_empty());
    }

    #[test]
    fn a_rook_mate_in_two_has_one_key_move() {
        let p = pos("8/8/8/8/7R/4K3/8/6k1 w - - 0 1");
        assert!(mates_in_one(&p).is_empty());
        assert_eq!(ucis(&mates_in_two(&p)), ["e3f3"]);
        assert_eq!(shortest_mates(&p, 2).map(|(n, m)| (n, ucis(&m))), {
            Some((2, vec!["e3f3".to_string()]))
        });
    }

    #[test]
    fn a_queen_mate_in_two_has_one_key_move() {
        let p = pos("8/8/8/k7/4Q3/2K5/8/8 w - - 0 1");
        assert_eq!(ucis(&mates_in_two(&p)), ["e4b7"]);
    }

    #[test]
    fn several_key_moves_are_all_listed() {
        let p = pos("8/6Q1/8/8/8/4K3/8/5k2 w - - 0 1");
        assert_eq!(ucis(&mates_in_two(&p)), ["g7g4", "g7g5", "g7g6", "g7g8"]);
    }

    #[test]
    fn the_shortest_mate_wins_over_a_mate_in_two() {
        let p = pos("6k1/5ppp/8/8/8/8/1Q6/R5K1 w - - 0 1");
        // A mate in one exists, so nothing is a mate in two.
        assert!(mates_in_two(&p).is_empty());
        let (n, moves) = shortest_mates(&p, 2).unwrap();
        assert_eq!((n, ucis(&moves)), (1, vec!["a1a8".into(), "b2b8".into()]));
        // Capped at one, a mate in two is not reported.
        assert!(shortest_mates(&pos("8/8/8/8/7R/4K3/8/6k1 w - - 0 1"), 1).is_none());
        assert!(shortest_mates(&p, 0).is_none());
    }

    #[test]
    fn a_stalemating_move_is_not_a_mate_in_two() {
        // 1.Qg6 stalemates: the black king has no move and no reply allows a
        // mate, but the position is a draw.
        let p = pos("7k/8/2K5/8/8/8/6Q1/8 w - - 0 1");
        let mut stalemating = p.clone();
        let g6 = p
            .legal_moves()
            .into_iter()
            .find(|m| UciMove::from_standard(*m).to_string() == "g2g6")
            .unwrap();
        stalemating.play_unchecked(g6);
        assert!(stalemating.is_stalemate());
        assert!(!ucis(&mates_in_two(&p)).contains(&"g2g6".to_string()));
    }

    #[test]
    fn a_position_with_no_forced_mate_in_two_gives_none() {
        let p = pos("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert!(shortest_mates(&p, 2).is_none());
    }
}
