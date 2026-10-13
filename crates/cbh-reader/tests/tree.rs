use cbh_reader::{
    Database, ReadLimits,
    tree::{BuildOptions, OpeningTree, Source, pack_move, position_hash, unpack_uci},
    v2::write::Writer,
};
use chess_core::{GameLimits, parse_pgn};
use shakmaty::{Chess, Position, uci::UciMove};
use std::{path::Path, sync::atomic::AtomicBool};

fn game(white: u16, black: u16, year: u16, result: &str, moves: &str) -> String {
    format!(
        "[WhiteElo \"{white}\"]\n[BlackElo \"{black}\"]\n[Date \"{year}.01.01\"]\n[Result \"{result}\"]\n\n{moves} {result}\n\n"
    )
}

/// Seven games: five Ruy Lopez (one by transposition, one with a long tail),
/// a Queen's Gambit, and a game whose 1.e4 is only in a variation.
fn games() -> String {
    [
        game(
            2700,
            2600,
            2001,
            "1-0",
            "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6",
        ),
        game(
            2600,
            2700,
            2002,
            "1/2-1/2",
            "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Bxc6 dxc6",
        ),
        game(2500, 2500, 2003, "0-1", "1. Nf3 Nc6 2. e4 e5 3. Bb5 Nf6"),
        game(
            2400,
            0,
            2004,
            "1-0",
            "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5",
        ),
        game(0, 2300, 2005, "*", "1. e4 e5 2. Nf3 Nc6 3. Bb5 Nf6"),
        game(2650, 2650, 2006, "1/2-1/2", "1. d4 d5 2. c4 dxc4"),
        game(2000, 2000, 2007, "1-0", "1. d4 (1. e4 e5 2. Nf3) 1... Nf6"),
    ]
    .concat()
}

fn family(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("T.2cbh");
    let games = parse_pgn(games().as_bytes(), &GameLimits::default()).unwrap();
    Writer::create(&path).unwrap().append_all(&games).unwrap();
    path
}

/// Hashes and packed moves of a line of UCI moves from the start.
fn line(moves: &[&str]) -> (Vec<u64>, Vec<u16>) {
    let mut pos = Chess::default();
    let mut hashes = vec![position_hash(pos.board(), pos.turn())];
    let mut packed = vec![];
    for uci in moves {
        let m = UciMove::from_ascii(uci.as_bytes())
            .unwrap()
            .to_move(&pos)
            .unwrap();
        packed.push(pack_move(&m).unwrap());
        pos.play_unchecked(m);
        hashes.push(position_hash(pos.board(), pos.turn()));
    }
    (hashes, packed)
}

fn options(min_games: usize) -> BuildOptions {
    BuildOptions {
        min_games,
        workers: 2,
        ..Default::default()
    }
}

fn ask(tree: &mut OpeningTree, db: &Database, moves: &[&str]) -> cbh_reader::tree::Answer {
    let (hashes, packed) = line(moves);
    tree.query(db, &hashes, &packed, 100, 100, &AtomicBool::new(false))
        .unwrap()
}

#[test]
fn counts_main_lines_and_transpositions() {
    let d = tempfile::tempdir().unwrap();
    let db = Database::open(&family(d.path()), ReadLimits::default()).unwrap();
    let file = d.path().join("trees/T.cwtree");
    let cancel = AtomicBool::new(false);
    assert!(OpeningTree::open(&file, &db).unwrap().is_none());
    assert!(OpeningTree::build(&db, &file, &options(2), &cancel, |_, _, _| {}).unwrap());
    let mut tree = OpeningTree::open(&file, &db).unwrap().unwrap();
    assert_eq!(tree.games(), 7);

    let start = ask(&mut tree, &db, &[]);
    assert_eq!(start.source, Source::Tree);
    let rows: Vec<(String, u64)> = start
        .moves
        .iter()
        .map(|m| (m.uci().unwrap(), m.games))
        .collect();
    assert_eq!(
        rows,
        [("e2e4".into(), 4), ("d2d4".into(), 2), ("g1f3".into(), 1)]
    );
    assert_eq!(start.games.len(), 7);

    // After 1.e4 e5 2.Nf3 Nc6 3.Bb5: four games plus the transposition.
    let ruy = ask(&mut tree, &db, &["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"]);
    let ordinals: Vec<u64> = ruy.games.iter().map(|g| g.0).collect();
    assert_eq!(ordinals, [1, 2, 3, 4, 5]);
    let a6 = &ruy.moves[0];
    assert_eq!((a6.uci().as_deref(), a6.games), (Some("a7a6"), 3));
    // Black to move: a loss, a draw and a loss for Black.
    assert_eq!((a6.wins, a6.draws, a6.losses), (0, 1, 2));
    assert_eq!(a6.score(), Some(100.0 / 6.0));
    // Black's ratings: 2600, 2700 and an unrated player.
    assert_eq!(a6.average_elo(), Some(2650));
    assert_eq!((a6.year_min, a6.year_max), (2001, 2004));
    let nf6 = &ruy.moves[1];
    assert_eq!((nf6.uci().as_deref(), nf6.games), (Some("g8f6"), 2));
    // Game 5 has no result: counted, but not scored.
    assert_eq!((nf6.wins, nf6.draws, nf6.losses), (1, 0, 0));

    // 1.e4 in game 7 is a variation: not counted.
    let e4 = ask(&mut tree, &db, &["e2e4"]);
    assert_eq!(
        e4.games.iter().map(|g| g.0).collect::<Vec<_>>(),
        [1, 2, 4, 5]
    );
}

#[test]
fn rare_positions_follow_the_line() {
    let d = tempfile::tempdir().unwrap();
    let db = Database::open(&family(d.path()), ReadLimits::default()).unwrap();
    let file = d.path().join("T.cwtree");
    let cancel = AtomicBool::new(false);
    OpeningTree::build(&db, &file, &options(5), &cancel, |_, _, _| {}).unwrap();
    let mut tree = OpeningTree::open(&file, &db).unwrap().unwrap();
    // Only positions of at least five games are stored, so 3...a6 4.Ba4 is
    // answered by replaying the games that left 3.Bb5 with 3...a6.
    let deep = ask(
        &mut tree,
        &db,
        &[
            "e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6", "b5a4", "g8f6",
        ],
    );
    assert_eq!(deep.source, Source::Line);
    assert_eq!(deep.games, [(1, 0), (4, deep.games[1].1)]);
    assert_eq!(unpack_uci(deep.games[1].1).as_deref(), Some("e1g1"));
    // Off every game: nothing.
    let none = ask(&mut tree, &db, &["h2h4"]);
    assert!(none.games.is_empty() && none.moves.is_empty());
}

#[test]
fn stale_and_journalled_changes() {
    let d = tempfile::tempdir().unwrap();
    let path = family(d.path());
    let file = d.path().join("T.cwtree");
    let cancel = AtomicBool::new(false);
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    OpeningTree::build(&db, &file, &options(2), &cancel, |_, _, _| {}).unwrap();

    // A change recorded through the journal keeps the tree current.
    let pending = OpeningTree::before_change(&file, &db, None)
        .unwrap()
        .unwrap();
    let extra = parse_pgn(
        game(2800, 2100, 2010, "1-0", "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6").as_bytes(),
        &GameLimits::default(),
    )
    .unwrap();
    let (ordinal, _) = Writer::open(&path).unwrap().append(&extra[0]).unwrap();
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    assert!(OpeningTree::open(&file, &db).unwrap().is_none());
    assert!(pending.commit(&db, Some(ordinal)).unwrap());
    let mut tree = OpeningTree::open(&file, &db).unwrap().unwrap();
    let ruy = ask(&mut tree, &db, &["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"]);
    assert_eq!(ruy.games.len(), 6);
    assert_eq!(ruy.moves[0].games, 4);

    // Replacing game 1 with a Queen's Gambit moves it out of the Ruy Lopez.
    let pending = OpeningTree::before_change(&file, &db, Some(1))
        .unwrap()
        .unwrap();
    let qgd = parse_pgn(
        game(2700, 2600, 2001, "1-0", "1. d4 d5 2. c4 e6").as_bytes(),
        &GameLimits::default(),
    )
    .unwrap();
    Writer::open(&path).unwrap().replace(1, &qgd[0]).unwrap();
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    assert!(pending.commit(&db, Some(1)).unwrap());
    let mut tree = OpeningTree::open(&file, &db).unwrap().unwrap();
    let ruy = ask(&mut tree, &db, &["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"]);
    assert_eq!(
        ruy.games.iter().map(|g| g.0).collect::<Vec<_>>(),
        [2, 3, 4, 5, 8]
    );
    assert_eq!(ruy.moves[0].games, 3);
    let qg = ask(&mut tree, &db, &["d2d4", "d7d5", "c2c4"]);
    assert_eq!(qg.games.iter().map(|g| g.0).collect::<Vec<_>>(), [1, 6]);

    // A change made without the journal makes the tree stale.
    Writer::open(&path).unwrap().set_deleted(2, true).unwrap();
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    assert!(OpeningTree::open(&file, &db).unwrap().is_none());
}

#[test]
fn cancelled_build_leaves_nothing() {
    let d = tempfile::tempdir().unwrap();
    let db = Database::open(&family(d.path()), ReadLimits::default()).unwrap();
    let dir = d.path().join("trees");
    let cancel = AtomicBool::new(true);
    assert!(
        !OpeningTree::build(
            &db,
            &dir.join("T.cwtree"),
            &options(2),
            &cancel,
            |_, _, _| {}
        )
        .unwrap()
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}
