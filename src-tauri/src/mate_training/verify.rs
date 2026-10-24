//! Checking that a position of a game is a forced mate in exactly `n` moves
//! with a single mating first move: exactly for `n` of 1 and 2, with a UCI
//! engine for 3 to 5.
use crate::commands::AppError;
use engine::{AnalysisRequest, EnginePayload, EngineSession, EngineSpec, Score};
use shakmaty::{Chess, EnPassantMode, Move, Position, fen::Fen, uci::UciMove};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// The result of checking one step of a chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StepCheck {
    /// A forced mate in exactly `n`, the game's move being its only first move.
    Valid,
    /// No forced mate was found.
    NotMate,
    /// The side to move mates in fewer moves.
    Shorter(u8),
    /// The side to move mates, but in more moves.
    Longer,
    /// More than one first move forces mate in `n`.
    NotUnique,
    /// The only mating first move is not the game's move.
    OtherMove,
    /// The engine gave no score in its time.
    Timeout,
}

impl StepCheck {
    /// The name the rejection counts are kept under.
    pub fn kind(self) -> &'static str {
        match self {
            StepCheck::Valid => "valid",
            StepCheck::NotMate => "notMate",
            StepCheck::Shorter(_) => "shorter",
            StepCheck::Longer => "longer",
            StepCheck::NotUnique => "notUnique",
            StepCheck::OtherMove => "otherMove",
            StepCheck::Timeout => "timeout",
        }
    }
}

pub trait MateVerifier {
    /// Checks the step-`n` position `pos`, whose game move is `played`.
    fn check(&mut self, pos: &Chess, n: u8, played: &Move) -> Result<StepCheck, AppError>;
}

/// Mates in 1 and 2, solved exactly.
#[derive(Default)]
pub struct SolverVerifier;

impl MateVerifier for SolverVerifier {
    fn check(&mut self, pos: &Chess, n: u8, played: &Move) -> Result<StepCheck, AppError> {
        use chess_core::mate::{mates_in_one, mates_in_two};
        let moves = match n {
            1 => {
                let moves = mates_in_one(pos);
                if moves.is_empty() {
                    return Ok(if mates_in_two(pos).is_empty() {
                        StepCheck::NotMate
                    } else {
                        StepCheck::Longer
                    });
                }
                moves
            }
            2 => {
                if !mates_in_one(pos).is_empty() {
                    return Ok(StepCheck::Shorter(1));
                }
                let moves = mates_in_two(pos);
                if moves.is_empty() {
                    return Ok(StepCheck::NotMate);
                }
                moves
            }
            _ => {
                return Err(AppError::new(
                    "invalidParameters",
                    "The solver checks mates in 1 and 2",
                ));
            }
        };
        Ok(if moves.len() > 1 {
            StepCheck::NotUnique
        } else if moves[0] != *played {
            StepCheck::OtherMove
        } else {
            StepCheck::Valid
        })
    }
}

/// One line of the engine's analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub depth: Option<u32>,
    pub score: Score,
    /// The line's first move, in UCI.
    pub first: String,
}

/// The moves of a positive mate score.
fn mate_moves(line: &Line) -> Option<i32> {
    match line.score {
        Score::Mate { moves, .. } if moves > 0 => Some(moves),
        _ => None,
    }
}

/// The step check for the engine's lines (by multipv number) in a position
/// with `legal` legal moves; `played` is the game's move in UCI. Only a
/// position with a single legal move can do without a second line: with
/// others, a mate in `n` whose second line never arrived is not known to be
/// unique, so it is a timeout.
pub fn classify(n: u8, lines: &BTreeMap<u32, Line>, played: &str, legal: usize) -> StepCheck {
    let Some(best) = lines.get(&1) else {
        return StepCheck::Timeout;
    };
    let Some(moves) = mate_moves(best) else {
        return StepCheck::NotMate;
    };
    if moves < i32::from(n) {
        return StepCheck::Shorter(moves as u8);
    }
    if moves > i32::from(n) {
        return StepCheck::Longer;
    }
    match lines.get(&2) {
        Some(second) if mate_moves(second).is_some_and(|m| m <= i32::from(n)) => {
            return StepCheck::NotUnique;
        }
        None if legal > 1 => return StepCheck::Timeout,
        _ => {}
    }
    if best.first != played {
        return StepCheck::OtherMove;
    }
    StepCheck::Valid
}

/// Whether the search can stop before its time is up: a mate shorter than
/// `n` is proven by its score, and a mate in `n` is settled once the best
/// line has been searched to a depth of `2n + 4` and the second line to the
/// same depth (a position with one legal move has none). Anything else waits
/// for the time cap: the engine often shows a longer mate, or none, before it
/// finds the shortest one.
pub fn settled(n: u8, lines: &BTreeMap<u32, Line>, legal: usize) -> bool {
    let Some(best) = lines.get(&1) else {
        return false;
    };
    match mate_moves(best) {
        Some(m) if m < i32::from(n) => true,
        Some(m) if m == i32::from(n) => {
            best.depth.is_some_and(|d| d >= 2 * u32::from(n) + 4)
                && (legal == 1
                    || lines
                        .get(&2)
                        .is_some_and(|second| second.depth >= best.depth))
        }
        _ => false,
    }
}

/// Mates in 1 to 5, checked with a UCI engine (`MultiPV` 2).
pub struct EngineVerifier {
    session: EngineSession,
    millis: u64,
    searches: u64,
}

impl EngineVerifier {
    /// Starts the engine, which must offer `MultiPV`; `millis` caps the time
    /// of each check.
    pub fn spawn(spec: EngineSpec, millis: u64) -> Result<Self, AppError> {
        let err = |e: engine::EngineError| AppError::new("engine", e.to_string());
        let mut session = EngineSession::spawn(spec, Duration::from_secs(5)).map_err(err)?;
        if !session.options().iter().any(|o| o.name == "MultiPV") {
            return Err(AppError::new(
                "engine",
                "The engine has no MultiPV option, which the uniqueness check needs",
            ));
        }
        session.set_option("MultiPV", "2").map_err(err)?;
        Ok(Self {
            session,
            millis,
            searches: 0,
        })
    }
}

impl MateVerifier for EngineVerifier {
    fn check(&mut self, pos: &Chess, n: u8, played: &Move) -> Result<StepCheck, AppError> {
        let legal = pos.legal_moves().len();
        if legal == 0 {
            return Ok(StepCheck::NotMate);
        }
        let err = |e: engine::EngineError| AppError::new("engine", e.to_string());
        self.searches += 1;
        let id = format!("mate-{}", self.searches);
        self.session
            .analyse(AnalysisRequest {
                request_id: id.clone(),
                search_moves: Vec::new(),
                start_fen: Fen::from_position(pos, EnPassantMode::Legal).to_string(),
                history: Vec::new(),
                root_turn: if pos.turn().is_white() {
                    "white"
                } else {
                    "black"
                }
                .into(),
            })
            .map_err(err)?;
        let deadline = Instant::now() + Duration::from_millis(self.millis);
        let mut lines: BTreeMap<u32, Line> = BTreeMap::new();
        let take = |info: engine::AnalysisInfo, lines: &mut BTreeMap<u32, Line>| {
            // Bounds are not results.
            if let (Some(score), Some(pv)) = (info.score, info.pv)
                && let Some(first) = pv.first()
                && !matches!(
                    &score,
                    Score::Cp { bound: Some(_), .. } | Score::Mate { bound: Some(_), .. }
                )
            {
                lines.insert(
                    info.multipv,
                    Line {
                        depth: info.depth,
                        score,
                        first: first.clone(),
                    },
                );
            }
        };
        while Instant::now() < deadline && !settled(n, &lines, legal) {
            let wait = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50));
            if let Some(e) = self.session.recv_event(wait).map_err(err)?
                && e.request_id == id
            {
                match e.payload {
                    EnginePayload::Info(info) => take(info, &mut lines),
                    EnginePayload::Error { message } => {
                        return Err(AppError::new("engine", message));
                    }
                    _ => {}
                }
            }
        }
        self.session.stop().map_err(err)?;
        // Lines that arrived with the stop still count.
        while let Some(e) = self.session.recv_event(Duration::ZERO).map_err(err)? {
            if e.request_id == id
                && let EnginePayload::Info(info) = e.payload
            {
                take(info, &mut lines);
            }
        }
        let played = UciMove::from_standard(*played).to_string();
        Ok(classify(n, &lines, &played, legal))
    }
}

/// The solver for mates in 1 and 2, the engine for the rest.
pub struct CombinedVerifier {
    solver: SolverVerifier,
    engine: Option<EngineVerifier>,
}

impl CombinedVerifier {
    pub fn new(engine: EngineVerifier) -> Self {
        Self {
            solver: SolverVerifier,
            engine: Some(engine),
        }
    }

    /// Mates in 1 and 2 only; a deeper step is an error.
    pub fn solver_only() -> Self {
        Self {
            solver: SolverVerifier,
            engine: None,
        }
    }
}

impl MateVerifier for CombinedVerifier {
    fn check(&mut self, pos: &Chess, n: u8, played: &Move) -> Result<StepCheck, AppError> {
        if n <= 2 {
            return self.solver.check(pos, n, played);
        }
        match &mut self.engine {
            Some(engine) => engine.check(pos, n, played),
            None => Err(AppError::new(
                "engine",
                "Mates in 3 to 5 are checked with an engine",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mate(moves: i32, depth: u32, first: &str) -> Line {
        Line {
            depth: Some(depth),
            score: Score::Mate { moves, bound: None },
            first: first.into(),
        }
    }
    fn cp(value: i32, first: &str) -> Line {
        Line {
            depth: Some(20),
            score: Score::Cp { value, bound: None },
            first: first.into(),
        }
    }
    fn lines(list: &[Line]) -> BTreeMap<u32, Line> {
        list.iter()
            .cloned()
            .enumerate()
            .map(|(i, l)| (i as u32 + 1, l))
            .collect()
    }

    #[test]
    fn a_mate_in_n_with_the_game_move_and_a_worse_second_line_is_valid() {
        let l = lines(&[mate(3, 14, "e1e8"), cp(300, "a1a2")]);
        assert_eq!(classify(3, &l, "e1e8", 20), StepCheck::Valid);
        let longer = lines(&[mate(3, 14, "e1e8"), mate(4, 14, "a1a2")]);
        assert_eq!(classify(3, &longer, "e1e8", 20), StepCheck::Valid);
    }

    #[test]
    fn no_line_is_a_timeout_and_a_plain_score_is_not_a_mate() {
        assert_eq!(classify(3, &lines(&[]), "e1e8", 20), StepCheck::Timeout);
        assert_eq!(
            classify(3, &lines(&[cp(900, "e1e8")]), "e1e8", 20),
            StepCheck::NotMate
        );
        // Being mated is not a mate for the side to move.
        assert_eq!(
            classify(3, &lines(&[mate(-3, 14, "e1e8")]), "e1e8", 20),
            StepCheck::NotMate
        );
        // A second line without the first is no result either.
        let only_second: BTreeMap<u32, Line> = [(2, mate(3, 14, "e1e8"))].into();
        assert_eq!(classify(3, &only_second, "e1e8", 20), StepCheck::Timeout);
    }

    #[test]
    fn a_shorter_or_longer_mate_is_not_a_mate_in_n() {
        assert_eq!(
            classify(4, &lines(&[mate(2, 14, "e1e8")]), "e1e8", 20),
            StepCheck::Shorter(2)
        );
        assert_eq!(
            classify(3, &lines(&[mate(5, 14, "e1e8")]), "e1e8", 20),
            StepCheck::Longer
        );
    }

    #[test]
    fn a_second_mate_in_n_makes_the_step_not_unique() {
        let l = lines(&[mate(3, 14, "e1e8"), mate(3, 14, "d1d8")]);
        assert_eq!(classify(3, &l, "e1e8", 20), StepCheck::NotUnique);
        // Uniqueness is judged before the game's move.
        assert_eq!(classify(3, &l, "a1a2", 20), StepCheck::NotUnique);
    }

    #[test]
    fn another_first_move_is_not_the_game_move() {
        let l = lines(&[mate(3, 14, "d1d8"), cp(100, "e1e8")]);
        assert_eq!(classify(3, &l, "e1e8", 20), StepCheck::OtherMove);
    }

    #[test]
    fn a_missing_second_line_leaves_the_mate_unique_only_with_one_legal_move() {
        let l = lines(&[mate(3, 14, "e1e8")]);
        assert_eq!(classify(3, &l, "e1e8", 1), StepCheck::Valid);
        // With other moves to play, uniqueness was not looked at.
        assert_eq!(classify(3, &l, "e1e8", 20), StepCheck::Timeout);
        // A plain score or another length needs no second line.
        assert_eq!(
            classify(3, &lines(&[cp(10, "e1e8")]), "e1e8", 20),
            StepCheck::NotMate
        );
        assert_eq!(
            classify(3, &lines(&[mate(2, 14, "e1e8")]), "e1e8", 20),
            StepCheck::Shorter(2)
        );
    }

    fn deep(depth: u32, score: Score, first: &str) -> Line {
        Line {
            depth: Some(depth),
            score,
            first: first.into(),
        }
    }
    const MATE_3: Score = Score::Mate {
        moves: 3,
        bound: None,
    };
    const CP: Score = Score::Cp {
        value: 40,
        bound: None,
    };

    #[test]
    fn a_plain_score_or_a_longer_mate_waits_for_the_time_cap() {
        // The engine often shows no mate, or a longer one, before it finds
        // the shortest, however deep it has searched.
        assert!(!settled(3, &lines(&[deep(30, CP, "a")]), 20));
        let longer = Score::Mate {
            moves: 5,
            bound: None,
        };
        assert!(!settled(3, &lines(&[deep(30, longer, "a")]), 20));
        // Nothing yet, or a line without a depth, is not settled either.
        assert!(!settled(3, &lines(&[]), 20));
        let unknown = Line {
            depth: None,
            score: MATE_3,
            first: "a".into(),
        };
        assert!(!settled(3, &lines(&[unknown]), 20));
    }

    #[test]
    fn a_shorter_mate_is_settled_at_any_depth() {
        let two = Score::Mate {
            moves: 2,
            bound: None,
        };
        assert!(settled(3, &lines(&[deep(2, two, "a")]), 20));
    }

    #[test]
    fn a_mate_in_n_waits_for_the_second_line_to_catch_up() {
        let best = deep(10, MATE_3, "a");
        assert!(!settled(3, &lines(std::slice::from_ref(&best)), 20));
        assert!(!settled(3, &lines(&[best.clone(), deep(9, CP, "b")]), 20));
        assert!(settled(3, &lines(&[best.clone(), deep(10, CP, "b")]), 20));
        // Too shallow, even with both lines.
        assert!(!settled(
            3,
            &lines(&[deep(9, MATE_3, "a"), deep(9, CP, "b")]),
            20
        ));
        // A position with one legal move has no second line.
        assert!(settled(3, &lines(&[best]), 1));
    }
}
