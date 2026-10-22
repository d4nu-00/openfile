use openfile::{engine_commands::render_info, registry::Registry};
#[test]
fn pv_conversion_uses_exact_root_and_stops_at_first_illegal_move() {
    let info = engine::AnalysisInfo {
        depth: Some(18),
        nodes: Some(9_007_199_254_740_993),
        multipv: 2,
        score: Some(engine::Score::Cp {
            value: -31,
            bound: None,
        }),
        pv: Some(vec!["e7e5".into(), "a1a8".into(), "g1f3".into()]),
    };
    let dto = render_info(
        info,
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        "black",
    );
    assert_eq!(dto.pv.as_deref(), Some(["e5".to_string()].as_slice()));
    assert_eq!(dto.raw_pv.as_ref().unwrap().len(), 3);
    assert_eq!(dto.root_turn, "black");
    assert_eq!(dto.diagnostics.len(), 1);
    let json = serde_json::to_value(dto).unwrap();
    assert_eq!(json["nodes"], "9007199254740993");
}
#[test]
fn analysis_input_is_derived_from_revisioned_document_tree() {
    let r = Registry::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.pgn");
    std::fs::write(&path, "1. e4 e5 (1... c5) *").unwrap();
    let collection = r.open_pgn(path, "pgn".into()).unwrap();
    let doc = r
        .load_pgn_game(collection.collection_id, 0, "game".into())
        .unwrap();
    let (start, history, fen, turn) = r.analysis_position(&doc.document_id, 3).unwrap();
    assert_eq!(history, vec!["e2e4", "c7c5"]);
    assert_eq!(turn, "white");
    assert!(fen.starts_with("rnbqkbnr/pp1ppppp/8/2p5/4P3"));
    assert!(start.ends_with("w KQkq - 0 1"));
    assert!(r.analysis_position(&doc.document_id, 99).is_err());
    r.close_document(doc.document_id.clone()).unwrap();
    assert!(r.analysis_position(&doc.document_id, 0).is_err());
}
#[test]
fn engine_settings_cannot_change_the_rules_of_standard_chess() {
    assert!(openfile::engine_commands::standard_option("UCI_Chess960", "true").is_err());
    assert!(openfile::engine_commands::standard_option("UCI_Variant", "atomic").is_err());
    assert!(openfile::engine_commands::standard_option("UCI_Chess960", "false").is_ok());
    assert!(openfile::engine_commands::standard_option("UCI_Variant", "chess").is_ok());
    assert!(openfile::engine_commands::standard_option("Threads", "2").is_ok());
}
#[test]
fn absent_principal_variation_is_distinct_from_supplied_invalid_variation() {
    let root = chess_core::Game::initial().start_fen;
    let engine::UciResponse::Info(absent) = engine::parse_line("info nodes 22").unwrap().unwrap()
    else {
        panic!()
    };
    let absent = serde_json::to_value(openfile::engine_commands::render_info(
        absent, &root, "white",
    ))
    .unwrap();
    assert!(absent.get("pv").is_none());
    assert!(absent.get("rawPv").is_none());
    let engine::UciResponse::Info(invalid) = engine::parse_line("info pv a1a8").unwrap().unwrap()
    else {
        panic!()
    };
    let invalid = serde_json::to_value(openfile::engine_commands::render_info(
        invalid, &root, "white",
    ))
    .unwrap();
    assert_eq!(invalid["pv"], serde_json::json!([]));
    assert_eq!(invalid["rawPv"], serde_json::json!(["a1a8"]));
    assert_eq!(invalid["diagnostics"].as_array().unwrap().len(), 1);
}
/// The engine crate's deterministic test engine, built by workspace tests.
fn mock_engine() -> Option<std::path::PathBuf> {
    let deps = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let name = format!("mock-uci{}", std::env::consts::EXE_SUFFIX);
    [deps.join(&name), deps.parent()?.join(&name)]
        .into_iter()
        .find(|p| p.exists())
}
#[test]
fn several_kibitzers_run_at_once_and_close_one_by_one() {
    use openfile::engine_commands::{EngineManager, MAX_KIBITZERS, probe};
    let Some(path) = mock_engine() else {
        panic!("mock-uci is missing: run `cargo test --workspace`");
    };
    assert_eq!(probe(path.clone()).unwrap()["name"], "Workbench mock");
    let m = EngineManager::default();
    let mut ids = vec![];
    for _ in 0..MAX_KIBITZERS {
        let ticket = m.begin_choice().unwrap();
        ids.push(m.choose(path.clone(), ticket).unwrap().engine_id);
    }
    // Each is its own engine; one more is refused.
    ids.dedup();
    assert_eq!(ids.len(), MAX_KIBITZERS);
    let ticket = m.begin_choice().unwrap();
    assert_eq!(
        m.choose(path.clone(), ticket).unwrap_err().code,
        "limitExceeded"
    );
    m.set_option(&ids[0], "Threads", "2").unwrap();
    m.close(&ids[0]).unwrap();
    assert!(m.set_option(&ids[0], "Threads", "2").is_err());
    m.set_option(&ids[1], "Threads", "2").unwrap();
    m.shutdown().unwrap();
    assert!(m.set_option(&ids[1], "Threads", "2").is_err());
}
#[test]
fn engine_lines_paste_as_one_undoable_edit() {
    let r = Registry::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.pgn");
    std::fs::write(&path, "1. e4 e5 2. Nf3 *").unwrap();
    let collection = r.open_pgn(path, "pgn".into()).unwrap();
    let doc = r
        .load_pgn_game(collection.collection_id, 0, "game".into())
        .unwrap()
        .document_id;
    let pasted = r
        .edit_document(
            doc.clone(),
            serde_json::from_str(
                r#"{"kind":"appendLine","parent":2,"moves":["f1c4","g8f6"],"comment":"0.35/22"}"#,
            )
            .unwrap(),
            "0".into(),
        )
        .unwrap();
    let last = pasted.selected.unwrap() as usize;
    assert_eq!(pasted.game.nodes[last].comments, ["0.35/22"]);
    // 2.Bc4 sits beside 2.Nf3 as a variation.
    assert_eq!(pasted.game.nodes[2].children.len(), 2);
    let undone = r
        .edit_document(
            doc.clone(),
            serde_json::from_str(r#"{"kind":"undo"}"#).unwrap(),
            "1".into(),
        )
        .unwrap();
    assert_eq!(undone.game.nodes.len(), 4);
    // An illegal line is refused and changes nothing.
    assert!(
        r.edit_document(
            doc,
            serde_json::from_str(r#"{"kind":"appendLine","parent":2,"moves":["e1e8"]}"#).unwrap(),
            "2".into(),
        )
        .is_err()
    );
}
#[test]
fn threat_mode_analyses_after_a_null_move_and_refuses_in_check() {
    use openfile::engine_commands::SearchMode;
    let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    let threat = SearchMode {
        threat: true,
        exclude: vec![],
    };
    let (start, history, fen, turn, search) = threat
        .apply(
            chess_core::Game::initial().start_fen,
            vec!["e2e4".into()],
            after_e4.into(),
            "black".into(),
        )
        .unwrap();
    // White moves again; the en passant square goes with the null move.
    assert_eq!(turn, "white");
    assert!(fen.starts_with("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq -"));
    assert_eq!(start, fen);
    assert!(history.is_empty() && search.is_empty());
    // Black in check (1.e4 f5 2.Qh5+): no threat to analyse.
    let check = "rnbqkbnr/ppppp1pp/8/5p1Q/4P3/8/PPPP1PPP/RNB1KBNR b KQkq - 1 2";
    assert!(
        threat
            .apply(check.into(), vec![], check.into(), "black".into())
            .is_err()
    );
}
#[test]
fn excluded_moves_leave_the_other_legal_moves_to_search() {
    use openfile::engine_commands::SearchMode;
    let start = chess_core::Game::initial().start_fen;
    let next_best = SearchMode {
        threat: false,
        exclude: vec!["e2e4".into(), "d2d4".into(), "not-a-move".into()],
    };
    let (.., search) = next_best
        .apply(start.clone(), vec![], start.clone(), "white".into())
        .unwrap();
    assert_eq!(search.len(), 18);
    assert!(!search.contains(&"e2e4".to_string()));
    assert!(search.contains(&"g1f3".to_string()));
    let everything = SearchMode {
        threat: false,
        exclude: {
            use shakmaty::Position;
            chess_core::model::parse_position(&start)
                .unwrap()
                .legal_moves()
                .iter()
                .map(|m| shakmaty::uci::UciMove::from_standard(*m).to_string())
                .collect()
        },
    };
    assert!(
        everything
            .apply(start.clone(), vec![], start, "white".into())
            .is_err()
    );
}
