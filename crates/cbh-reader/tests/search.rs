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
    // Rating difference at most N: 210 and 170 points; unrated games never.
    let difference = |max| GameFilter {
        elo_difference: Some(max),
        ..all.clone()
    };
    assert_eq!(hits(&mut db, difference(200)), [2]);
    assert_eq!(hits(&mut db, difference(250)), [1, 2]);
    // The hit limit stops the scan and says so.
    let r = db.search(&all, 2, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(r.truncated);
    assert_eq!(r.ordinals, [1, 2]);
    let r = db.search(&all, 10, &AtomicBool::new(true), |_| {}).unwrap();
    assert!(r.cancelled && r.ordinals.is_empty());
}

#[test]
fn quick_filter_words_must_each_match_a_field() {
    let (_d, mut db) = db();
    let text = |t: &str| GameFilter {
        text: t.into(),
        ..Default::default()
    };
    // Player names, in any case.
    assert_eq!(hits(&mut db, text("MÜLLER")), [1, 2, 3]);
    // A name and a year.
    assert_eq!(hits(&mut db, text("müller 2024")), [1]);
    // Tournament or site, an ECO prefix and a result.
    assert_eq!(hits(&mut db, text("oslo b9")), [2]);
    assert_eq!(hits(&mut db, text("rapid 1/2")), [3]);
    assert_eq!(hits(&mut db, text("c 1-0")), [1]);
    // The annotator.
    assert_eq!(hits(&mut db, text("lasker")), [2]);
    assert!(hits(&mut db, text("müller nowhere")).is_empty());
}

#[test]
fn quick_filter_expressions_scope_names_and_database_fields() {
    let (_d, mut db) = db();
    let query = |text: &str| GameFilter {
        text: text.into(),
        ..Default::default()
    };
    assert_eq!(hits(&mut db, query("müller-")), [1, 3]);
    assert_eq!(hits(&mut db, query("-müller")), [2]);
    assert_eq!(hits(&mut db, query("müller- 2024")), [1]);
    assert_eq!(hits(&mut db, query("White:\"Müller, Jörg\"")), [1]);
    assert_eq!(hits(&mut db, query("Site:Oslo")), [2, 3]);
    assert_eq!(hits(&mut db, query("Event:\"Club Open\"")), [1]);
    assert_eq!(hits(&mut db, query("B80-B99")), [2]);
    assert_eq!(hits(&mut db, query("1999-2023")), [2, 3]);
    assert_eq!(hits(&mut db, query(">2200")), [1, 2]);
    assert!(hits(&mut db, query("<2200")).is_empty());
    assert!(hits(&mut db, query("!>2200")).is_empty());
    assert_eq!(hits(&mut db, query("!rapid")), [1]);
    assert_eq!(hits(&mut db, query("rapid !1/2")), [2]);
    assert_eq!(hits(&mut db, query("rapid 1-0 1/2")), [2, 3]);
    assert_eq!(hits(&mut db, query(">2200 !rapid")), [1]);
}

#[test]
fn malformed_quick_filter_expressions_fail_instead_of_dropping_terms() {
    let (_d, mut db) = db();
    for text in [
        "B99-B10",
        "B9-B10",
        "2024-1999",
        "Event:\"Club Open",
        ">abc",
        "\"\"",
    ] {
        let filter = GameFilter {
            text: text.into(),
            ..Default::default()
        };
        assert!(
            db.search(&filter, 100, &AtomicBool::new(false), |_| {})
                .is_err(),
            "{text}"
        );
    }
    let filter = GameFilter {
        text: (1..=17)
            .map(|n| format!("word{n}"))
            .collect::<Vec<_>>()
            .join(" "),
        ..Default::default()
    };
    assert!(
        db.search(&filter, 100, &AtomicBool::new(false), |_| {})
            .is_err()
    );
}

#[test]
fn result_terms_do_not_match_unrelated_header_text() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Result.2cbh");
    let game = parse_pgn(
        br#"[Event "1-0 Cup"]
[White "A"]
[Black "B"]
[Result "1/2-1/2"]

1. e4 e5 1/2-1/2"#,
        &GameLimits::default(),
    )
    .unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&game).unwrap();
    drop(w);
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    assert!(
        hits(
            &mut db,
            GameFilter {
                text: "1-0".into(),
                ..Default::default()
            }
        )
        .is_empty()
    );
}

/// Games ending in mate, stalemate, check and quietly, some with comments.
const ENDINGS: &str = r#"[White "Mate, A"]
[Black "B"]
[Result "1-0"]

1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# {Scholar's mate} 1-0

[White "Stale, A"]
[Black "B"]
[SetUp "1"]
[FEN "7k/5Q2/6K1/8/8/8/8/8 w - - 0 1"]
[Result "1/2-1/2"]

1. Qf6 {A blunder: stalemate? No.} Kg8 2. Qe7 Kh8 3. Qf7 1/2-1/2

[White "Check, A"]
[Black "B"]
[Result "*"]

1. e4 f6 2. Qh5+ *

[White "Quiet, A"]
[Black "B"]
[Result "*"]

1. d4 {A quiet SCHOLARLY game} (1. e4 {mate in a variation}) d5 *"#;

fn endings() -> (tempfile::TempDir, Database) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Endings.2cbh");
    let games = parse_pgn(ENDINGS.as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    (d, Database::open(&path, ReadLimits::default()).unwrap())
}

#[test]
fn final_positions_and_full_text() {
    let (_d, mut db) = endings();
    let f = |patch: fn(&mut GameFilter)| {
        let mut f = GameFilter::default();
        patch(&mut f);
        f
    };
    assert_eq!(hits(&mut db, f(|f| f.mate = true)), [1]);
    assert_eq!(hits(&mut db, f(|f| f.stalemate = true)), [2]);
    // Check that is not mate.
    assert_eq!(hits(&mut db, f(|f| f.check = true)), [3]);
    assert_eq!(
        hits(
            &mut db,
            f(|f| {
                f.mate = true;
                f.check = true;
            })
        ),
        [1, 3]
    );
    // Combined with header fields.
    assert!(
        hits(
            &mut db,
            f(|f| {
                f.mate = true;
                f.white.last = "Quiet".into();
            })
        )
        .is_empty()
    );
    // Text in any comment, variations included, case-insensitively.
    assert_eq!(hits(&mut db, f(|f| f.full_text = "scholar".into())), [1, 4]);
    assert_eq!(hits(&mut db, f(|f| f.full_text = "MATE".into())), [1, 2, 4]);
    assert_eq!(
        hits(
            &mut db,
            f(|f| {
                f.full_text = "scholar".into();
                f.full_text_not = true;
            })
        ),
        [2, 3]
    );
}

#[test]
fn hit_limits_keep_the_first_hits_in_entry_order() {
    let (_d, mut db) = endings();
    for limit in 1..=4 {
        let r = db
            .search(
                &GameFilter::default(),
                limit,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(r.ordinals, (1..=limit as u64).collect::<Vec<_>>());
        assert_eq!(r.truncated, limit < 4, "limit {limit}");
    }
}

const ANNOTATED: &str = r#"[White "A, A"]
[Black "B, B"]
[Result "1-0"]

1. e4 {The Kingpin} e5 2. Nf3! $18 (2. Qh5 {Wild}) Nc6 1-0

[White "C, C"]
[Black "D, D"]
[Result "0-1"]

1. d4 {[%csl Re4][%cal Ga2a4] a kingside plan} d5 2. c4!! 0-1

[White "E, E"]
[Black "F, F"]
[Result "1/2-1/2"]

1. c4 c5 1/2-1/2"#;

#[test]
fn annotation_and_medal_criteria() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Annotated.2cbh");
    let games = parse_pgn(ANNOTATED.as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    w.toggle_user_medal(&[3]).unwrap();
    drop(w);
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let f = |patch: &dyn Fn(&mut GameFilter)| {
        let mut f = GameFilter::default();
        patch(&mut f);
        f
    };
    let a = |patch: &dyn Fn(&mut cbh_reader::search::AnnotationQuery)| {
        f(&|f| patch(&mut f.annotations))
    };
    // Texts: both must be there; whole words only when asked.
    assert_eq!(hits(&mut db, a(&|q| q.text1 = "king".into())), [1, 2]);
    assert_eq!(
        hits(
            &mut db,
            a(&|q| {
                q.text1 = "king".into();
                q.whole_word = true;
            })
        ),
        Vec::<u64>::new()
    );
    assert_eq!(
        hits(
            &mut db,
            a(&|q| {
                q.text1 = "kingpin".into();
                q.text2 = "wild".into();
                q.whole_word = true;
            })
        ),
        [1]
    );
    // Symbols: every one given must be in the game.
    assert_eq!(hits(&mut db, a(&|q| q.symbols = vec![1, 18])), [1]);
    assert_eq!(hits(&mut db, a(&|q| q.symbols = vec![3])), [2]);
    assert_eq!(hits(&mut db, a(&|q| q.any_symbols = true)), [1, 2]);
    assert_eq!(hits(&mut db, a(&|q| q.any_text = true)), [1, 2]);
    assert_eq!(hits(&mut db, a(&|q| q.colours = true)), [2]);
    assert_eq!(hits(&mut db, a(&|q| q.variations = true)), [1]);
    // Medals: any of the bits given.
    assert_eq!(hits(&mut db, f(&|f| f.medals = 0x8000)), [3]);
    assert_eq!(hits(&mut db, f(&|f| f.medals = 0x0001)), Vec::<u64>::new());
}

#[test]
fn annotation_kinds_without_a_writer() {
    use cbh_reader::{
        annotations::{Entry, Kind},
        search::AnnotationQuery,
    };
    let entries = [
        Entry {
            position: 3,
            kind: Kind::Critical(2),
        },
        Entry {
            position: 4,
            kind: Kind::Other(0x09),
        },
        Entry {
            position: 5,
            kind: Kind::Other(0x11),
        },
    ];
    let q = |patch: &dyn Fn(&mut AnnotationQuery)| {
        let mut q = AnnotationQuery::default();
        patch(&mut q);
        q.matches(&entries)
    };
    assert!(q(&|q| q.critical_middlegame = true));
    assert!(!q(&|q| q.critical_opening = true));
    assert!(q(&|q| q.training = true));
    assert!(q(&|q| q.multimedia = true));
    assert!(!q(&|q| q.pawn_structure = true));
    assert!(!q(&|q| q.any_text = true));
}
