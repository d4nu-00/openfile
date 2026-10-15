use chess_workbench::registry::Registry;

/// A new 2CBH database with `games` saved into it; returns its id.
fn database(r: &Registry, d: &tempfile::TempDir, games: &[&str]) -> String {
    let db = r
        .create_database(d.path().join("Openings"), "create".into())
        .unwrap();
    for (i, pgn) in games.iter().enumerate() {
        let path = d.path().join(format!("g{i}.pgn"));
        std::fs::write(&path, pgn).unwrap();
        let c = r.open_pgn(path, format!("o{i}")).unwrap();
        let doc = r
            .load_pgn_game(c.collection_id, 0, format!("l{i}"))
            .unwrap()
            .document_id;
        r.save_game(doc.clone(), db.database_id.clone(), "0".into(), false)
            .unwrap();
        r.close_document(doc).unwrap();
    }
    db.database_id
}

fn ecos(r: &Registry, db: &str) -> Vec<Option<String>> {
    r.read_page(db.into(), "1".into(), 10, "p".into())
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.eco)
        .collect()
}

#[test]
fn classify_opening_names_the_shown_game() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let path = d.path().join("g.pgn");
    std::fs::write(&path, "[ECO \"C40\"]\n\n1. e4 e5 2. Nf3 Nf6 3. Nxe5 d6 *\n").unwrap();
    let c = r.open_pgn(path, "o".into()).unwrap();
    let doc = r
        .load_pgn_game(c.collection_id, 0, "l".into())
        .unwrap()
        .document_id;
    let found = r.classify_game(&doc).unwrap();
    assert_eq!(found.eco.as_deref(), Some("C42"));
    assert_eq!(found.name.as_deref(), Some("Petrov's Defense"));
    assert_eq!(found.ply, Some(6));
    assert_eq!(found.current.as_deref(), Some("C40"));
}

#[test]
fn set_eco_codes_writes_the_codes_of_an_editable_database() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = database(
        &r,
        &d,
        &[
            "[ECO \"A00\"]\n\n1. e4 e5 2. Nf3 Nf6 *\n",
            "1. d4 d5 2. c4 e6 *\n",
            "*\n",
        ],
    );
    // Only games without a code: game 1 keeps its (wrong) code.
    let mut steps = vec![];
    let only = r
        .set_eco_codes(db.clone(), true, |done, total| steps.push((done, total)))
        .unwrap();
    assert_eq!(
        (
            only.games.as_str(),
            only.changed.as_str(),
            only.kept.as_str()
        ),
        ("3", "1", "1")
    );
    assert_eq!(only.unclassified, "1");
    assert!(!steps.is_empty());
    assert_eq!(
        ecos(&r, &db),
        [Some("A00"), Some("D30"), None].map(|e| e.map(String::from))
    );
    // Every game: game 1 is corrected.
    let all = r.set_eco_codes(db.clone(), false, |_, _| {}).unwrap();
    assert_eq!((all.changed.as_str(), all.unchanged.as_str()), ("1", "1"));
    assert_eq!(
        ecos(&r, &db),
        [Some("C42"), Some("D30"), None].map(|e| e.map(String::from))
    );
    // A read-only database is refused before the scan.
    r.close_database(db).unwrap();
    let ro = r
        .open_database(d.path().join("Openings.2cbh"), "ro".into())
        .unwrap();
    assert!(r.set_eco_codes(ro.database_id, false, |_, _| {}).is_err());
}
