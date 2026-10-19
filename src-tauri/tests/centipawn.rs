use openfile::{
    centipawn::{Category, CentipawnParams},
    registry::Registry,
};
use std::sync::atomic::AtomicBool;

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

fn document(r: &Registry, d: &tempfile::TempDir, pgn: &str) -> String {
    let path = d.path().join("g.pgn");
    std::fs::write(&path, pgn).unwrap();
    let c = r.open_pgn(path, "o".into()).unwrap();
    r.load_pgn_game(c.collection_id, 0, "l".into())
        .unwrap()
        .document_id
}

fn params(every_move: bool) -> CentipawnParams {
    CentipawnParams {
        engine: mock_engine(),
        options: Default::default(),
        seconds: 0,
        every_move,
    }
}

#[test]
fn moves_after_the_opening_are_classified_and_summarised() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    // A known opening, then moves the table does not know.
    let doc = document(
        &r,
        &d,
        "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 d6 8. c3 O-O 9. a3 Kh8 10. Kh1 Rb8 11. Qe2 Qd7 *\n",
    );
    let mut steps = vec![];
    let report = r
        .centipawn_analysis(&doc, &params(false), &AtomicBool::new(false), |d, t| {
            steps.push((d, t))
        })
        .unwrap();
    assert!(!report.cancelled);
    assert_eq!(report.millis_per_move, 200);
    // The table knows the Ruy Lopez to 8...O-O; at least ten plies.
    assert!(report.book_plies >= 10, "{}", report.book_plies);
    let counted = 22 - u32::from(report.book_plies);
    assert_eq!(report.white.moves + report.black.moves, counted);
    assert_eq!(steps.last().map(|s| s.0), Some(report.positions));
    // The mock engine says +0.17 for the side to move, with 1...e5 as its
    // move: every counted move loses 34 cp, an inaccuracy.
    assert_eq!(report.white.average_loss, Some(34.0));
    assert_eq!(report.moves.len() as u32, counted);
    assert!(report.moves.iter().all(|m| m.loss == 34));
    assert!(
        report
            .moves
            .iter()
            .all(|m| m.category == Some(Category::Inaccuracy))
    );
    assert!(report.moves.iter().all(|m| !m.san.is_empty()));
    assert!(
        report
            .moves
            .windows(2)
            .all(|pair| pair[0].ply < pair[1].ply)
    );
    assert!(
        report
            .moves
            .iter()
            .all(|m| m.move_number == u32::from(m.ply).div_ceil(2))
    );
    assert!(
        report
            .white
            .accuracy
            .is_some_and(|a| (a - 79.7).abs() < 0.2)
    );
    assert!(
        report
            .black
            .accuracy
            .is_some_and(|a| (a - 79.7).abs() < 0.2)
    );
    let inaccuracies = report.white.categories[Category::Inaccuracy as usize]
        + report.black.categories[Category::Inaccuracy as usize];
    assert_eq!(inaccuracies, counted);
    // The summary first, then one text per error.
    assert_eq!(report.items[0].node, 0);
    assert!(
        report.items[0]
            .comment
            .starts_with("Centipawn Analysis after ply")
    );
    assert!(report.items[0].comment.contains("Inaccuracy: White"));
    assert_eq!(report.items.len() as u32, 1 + counted);
    assert!(report.items[1..].iter().all(|i| i.comment == "Inaccuracy"));

    // The texts go in as one edit, the summary before the first move.
    let items: Vec<serde_json::Value> = report
        .items
        .iter()
        .map(|i| serde_json::json!({"node": i.node, "comment": i.comment, "line": []}))
        .collect();
    let edited = r
        .edit_document(
            doc.clone(),
            serde_json::from_value(serde_json::json!({"kind": "addAnalysis", "items": items}))
                .unwrap(),
            report.revision.clone(),
        )
        .unwrap();
    assert!(edited.game.nodes[0].comments[0].starts_with("Centipawn Analysis"));
    assert_eq!(
        edited
            .game
            .nodes
            .iter()
            .filter(|n| n.comments.iter().any(|c| c == "Inaccuracy"))
            .count() as u32,
        counted
    );
    // The start takes no symbol.
    assert!(
        r.edit_document(
            doc.clone(),
            serde_json::from_value(serde_json::json!({
                "kind": "addAnalysis",
                "items": [{"node": 0, "nag": 2, "line": []}]
            }))
            .unwrap(),
            "1".into(),
        )
        .is_err()
    );

    // Cancelled before it starts: nothing to add.
    let stopped = r
        .centipawn_analysis(&doc, &params(true), &AtomicBool::new(true), |_, _| {})
        .unwrap();
    assert!(stopped.cancelled && stopped.items.is_empty());
    // Out of range seconds are refused.
    let mut slow = params(false);
    slow.seconds = 61;
    assert!(
        r.centipawn_analysis(&doc, &slow, &AtomicBool::new(false), |_, _| {})
            .is_err()
    );
}

#[test]
fn a_game_inside_its_opening_has_nothing_to_analyse() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let doc = document(&r, &d, "1. e4 e5 2. Nf3 *\n");
    let err = r
        .centipawn_analysis(&doc, &params(false), &AtomicBool::new(false), |_, _| {})
        .unwrap_err();
    assert_eq!(err.code, "noMoves");
}

#[test]
fn review_uses_the_start_positions_move_number() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let doc = document(
        &r,
        &d,
        "[SetUp \"1\"]\n[FEN \"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 18\"]\n\n18... e5 19. Nf3 Nc6 *\n",
    );
    let report = r
        .centipawn_analysis(&doc, &params(false), &AtomicBool::new(false), |_, _| {})
        .unwrap();
    let first = &report.moves[0];
    assert_eq!(
        (first.move_number, first.white, first.san.as_str()),
        (18, false, "e5")
    );
    assert_eq!(report.moves[1].move_number, 19);
}
