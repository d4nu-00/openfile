use cbh_reader::{
    Database, ReadLimits,
    position::{PositionIndex, Target},
    v2::write::Writer,
};
use chess_core::{GameLimits, parse_pgn};
use std::sync::atomic::AtomicBool;

const GAMES: &str = "\
[Result \"*\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 *\n\n\
[Result \"*\"]\n\n1. Nf3 Nc6 2. e4 e5 (2... d6) 3. Bb5 *\n\n\
[Result \"*\"]\n\n1. d4 d5 2. c4 dxc4 3. e4 *\n\n\
[Result \"*\"]\n\n1. e4 e6 2. d4 d5 (2... b6 3. Nf3 Bb7) *\n";
const RUY: &str = "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3";

fn family() -> (tempfile::TempDir, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("P.2cbh");
    let games = parse_pgn(GAMES.as_bytes(), &GameLimits::default()).unwrap();
    Writer::create(&path).unwrap().append_all(&games).unwrap();
    (d, path)
}

#[test]
fn index_and_full_replay_agree() {
    let (d, path) = family();
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let cancel = AtomicBool::new(false);
    let index = d.path().join("idx/P.cwpi");
    assert!(PositionIndex::open(&index, &db).unwrap().is_none());
    assert!(PositionIndex::build(&db, &index, &cancel, |_, _| {}).unwrap());
    let mut opened = PositionIndex::open(&index, &db).unwrap().unwrap();
    for (fen, expected) in [
        // Reached by a transposition in game 2.
        (RUY, vec![1, 2]),
        // Only in a variation of game 4.
        (
            "rn1qkbnr/pbpp1ppp/1p2p3/8/3PP3/5N2/PPP2PPP/RNBQKB1R w KQkq - 2 4",
            vec![4],
        ),
        // Black to move after 1.e4: games 1 and 4 (not 2, wrong order).
        (
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
            vec![1, 4],
        ),
        // Same board, White to move: nowhere.
        (
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1",
            vec![],
        ),
    ] {
        let target = Target::from_fen(fen).unwrap();
        let with = db
            .find_position(&target, Some(&mut opened), 10, &cancel, |_, _| {})
            .unwrap();
        let without = db
            .find_position(&target, None, 10, &cancel, |_, _| {})
            .unwrap();
        let ordinals = |r: &cbh_reader::position::PositionResult| {
            r.hits.iter().map(|h| h.0).collect::<Vec<_>>()
        };
        assert_eq!(ordinals(&with), expected, "{fen}");
        assert_eq!(with.hits, without.hits);
        assert!(with.indexed && !without.indexed);
        assert!(with.replayed <= without.replayed);
    }
    // The node is where the position first occurs: after 3.Bb5 in game 1.
    let hit = db
        .find_position(
            &Target::from_fen(RUY).unwrap(),
            None,
            10,
            &cancel,
            |_, _| {},
        )
        .unwrap()
        .hits[0];
    let game = db.load(1).unwrap().game.unwrap();
    assert_eq!(game.position_at(hit.1).unwrap().fen, RUY);
}

#[test]
fn stale_indexes_are_not_used_and_cancelled_builds_leave_nothing() {
    let (d, path) = family();
    let index = d.path().join("P.cwpi");
    let cancel = AtomicBool::new(false);
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    PositionIndex::build(&db, &index, &cancel, |_, _| {}).unwrap();
    drop(db);
    let extra = parse_pgn(b"1. c4 *", &GameLimits::default()).unwrap();
    Writer::open(&path).unwrap().append_all(&extra).unwrap();
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    assert!(PositionIndex::open(&index, &db).unwrap().is_none());
    let other = d.path().join("Q.cwpi");
    assert!(!PositionIndex::build(&db, &other, &AtomicBool::new(true), |_, _| {}).unwrap());
    assert!(!other.exists());
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 5);
}
