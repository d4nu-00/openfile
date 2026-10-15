use chess_workbench::{clipboard::RowRange, registry::Registry};

fn range(start: u64, end: u64) -> RowRange {
    serde_json::from_value(serde_json::json!({
        "start": start.to_string(),
        "end": end.to_string()
    }))
    .unwrap()
}

fn database(r: &Registry, dir: &std::path::Path) -> String {
    let db = r.create_database(dir.join("A.2cbh"), "c".into()).unwrap();
    let pgn = dir.join("g.pgn");
    let mut text = String::new();
    for white in ["Alpha, A", "Bravo, B", "Charlie, C"] {
        text += &format!(
            "[White \"{white}\"]\n[Black \"Other, O\"]\n[Event \"Club\"]\n[Annotator \"Old, A\"]\n[Result \"1-0\"]\n\n1. e4 e5 1-0\n\n"
        );
    }
    std::fs::write(&pgn, text).unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..3 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.database_id.clone(), "0".into(), false)
            .unwrap();
    }
    db.database_id
}

fn rows(r: &Registry, db: &str) -> Vec<serde_json::Value> {
    r.read_rows(db.into(), "1".into(), 200, "r".into())
        .unwrap()
        .entries
        .into_iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect()
}

#[test]
fn mark_games_toggles_the_user_medal_of_the_selection() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = database(&r, d.path());
    let mut steps = vec![];
    let all = r
        .mark_rows(db.clone(), vec![range(1, 4)], |a, b| steps.push((a, b)))
        .unwrap();
    assert_eq!((all.changed.as_str(), all.marked.as_str()), ("3", "3"));
    assert_eq!(steps.last(), Some(&(3, 3)));
    // Each game toggles on its own, as in ChessBase.
    let some = r
        .mark_rows(db.clone(), vec![range(2, 3)], |_, _| {})
        .unwrap();
    assert_eq!((some.changed.as_str(), some.marked.as_str()), ("1", "0"));
    let marked: Vec<bool> = rows(&r, &db)
        .iter()
        .map(|e| e.get("marked").is_some())
        .collect();
    assert_eq!(marked, vec![true, false, true]);
    // Replace keeps the medal: it is not part of the game.
    let doc = r
        .load_entry(db.clone(), "1".into(), "g".into())
        .unwrap()
        .document_id
        .unwrap();
    r.replace_game(doc, "0".into(), false).unwrap();
    assert!(rows(&r, &db)[0].get("marked").is_some());
}

#[test]
fn edit_game_data_sets_names_of_the_selection() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = database(&r, d.path());
    let names = |v: serde_json::Value| -> cbh_reader::v2::write::SelectionNames {
        serde_json::from_value(v).unwrap()
    };
    let done = r
        .set_names_of_rows(
            db.clone(),
            vec![range(1, 3)],
            names(serde_json::json!({
                "tournament": ["Wijk aan Zee", "Tata Steel"],
                "annotator": "",
                "source": ["Mega 2025", "ChessBase"]
            })),
            |_, _| {},
        )
        .unwrap();
    assert_eq!(done.changed, "2");
    let e = rows(&r, &db);
    for row in &e[..2] {
        assert_eq!(row["event"], "Tata Steel");
        assert_eq!(row["site"], "Wijk aan Zee");
        assert_eq!(row["source"], "Mega 2025");
        assert!(row.get("annotator").is_none());
    }
    assert_eq!(e[2]["event"], "Club");
    assert_eq!(e[2]["annotator"], "Old, A");
    // The moves are unchanged; only the names were written.
    let game = r.load_entry(db.clone(), "1".into(), "g".into()).unwrap();
    assert_eq!(game.game.unwrap().nodes.len(), 3);
    // Nothing chosen, or a field with a line break, is refused.
    assert!(
        r.set_names_of_rows(
            db.clone(),
            vec![range(1, 2)],
            names(serde_json::json!({})),
            |_, _| {}
        )
        .is_err()
    );
    assert!(
        r.set_names_of_rows(
            db,
            vec![range(1, 2)],
            names(serde_json::json!({ "annotator": "a\nb" })),
            |_, _| {}
        )
        .is_err()
    );
}
