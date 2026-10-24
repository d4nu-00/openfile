use engine::EngineSpec;
use openfile::mate_training::verify::{
    CombinedVerifier, EngineVerifier, MateVerifier, SolverVerifier, StepCheck,
};
use shakmaty::{Chess, Move, uci::UciMove};

fn pos(fen: &str) -> Chess {
    chess_core::model::parse_position(fen).unwrap()
}

fn mv(pos: &Chess, uci: &str) -> Move {
    uci.parse::<UciMove>().unwrap().to_move(pos).unwrap()
}

fn check(v: &mut impl MateVerifier, fen: &str, n: u8, played: &str) -> StepCheck {
    let p = pos(fen);
    let m = mv(&p, played);
    v.check(&p, n, &m).unwrap()
}

const BACK_RANK: &str = "6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1";
const TWO_MATES: &str = "6k1/5ppp/8/8/8/8/1Q6/R5K1 w - - 0 1";
const ROOK_IN_TWO: &str = "8/8/8/8/7R/4K3/8/6k1 w - - 0 1";
const QUEEN_IN_TWO_MANY: &str = "8/6Q1/8/8/8/4K3/8/5k2 w - - 0 1";

#[test]
fn the_solver_accepts_a_unique_mate_in_one_that_was_played() {
    let mut v = SolverVerifier;
    assert_eq!(check(&mut v, BACK_RANK, 1, "a1a8"), StepCheck::Valid);
}

#[test]
fn the_solver_rejects_two_mates_in_one_and_another_first_move() {
    let mut v = SolverVerifier;
    assert_eq!(check(&mut v, TWO_MATES, 1, "a1a8"), StepCheck::NotUnique);
    assert_eq!(check(&mut v, BACK_RANK, 1, "g1f1"), StepCheck::OtherMove);
}

#[test]
fn the_solver_tells_no_mate_from_a_longer_one_for_n_of_one() {
    let mut v = SolverVerifier;
    assert_eq!(check(&mut v, ROOK_IN_TWO, 1, "e3f3"), StepCheck::Longer);
    assert_eq!(
        check(
            &mut v,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            1,
            "e2e4"
        ),
        StepCheck::NotMate
    );
}

#[test]
fn the_solver_checks_mates_in_two_exactly() {
    let mut v = SolverVerifier;
    assert_eq!(check(&mut v, ROOK_IN_TWO, 2, "e3f3"), StepCheck::Valid);
    assert_eq!(check(&mut v, ROOK_IN_TWO, 2, "h4h3"), StepCheck::OtherMove);
    assert_eq!(
        check(&mut v, QUEEN_IN_TWO_MANY, 2, "g7g4"),
        StepCheck::NotUnique
    );
    // A mate in one is a shorter mate, not a mate in two.
    assert_eq!(check(&mut v, BACK_RANK, 2, "a1a8"), StepCheck::Shorter(1));
    assert_eq!(
        check(
            &mut v,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            2,
            "e2e4"
        ),
        StepCheck::NotMate
    );
}

#[test]
fn the_solver_refuses_deeper_mates() {
    let p = pos(BACK_RANK);
    let m = mv(&p, "a1a8");
    assert!(SolverVerifier.check(&p, 3, &m).is_err());
}

fn mock_engine() -> std::path::PathBuf {
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let name = format!("mock-uci{}", std::env::consts::EXE_SUFFIX);
    [deps.join(&name), deps.parent().unwrap().join(&name)]
        .into_iter()
        .find(|p| p.exists())
        .expect("mock-uci is missing: run `cargo test --workspace`")
}

fn mock(mode: &str) -> EngineSpec {
    EngineSpec {
        executable: mock_engine(),
        arguments: vec![mode.into()],
    }
}

#[test]
fn an_engine_that_finds_no_mate_gives_not_mate() {
    // The mock's lines are centipawn scores.
    let mut v = EngineVerifier::spawn(mock("legal"), 300).unwrap();
    assert_eq!(
        check(&mut v, "6k1/5ppp/8/8/8/8/5PPP/3RR1K1 w - - 0 1", 3, "e1e8"),
        StepCheck::NotMate
    );
}

#[test]
fn an_engine_that_gives_no_score_in_its_time_is_a_timeout() {
    let mut v = EngineVerifier::spawn(mock("silent"), 200).unwrap();
    assert_eq!(
        check(&mut v, "6k1/5ppp/8/8/8/8/5PPP/3RR1K1 w - - 0 1", 3, "e1e8"),
        StepCheck::Timeout
    );
}

#[test]
fn an_engine_without_multipv_cannot_check_uniqueness() {
    // Mode "normal" has no MultiPV option.
    assert!(EngineVerifier::spawn(mock("normal"), 200).is_err());
}

#[test]
fn the_combined_verifier_solves_short_mates_without_the_engine() {
    // The silent engine would time out on anything it were asked.
    let mut v = CombinedVerifier::new(EngineVerifier::spawn(mock("silent"), 100).unwrap());
    assert_eq!(check(&mut v, BACK_RANK, 1, "a1a8"), StepCheck::Valid);
    assert_eq!(check(&mut v, ROOK_IN_TWO, 2, "e3f3"), StepCheck::Valid);
    assert_eq!(
        check(&mut v, "6k1/5ppp/8/8/8/8/5PPP/3RR1K1 w - - 0 1", 3, "e1e8"),
        StepCheck::Timeout
    );
}

/// Run with `OPENFILE_TEST_STOCKFISH=/path/to/stockfish cargo test -p openfile
/// --test mate_training -- --ignored`.
fn stockfish() -> EngineSpec {
    let path = std::env::var_os("OPENFILE_TEST_STOCKFISH")
        .expect("set OPENFILE_TEST_STOCKFISH to a Stockfish executable");
    EngineSpec {
        executable: path.into(),
        arguments: vec![],
    }
}

/// White mates in three, and only with 1.Kg3 (checked move by move).
const ROOK_IN_THREE: &str = "8/8/8/4p3/1R5K/8/8/6k1 w - - 0 1";

#[test]
#[ignore = "needs OPENFILE_TEST_STOCKFISH"]
fn stockfish_confirms_a_unique_mate_in_three() {
    let mut v = EngineVerifier::spawn(stockfish(), 3000).unwrap();
    assert_eq!(check(&mut v, ROOK_IN_THREE, 3, "h4g3"), StepCheck::Valid);
}

#[test]
#[ignore = "needs OPENFILE_TEST_STOCKFISH"]
fn stockfish_finds_another_first_move_and_a_different_length() {
    let mut v = EngineVerifier::spawn(stockfish(), 3000).unwrap();
    assert_eq!(
        check(&mut v, ROOK_IN_THREE, 3, "b4b2"),
        StepCheck::OtherMove
    );
    assert_eq!(
        check(&mut v, ROOK_IN_THREE, 4, "h4g3"),
        StepCheck::Shorter(3)
    );
    assert_eq!(check(&mut v, ROOK_IN_THREE, 2, "h4g3"), StepCheck::Longer);
}

#[test]
#[ignore = "needs OPENFILE_TEST_STOCKFISH"]
fn stockfish_treats_a_position_without_mate_as_not_a_mate() {
    let mut v = EngineVerifier::spawn(stockfish(), 500).unwrap();
    assert_eq!(
        check(
            &mut v,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            3,
            "e2e4"
        ),
        StepCheck::NotMate
    );
}
