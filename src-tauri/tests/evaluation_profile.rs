use openfile::{
    evaluation_profile::{EvalCache, InstantParams},
    registry::Registry,
};
use std::sync::atomic::AtomicBool;

/// The mock engine copied as `mock-uci-legal`: every position scores +0.30
/// for the side to move (see `mock-uci.rs`).
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

#[test]
fn instant_analysis_fills_the_profile_reuses_evaluations_and_stops() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let path = d.path().join("g.pgn");
    std::fs::write(&path, "1. e4 e5 2. Nf3 *").unwrap();
    let c = r.open_pgn(path, "o".into()).unwrap();
    let doc = r
        .load_pgn_game(c.collection_id, 0, "l".into())
        .unwrap()
        .document_id;
    let params = InstantParams {
        engine: legal_engine(&d),
        options: Default::default(),
        level: 1,
        endless: false,
    };
    let mut updates = 0;
    let done = r
        .instant_analysis(&doc, &params, &AtomicBool::new(false), |_| updates += 1)
        .unwrap();
    assert!(done.finished && !done.cancelled);
    assert_eq!(
        (done.pass, done.millis_per_position, done.total),
        (1, 35, 4)
    );
    // +0.30 for the side to move, from White's side.
    let cps: Vec<_> = done.profile.iter().map(|p| p.cp.unwrap()).collect();
    assert_eq!(cps, [30, -30, 30, -30]);
    assert!(done.profile.iter().all(|p| p.depth == 1));
    // The positions are kept for this time, and a second run reuses them.
    let start = chess_core::model::parse_position(&chess_core::Game::initial().start_fen).unwrap();
    assert!(
        EvalCache::global()
            .lock()
            .unwrap()
            .get("Workbench mock", &start, 35)
            .is_some()
    );
    let t = std::time::Instant::now();
    let again = r
        .instant_analysis(&doc, &params, &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(again.profile, done.profile);
    assert!(t.elapsed() < std::time::Duration::from_millis(4 * 35 + 2_000));
    // Cancelled from the start: nothing is evaluated beyond the cache.
    let longer = InstantParams {
        level: 6,
        ..params.clone()
    };
    let stopped = r
        .instant_analysis(&doc, &longer, &AtomicBool::new(true), |_| {})
        .unwrap();
    assert!(stopped.cancelled && !stopped.finished);
    assert!(stopped.profile.is_empty());
    let invalid = InstantParams { level: 7, ..params };
    assert!(
        r.instant_analysis(&doc, &invalid, &AtomicBool::new(false), |_| {})
            .is_err()
    );
}
