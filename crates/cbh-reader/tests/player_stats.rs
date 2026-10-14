use cbh_reader::{
    Database, ReadLimits,
    player_stats::{Group, PlayerGames, eco_code},
    v2::write::Writer,
};
use chess_core::{GameLimits, parse_pgn};
use std::sync::atomic::AtomicBool;

/// Alpha's games: three as White, two as Black, one line without a result,
/// one deleted later; plus games between others.
fn games() -> String {
    let mut text = String::new();
    let mut add = |event: &str,
                   year: u32,
                   white: &str,
                   black: &str,
                   result: &str,
                   elo: (u32, u32),
                   eco: &str| {
        text += &format!(
            "[Event \"{event}\"]\n[Site \"Oslo\"]\n[Date \"{year}.05.01\"]\n\
             [White \"{white}\"]\n[Black \"{black}\"]\n[Result \"{result}\"]\n\
             [WhiteElo \"{}\"]\n[BlackElo \"{}\"]\n[ECO \"{eco}\"]\n\n1. e4 {result}\n\n",
            elo.0, elo.1
        );
    };
    add(
        "Club",
        2020,
        "Alpha, A",
        "Bravo, B",
        "1-0",
        (2600, 2400),
        "B90",
    );
    add(
        "Club",
        2020,
        "Charlie, C",
        "Alpha, A",
        "1/2-1/2",
        (2500, 2600),
        "B90",
    );
    add(
        "Club",
        2020,
        "Alpha, A",
        "Charlie, C",
        "0-1",
        (2600, 2500),
        "C42",
    );
    add(
        "Open",
        2021,
        "Bravo, B",
        "Alpha, A",
        "0-1",
        (2400, 2650),
        "",
    );
    add(
        "Open",
        2021,
        "Alpha, A",
        "Bravo, B",
        "1-0",
        (2650, 0),
        "B90",
    );
    add(
        "Open",
        2021,
        "Alpha, A",
        "Delta, D",
        "*",
        (2650, 2300),
        "A00",
    );
    add(
        "Open",
        2021,
        "Bravo, B",
        "Charlie, C",
        "1-0",
        (2400, 2500),
        "D00",
    );
    add(
        "Open",
        2021,
        "Alpha, A",
        "Echo, E",
        "1-0",
        (2650, 2200),
        "E99",
    );
    text
}

fn db() -> (tempfile::TempDir, Database) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Stats.2cbh");
    let games = parse_pgn(games().as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    // The last of Alpha's games is deleted.
    w.set_deleted(8, true).unwrap();
    drop(w);
    (d, Database::open(&path, ReadLimits::default()).unwrap())
}

fn alpha(db: &mut Database) -> u32 {
    (1..=8)
        .find_map(|o| {
            let e = db.page(o, 1).unwrap().entries.remove(0);
            (e.white.as_deref() == Some("Alpha, A"))
                .then_some(e.white_id)
                .flatten()
        })
        .unwrap()
}

fn scan(db: &mut Database, id: u32) -> PlayerGames {
    db.player_games(id, 3, &AtomicBool::new(false), |_, _| {})
        .unwrap()
        .unwrap()
}

#[test]
fn scores_by_colour_opening_opponent_tournament_and_year() {
    let (_d, mut db) = db();
    let id = alpha(&mut db);
    let games = scan(&mut db, id);
    assert_eq!(games.games.len(), 6);
    assert_eq!(games.deleted, 1);
    let s = db.player_statistics(id, &games).unwrap();
    assert_eq!(s.name.as_deref(), Some("Alpha, A"));
    assert_eq!(s.unscored, 1);
    // As White: 1-0, 0-1, 1-0 (the line is not scored).
    assert_eq!((s.white.games, s.white.wins, s.white.losses), (3, 2, 1));
    assert_eq!(s.white.percent, Some(67));
    // As Black: ½, 1.
    assert_eq!((s.black.games, s.black.points), (2, 1.5));
    assert_eq!(s.total.games, 5);
    assert_eq!(s.total.points, 3.5);
    assert_eq!(s.total.percent, Some(70));
    // Rated opponents: Bravo 2400, Charlie 2500, Charlie 2500, Bravo 2400
    // (Bravo unrated in one game) → 4 games, 2.5 points, average 2450.
    assert_eq!(s.total.average_elo, Some(2450));
    // 63 % → +95 by FIDE's table.
    assert_eq!(s.total.performance, Some(2450 + 95));
    assert_eq!((s.first_year, s.last_year), (2020, 2021));

    let ecos: Vec<_> = s.openings.iter().map(|r| r.eco.clone()).collect();
    assert_eq!(
        ecos,
        [
            Some("A00".into()),
            Some("B90".into()),
            Some("C42".into()),
            None
        ]
    );
    let b90 = &s.openings[1];
    assert_eq!((b90.white.games, b90.white.percent), (2, Some(100)));
    assert_eq!((b90.black.games, b90.black.percent), (1, Some(50)));

    let names: Vec<_> = s
        .opponents
        .iter()
        .map(|r| r.name.clone().unwrap())
        .collect();
    assert_eq!(names, ["Bravo, B", "Charlie, C", "Delta, D"]);
    assert_eq!(s.opponents[0].score.games, 3);
    assert_eq!(s.opponents[0].score.percent, Some(100));
    // The latest rated game against Bravo.
    assert_eq!(s.opponents[0].elo, Some(2400));
    assert_eq!(s.opponents[2].score.games, 0);

    let events: Vec<_> = s
        .tournaments
        .iter()
        .map(|r| (r.name.clone().unwrap(), r.year))
        .collect();
    assert_eq!(events, [("Open".into(), 2021), ("Club".into(), 2020)]);
    let years: Vec<_> = s
        .years
        .iter()
        .map(|r| (r.year, r.score.games, r.elo))
        .collect();
    assert_eq!(years, [(2020, 3, Some(2600)), (2021, 2, Some(2650))]);

    let open = |g: Group| games.games.iter().filter(|x| g.contains(x)).count();
    assert_eq!(open(Group::All), 6);
    assert_eq!(open(Group::Color { white: false }), 2);
    assert_eq!(
        open(Group::Opening {
            eco: 191,
            white: Some(true)
        }),
        2
    );
    assert_eq!(open(Group::Year { year: 2021 }), 3);
    assert_eq!(eco_code(290).as_deref(), Some("C89"));
    assert_eq!(eco_code(1).as_deref(), Some("A00"));
    assert_eq!(eco_code(500).as_deref(), Some("E99"));
}

#[test]
fn more_workers_find_the_same_games_in_entry_order() {
    let (_d, mut db) = db();
    let id = alpha(&mut db);
    let one = db
        .player_games(id, 1, &AtomicBool::new(false), |_, _| {})
        .unwrap()
        .unwrap();
    for workers in [2, 5, 16] {
        let many = db
            .player_games(id, workers, &AtomicBool::new(false), |_, _| {})
            .unwrap()
            .unwrap();
        assert_eq!(many.games, one.games);
        assert_eq!(many.deleted, one.deleted);
    }
    assert!(one.games.windows(2).all(|w| w[0].ordinal < w[1].ordinal));
}

#[test]
fn a_cancelled_scan_returns_nothing() {
    let (_d, mut db) = db();
    let id = alpha(&mut db);
    assert!(
        db.player_games(id, 2, &AtomicBool::new(true), |_, _| {})
            .unwrap()
            .is_none()
    );
}
