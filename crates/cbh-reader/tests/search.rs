use cbh_reader::{
    Database, ReadLimits,
    search::{EloMode, GameFilter, PlayerQuery},
    v2::write::Writer,
};
use chess_core::{GameLimits, parse_pgn};
use std::sync::atomic::AtomicBool;

const GAMES: &str = r#"[Event "Club Open"]
[Site "Göteborg"]
[Date "2024.05.17"]
[White "Müller, Jörg"]
[Black "Zeta, Ana"]
[Result "1-0"]
[WhiteElo "2310"]
[BlackElo "2100"]
[ECO "C42"]

1. e4 e5 2. Nf3 Nf6 1-0

[Event "Rapid"]
[Site "Oslo"]
[Date "2023.01.02"]
[White "Zeta, Ana"]
[Black "Müller, Jörg"]
[Result "1-0"]
[WhiteElo "2150"]
[BlackElo "2320"]
[ECO "B90"]
[Annotator "Lasker, Emanuel"]

1. e4 c5 1-0

[Event "Rapid"]
[Site "Oslo"]
[Date "1999.??.??"]
[White "Müller, Anna"]
[Black "Other, Player"]
[Result "1/2-1/2"]
[ECO "A00"]

1. a3 a6 2. b3 b6 3. c3 c6 1/2-1/2"#;

fn db() -> (tempfile::TempDir, Database) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Search.2cbh");
    let games = parse_pgn(GAMES.as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    w.set_deleted(3, true).unwrap();
    drop(w);
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    (d, db)
}

fn hits(db: &mut Database, f: GameFilter) -> Vec<u64> {
    db.search(&f, 100, &AtomicBool::new(false), |_| {})
        .unwrap()
        .ordinals
}

fn player(last: &str, first: &str) -> PlayerQuery {
    PlayerQuery {
        last: last.into(),
        first: first.into(),
    }
}

#[test]
fn players_colours_and_wins() {
    let (_d, mut db) = db();
    let f = |white, black| GameFilter {
        white,
        black,
        ..Default::default()
    };
    assert_eq!(
        hits(&mut db, f(player("müller", ""), PlayerQuery::default())),
        [1, 3]
    );
    assert_eq!(
        hits(&mut db, f(player("Müller", "J"), PlayerQuery::default())),
        [1]
    );
    assert_eq!(
        hits(&mut db, f(player("Müller", ""), player("zeta", ""))),
        [1]
    );
    let mut either = f(player("Müller", "Jörg"), PlayerQuery::default());
    either.ignore_colors = true;
    assert_eq!(hits(&mut db, either.clone()), [1, 2]);
    either.wins_only = true;
    assert_eq!(hits(&mut db, either), [1]);
}

#[test]
fn names_ranges_results_and_deletion() {
    let (_d, mut db) = db();
    let all = GameFilter::default();
    assert_eq!(hits(&mut db, all.clone()), [1, 2, 3]);
    let with = |g: GameFilter| g;
    assert_eq!(
        hits(
            &mut db,
            with(GameFilter {
                tournament: "oslo".into(),
                ..all.clone()
            })
        ),
        [2, 3]
    );
    assert_eq!(
        hits(
            &mut db,
            with(GameFilter {
                annotator: "lasker".into(),
                ..all.clone()
            })
        ),
        [2]
    );
    assert_eq!(
        hits(
            &mut db,
            with(GameFilter {
                year_min: Some(2000),
                year_max: Some(2023),
                ..all.clone()
            })
        ),
        [2]
    );
    assert_eq!(
        hits(
            &mut db,
            with(GameFilter {
                eco_from: "B00".into(),
                eco_to: "C99".into(),
                ..all.clone()
            })
        ),
        [1, 2]
    );
    assert_eq!(
        hits(
            &mut db,
            with(GameFilter {
                moves_min: Some(3),
                ..all.clone()
            })
        ),
        [3]
    );
    assert_eq!(
        hits(
            &mut db,
            with(GameFilter {
                results: vec!["1/2-1/2".into()],
                ..all.clone()
            })
        ),
        [3]
    );
    assert_eq!(
        hits(
            &mut db,
            with(GameFilter {
                exclude_deleted: true,
                ..all.clone()
            })
        ),
        [1, 2]
    );
    let elo = |mode, min| GameFilter {
        elo_mode: mode,
        elo_min: Some(min),
        ..all.clone()
    };
    assert_eq!(hits(&mut db, elo(EloMode::One, 2300)), [1, 2]);
    assert_eq!(hits(&mut db, elo(EloMode::Both, 2300)), Vec::<u64>::new());
    assert_eq!(hits(&mut db, elo(EloMode::Average, 2200)), [1, 2]);
    // The hit limit stops the scan and says so.
    let r = db.search(&all, 2, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(r.truncated);
    assert_eq!(r.ordinals, [1, 2]);
    let r = db.search(&all, 10, &AtomicBool::new(true), |_| {}).unwrap();
    assert!(r.cancelled && r.ordinals.is_empty());
}
