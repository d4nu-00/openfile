use cbh_reader::{
    Database, ReadLimits,
    crosstable::Layout,
    name_list::{NameKind, NameList},
    v2::write::Writer,
};
use chess_core::{GameLimits, parse_pgn};
use std::sync::atomic::AtomicBool;

/// A four-player round robin (A beats everyone, B beats C, C and D draw,
/// B and D draw; A and C have a game without a result) and a one-game
/// other event.
fn games() -> String {
    let mut text = String::new();
    let mut add = |event: &str, round: u32, white: &str, black: &str, result: &str, elo: u32| {
        text += &format!(
            "[Event \"{event}\"]\n[Site \"Oslo\"]\n[Date \"2020.05.0{round}\"]\n[Round \"{round}\"]\n\
             [White \"{white}\"]\n[Black \"{black}\"]\n[Result \"{result}\"]\n\
             [WhiteElo \"{elo}\"]\n[BlackElo \"2400\"]\n\n1. e4 {result}\n\n"
        );
    };
    add("Club", 1, "Alpha, A", "Delta, D", "1-0", 2600);
    add("Club", 1, "Bravo, B", "Charlie, C", "1-0", 2500);
    add("Club", 2, "Charlie, C", "Alpha, A", "0-1", 2400);
    add("Club", 2, "Delta, D", "Bravo, B", "1/2-1/2", 2400);
    add("Club", 3, "Alpha, A", "Bravo, B", "1-0", 2600);
    add("Club", 3, "Charlie, C", "Delta, D", "1/2-1/2", 2400);
    add("Club", 4, "Alpha, A", "Charlie, C", "*", 2600);
    add("Blitz", 1, "Alpha, A", "Echo, E", "0-1", 2600);
    text
}

fn db() -> (tempfile::TempDir, Database) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Names.2cbh");
    let games = parse_pgn(games().as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    (d, Database::open(&path, ReadLimits::default()).unwrap())
}

fn list(db: &Database, dir: &std::path::Path, kind: NameKind) -> NameList {
    let path = dir.join(format!("{}.cwnames", kind.name()));
    assert!(NameList::build(db, kind, &path, 3, &AtomicBool::new(false), |_, _| {}).unwrap());
    NameList::open(&path, db, kind).unwrap().unwrap()
}

#[test]
fn players_and_tournaments_are_listed_alphabetically_with_counts() {
    let (d, mut db) = db();
    let mut players = list(&db, d.path(), NameKind::Players);
    let rows = players.read(0, 10).unwrap();
    let names: Vec<_> = rows
        .iter()
        .map(|r| db.player_name(r.id).unwrap().unwrap())
        .collect();
    assert_eq!(
        names,
        ["Alpha, A", "Bravo, B", "Charlie, C", "Delta, D", "Echo, E"]
    );
    let games: Vec<_> = rows.iter().map(|r| r.games).collect();
    assert_eq!(games, [5, 3, 4, 3, 1]);
    assert_eq!((rows[0].first_year, rows[0].last_year), (2020, 2020));
    assert_eq!(
        players.find(&mut db, NameKind::Players, "charl").unwrap(),
        2
    );
    assert_eq!(
        players
            .find(&mut db, NameKind::Players, "Delta, D")
            .unwrap(),
        3
    );
    assert_eq!(players.find(&mut db, NameKind::Players, "zz").unwrap(), 5);

    let mut events = list(&db, d.path(), NameKind::Tournaments);
    let rows = events.read(0, 10).unwrap();
    let names: Vec<_> = rows
        .iter()
        .map(|r| db.tournament_name(r.id).unwrap().0.unwrap())
        .collect();
    assert_eq!(names, ["Blitz", "Club"]);
    assert_eq!(rows[1].games, 7);
    assert_eq!(
        events.find(&mut db, NameKind::Tournaments, "cl").unwrap(),
        1
    );

    // A cancelled build leaves nothing; a changed database is not current.
    let path = d.path().join("other.cwnames");
    assert!(
        !NameList::build(
            &db,
            NameKind::Players,
            &path,
            2,
            &AtomicBool::new(true),
            |_, _| {}
        )
        .unwrap()
    );
    assert!(!path.exists());
    assert!(
        NameList::open(
            &d.path().join("players.cwnames"),
            &db,
            NameKind::Tournaments
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn round_robin_and_swiss_crosstables_score_and_rank() {
    let (d, mut db) = db();
    let mut events = list(&db, d.path(), NameKind::Tournaments);
    let club = events.read(1, 1).unwrap()[0].id;
    let t = db
        .crosstable(club, None, &AtomicBool::new(false))
        .unwrap()
        .unwrap();
    assert_eq!(t.event.as_deref(), Some("Club"));
    assert_eq!(t.layout, Layout::RoundRobin);
    assert_eq!((t.games, t.unscored, t.rounds), (6, 1, 4));
    let names: Vec<_> = t.players.iter().map(|p| p.name.clone().unwrap()).collect();
    assert_eq!(names, ["Alpha, A", "Bravo, B", "Delta, D", "Charlie, C"]);
    let points: Vec<_> = t.players.iter().map(|p| p.points).collect();
    assert_eq!(points, [3.0, 1.5, 1.0, 0.5]);
    let b = &t.players[1];
    assert_eq!(b.sonneborn_berger, 1.0);
    assert_eq!(b.buchholz, 3.0 + 1.0 + 0.5);
    // Cells by opponent row; a player has no game against himself.
    let marks = |p: usize| -> Vec<String> {
        t.players[p]
            .cells
            .iter()
            .map(|c| c.iter().map(|g| g.mark.clone()).collect())
            .collect()
    };
    assert_eq!(marks(0), ["", "1", "1", "1"]);
    assert_eq!(marks(1), ["0", "", "½", "1"]);
    assert_eq!(marks(3), ["0", "0", "½", ""]);
    // A won all three against an average of 2400: 2400 + 800.
    assert_eq!(t.players[0].elo, Some(2600));
    assert_eq!(t.players[0].performance, Some(3200));
    // First ratings 2600, 2500, 2400 and 2400.
    assert_eq!(t.average_elo, Some(2475));
    assert_eq!(t.category, Some(9));

    let swiss = db
        .crosstable(club, Some(Layout::Swiss), &AtomicBool::new(false))
        .unwrap()
        .unwrap();
    assert_eq!(swiss.layout, Layout::Swiss);
    let a = &swiss.players[0];
    assert_eq!(a.cells.len(), 4);
    let first = &a.cells[0][0];
    assert_eq!(
        (first.round, first.white, first.mark.as_str()),
        (1, true, "1")
    );
    // Delta is ranked third.
    assert_eq!(first.opponent, 2);
    assert!(a.cells[3].is_empty(), "the unscored game is not shown");
}
