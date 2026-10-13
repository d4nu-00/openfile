use chess_workbench::registry::Registry;

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
        .search_games(db.database_id.clone(), filter, "s".into())
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
