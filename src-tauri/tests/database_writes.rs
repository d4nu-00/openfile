use openfile::registry::Registry;

fn pgn_document(r: &Registry, d: &tempfile::TempDir, text: &str) -> String {
    let path = d.path().join("games.pgn");
    std::fs::write(&path, text).unwrap();
    let c = r.open_pgn(path, "open".into()).unwrap();
    r.load_pgn_game(c.collection_id, 0, "load".into())
        .unwrap()
        .document_id
}

#[test]
fn created_databases_take_saves_replacements_and_deletions() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = r
        .create_database(d.path().join("Mine"), "create".into())
        .unwrap();
    assert!(db.writable);
    assert!(d.path().join("Mine.2cbh").exists());
    let doc = pgn_document(
        &r,
        &d,
        "[White \"Ada, Lovelace\"]\n[Result \"1-0\"]\n\n1. e4 {Best by test} e5 2. Nf3 1-0\n",
    );
    let saved = r
        .save_game(doc.clone(), db.database_id.clone(), "0".into(), false)
        .unwrap();
    assert_eq!(
        (saved.ordinal.as_str(), saved.entry_count.as_str()),
        ("1", "1")
    );
    let page = r
        .read_page(db.database_id.clone(), "1".into(), 10, "p".into())
        .unwrap();
    assert_eq!(page.entries[0].white.as_deref(), Some("Ada, Lovelace"));

    // A game loaded from the database replaces its own entry.
    let loaded = r
        .load_entry(db.database_id.clone(), "1".into(), "l".into())
        .unwrap();
    let id = loaded.document_id.unwrap();
    r.edit_document(
        id.clone(),
        serde_json::from_str(r#"{"kind":"appendMove","parent":3,"uci":"b8c6"}"#).unwrap(),
        "0".into(),
    )
    .unwrap();
    let replaced = r.replace_game(id.clone(), "1".into(), false).unwrap();
    assert_eq!(replaced.entry_count, "1");
    let again = r
        .load_entry(db.database_id.clone(), "1".into(), "l2".into())
        .unwrap();
    assert_eq!(again.game.unwrap().nodes.len(), 5);

    r.set_deleted(db.database_id.clone(), "1".into(), true)
        .unwrap();
    let page = r
        .read_page(db.database_id.clone(), "1".into(), 10, "p".into())
        .unwrap();
    assert!(page.entries[0].deleted);
    // After Save, the document is linked to its new entry, as in the reference app;
    // a PGN document that was never saved has no entry to replace.
    assert!(r.replace_game(doc, "0".into(), false).is_ok());
    let fresh = pgn_document(&r, &d, "1. c4 *\n");
    assert_eq!(
        r.replace_game(fresh, "0".into(), false).unwrap_err().code,
        "noOrigin"
    );
}

#[test]
fn read_only_and_already_open_databases_are_refused() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = r
        .create_database(d.path().join("Mine.2cbh"), "create".into())
        .unwrap();
    // A family open for editing cannot be opened again, in either mode.
    let path = d.path().join("Mine.2cbh");
    assert_eq!(
        r.open_database(path.clone(), "again".into())
            .unwrap_err()
            .code,
        "alreadyOpen"
    );
    r.close_database(db.database_id).unwrap();
    let read_only = r.open_database(path.clone(), "ro".into()).unwrap();
    assert!(!read_only.writable);
    // Nor can one that is open read-only be opened for editing.
    assert_eq!(
        r.open_database_with(path.clone(), "rw".into(), true)
            .unwrap_err()
            .code,
        "alreadyOpen"
    );
    let doc = pgn_document(&r, &d, "1. d4 *\n");
    assert_eq!(
        r.save_game(doc, read_only.database_id.clone(), "0".into(), false)
            .unwrap_err()
            .code,
        "readOnly"
    );
    assert!(
        r.set_deleted(read_only.database_id, "1".into(), true)
            .is_err()
    );
    // An existing family is never overwritten by New Database.
    assert!(r.create_database(path, "dup".into()).is_err());
}

#[test]
fn drawn_marks_undo_and_survive_a_database_save() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = r
        .create_database(d.path().join("Marks"), "create".into())
        .unwrap();
    let doc = pgn_document(&r, &d, "1. e4 e5 *\n");
    let edit = |json: &str, revision: &str| {
        r.edit_document(
            doc.clone(),
            serde_json::from_str(json).unwrap(),
            revision.into(),
        )
        .unwrap()
    };
    let drawn = edit(
        r#"{"kind":"setMarks","node":1,"squares":[{"color":"G","square":"d5"}],"arrows":[{"color":"R","from":"g1","to":"f3"}]}"#,
        "0",
    );
    assert!(drawn.dirty);
    assert_eq!(drawn.game.nodes[1].squares.len(), 1);
    let undone = edit(r#"{"kind":"undo"}"#, "1");
    assert!(undone.game.nodes[1].squares.is_empty());
    let redone = edit(r#"{"kind":"redo"}"#, "2");
    assert_eq!(redone.game.nodes[1].arrows.len(), 1);
    let pgn = r.document_pgn(doc.clone()).unwrap();
    assert!(pgn.contains("[%csl Gd5]") && pgn.contains("[%cal Rg1f3]"));

    let saved = r
        .save_game(doc.clone(), db.database_id.clone(), "3".into(), false)
        .unwrap();
    let loaded = r
        .load_entry(db.database_id.clone(), saved.ordinal, "l".into())
        .unwrap();
    let node = &loaded.game.unwrap().nodes[1];
    assert_eq!(
        (node.squares[0].square.as_str(), node.arrows[0].to.as_str()),
        ("d5", "f3")
    );
    let cleared = edit(r#"{"kind":"clearMarks"}"#, "3");
    assert!(
        cleared
            .game
            .nodes
            .iter()
            .all(|n| n.squares.is_empty() && n.arrows.is_empty())
    );
}
