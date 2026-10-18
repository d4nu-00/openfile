use openfile::registry::Registry;

#[test]
fn searches_page_through_hits_and_clear() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = r
        .create_database(d.path().join("S.2cbh"), "c".into())
        .unwrap();
    let pgn = d.path().join("g.pgn");
    let mut text = String::new();
    for i in 0..5 {
        let white = if i % 2 == 0 {
            "Even, Player"
        } else {
            "Odd, Player"
        };
        text += &format!("[White \"{white}\"]\n[Result \"1-0\"]\n\n1. e4 1-0\n\n");
    }
    std::fs::write(&pgn, text).unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..5 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.database_id.clone(), "0".into(), false)
            .unwrap();
    }
    let filter = serde_json::from_str(r#"{"white":{"last":"even"}}"#).unwrap();
    let found = r
        .search_games(db.database_id.clone(), filter, "s".into(), |_, _| {})
        .unwrap();
    assert_eq!((found.hits.as_str(), found.scanned.as_str()), ("3", "5"));
    let page = r
        .read_search_page(db.database_id.clone(), "1".into(), 2, "p".into())
        .unwrap();
    let ordinals: Vec<_> = page.entries.iter().map(|e| e.ordinal.as_str()).collect();
    assert_eq!(ordinals, ["1", "3"]);
    assert_eq!(page.next.as_deref(), Some("3"));
    let rest = r
        .read_search_page(db.database_id.clone(), "3".into(), 2, "p".into())
        .unwrap();
    assert_eq!(rest.entries[0].ordinal, "5");
    assert_eq!(rest.next, None);
    r.clear_search(db.database_id.clone()).unwrap();
    assert_eq!(
        r.read_search_page(db.database_id, "1".into(), 2, "p".into())
            .unwrap_err()
            .code,
        "noSearch"
    );
}

#[test]
fn position_search_uses_the_booster_and_reports_nodes() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("boosters"));
    let db = r
        .create_database(d.path().join("P.2cbh"), "c".into())
        .unwrap();
    assert!(!db.booster);
    let pgn = d.path().join("g.pgn");
    std::fs::write(
        &pgn,
        "1. e4 e5 2. Nf3 *\n\n1. d4 d5 *\n\n1. Nf3 e5 2. e4 *\n",
    )
    .unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..3 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.database_id.clone(), "0".into(), false)
            .unwrap();
    }
    let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2";
    let plain = r
        .find_position(db.database_id.clone(), fen.into(), "f".into(), |_, _| {})
        .unwrap();
    assert!(!plain.indexed);
    assert_eq!(plain.hits, "2");
    let mut seen = vec![];
    assert!(
        r.create_search_booster(db.database_id.clone(), |done, total| seen
            .push((done, total)))
            .unwrap()
    );
    assert_eq!(seen.last(), Some(&(3, 3)));
    assert!(r.has_booster(&db.database_id).unwrap());
    let fast = r
        .find_position(db.database_id.clone(), fen.into(), "f".into(), |_, _| {})
        .unwrap();
    assert!(fast.indexed);
    assert_eq!(fast.hits, "2");
    let page = r
        .read_search_page(db.database_id.clone(), "1".into(), 10, "p".into())
        .unwrap();
    let found: Vec<_> = page
        .entries
        .iter()
        .map(|e| (e.ordinal.as_str(), e.found_node))
        .collect();
    assert_eq!(found, [("1", Some(3)), ("3", Some(3))]);
    // A pattern from the position tab: a white knight on f3 and a black
    // pawn on e5, in either colour (mirrored). It does not use the booster.
    let query: cbh_reader::pattern::PositionQuery =
        serde_json::from_str(r#"{"lookFor":[["f3","N"],["e5","p"]],"horizontal":true}"#).unwrap();
    let mut progress = vec![];
    let pattern = r
        .search_position(db.database_id.clone(), query, "q".into(), |d, t| {
            progress.push((d, t))
        })
        .unwrap();
    assert!(!pattern.indexed);
    assert_eq!(pattern.hits, "2");
    assert_eq!(progress.last(), Some(&(3, 3)));
    let page = r
        .read_search_page(db.database_id.clone(), "1".into(), 10, "p".into())
        .unwrap();
    let found: Vec<_> = page
        .entries
        .iter()
        .map(|e| (e.ordinal.as_str(), e.found_node))
        .collect();
    assert_eq!(found, [("1", Some(3)), ("3", Some(2))]);
    let empty: cbh_reader::pattern::PositionQuery = serde_json::from_str("{}").unwrap();
    assert_eq!(
        r.search_position(db.database_id.clone(), empty, "q".into(), |_, _| {})
            .unwrap_err()
            .code,
        "invalidQuery"
    );
    r.delete_search_booster(db.database_id.clone()).unwrap();
    assert!(!r.has_booster(&db.database_id).unwrap());
}
