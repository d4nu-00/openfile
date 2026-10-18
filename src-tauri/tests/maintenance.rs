use openfile::registry::Registry;

/// A database opened for editing with games by White's name; Bravo's and
/// Charlie's are the same game.
fn database(r: &Registry, dir: &std::path::Path) -> String {
    let db = r.create_database(dir.join("A.2cbh"), "c".into()).unwrap();
    let pgn = dir.join("g.pgn");
    let mut text = String::new();
    for (white, moves) in [
        ("Alpha, A", "1. d4 d5 2. c4 e6 3. Nc3 Nf6"),
        ("Bravo, B", "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6"),
        ("Bravo, B", "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6"),
        ("Delta, D", "1. c4 c5 2. Nc3 Nc6 3. g3 g6"),
    ] {
        text += &format!(
            "[White \"{white}\"]\n[Black \"Other, O\"]\n[Date \"2020.01.01\"]\n[Result \"1-0\"]\n\n{moves} 1-0\n\n"
        );
    }
    std::fs::write(&pgn, text).unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..4 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.database_id.clone(), "0".into(), false)
            .unwrap();
    }
    db.database_id
}

fn rows(r: &Registry, db: &str) -> Vec<(String, bool)> {
    r.read_rows(db.into(), "1".into(), 200, "r".into())
        .unwrap()
        .entries
        .into_iter()
        .map(|e| (e.white.unwrap_or_default(), e.deleted))
        .collect()
}

#[test]
fn integrity_doubles_and_removing_deleted_games() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("indexes"));
    let db = database(&r, d.path());

    let report = r.check_integrity(db.clone(), |_, _| {}).unwrap();
    assert_eq!((report.checked, report.games), (4, 4));
    assert!(report.problems.is_empty());

    // Game 4 is open in a tab; its entry number will change.
    let open = r.load_entry(db.clone(), "4".into(), "g".into()).unwrap();
    let options = cbh_reader::maintain::DoubleOptions::default();
    let doubles = r
        .find_double_games(db.clone(), options.clone(), |_, _| {})
        .unwrap();
    assert_eq!(doubles.marked, "1");
    assert_eq!(doubles.pairs, vec![("3".to_string(), "2".to_string())]);
    assert_eq!(
        rows(&r, &db),
        vec![
            ("Alpha, A".into(), false),
            ("Bravo, B".into(), false),
            ("Bravo, B".into(), true),
            ("Delta, D".into(), false),
        ]
    );
    // Marked games are not doubles any more.
    assert_eq!(
        r.find_double_games(db.clone(), options, |_, _| {})
            .unwrap()
            .marked,
        "0"
    );

    let mut steps = vec![];
    let compacted = r
        .remove_deleted_games(db.clone(), |done, total| steps.push((done, total)))
        .unwrap();
    assert_eq!(
        (
            compacted.kept.as_str(),
            compacted.removed.as_str(),
            compacted.entry_count.as_str(),
            compacted.cancelled
        ),
        ("3", "1", "3", false)
    );
    assert_eq!(steps.last(), Some(&(4, 4)));
    // Every open game from the database follows: the saved games too.
    assert!(
        compacted
            .moved
            .contains(&("4".to_string(), "3".to_string()))
    );
    assert!(
        compacted
            .moved
            .contains(&("3".to_string(), "0".to_string()))
    );
    assert_eq!(
        rows(&r, &db),
        vec![
            ("Alpha, A".into(), false),
            ("Bravo, B".into(), false),
            ("Delta, D".into(), false),
        ]
    );
    // The open game now replaces entry 3, the same game.
    let saved = r
        .replace_game(open.document_id.clone().unwrap(), "0".into(), false)
        .unwrap();
    assert_eq!(saved.ordinal, "3");
    // The database can still be written after the swap.
    let doc = r.load_entry(db.clone(), "1".into(), "g".into()).unwrap();
    assert_eq!(
        r.save_game(doc.document_id.unwrap(), db.clone(), "0".into(), false)
            .unwrap()
            .entry_count,
        "4"
    );
    assert!(!d.path().join("A.compact-backup.2cbh").exists());
    assert!(!d.path().join("A.compacting.2cbh").exists());
}

#[test]
fn read_only_databases_are_checked_but_not_changed() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = database(&r, d.path());
    r.close_database(db).unwrap();
    let ro = r
        .open_database_with(d.path().join("A.2cbh"), "o".into(), false)
        .unwrap()
        .database_id;
    assert_eq!(r.check_integrity(ro.clone(), |_, _| {}).unwrap().checked, 4);
    let options = cbh_reader::maintain::DoubleOptions::default();
    assert_eq!(
        r.find_double_games(ro.clone(), options, |_, _| {})
            .unwrap_err()
            .code,
        "readOnly"
    );
    assert_eq!(
        r.remove_deleted_games(ro, |_, _| {}).unwrap_err().code,
        "readOnly"
    );
}
