use chess_workbench::{engine_commands::render_info, registry::Registry};
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
        pv: vec!["e7e5".into(), "a1a8".into(), "g1f3".into()],
    };
    let dto = render_info(
        info,
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        "black",
    );
    assert_eq!(dto.pv, vec!["e5"]);
    assert_eq!(dto.raw_pv.len(), 3);
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
    assert!(chess_workbench::engine_commands::standard_option("UCI_Chess960", "true").is_err());
    assert!(chess_workbench::engine_commands::standard_option("UCI_Variant", "atomic").is_err());
    assert!(chess_workbench::engine_commands::standard_option("UCI_Chess960", "false").is_ok());
    assert!(chess_workbench::engine_commands::standard_option("UCI_Variant", "chess").is_ok());
    assert!(chess_workbench::engine_commands::standard_option("Threads", "2").is_ok());
}
