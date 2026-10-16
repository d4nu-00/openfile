use cbh_reader::name_list::NameKind;
use chess_workbench::{clipboard::RowRange, error_report::ErrorReportParams, registry::Registry};
use std::sync::atomic::AtomicBool;

/// The mock engine copied as `mock-uci-legal`: line 1 is the first legal
/// move in UCI order, scored +0.30 for the side to move.
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

fn rows(start: u32, end: u32) -> Vec<RowRange> {
    serde_json::from_value(
        serde_json::json!([{ "start": start.to_string(), "end": end.to_string() }]),
    )
    .unwrap()
}

#[test]
fn error_report_counts_the_player_s_losses() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let db = r
        .create_database(d.path().join("E.2cbh"), "c".into())
        .unwrap()
        .database_id;
    let pgn = d.path().join("g.pgn");
    // Alpha has White in the first game and Black in the second; the third
    // is someone else's. 1.a3 is the mock's best move in the start position.
    std::fs::write(
        &pgn,
        "[White \"Alpha, A\"]\n[Black \"Bravo, B\"]\n[WhiteElo \"2400\"]\n[Result \"1-0\"]\n\n1. a3 e5 2. Nf3 Nc6 1-0\n\n\
         [White \"Bravo, B\"]\n[Black \"Alpha, A\"]\n[BlackElo \"2500\"]\n[Result \"0-1\"]\n\n1. e4 e5 2. Nf3 Nc6 0-1\n\n\
         [White \"Charlie, C\"]\n[Black \"Bravo, B\"]\n[Result \"1-0\"]\n\n1. d4 1-0\n",
    )
    .unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..3 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.clone(), "0".into(), false).unwrap();
    }
    r.name_list(db.clone(), NameKind::Players, |_, _| {})
        .unwrap();
    // The player in most games comes first.
    let players = r.selection_players(db.clone(), rows(1, 4)).unwrap();
    assert_eq!(players[0].name.as_deref(), Some("Bravo, B"));
    assert_eq!(players[0].games, 3);
    let alpha = players
        .iter()
        .find(|p| p.name.as_deref() == Some("Alpha, A"))
        .unwrap()
        .id;

    let params = ErrorReportParams {
        engine: legal_engine(&d),
        options: Default::default(),
        slower: false,
        player: Some(alpha),
        ranges: rows(1, 4),
    };
    let mut steps = vec![];
    let report = r
        .error_report(db.clone(), &params, &AtomicBool::new(false), |p| {
            steps.push(p.games_done)
        })
        .unwrap();
    assert!(!report.cancelled);
    assert_eq!(report.depth, 9);
    assert_eq!(report.player.as_deref(), Some("Alpha, A"));
    // The third game has no move of Alpha's.
    assert_eq!((report.games, report.skipped), (2, 1));
    // Alpha's moves: 1.a3 (the best, no loss) and 2.Nf3; then 1...e5 and
    // 2...Nc6. Every other move loses +0.30 − (−0.30) = 60.
    assert_eq!(report.moves, 4);
    assert_eq!(report.average_loss, Some(45.0));
    // Losses over weights: 180 / (4 × 2).
    assert_eq!(report.error_value, Some(22.5));
    assert_eq!(report.severity, [0, 3, 0, 0]);
    assert_eq!(report.worst.len(), 3);
    assert_eq!(report.worst[0].loss, 60);
    assert_eq!(report.rating, Some(2450));
    // Too few moves for a Blunder Elo.
    assert_eq!(report.blunder_elo, None);
    assert_eq!(report.phases["opening"].moves, 4);
    assert_eq!(steps.last(), Some(&3));

    // Both sides count every move of every game.
    let both = r
        .error_report(
            db.clone(),
            &ErrorReportParams {
                player: None,
                ..params.clone()
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    assert_eq!((both.games, both.moves), (3, 9));

    // Cancelling stops at once.
    let stopped = r
        .error_report(db.clone(), &params, &AtomicBool::new(true), |_| {})
        .unwrap();
    assert!(stopped.cancelled);

    // Nothing selected is refused.
    assert!(
        r.error_report(
            db,
            &ErrorReportParams {
                ranges: vec![],
                ..params
            },
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err()
    );
}
