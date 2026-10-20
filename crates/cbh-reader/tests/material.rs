use cbh_reader::{Database, ReadLimits, search::GameFilter, v2::write::Writer};
use chess_core::{GameLimits, parse_pgn};
use std::sync::atomic::AtomicBool;

fn database() -> (tempfile::TempDir, Database) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Material.2cbh");
    // Distinct roots, a rook capture, a promotion, and a branch-only capture.
    let pgn = r#"[White "Rooks"]
[SetUp "1"]
[FEN "4k3/7p/8/8/8/8/1P2K3/R6r w - - 0 1"]

1. Ra2 *

[White "Pawns"]
[SetUp "1"]
[FEN "4k3/7p/8/8/8/8/P7/4K3 w - - 0 1"]

1. a4 h5 2. a5 h4 *

[White "Capture"]
[SetUp "1"]
[FEN "4k3/7p/8/8/8/8/1P2K3/R6r w - - 0 1"]

1. Rxh1 *

[White "Promotion"]
[SetUp "1"]
[FEN "4k3/P7/8/8/8/8/7p/4K3 w - - 0 1"]

1. a8=Q+ Kf7 *

[White "Branch"]
[SetUp "1"]
[FEN "4k3/7p/8/8/8/8/1P2K3/R6r w - - 0 1"]

1. Ra2 (1. Rxh1) *"#;
    let games = parse_pgn(pgn.as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    (d, Database::open(&path, ReadLimits::default()).unwrap())
}
fn run(db: &mut Database, json: &str) -> cbh_reader::search::SearchResult {
    let f: GameFilter = serde_json::from_str(json).unwrap();
    db.search(&f, 100, &AtomicBool::new(false), |_| {}).unwrap()
}

#[test]
fn endgame_ranges_match_any_position_after_header_filters() {
    let (_d, mut db) = database();
    let pawn_endings = r#"{"material":{"white":{"queens":{"max":0},"rooks":{"max":0},"minors":{"max":0}},"black":{"queens":{"max":0},"rooks":{"max":0},"minors":{"max":0}}}}"#;
    assert_eq!(run(&mut db, pawn_endings).ordinals, [2, 4]);
    assert_eq!(
        run(
            &mut db,
            r#"{"white":{"last":"Pawns"},"material":{"balance":{"min":0,"max":0}}}"#
        )
        .ordinals,
        [2]
    );
    assert_eq!(
        run(
            &mut db,
            r#"{"material":{"white":{"rooks":{"min":1}},"black":{"rooks":{"max":0}}}}"#
        )
        .ordinals,
        [3]
    );
}

#[test]
fn signed_balance_and_colour_reversal() {
    let (_d, mut db) = database();
    // Q=9,R=5,B=N=3,P=1. After Rxh1 white has five extra points.
    assert_eq!(
        run(&mut db, r#"{"material":{"balance":{"min":5,"max":5}}}"#).ordinals,
        [3]
    );
    assert!(
        run(&mut db, r#"{"material":{"balance":{"min":-5,"max":-5}}}"#)
            .ordinals
            .is_empty()
    );
    assert_eq!(
        run(
            &mut db,
            r#"{"material":{"balance":{"min":-5,"max":-5},"ignoreColors":true}}"#
        )
        .ordinals,
        [3]
    );
    // Promotion introduces a queen and changes balance by eight points.
    assert_eq!(
        run(
            &mut db,
            r#"{"material":{"white":{"queens":{"min":1}},"balance":{"min":8,"max":8}}}"#
        )
        .ordinals,
        [4]
    );
}

#[test]
fn variations_and_duration_do_not_join_separate_lines() {
    let (_d, mut db) = database();
    assert_eq!(
        run(
            &mut db,
            r#"{"material":{"balance":{"min":5},"lines":true}}"#
        )
        .ordinals,
        [3, 4, 5]
    );
    assert_eq!(
        run(&mut db, r#"{"material":{"balance":{"min":5},"length":2}}"#).ordinals,
        [4]
    );
    // Last=1 includes both positions before move 1 and after White's move.
    assert_eq!(
        run(
            &mut db,
            r#"{"material":{"white":{"queens":{"min":1}},"last":1}}"#
        )
        .ordinals,
        [4]
    );
    assert!(
        run(
            &mut db,
            r#"{"material":{"white":{"queens":{"min":1}},"first":3}}"#
        )
        .ordinals
        .is_empty()
    );
}

#[test]
fn invalid_material_is_rejected_before_scanning() {
    let (_d, mut db) = database();
    for json in [
        r#"{"material":{"white":{"rooks":{"min":2,"max":1}}}}"#,
        r#"{"material":{"balance":{"min":10,"max":-10}}}"#,
        r#"{"material":{"length":0}}"#,
        r#"{"material":{"first":5,"last":2}}"#,
        r#"{"material":{"white":{"pawns":{"max":17}}}}"#,
    ] {
        let f = serde_json::from_str(json).unwrap();
        assert!(
            db.search(&f, 100, &AtomicBool::new(false), |_| {}).is_err(),
            "{json}"
        );
    }
}

#[test]
fn selected_scan_keeps_identity_nodes_limits_and_progress() {
    let (_d, mut db) = database();
    let f: GameFilter = serde_json::from_str(r#"{"material":{"balance":{"min":5}}}"#).unwrap();
    let mut progress = vec![];
    let r = db
        .search_selected(
            &f,
            Some(&[1, 3, 5]),
            100,
            &AtomicBool::new(false),
            |d, t| progress.push((d, t)),
        )
        .unwrap();
    assert_eq!(r.ordinals, [3]);
    assert_eq!(r.nodes, Some(vec![1]));
    assert_eq!(r.scanned, 3);
    assert_eq!(progress.last(), Some(&(3, 3)));
    let r = db
        .search_selected(&f, Some(&[]), 100, &AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert!(r.ordinals.is_empty());
    assert_eq!(r.scanned, 0);
    let r = db.search(&f, 1, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(r.ordinals, [3]);
    assert!(r.truncated);
    let r = db.search(&f, 100, &AtomicBool::new(true), |_| {}).unwrap();
    assert!(r.cancelled);
    assert!(r.ordinals.is_empty());
    for picked in [&[0][..], &[3, 1], &[1, 1], &[6]] {
        assert!(
            db.search_selected(&f, Some(picked), 100, &AtomicBool::new(false), |_, _| {})
                .is_err()
        );
    }
}

#[test]
fn duration_cannot_switch_colour_orientation_mid_run() {
    let games = parse_pgn(
        br#"[SetUp "1"]
[FEN "4k3/2pp4/8/8/8/8/1P2K3/R6r w - - 0 1"]

1. Rxh1 *"#,
        &GameLimits::default(),
    )
    .unwrap();
    // Black leads by 1 at the root; White leads by 4 after the capture.
    // Either orientation matches separately, but neither lasts for two plies.
    let q: cbh_reader::material::MaterialQuery =
        serde_json::from_str(r#"{"balance":{"min":1},"ignoreColors":true,"length":2}"#).unwrap();
    assert_eq!(q.find_in(&games[0], &AtomicBool::new(false)).unwrap(), None);
}
