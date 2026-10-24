use cbh_reader::{Database, ReadLimits, ending::Tally, v2::write::Writer};
use chess_core::{
    Game, GameLimits, parse_pgn,
    training::{TrainingKind, square_name},
};
use engine::EngineSpec;
use openfile::{
    clipboard::RowRange,
    commands::AppError,
    mate_training::{
        build::{
            BuildOptions, Collected, combination, normalise_combination, ordered, puzzle_game,
        },
        run::MateTrainingParams,
        verify::{CombinedVerifier, EngineVerifier, MateVerifier, SolverVerifier, StepCheck},
    },
    registry::Registry,
};
use shakmaty::{Chess, EnPassantMode, Move, fen::Epd, uci::UciMove};
use std::collections::HashMap;

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
fn stockfish_finds_the_second_mate_in_three_that_shows_late() {
    // 1.Ne7+ was played, but 1.Rh8+ mates in three too. Stockfish shows the
    // second line's mate only from depth 12, so a search that stops at the
    // first depth where the best line is a mate in three calls it unique.
    let mut v = EngineVerifier::spawn(stockfish(), 800).unwrap();
    assert_eq!(
        check(
            &mut v,
            "6k1/5p2/4pPnR/1p1pPNK1/1p1P2P1/1Pr5/P1r5/7R w - - 0 42",
            3,
            "f5e7"
        ),
        StepCheck::NotUnique
    );
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

// ---- the builder ----

/// The position without its move counters.
fn epd(pos: &Chess) -> String {
    Epd::from_position(pos, EnPassantMode::Legal).to_string()
}

/// Verifier results by (position, n); everything else is not a mate.
#[derive(Default)]
struct Script {
    results: HashMap<(String, u8), StepCheck>,
    calls: Vec<(String, u8)>,
}

impl Script {
    fn valid(mut self, steps: &[(&str, u8)]) -> Self {
        for (epd, n) in steps {
            self.results.insert((epd.to_string(), *n), StepCheck::Valid);
        }
        self
    }
    fn says(mut self, epd: &str, n: u8, check: StepCheck) -> Self {
        self.results.insert((epd.to_string(), n), check);
        self
    }
}

impl MateVerifier for Script {
    fn check(&mut self, pos: &Chess, n: u8, _played: &Move) -> Result<StepCheck, AppError> {
        let key = (epd(pos), n);
        self.calls.push(key.clone());
        Ok(self
            .results
            .get(&key)
            .copied()
            .unwrap_or(StepCheck::NotMate))
    }
}

// Step positions of the test games: the position 2i-1 plies before the end.
const FOOL_1: &str = "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq -";
const FOOL_2: &str = "rnbqkbnr/pppppppp/8/8/8/5P2/PPPPP1PP/RNBQKBNR b KQkq -";
const FOOL_B_1: &str = "rnbqkbnr/pppp1ppp/8/4p3/5PP1/8/PPPPP2P/RNBQKBNR b KQkq -";
const FOOL_B_2: &str = "rnbqkbnr/pppppppp/8/8/6P1/8/PPPPPP1P/RNBQKBNR b KQkq -";
const SCHOLAR_1: &str = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq -";
const SCHOLAR_2: &str = "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/8/PPPP1PPP/RNBQK1NR w KQkq -";
const SCHOLAR_3: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq -";
const SCHOLAR_4: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -";
const RA8: &str = "6k1/5ppp/8/8/8/8/8/R5K1 w - -";
const RB8: &str = "6k1/5ppp/8/8/8/8/8/1R4K1 w - -";
const RC8: &str = "6k1/5ppp/8/8/8/8/8/2R3K1 w - -";
const NF7: &str = "6rk/6pp/8/6N1/8/8/8/6K1 w - -";
const PROMOTE: &str = "7k/P5pp/8/8/8/8/8/6K1 w - -";

const HEADER: &str = "[Event \"Club Open\"]\n[Site \"Oslo\"]\n[Date \"2024.05.17\"]\n[Round \"3\"]\n[White \"Alpha, A\"]\n[Black \"Beta, B\"]\n[WhiteElo \"2310\"]\n[BlackElo \"2100\"]\n[ECO \"C20\"]\n";

fn game(result: &str, setup: Option<&str>, moves: &str) -> Game {
    let fen = setup
        .map(|f| format!("[SetUp \"1\"]\n[FEN \"{f}\"]\n"))
        .unwrap_or_default();
    let text = format!("{HEADER}[Result \"{result}\"]\n{fen}\n{moves} {result}\n");
    parse_pgn(text.as_bytes(), &GameLimits::default())
        .unwrap()
        .remove(0)
}

fn fools_mate() -> Game {
    game("0-1", None, "1. f3 e5 2. g4 Qh4#")
}
fn fools_mate_b() -> Game {
    game("0-1", None, "1. g4 e5 2. f4 Qh4#")
}
fn scholars_mate() -> Game {
    game("1-0", None, "1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7#")
}
fn rook_mate(fen: &str, mv: &str) -> Game {
    game("1-0", Some(&format!("{fen} 0 1")), &format!("1. {mv}"))
}
fn back_rank(file: char) -> Game {
    let (fen, mv) = match file {
        'a' => (RA8, "Ra8#"),
        'b' => (RB8, "Rb8#"),
        _ => (RC8, "Rc8#"),
    };
    rook_mate(fen, mv)
}
fn smothered() -> Game {
    rook_mate(NF7, "Nf7#")
}

fn options(min: u8, max: u8, per: u32) -> BuildOptions {
    BuildOptions {
        min_depth: min,
        max_depth: max,
        per,
        combinations: None,
        pool_minors: false,
    }
}

fn collected(min: u8, max: u8, per: u32) -> Collected {
    Collected::new(&options(min, max, per)).unwrap()
}

fn plies(game: &Game) -> Vec<String> {
    let mut id = 0;
    let mut out = vec![];
    while let Some(&next) = game.nodes[id as usize].children.first() {
        out.push(game.nodes[next as usize].uci.clone().unwrap());
        id = next;
    }
    out
}

fn start(game: &Game) -> String {
    epd(&chess_core::model::parse_position(&game.start_fen).unwrap())
}

#[test]
fn combinations_are_written_in_piece_order_and_pool_the_minors_on_request() {
    let t = |k, q, r, b, n, p| Tally {
        kings: k,
        queens: q,
        rooks: r,
        bishops: b,
        knights: n,
        pawns: p,
    };
    assert_eq!(combination(&t(0, 1, 0, 0, 1, 0), false), "Q+N");
    assert_eq!(combination(&t(1, 0, 1, 0, 0, 0), false), "R+K");
    assert_eq!(combination(&t(0, 1, 0, 1, 0, 1), false), "Q+B+P");
    assert_eq!(combination(&t(0, 0, 0, 0, 1, 0), false), "N");
    // Two rooks are still one kind; bishops and knights stay apart.
    assert_eq!(combination(&t(0, 0, 2, 1, 1, 0), false), "R+B+N");
    assert_eq!(combination(&t(0, 1, 0, 1, 0, 0), true), "Q+m");
    assert_eq!(combination(&t(1, 1, 0, 0, 1, 0), true), "Q+m+K");
    assert_eq!(combination(&t(0, 0, 0, 1, 1, 0), true), "m");
}

#[test]
fn a_combination_filter_is_normalised_before_comparing() {
    assert_eq!(
        normalise_combination("n + q", false).as_deref(),
        Some("Q+N")
    );
    assert_eq!(
        normalise_combination(" r+k ", false).as_deref(),
        Some("R+K")
    );
    assert_eq!(
        normalise_combination("k+p+b", false).as_deref(),
        Some("B+P+K")
    );
    assert_eq!(normalise_combination("Q+Q", false).as_deref(), Some("Q"));
    // Bishop = Knight: B and N are the minor m; m is not a piece otherwise.
    assert_eq!(normalise_combination("n+q", true).as_deref(), Some("Q+m"));
    assert_eq!(normalise_combination("b+n", true).as_deref(), Some("m"));
    assert_eq!(normalise_combination("Q+M", true).as_deref(), Some("Q+m"));
    assert_eq!(normalise_combination("Q+M", false), None);
    for bad in ["", "+", "x+q", "q+", "qr", "q++r", "q r"] {
        assert_eq!(normalise_combination(bad, false), None, "{bad:?}");
    }
}

#[test]
fn options_are_checked() {
    assert!(options(1, 5, 20).check().is_ok());
    assert!(options(0, 5, 20).check().is_err());
    assert!(options(3, 2, 20).check().is_err());
    assert!(options(1, 6, 20).check().is_err());
    assert!(options(1, 5, 0).check().is_err());
    let mut o = options(1, 5, 20);
    o.combinations = Some(vec!["Q+X".into()]);
    assert!(o.check().is_err());
    assert!(Collected::new(&o).is_err());
    o.combinations = Some(vec!["q+n".into()]);
    assert!(o.check().is_ok());
}

#[test]
fn a_chain_stops_at_its_first_invalid_step() {
    let mut c = collected(1, 5, 10);
    // Step 4 would be valid, but step 3 is not.
    let mut v = Script::default()
        .valid(&[(SCHOLAR_1, 1), (SCHOLAR_2, 2), (SCHOLAR_4, 4)])
        .says(SCHOLAR_3, 3, StepCheck::OtherMove);
    c.add_game(7, &scholars_mate(), &mut v).unwrap();
    assert_eq!(
        v.calls,
        [
            (SCHOLAR_1.to_string(), 1),
            (SCHOLAR_2.to_string(), 2),
            (SCHOLAR_3.to_string(), 3)
        ]
    );
    assert_eq!(c.rejections.get("otherMove"), Some(&1));
    assert_eq!(c.rejections.get("notMate"), None);
    assert_eq!(c.scanned, 1);
    assert_eq!(c.puzzles(1).len(), 1);
    assert_eq!(c.puzzles(2).len(), 1);
    assert!(c.puzzles(3).is_empty() && c.puzzles(4).is_empty());
    assert_eq!(c.chains().len(), 1);
    assert_eq!(c.chains()[0].steps.len(), 2);
    assert_eq!(c.chains()[0].combination, "Q+B");
    assert_eq!(c.chains()[0].ordinal, 7);
}

#[test]
fn a_chain_runs_to_the_last_step_that_exists_and_is_valid() {
    let mut c = collected(1, 5, 10);
    let mut v = Script::default().valid(&[
        (SCHOLAR_1, 1),
        (SCHOLAR_2, 2),
        (SCHOLAR_3, 3),
        (SCHOLAR_4, 4),
    ]);
    c.add_game(1, &scholars_mate(), &mut v).unwrap();
    // The game is too short for a step 5, which is never asked about.
    assert_eq!(v.calls.len(), 4);
    assert_eq!(c.chains()[0].steps.len(), 4);
    for d in 1..=4 {
        assert_eq!(c.puzzles(d).len(), 1, "depth {d}");
    }
    assert!(c.puzzles(5).is_empty());
    assert!(c.rejections.is_empty());
}

#[test]
fn the_deepest_step_asked_ends_the_chain() {
    let mut c = collected(1, 2, 10);
    let mut v = Script::default().valid(&[
        (SCHOLAR_1, 1),
        (SCHOLAR_2, 2),
        (SCHOLAR_3, 3),
        (SCHOLAR_4, 4),
    ]);
    c.add_game(1, &scholars_mate(), &mut v).unwrap();
    assert_eq!(v.calls.len(), 2);
    assert_eq!(c.chains()[0].steps.len(), 2);
    assert!(c.puzzles(3).is_empty());
}

#[test]
fn depths_below_the_first_are_verified_but_not_kept() {
    let mut c = collected(2, 3, 10);
    let mut v = Script::default().valid(&[(SCHOLAR_1, 1), (SCHOLAR_2, 2), (SCHOLAR_3, 3)]);
    c.add_game(1, &scholars_mate(), &mut v).unwrap();
    assert!(c.puzzles(1).is_empty());
    assert_eq!(c.puzzles(2).len(), 1);
    assert_eq!(c.puzzles(3).len(), 1);
    // The chain still begins with the mate in 1.
    assert_eq!(c.chains()[0].steps.len(), 3);
    assert_eq!(v.calls.len(), 3);
}

#[test]
fn a_game_with_one_valid_step_makes_a_puzzle_but_no_chain() {
    let mut c = collected(1, 5, 10);
    let mut v = Script::default().valid(&[(RA8, 1)]);
    c.add_game(3, &back_rank('a'), &mut v).unwrap();
    assert_eq!(c.puzzles(1).len(), 1);
    assert!(c.chains().is_empty());
    assert_eq!(c.count("R", 1), 1);
}

#[test]
fn quotas_hold_per_combination_and_depth() {
    let mut c = collected(1, 5, 2);
    let mut v = Script::default().valid(&[(RA8, 1), (RB8, 1), (RC8, 1), (NF7, 1)]);
    for (i, g) in [back_rank('a'), back_rank('b'), back_rank('c'), smothered()]
        .iter()
        .enumerate()
    {
        c.add_game(i as u64 + 1, g, &mut v).unwrap();
    }
    assert_eq!(c.count("R", 1), 2);
    assert_eq!(c.count("N", 1), 1);
    // The third rook mate was not even verified: nothing could be kept.
    assert!(!v.calls.contains(&(RC8.to_string(), 1)));
    assert_eq!(
        c.puzzles(1)
            .iter()
            .map(|p| (p.combination.as_str(), p.ordinal))
            .collect::<Vec<_>>(),
        [("R", 1), ("R", 2), ("N", 4)]
    );
}

#[test]
fn a_full_quota_at_one_depth_does_not_stop_a_deeper_one_from_filling() {
    let mut c = collected(1, 2, 1);
    // Game 1 fills the Q quota at depth 1 only.
    let mut v = Script::default().valid(&[(FOOL_1, 1), (FOOL_B_1, 1), (FOOL_B_2, 2)]);
    c.add_game(1, &fools_mate(), &mut v).unwrap();
    assert_eq!((c.count("Q", 1), c.count("Q", 2)), (1, 0));
    // Game 2: depth 1 is full, depth 2 has room, so step 1 is still checked.
    c.add_game(2, &fools_mate_b(), &mut v).unwrap();
    assert_eq!((c.count("Q", 1), c.count("Q", 2)), (1, 1));
    assert_eq!(c.puzzles(2)[0].ordinal, 2);
    assert!(v.calls.contains(&(FOOL_B_1.to_string(), 1)));
    // Game 1's chain (steps 1 only) is too short; game 2's has two steps.
    assert_eq!(c.chains().len(), 1);
    assert_eq!(c.chains()[0].ordinal, 2);
}

#[test]
fn a_game_is_not_verified_when_nothing_it_makes_has_room() {
    let mut c = collected(1, 1, 1);
    let mut v = Script::default().valid(&[(FOOL_1, 1), (FOOL_B_1, 1)]);
    c.add_game(1, &fools_mate(), &mut v).unwrap();
    let calls = v.calls.len();
    c.add_game(2, &fools_mate_b(), &mut v).unwrap();
    assert_eq!(v.calls.len(), calls);
    assert_eq!(c.count("Q", 1), 1);
}

#[test]
fn a_chain_still_checks_further_when_only_the_chain_has_room() {
    // Depth 1 is full for Q, but the chain quota has room, so both steps of
    // the second game are verified and its chain is kept.
    let mut c = collected(1, 2, 1);
    let mut v = Script::default().valid(&[(FOOL_1, 1), (FOOL_B_1, 1), (FOOL_B_2, 2)]);
    c.add_game(1, &fools_mate(), &mut v).unwrap();
    c.add_game(2, &fools_mate_b(), &mut v).unwrap();
    assert_eq!(c.chain_count("Q"), 1);
    // A third chain would exceed the quota of 1.
    let mut v3 = Script::default().valid(&[(FOOL_1, 1), (FOOL_2, 2)]);
    c.add_game(3, &fools_mate(), &mut v3).unwrap();
    assert_eq!(c.chain_count("Q"), 1);
    assert!(v3.calls.is_empty());
}

#[test]
fn a_position_already_collected_for_a_depth_is_skipped() {
    let mut c = collected(1, 5, 10);
    let mut v = Script::default().valid(&[(FOOL_1, 1), (FOOL_2, 2)]);
    c.add_game(1, &fools_mate(), &mut v).unwrap();
    // The same game again, and a move-order transposition of its step 1.
    c.add_game(2, &fools_mate(), &mut v).unwrap();
    c.add_game(3, &game("0-1", None, "1. g4 e5 2. f3 Qh4#"), &mut v)
        .unwrap();
    assert_eq!(c.puzzles(1).len(), 1);
    assert_eq!(c.puzzles(2).len(), 1);
    assert_eq!(c.chains().len(), 1);
    assert!(c.rejections.get("duplicate").copied().unwrap_or(0) >= 3);
}

#[test]
fn games_not_ending_in_mate_or_in_chess960_are_counted_and_skipped() {
    let mut c = collected(1, 5, 10);
    let mut v = Script::default().valid(&[(RA8, 1)]);
    c.add_game(1, &game("*", None, "1. e4 e5"), &mut v).unwrap();
    c.add_game(
        2,
        &game("1-0", Some("6k1/5p1p/8/8/8/8/8/R5K1 w - - 0 1"), "1. Ra8+"),
        &mut v,
    )
    .unwrap();
    let mut chess960 = back_rank('a');
    chess960.tags.insert("Variant".into(), "Chess960".into());
    c.add_game(3, &chess960, &mut v).unwrap();
    assert_eq!(c.scanned, 3);
    assert_eq!(c.rejections.get("notMateGame"), Some(&2));
    assert_eq!(c.rejections.get("chess960"), Some(&1));
    assert!(v.calls.is_empty());
    assert!(c.puzzles(1).is_empty());
}

#[test]
fn a_combination_filter_keeps_only_the_listed_combinations() {
    let mut o = options(1, 5, 10);
    o.combinations = Some(vec!["n".into(), "R + K".into()]);
    let mut c = Collected::new(&o).unwrap();
    let mut v = Script::default().valid(&[(RA8, 1), (NF7, 1), (FOOL_1, 1)]);
    c.add_game(1, &back_rank('a'), &mut v).unwrap();
    c.add_game(2, &smothered(), &mut v).unwrap();
    c.add_game(3, &fools_mate(), &mut v).unwrap();
    // Back-rank mate by a rook alone is R, not R+K; the fool's mate is Q.
    assert_eq!(c.count("N", 1), 1);
    assert_eq!(c.puzzles(1).len(), 1);
    assert_eq!(c.filtered_out, 2);
    assert!(!v.calls.contains(&(FOOL_1.to_string(), 1)));
}

#[test]
fn the_builder_is_full_when_every_listed_quota_is_met() {
    let mut o = options(1, 1, 1);
    o.combinations = Some(vec!["R".into(), "N".into()]);
    let mut c = Collected::new(&o).unwrap();
    let mut v = Script::default().valid(&[(RA8, 1), (NF7, 1)]);
    assert!(!c.full());
    c.add_game(1, &back_rank('a'), &mut v).unwrap();
    assert!(!c.full());
    c.add_game(2, &smothered(), &mut v).unwrap();
    assert!(c.full());
    // Without a filter there is no end to the combinations.
    let mut open = collected(1, 1, 1);
    open.add_game(1, &back_rank('a'), &mut v).unwrap();
    assert!(!open.full());
}

#[test]
fn pooled_minors_share_one_combination() {
    let mut o = options(1, 1, 10);
    o.pool_minors = true;
    let mut c = Collected::new(&o).unwrap();
    let mut v = Script::default().valid(&[(NF7, 1)]);
    c.add_game(1, &smothered(), &mut v).unwrap();
    assert_eq!(c.combinations(), ["m"]);
}

#[test]
fn each_depth_database_interleaves_the_combinations() {
    let mut c = collected(1, 1, 10);
    let mut v = Script::default().valid(&[(RA8, 1), (RB8, 1), (NF7, 1), (RC8, 1)]);
    for (i, g) in [back_rank('a'), back_rank('b'), smothered(), back_rank('c')]
        .iter()
        .enumerate()
    {
        c.add_game(i as u64 + 1, g, &mut v).unwrap();
    }
    assert_eq!(c.combinations(), ["R", "N"]);
    let (depths, chains) = ordered(&c);
    assert_eq!(depths.len(), 5);
    assert!(depths[1..].iter().all(Vec::is_empty) && chains.is_empty());
    // R1, N1, R2, R3: the first of each combination, then the second, ...
    assert_eq!(
        depths[0].iter().map(start).collect::<Vec<_>>(),
        [RA8, NF7, RB8, RC8]
    );
}

#[test]
fn the_chain_database_keeps_each_chain_together_from_mate_in_one() {
    let mut c = collected(1, 5, 10);
    let mut v = Script::default().valid(&[
        (FOOL_1, 1),
        (FOOL_2, 2),
        (SCHOLAR_1, 1),
        (SCHOLAR_2, 2),
        (SCHOLAR_3, 3),
    ]);
    c.add_game(1, &fools_mate(), &mut v).unwrap();
    c.add_game(2, &scholars_mate(), &mut v).unwrap();
    let (_, chains) = ordered(&c);
    assert_eq!(
        chains.iter().map(start).collect::<Vec<_>>(),
        [FOOL_1, FOOL_2, SCHOLAR_1, SCHOLAR_2, SCHOLAR_3]
    );
}

fn comment(game: &Game) -> String {
    game.nodes[1].pre_comments.join(" ")
}

#[test]
fn a_puzzle_starts_at_its_step_and_plays_the_rest_of_the_game() {
    let g = puzzle_game(&scholars_mate(), 2, "Q+B", 12345).unwrap();
    assert_eq!(start(&g), SCHOLAR_2);
    assert_eq!(g.tags["SetUp"], "1");
    assert_eq!(plies(&g), ["d1h5", "g8f6", "h5f7"]);
    // No variations: every node has at most one child.
    assert!(g.nodes.iter().all(|n| n.children.len() <= 1));
    assert_eq!(comment(&g), "Mate in 2 · Q+B · from game 12345");
    // The longest step starts from the initial position.
    let g = puzzle_game(&scholars_mate(), 4, "Q+B", 1).unwrap();
    assert_eq!(start(&g), SCHOLAR_4);
    assert_eq!(plies(&g).len(), 7);
}

fn text(block: &chess_core::training::Block) -> String {
    block.text(0).unwrap_or_default().to_string()
}

#[test]
fn a_puzzle_asks_for_each_move_of_the_mating_side() {
    let g = puzzle_game(&scholars_mate(), 3, "Q+B", 9).unwrap();
    // Plies 1, 3 and 5 are White's; 2 and 4 are Black's replies.
    let asked: Vec<usize> = (1..=5).filter(|&n| g.nodes[n].training.is_some()).collect();
    assert_eq!(asked, [1, 3, 5]);
    let q = |n: usize| g.nodes[n].training.as_ref().unwrap();
    assert_eq!(text(&q(1).question), "White to move: mate in 3");
    assert_eq!(text(&q(3).question), "Continue: mate in 2");
    assert_eq!(text(&q(5).question), "Mate in 1");
    for n in [1, 3, 5] {
        let t = q(n);
        assert_eq!(t.kind, TrainingKind::MoveInput);
        assert_eq!((t.seconds, t.score), (0, 1));
        assert_eq!(
            text(&t.wrong),
            "Not the mate. Look at the squares around the king."
        );
        assert_eq!(t.answers.len(), 1);
        assert_eq!(t.answers[0].points, 1);
    }
    // Only the first question has the hint.
    assert_eq!(text(&q(1).help[0]), "Pieces in the mate: Q+B");
    assert!(q(1).help[1].is_empty());
    assert!(q(3).help[0].is_empty() && q(5).help[0].is_empty());
    // The answers are the moves of the line: Bc4, Qh5 and Qxf7.
    let squares = |n: usize| {
        let a = &q(n).answers[0];
        (square_name(a.from).unwrap(), square_name(a.to).unwrap())
    };
    assert_eq!(squares(1), ("f1".into(), "c4".into()));
    assert_eq!(squares(3), ("d1".into(), "h5".into()));
    assert_eq!(squares(5), ("h5".into(), "f7".into()));
}

#[test]
fn black_puzzles_say_black_to_move_and_a_mate_in_one_has_one_question() {
    let g = puzzle_game(&fools_mate(), 1, "Q", 2).unwrap();
    assert_eq!(plies(&g), ["d8h4"]);
    assert_eq!(
        text(&g.nodes[1].training.as_ref().unwrap().question),
        "Black to move: mate in 1"
    );
    assert_eq!(comment(&g), "Mate in 1 · Q · from game 2");
}

#[test]
fn a_puzzle_keeps_the_source_players_event_and_ratings() {
    let g = puzzle_game(&fools_mate(), 1, "Q", 2).unwrap();
    for (tag, value) in [
        ("White", "Alpha, A"),
        ("Black", "Beta, B"),
        ("Event", "Club Open"),
        ("Site", "Oslo"),
        ("Date", "2024.05.17"),
        ("Round", "3"),
        ("Result", "0-1"),
        ("WhiteElo", "2310"),
        ("BlackElo", "2100"),
        ("Annotator", "OpenFile mate training"),
    ] {
        assert_eq!(g.tags.get(tag).map(String::as_str), Some(value), "{tag}");
    }
    assert!(!g.tags.contains_key("ECO"));
}

#[test]
fn a_step_beyond_the_game_is_an_error() {
    assert!(puzzle_game(&fools_mate(), 3, "Q", 1).is_err());
    assert!(puzzle_game(&fools_mate(), 0, "Q", 1).is_err());
    // A game that does not end in mate has no puzzle.
    assert!(puzzle_game(&game("*", None, "1. e4 e5"), 1, "Q", 1).is_err());
}

fn read_back(games: &[Game]) -> Vec<Game> {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Back.2cbh");
    let mut w = Writer::create(&path).unwrap();
    w.append_all(games).unwrap();
    drop(w);
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    (1..=games.len() as u64)
        .map(|n| db.load(n).unwrap().game.unwrap())
        .collect()
}

#[test]
fn a_mate_by_promotion_round_trips_through_the_database() {
    let promotion = rook_mate(PROMOTE, "a8=Q#");
    let puzzle = puzzle_game(&promotion, 1, "Q", 5).unwrap();
    assert_eq!(plies(&puzzle), ["a7a8q"]);
    let back = read_back(&[puzzle]).remove(0);
    assert_eq!(plies(&back), ["a7a8q"]);
    let t = back.nodes[1].training.as_ref().unwrap();
    let a = &t.answers[0];
    assert_eq!(
        (square_name(a.from).unwrap(), square_name(a.to).unwrap()),
        ("a7".into(), "a8".into())
    );
    assert_eq!(a.points, 1);
    assert_eq!(text(&t.question), "White to move: mate in 1");
}

#[test]
fn a_mate_by_underpromotion_cannot_be_a_question() {
    // The stored answer holds squares only, which the replay reads as a
    // queen promotion, so the puzzle is refused.
    let promotion = rook_mate(PROMOTE, "a8=R#");
    assert!(puzzle_game(&promotion, 1, "R", 5).is_err());
    let mut c = collected(1, 1, 10);
    let mut v = Script::default().valid(&[(PROMOTE, 1)]);
    c.add_game(1, &promotion, &mut v).unwrap();
    assert!(c.puzzles(1).is_empty());
    assert_eq!(c.rejections.get("underpromotion"), Some(&1));
}

#[test]
fn a_step_position_with_a_legal_en_passant_capture_cannot_be_stored() {
    // After 1.d4 Black may take en passant; the database cannot hold a
    // set-up with an en-passant square, so there is no puzzle to write.
    let source = game(
        "0-1",
        Some("6k1/8/8/8/4p3/r7/3P2PP/7K w - - 0 1"),
        "1. d4 Ra1#",
    );
    assert!(puzzle_game(&source, 1, "R", 5).is_err());
    let mut c = collected(1, 1, 10);
    let step = "6k1/8/8/8/3Pp3/r7/6PP/7K b - d3";
    let mut v = Script::default().valid(&[(step, 1)]);
    c.add_game(1, &source, &mut v).unwrap();
    assert!(c.puzzles(1).is_empty());
    assert_eq!(c.rejections.get("unstorable"), Some(&1));
}

#[test]
fn built_puzzles_survive_a_round_trip_with_their_questions() {
    let g = puzzle_game(&scholars_mate(), 3, "Q+B", 9).unwrap();
    let back = read_back(std::slice::from_ref(&g)).remove(0);
    assert_eq!(start(&back), start(&g));
    assert_eq!(plies(&back), plies(&g));
    for n in [1, 3, 5] {
        assert_eq!(back.nodes[n].training, g.nodes[n].training, "question {n}");
    }
    assert_eq!(comment(&back), comment(&g));
}

// ---- the command ----

fn rows(start: u64, end: u64) -> Vec<RowRange> {
    vec![
        serde_json::from_value(serde_json::json!({
            "start": start.to_string(),
            "end": end.to_string()
        }))
        .unwrap(),
    ]
}

fn rook_in_two() -> Game {
    game(
        "1-0",
        Some("8/8/8/8/7R/4K3/8/6k1 w - - 0 1"),
        "1. Kf3 Kf1 2. Rh1#",
    )
}

/// Entries 1 to 7: a mate in 2 (R+K), two back-rank mates (R), a game that is
/// not a mate, a smothered mate (N), the first game again, and a third
/// back-rank mate (R).
fn source(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("Source.2cbh");
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&[
        rook_in_two(),
        back_rank('a'),
        back_rank('b'),
        game("*", None, "1. e4 e5"),
        smothered(),
        rook_in_two(),
        back_rank('c'),
    ])
    .unwrap();
    path
}

fn params(r: &Registry, dir: &std::path::Path, max: u8) -> MateTrainingParams {
    let db = r
        .open_database(source(dir), "o".into())
        .unwrap()
        .database_id;
    MateTrainingParams {
        database_id: db,
        ranges: rows(1, 8),
        options: options(1, max, 10),
        engine: None,
        millis: 1000,
        folder: dir.join("out"),
        name: "Train".into(),
    }
}

fn out(dir: &std::path::Path) -> std::path::PathBuf {
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    out
}

fn files(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn games_of(path: &str) -> Vec<Game> {
    let mut db = Database::open(std::path::Path::new(path), ReadLimits::default()).unwrap();
    let n = db.info().entry_count;
    (1..=n).map(|i| db.load(i).unwrap().game.unwrap()).collect()
}

#[test]
fn builds_one_database_per_depth_and_one_of_chains() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let p = params(&r, d.path(), 2);
    out(d.path());
    let mut seen = vec![];
    let done = r
        .build_mate_training(p, |scanned, total, c| {
            seen.push((scanned, total, c.puzzles(1).len()))
        })
        .unwrap();
    assert!(!done.stopped && !done.cancelled);
    assert_eq!((done.scanned.as_str(), done.total.as_str()), ("7", "7"));
    assert_eq!(seen.last().map(|s| (s.0, s.1)), Some((7, 7)));
    assert_eq!(
        files(&d.path().join("out"))
            .into_iter()
            .filter(|f| f.ends_with(".2cbh"))
            .collect::<Vec<_>>(),
        [
            "Train - Mate chains.2cbh",
            "Train - Mate in 1.2cbh",
            "Train - Mate in 2.2cbh"
        ]
    );
    let one = &done.databases[0];
    assert_eq!((one.depth, one.games), (Some(1), 5));
    // R+K first, then R, then N; the second of each follows: R+K, R, N, R, R.
    // (The repeated game is a duplicate.)
    let starts: Vec<String> = games_of(&one.path).iter().map(start).collect();
    assert_eq!(starts, ["8/8/8/8/7R/5K2/8/5k2 w - -", RA8, NF7, RB8, RC8]);
    let two = &done.databases[1];
    assert_eq!((two.depth, two.games), (Some(2), 1));
    let chains = &done.databases[2];
    assert_eq!((chains.depth, chains.games), (None, 2));
    // Every game carries its questions: 1 in the mate in 1, 2 in the mate in 2.
    let questions = |g: &Game| g.nodes.iter().filter(|n| n.training.is_some()).count();
    assert!(games_of(&one.path).iter().all(|g| questions(g) == 1));
    assert_eq!(questions(&games_of(&two.path)[0]), 2);
    assert_eq!(done.puzzles, [5, 1, 0, 0, 0]);
    assert_eq!(done.chains, 1);
    assert_eq!(
        done.rejections
            .iter()
            .find(|r| r.kind == "notMateGame")
            .map(|r| r.count.as_str()),
        Some("1")
    );
    let combos: Vec<_> = done
        .combinations
        .iter()
        .map(|c| (c.combination.as_str(), c.depths[0]))
        .collect();
    assert_eq!(combos, [("R+K", 1), ("R", 3), ("N", 1)]);
}

#[test]
fn the_quota_is_met_per_combination_and_the_scan_ends_when_all_are_full() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let mut p = params(&r, d.path(), 1);
    out(d.path());
    p.options.per = 1;
    p.options.combinations = Some(vec!["r".into(), "n".into()]);
    let done = r.build_mate_training(p, |_, _, _| {}).unwrap();
    assert!(done.complete);
    // Games 2 (R) and 5 (N) fill the quotas; the scan ends there.
    assert_eq!(done.scanned, "5");
    assert_eq!(done.puzzles, [2, 0, 0, 0, 0]);
}

#[test]
fn an_existing_target_is_never_overwritten() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let p = params(&r, d.path(), 2);
    let out = out(d.path());
    drop(Writer::create(&out.join("Train - Mate in 2.2cbh")).unwrap());
    let before = files(&out);
    let e = r.build_mate_training(p, |_, _, _| {}).unwrap_err();
    assert_eq!(e.code, "exists");
    assert_eq!(files(&out), before);
}

#[test]
fn stop_keeps_what_was_found() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let p = params(&r, d.path(), 2);
    let out = out(d.path());
    let id = p.database_id.clone();
    let done = r
        .build_mate_training(p, |scanned, _, _| {
            if scanned == 1 {
                r.stop_mate_training(id.clone()).unwrap();
            }
        })
        .unwrap();
    assert!(done.stopped && !done.cancelled);
    assert_eq!(done.scanned, "1");
    // The first game gives a mate in 1, a mate in 2 and a chain.
    assert_eq!(done.puzzles, [1, 1, 0, 0, 0]);
    assert!(files(&out).contains(&"Train - Mate in 1.2cbh".to_string()));
    assert_eq!(done.databases.len(), 3);
}

#[test]
fn cancel_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let p = params(&r, d.path(), 2);
    let out = out(d.path());
    let id = p.database_id.clone();
    let done = r
        .build_mate_training(p, |scanned, _, _| {
            if scanned == 1 {
                r.cancel_mate_training(id.clone()).unwrap();
            }
        })
        .unwrap();
    assert!(done.cancelled && !done.stopped);
    assert!(done.databases.is_empty());
    assert!(files(&out).is_empty());
}

#[test]
fn depths_beyond_two_need_an_engine() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let p = params(&r, d.path(), 3);
    let out = out(d.path());
    let e = r.build_mate_training(p, |_, _, _| {}).unwrap_err();
    assert_eq!(e.code, "engine");
    assert!(files(&out).is_empty());
}

#[test]
fn a_name_that_is_a_path_or_empty_is_refused() {
    for name in ["", "../x", "a/b", "a\\b", "  ", ".."] {
        let d = tempfile::tempdir().unwrap();
        let r = Registry::default();
        let mut p = params(&r, d.path(), 1);
        out(d.path());
        p.name = name.into();
        let e = r.build_mate_training(p, |_, _, _| {}).unwrap_err();
        assert_eq!(e.code, "invalidPath", "{name:?}");
    }
}

#[test]
fn a_missing_folder_is_refused_and_nothing_is_created() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let p = params(&r, d.path(), 1);
    let e = r.build_mate_training(p, |_, _, _| {}).unwrap_err();
    assert_eq!(e.code, "invalidPath");
    assert!(!d.path().join("out").exists());
}
