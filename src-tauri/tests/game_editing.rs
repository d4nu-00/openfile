use chess_workbench::{documents::Edit, registry::Registry};

const PGN: &str =
    "[White \"A\"]\n[Black \"B\"]\n[Result \"*\"]\n\n1. e4 e5 (1... c5 2. Nf3) 2. Nf3 Nc6 *\n";

fn sans(r: &Registry, id: &str) -> String {
    let pgn = r.document_pgn(id.into()).unwrap();
    pgn.split("\n\n").nth(1).unwrap().trim().to_string()
}

#[test]
fn structural_edits_select_and_undo() {
    let r = Registry::default();
    let doc = r.paste_game(PGN.into(), "p".into()).unwrap();
    let id = doc.document_id.clone();
    let c5 = doc
        .game
        .nodes
        .iter()
        .find(|n| n.san.as_deref() == Some("c5"))
        .unwrap()
        .id;

    let promoted = r
        .edit_document(id.clone(), Edit::PromoteVariation { node: c5 }, "0".into())
        .unwrap();
    assert_eq!(promoted.selected, Some(c5));
    assert_eq!(sans(&r, &id), "1. e4 c5 (1... e5 2. Nf3 Nc6) 2. Nf3 *");

    let e5 = promoted
        .game
        .nodes
        .iter()
        .find(|n| n.san.as_deref() == Some("e5"))
        .unwrap()
        .id;
    let deleted = r
        .edit_document(id.clone(), Edit::DeleteVariation { node: e5 }, "1".into())
        .unwrap();
    assert_eq!(deleted.selected, Some(1));
    assert_eq!(sans(&r, &id), "1. e4 c5 2. Nf3 *");
    assert!(deleted.dirty);

    // Undo twice brings the original back; redo repeats the promotion.
    r.edit_document(id.clone(), Edit::Undo, "2".into()).unwrap();
    let back = r.edit_document(id.clone(), Edit::Undo, "3".into()).unwrap();
    assert_eq!(sans(&r, &id), "1. e4 e5 (1... c5 2. Nf3) 2. Nf3 Nc6 *");
    assert_eq!(back.revision, "4");
    r.edit_document(id.clone(), Edit::Redo, "4".into()).unwrap();
    assert_eq!(sans(&r, &id), "1. e4 c5 (1... e5 2. Nf3 Nc6) 2. Nf3 *");
    // A new edit clears what could be redone.
    r.edit_document(id.clone(), Edit::DeleteRemaining { node: 1 }, "5".into())
        .unwrap();
    assert_eq!(sans(&r, &id), "1. e4 *");
    let same = r.edit_document(id.clone(), Edit::Redo, "6".into()).unwrap();
    assert_eq!(same.revision, "6");
}

#[test]
fn new_games_game_data_and_copy() {
    let r = Registry::default();
    let doc = r.new_document(None, "n".into()).unwrap();
    assert_eq!(doc.game.nodes.len(), 1);
    let id = doc.document_id;
    let tags = [
        ("White", "Kasparov, Garry"),
        ("Result", "1-0"),
        ("WhiteElo", "2851"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let edited = r
        .edit_document(id.clone(), Edit::SetTags { tags }, "0".into())
        .unwrap();
    assert_eq!(edited.game.tags["White"], "Kasparov, Garry");
    assert!(
        r.document_pgn(id.clone())
            .unwrap()
            .contains("[WhiteElo \"2851\"]")
    );
    let bad = [("Result", "3-0")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    assert!(
        r.edit_document(id, Edit::SetTags { tags: bad }, "1".into())
            .is_err()
    );

    let setup = r
        .new_document(Some("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1".into()), "s".into())
        .unwrap();
    assert_eq!(setup.game.start_fen, "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
    assert!(
        r.new_document(Some("8/8/8/8/8/8/8/8 w - - 0 1".into()), "s".into())
            .is_err()
    );
    assert_eq!(
        r.paste_game("not a game".into(), "p".into())
            .unwrap_err()
            .code,
        "invalidPgn"
    );
}
