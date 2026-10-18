use openfile::{deep_analysis::DeepParams, documents::Edit, registry::Registry};
use std::sync::atomic::{AtomicBool, Ordering};

/// The mock engine copied as `mock-uci-legal`, which scores the legal moves
/// of any position (see `mock-uci.rs`).
fn legal_engine(d: &tempfile::TempDir) -> std::path::PathBuf {
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let name = format!("mock-uci{}", std::env::consts::EXE_SUFFIX);
    let mock = [deps.join(&name), deps.parent().unwrap().join(&name)]
        .into_iter()
        .find(|p| p.exists())
        .expect("mock-uci is missing: run `cargo test --workspace`");
    let copy = d
        .path()
        .join(format!("mock-uci-legal{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(mock, &copy).unwrap();
    copy
}

fn document(r: &Registry, d: &tempfile::TempDir, pgn: &str) -> String {
    let path = d.path().join("g.pgn");
    std::fs::write(&path, pgn).unwrap();
    let c = r.open_pgn(path, "o".into()).unwrap();
    r.load_pgn_game(c.collection_id, 0, "l".into())
        .unwrap()
        .document_id
}

fn params(engine: std::path::PathBuf, node: u32) -> DeepParams {
    DeepParams {
        engine,
        options: Default::default(),
        node,
        time_limit_secs: 0,
        // 10 ms for the first iteration's positions instead of 4 s.
        base_millis: Some(10),
    }
}

#[test]
fn deep_analysis_builds_a_tree_that_goes_in_as_one_edit() {
    let d = tempfile::tempdir().unwrap();
    let engine = legal_engine(&d);
    let r = Registry::default();
    let doc = document(&r, &d, "1. e4 e5 *\n");
    let stop = AtomicBool::new(false);
    let mut seen = vec![];
    let report = r
        .deep_analysis(
            &doc,
            &params(engine, 2),
            &stop,
            &AtomicBool::new(false),
            |p| {
                seen.push(p.iteration);
                // Stop during the second iteration.
                if p.iteration >= 2 {
                    stop.store(true, Ordering::Relaxed);
                }
            },
        )
        .unwrap();
    assert!(!report.cancelled);
    assert!(report.iterations >= 1, "{}", report.iterations);
    assert!(report.positions >= 7, "{}", report.positions);
    assert!(!report.lines.is_empty());
    assert!(!report.best.is_empty());
    // The mock scores White's first legal moves in UCI order 30, 10, -10,
    // -30, ...; the window keeps those four. Progress arrives at most every
    // 200 ms, so how much of the second iteration ran before the stop, and
    // which of them its minimax prefers, depends on the machine's speed.
    let kept = [
        ("a3", "a2a3"),
        ("a4", "a2a4"),
        ("Na3", "b1a3"),
        ("Nc3", "b1c3"),
    ];
    let (_, uci) = kept
        .iter()
        .find(|(san, _)| *san == report.best[0])
        .unwrap_or_else(|| panic!("{:?}", report.best));
    // The best line (in UCI) comes first.
    assert_eq!(report.lines[0].moves[0], *uci);
    // Every line ends with a score comment.
    assert!(report.lines.iter().all(|l| l.comment.is_some()));
    let applied = r
        .edit_document(
            doc.clone(),
            Edit::AddLines {
                parent: report.node,
                lines: report.lines.clone(),
            },
            report.revision.clone(),
        )
        .unwrap();
    // After 1.e4 e5 the game had no continuation: the best line becomes
    // the main line, the others its variations.
    assert!(applied.game.nodes.len() > 3);
    let undone = r.edit_document(doc, Edit::Undo, applied.revision).unwrap();
    assert_eq!(undone.game.nodes.len(), 3);
}

#[test]
fn cancelling_returns_nothing() {
    let d = tempfile::tempdir().unwrap();
    let engine = legal_engine(&d);
    let r = Registry::default();
    let doc = document(&r, &d, "1. d4 *\n");
    let cancel = AtomicBool::new(false);
    let report = r
        .deep_analysis(
            &doc,
            &params(engine, 1),
            &AtomicBool::new(false),
            &cancel,
            |_| cancel.store(true, Ordering::Relaxed),
        )
        .unwrap();
    assert!(report.cancelled);
    assert!(report.lines.is_empty());
}

#[test]
fn a_time_limit_ends_the_analysis() {
    let d = tempfile::tempdir().unwrap();
    let engine = legal_engine(&d);
    let r = Registry::default();
    let doc = document(&r, &d, "1. e4 *\n");
    let mut p = params(engine, 0);
    p.time_limit_secs = 1;
    let started = std::time::Instant::now();
    let report = r
        .deep_analysis(
            &doc,
            &p,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    // Stopped by the limit (or the node bound), well before 30 iterations
    // of growing times would end.
    assert!(started.elapsed().as_secs() < 10);
    assert!(!report.lines.is_empty());
    assert!(
        r.deep_analysis(
            &doc,
            &DeepParams {
                base_millis: Some(5),
                ..params(std::path::PathBuf::from("/nonexistent"), 0)
            },
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            |_| {},
        )
        .is_err()
    );
}

/// With `UCI_ENGINE` set (a real engine), a short Deep Analysis of the
/// start position gives lines a real engine finds.
#[test]
fn a_real_engine_builds_a_tree_when_one_is_given() {
    let Some(engine) = std::env::var_os("UCI_ENGINE") else {
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let doc = document(&r, &d, "*\n");
    let mut p = params(engine.into(), 0);
    // 100 ms for the first iteration's positions; at most 3 s in all.
    p.base_millis = Some(100);
    p.time_limit_secs = 3;
    let report = r
        .deep_analysis(
            &doc,
            &p,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    eprintln!(
        "iterations {}, positions {}, lines {}, best {:?} {:?}",
        report.iterations,
        report.positions,
        report.lines.len(),
        report.best,
        report.score
    );
    assert!(report.lines.len() >= 2);
    assert!(
        ["e4", "d4", "Nf3", "c4", "e3", "g3"].contains(&report.best[0].as_str()),
        "{:?}",
        report.best
    );
    r.edit_document(
        doc,
        Edit::AddLines {
            parent: 0,
            lines: report.lines,
        },
        report.revision,
    )
    .unwrap();
}
