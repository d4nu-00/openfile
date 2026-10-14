use chess_workbench::{blunder_check::BlunderParams, registry::Registry};
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

fn params(side: &str, threshold: i32) -> BlunderParams {
    BlunderParams {
        engine: mock_engine(),
        options: Default::default(),
        millis_per_move: 100,
        threshold,
        side: side.into(),
    }
}

#[test]
fn moves_losing_the_threshold_are_found_and_annotated_in_one_edit() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let doc = document(&r, &d, "1. e4 e5 *\n");
    let mut steps = vec![];
    // The mock engine always says +0.17 for the side to move with e7e5.
    let report = r
        .blunder_check(
            &doc,
            &params("both", 30),
            &AtomicBool::new(false),
            |d, t| steps.push((d, t)),
        )
        .unwrap();
    assert!(!report.cancelled);
    assert_eq!(report.checked, 3);
    assert_eq!(steps.last(), Some(&(3, 3)));
    // 1.e4: +0.17 before, -0.17 for White after: a loss of 34. 1...e5 is
    // the engine's own move, so it is not a finding.
    assert_eq!(report.findings.len(), 1);
    let f = &report.findings[0];
    assert_eq!((f.node, f.loss), (1, 34));
    assert_eq!((f.best.as_str(), f.played.as_str()), ("+0.17", "-0.17"));
    // Every position is "=" for ChessBase's symbols, so none changes.
    assert!(report.evaluations.is_empty());
    // A larger threshold, or only Black's moves, finds nothing.
    let none = r
        .blunder_check(
            &doc,
            &params("both", 35),
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
    assert!(none.findings.is_empty());
    let black = r
        .blunder_check(
            &doc,
            &params("black", 30),
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
    assert!(black.findings.is_empty());
    // A cancelled check reports nothing.
    let cancelled = r
        .blunder_check(&doc, &params("both", 30), &AtomicBool::new(true), |_, _| {})
        .unwrap();
    assert!(cancelled.cancelled && cancelled.findings.is_empty());
    // Bad parameters are refused before the engine starts.
    assert!(
        r.blunder_check(&doc, &params("both", 5), &AtomicBool::new(false), |_, _| {})
            .is_err()
    );

    // The annotations go in as one edit: symbol, comment, and the engine's
    // line as a variation.
    let edited = r
        .edit_document(
            doc.clone(),
            serde_json::from_str(
                r#"{"kind":"addAnalysis","items":[{"node":1,"nag":2,"comment":"+0.17","line":["d2d4","d7d5"],"lineComment":"+0.17/12"}]}"#,
            )
            .unwrap(),
            "0".into(),
        )
        .unwrap();
    let e4 = &edited.game.nodes[1];
    assert_eq!(
        (e4.nags.as_slice(), e4.comments.as_slice()),
        (&[2u16][..], &["+0.17".to_string()][..])
    );
    assert_eq!(edited.game.nodes[0].children.len(), 2);
    let pgn = r.document_pgn(doc.clone()).unwrap();
    assert!(
        pgn.contains("1. e4 $2 {+0.17} (1. d4 d5 {+0.17/12}) 1... e5"),
        "{pgn}"
    );
    // An item that does not apply leaves the game unchanged.
    assert!(
        r.edit_document(
            doc.clone(),
            serde_json::from_str(
                r#"{"kind":"addAnalysis","items":[{"node":2,"nag":4},{"node":99}]}"#
            )
            .unwrap(),
            "1".into(),
        )
        .is_err()
    );
    assert!(r.document_pgn(doc).unwrap().contains("e5 *"));
}

#[test]
fn evaluation_symbols_replace_evaluations_but_not_other_position_symbols() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    // 1.e4 has "!" and "=", 1...e5 "with attack" ($40).
    let doc = document(&r, &d, "1. e4 $1 $11 e5 $40 *\n");
    let edited = r
        .edit_document(
            doc.clone(),
            serde_json::from_str(
                r#"{"kind":"addAnalysis","items":[{"node":1,"evaluation":14},{"node":2,"evaluation":15}]}"#,
            )
            .unwrap(),
            "0".into(),
        )
        .unwrap();
    assert_eq!(edited.game.nodes[1].nags, vec![1, 14]);
    assert_eq!(edited.game.nodes[2].nags, vec![40]);
}

/// With a real engine (`UCI_ENGINE`), the blunder of the Scholar's mate is
/// found and the engine's line refutes it differently.
#[test]
fn a_real_engine_finds_a_real_blunder() {
    let Some(engine) = std::env::var_os("UCI_ENGINE") else {
        eprintln!("UCI_ENGINE is not set: real-engine Blunder Check skipped");
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let doc = document(&r, &d, "1. e4 e5 2. Qh5 Nc6 3. Bc4 Nf6 4. Qxf7# 1-0\n");
    let report = r
        .blunder_check(
            &doc,
            &BlunderParams {
                engine: engine.into(),
                options: Default::default(),
                millis_per_move: 300,
                threshold: 100,
                side: "black".into(),
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
    let nf6 = report
        .findings
        .iter()
        .find(|f| f.node == 6)
        .expect("3...Nf6 found");
    assert!(nf6.loss > 1_000, "{nf6:?}");
    assert_eq!(nf6.played, "+M1");
    assert_ne!(nf6.line.first().map(String::as_str), Some("g8f6"));
    // The other black moves keep the balance.
    assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
}
