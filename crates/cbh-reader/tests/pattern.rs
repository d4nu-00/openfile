use cbh_reader::{
    Database, ReadLimits,
    pattern::PositionQuery,
    position::{PositionIndex, PositionResult},
    v2::write::Writer,
};
use chess_core::{GameLimits, parse_pgn};
use std::sync::atomic::AtomicBool;

/// 1: the Italian; 2: a knight reaches f5 on move 4 and stays; 3: the
/// Italian with colours reversed and a knight on c4; 4: the knight reaches
/// f5 only in a variation.
const GAMES: &str = r#"[White "A"]
[Black "B"]
[Result "*"]

1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 *

[White "C"]
[Black "D"]
[Result "*"]

1. e4 e5 2. Nf3 d6 3. Nh4 a6 4. Nf5 a5 5. d3 a4 6. c3 h6 *

[White "E"]
[Black "F"]
[Result "*"]

1. e3 e5 2. Nc3 Nf6 3. Bc4 Nd5 4. Nf3 Nf4 5. Bb5 *

[White "G"]
[Black "H"]
[Result "*"]

1. e4 e5 2. Nf3 d6 3. Nh4 (3. d4 a6) 3... a6 4. Nf3 (4. Nf5 a5) 4... a5 *"#;

fn db() -> (tempfile::TempDir, Database) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Pattern.2cbh");
    let games = parse_pgn(GAMES.as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    (d, Database::open(&path, ReadLimits::default()).unwrap())
}

fn run(db: &mut Database, q: &PositionQuery, index: Option<&mut PositionIndex>) -> PositionResult {
    let compiled = q.compile().unwrap();
    db.find_query(&compiled, index, 100, &AtomicBool::new(false), |_, _| {})
        .unwrap()
}

fn games(db: &mut Database, q: PositionQuery) -> Vec<(u64, u32)> {
    run(db, &q, None).hits
}

fn pieces(list: &[(&str, char)]) -> Vec<(String, char)> {
    list.iter().map(|(s, c)| (s.to_string(), *c)).collect()
}

#[test]
fn look_for_or_and_exclude_boards() {
    let (_d, mut db) = db();
    // A white knight on f5: game 2 after 4.Nf5 (ply 7). Not in game 4's
    // variation unless lines are searched.
    let knight = PositionQuery {
        look_for: pieces(&[("f5", 'N')]),
        ..Default::default()
    };
    assert_eq!(games(&mut db, knight.clone()), [(2, 7)]);
    let lines = games(
        &mut db,
        PositionQuery {
            lines: true,
            ..knight.clone()
        },
    );
    assert_eq!(lines.iter().map(|h| h.0).collect::<Vec<_>>(), [2, 4]);

    // A white bishop on c4 or b5, and nothing at all on d5.
    let bishops = PositionQuery {
        or: pieces(&[("c4", 'B'), ("b5", 'B')]),
        exclude: pieces(&[("d5", 'X'), ("d5", 'x')]),
        ..Default::default()
    };
    // Game 1 at 3.Bc4; game 3 at 3.Bc4 (d5 still empty).
    assert_eq!(games(&mut db, bishops), [(1, 5), (3, 5)]);

    // Any white piece on f5 and a black pawn on e5.
    let any = PositionQuery {
        look_for: pieces(&[("f5", 'X'), ("e5", 'p')]),
        ..Default::default()
    };
    assert_eq!(games(&mut db, any), [(2, 7)]);
    // The boards hold one test per piece kind: an 'Or' board with a knight
    // and a bishop on c4 needs both there, so it finds nothing.
    let both = PositionQuery {
        or: pieces(&[("c4", 'N'), ("c4", 'B')]),
        ..Default::default()
    };
    assert!(games(&mut db, both).is_empty());
    // Several pieces on one square of the 'Exclude' board: neither a white
    // knight nor bishop on c4. Game 3 has its bishop there until 5.Bb5.
    let neither = PositionQuery {
        look_for: pieces(&[("f3", 'N'), ("e5", 'p')]),
        exclude: pieces(&[("c4", 'N'), ("c4", 'B')]),
        ..Default::default()
    };
    assert_eq!(games(&mut db, neither), [(1, 3), (2, 3), (3, 9), (4, 3)]);
}

#[test]
fn mirrors_moves_and_length() {
    let (_d, mut db) = db();
    let knight = PositionQuery {
        look_for: pieces(&[("f5", 'N')]),
        ..Default::default()
    };
    // Horizontal: ranks flipped and colours swapped, i.e. a black knight on
    // f4 (game 3 after 4...Nf4). Vertical: a white knight on c5 (none).
    assert_eq!(
        games(
            &mut db,
            PositionQuery {
                horizontal: true,
                ..knight.clone()
            }
        ),
        [(2, 7), (3, 8)]
    );
    assert_eq!(
        games(
            &mut db,
            PositionQuery {
                vertical: true,
                ..knight.clone()
            }
        ),
        [(2, 7)]
    );
    // The knight stays on f5 for 6 positions (plies 7–12); 7 are too many.
    for (length, expect) in [(6, vec![(2, 7)]), (7, vec![])] {
        let q = PositionQuery {
            length: Some(length),
            ..knight.clone()
        };
        assert_eq!(games(&mut db, q), expect, "length {length}");
    }
    // Moves 1–3 end before 4.Nf5; from move 5 on the run starts at ply 8.
    let window = |first, last| PositionQuery {
        first,
        last,
        ..knight.clone()
    };
    assert!(games(&mut db, window(None, Some(3))).is_empty());
    assert_eq!(games(&mut db, window(Some(5), None)), [(2, 8)]);
    // White to move only: the first is after 4...a5.
    let side = PositionQuery {
        white_to_move: Some(true),
        ..knight.clone()
    };
    assert_eq!(games(&mut db, side), [(2, 8)]);
}

#[test]
fn exact_positions_with_mirrors_use_the_booster() {
    let (d, mut db) = db();
    let index_path = d.path().join("p.cwpi");
    assert!(PositionIndex::build(&db, &index_path, &AtomicBool::new(false), |_, _| {}).unwrap());
    let mut index = PositionIndex::open(&index_path, &db).unwrap().unwrap();
    // After 1.e4 e5, White to move.
    let mut look_for = vec![];
    for (r, row) in "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR"
        .split('/')
        .enumerate()
    {
        let mut file = 0u8;
        for c in row.chars() {
            match c.to_digit(10) {
                Some(n) => file += n as u8,
                None => {
                    look_for.push((format!("{}{}", (b'a' + file) as char, 8 - r), c));
                    file += 1;
                }
            }
        }
    }
    let q = PositionQuery {
        look_for,
        exact: true,
        white_to_move: Some(true),
        ..Default::default()
    };
    let plain = run(&mut db, &q, None);
    let boosted = run(&mut db, &q, Some(&mut index));
    assert!(boosted.indexed);
    assert_eq!(plain.hits, boosted.hits);
    assert_eq!(
        plain.hits.iter().map(|h| h.0).collect::<Vec<_>>(),
        [1, 2, 4]
    );
    // Mirrored horizontally (colours swapped, Black to move): game 3 after
    // 1.e3 e5 is not it, but the mirror of 1.e4 e5 is 1.e4 e5 again.
    let h = PositionQuery {
        horizontal: true,
        ..q.clone()
    };
    assert_eq!(run(&mut db, &h, Some(&mut index)).hits, plain.hits);
}

#[test]
fn bad_queries_are_refused() {
    for q in [
        PositionQuery::default(),
        PositionQuery {
            look_for: vec![("z9".into(), 'N')],
            ..Default::default()
        },
        PositionQuery {
            look_for: vec![("e4".into(), 'Y')],
            ..Default::default()
        },
        PositionQuery {
            look_for: vec![("e4".into(), 'X')],
            exact: true,
            ..Default::default()
        },
        PositionQuery {
            look_for: vec![("e4".into(), 'N'), ("e4".into(), 'B')],
            exact: true,
            ..Default::default()
        },
    ] {
        assert!(q.compile().is_err(), "{q:?}");
    }
}
