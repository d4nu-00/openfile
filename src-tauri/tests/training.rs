use openfile::registry::Registry;

fn document(r: &Registry, d: &tempfile::TempDir, pgn: &str) -> String {
    let path = d.path().join("g.pgn");
    std::fs::write(&path, pgn).unwrap();
    let c = r.open_pgn(path, "o".into()).unwrap();
    r.load_pgn_game(c.collection_id, 0, "l".into())
        .unwrap()
        .document_id
}

fn edit(r: &Registry, doc: &str, json: serde_json::Value, revision: &str) -> serde_json::Value {
    let response = r
        .edit_document(
            doc.into(),
            serde_json::from_value(json).unwrap(),
            revision.into(),
        )
        .unwrap();
    serde_json::to_value(&response).unwrap()
}

#[test]
fn training_questions_are_edited_saved_and_cleared() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let doc = document(&r, &d, "1. e4 e5 2. Nf3 *\n");
    // A question on 2.Nf3 (the position after 1...e5), with 2.Nf3 and
    // 2.Bc4 as answers.
    let question = serde_json::json!({
        "kind": "setTraining",
        "node": 3,
        "training": {
            "kind": "moveInput",
            "seconds": 120,
            "score": 10,
            "question": {"texts": [{"language": 0, "text": "White to play"}]},
            "help": [{"texts": [{"language": 0, "text": "Develop"}]}, {}],
            "answers": [
                {"points": 10, "feedback": {"texts": [{"language": 0, "text": "Good"}]}},
                {"points": 5}
            ]
        },
        "moves": ["Nf3", "f1c4"]
    });
    let view = edit(&r, &doc, question, "0");
    let node = &view["game"]["nodes"][3];
    assert_eq!(node["training"]["kind"], "moveInput");
    assert_eq!(
        node["training"]["question"]["texts"][0]["text"],
        "White to play"
    );
    assert_eq!(node["trainingMoves"], serde_json::json!(["Nf3", "Bc4"]));
    // An illegal answer is refused and changes nothing.
    let bad = serde_json::json!({
        "kind": "setTraining",
        "node": 3,
        "training": {"kind": "moveInput", "seconds": 0, "score": 1,
            "question": {"texts": [{"language": 0, "text": "?"}]}, "answers": [{"points": 1}]},
        "moves": ["Ke3"]
    });
    assert!(
        r.edit_document(
            doc.clone(),
            serde_json::from_value(bad).unwrap(),
            "1".into()
        )
        .is_err()
    );

    // Saved into a 2CBH and read back, the question is the same.
    let db = r
        .create_database(d.path().join("T.2cbh"), "c".into())
        .unwrap()
        .database_id;
    r.save_game(doc.clone(), db.clone(), "1".into(), false)
        .unwrap();
    let loaded = r.load_entry(db.clone(), "1".into(), "l".into()).unwrap();
    let back = serde_json::to_value(&loaded).unwrap();
    assert_eq!(back["annotationsStatus"], "available");
    assert_eq!(
        back["game"]["nodes"][3]["training"],
        view["game"]["nodes"][3]["training"]
    );
    assert_eq!(
        back["game"]["nodes"][3]["trainingMoves"],
        serde_json::json!(["Nf3", "Bc4"])
    );

    // PGN cannot hold it: the export asks first.
    let out = d.path().join("out.pgn");
    let refused = r
        .save_document(doc.clone(), out.clone(), "1".into(), false)
        .unwrap_err();
    assert_eq!(refused.code, "incompleteAnnotations");

    // Delete Training Commentary removes it, as one undoable edit.
    let cleared = edit(&r, &doc, serde_json::json!({"kind": "clearTraining"}), "1");
    assert!(cleared["game"]["nodes"][3]["training"].is_null());
    let undone = edit(&r, &doc, serde_json::json!({"kind": "undo"}), "2");
    assert_eq!(undone["game"]["nodes"][3]["training"]["score"], 10);
}
