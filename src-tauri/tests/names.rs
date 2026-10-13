use cbh_reader::name_list::NameKind;
use chess_workbench::registry::Registry;

#[test]
fn panes_list_names_find_them_and_build_a_crosstable() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let db = r
        .create_database(d.path().join("N.2cbh"), "c".into())
        .unwrap()
        .database_id;
    let pgn = d.path().join("g.pgn");
    std::fs::write(
        &pgn,
        "[Event \"Club\"]\n[Round \"1\"]\n[White \"Bravo, B\"]\n[Black \"Alpha, A\"]\n[Result \"0-1\"]\n\n1. e4 0-1\n\n\
         [Event \"Club\"]\n[Round \"2\"]\n[White \"Alpha, A\"]\n[Black \"Charlie, C\"]\n[Result \"1/2-1/2\"]\n\n1. d4 1/2-1/2\n",
    )
    .unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..2 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.clone(), "0".into(), false).unwrap();
    }
    // Names are read only once the list is prepared.
    assert_eq!(
        r.read_names(db.clone(), NameKind::Players, "1".into(), 10, "n".into())
            .unwrap_err()
            .code,
        "noNameList"
    );
    let listed = r
        .name_list(db.clone(), NameKind::Players, |_, _| {})
        .unwrap();
    assert_eq!(listed.rows.as_deref(), Some("3"));
    assert!(d.path().join("name-lists").is_dir());
    let page = r
        .read_names(db.clone(), NameKind::Players, "1".into(), 10, "n".into())
        .unwrap();
    let names: Vec<_> = page.rows.iter().map(|n| n.name.clone().unwrap()).collect();
    assert_eq!(names, ["Alpha, A", "Bravo, B", "Charlie, C"]);
    assert_eq!(page.rows[0].games, 2);
    assert_eq!(
        r.find_name(db.clone(), NameKind::Players, "char".into())
            .unwrap(),
        "3"
    );
    // A prefix past the end finds the last name.
    assert_eq!(
        r.find_name(db.clone(), NameKind::Players, "zz".into())
            .unwrap(),
        "3"
    );
    assert!(
        r.read_names(db.clone(), NameKind::Players, "1".into(), 201, "n".into())
            .is_err()
    );

    r.name_list(db.clone(), NameKind::Tournaments, |_, _| {})
        .unwrap();
    let club = r
        .read_names(db.clone(), NameKind::Tournaments, "1".into(), 1, "n".into())
        .unwrap()
        .rows[0]
        .id;
    let table = r.crosstable(db.clone(), club, None).unwrap();
    assert_eq!(table.players.len(), 3);
    assert_eq!(table.players[0].name.as_deref(), Some("Alpha, A"));
    assert_eq!(table.players[0].points, 1.5);

    // Games of a player through the search.
    let filter = serde_json::from_str(&format!(r#"{{"playerId":{}}}"#, page.rows[2].id)).unwrap();
    assert_eq!(
        r.search_games(db.clone(), filter, "s".into()).unwrap().hits,
        "1"
    );

    // Saving drops the lists; they are prepared again.
    r.set_deleted(db.clone(), "1".into(), true).unwrap();
    assert!(
        r.read_names(db.clone(), NameKind::Players, "1".into(), 10, "n".into())
            .is_err()
    );
    assert_eq!(
        r.name_list(db.clone(), NameKind::Players, |_, _| {})
            .unwrap()
            .rows
            .as_deref(),
        Some("3")
    );
}
