use chess_workbench::{clipboard::RowRange, registry::Registry};

fn range(start: u64, end: u64) -> RowRange {
    serde_json::from_value(serde_json::json!({
        "start": start.to_string(),
        "end": end.to_string()
    }))
    .unwrap()
}

/// A database with games by White's name: Echo, Delta, Charlie, Bravo.
fn source(r: &Registry, dir: &std::path::Path) -> String {
    let db = r.create_database(dir.join("A.2cbh"), "c".into()).unwrap();
    let pgn = dir.join("g.pgn");
    let mut text = String::new();
    for white in ["Echo", "Delta", "Charlie", "Bravo"] {
        text += &format!(
            "[White \"{white}, P\"]\n[Black \"Other, O\"]\n[Result \"1-0\"]\n\n1. e4 {{A comment}} e5 1-0\n\n"
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

fn whites(r: &Registry, db: &str) -> Vec<String> {
    r.read_rows(db.into(), "1".into(), 200, "r".into())
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.white.unwrap_or_default())
        .collect()
}

#[test]
fn copies_rows_of_the_list_and_pastes_them_into_another_database() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let a = source(&r, d.path());
    let b = r
        .create_database(d.path().join("B.2cbh"), "c".into())
        .unwrap();
    assert_eq!(b.name, "B");
    let b = b.database_id;
    assert_eq!(
        r.paste_games(b.clone(), |_, _| {}).unwrap_err().code,
        "emptyClip"
    );

    // Rows 2–3 of the list sorted by White: Charlie, Delta.
    r.sort_list(
        a.clone(),
        Some(cbh_reader::sort::SortColumn::White),
        false,
        |_, _, _| {},
    )
    .unwrap();
    let copied = r.copy_rows(a.clone(), vec![range(2, 4)]).unwrap();
    assert_eq!((copied.games.as_str(), copied.truncated), ("2", false));
    assert_eq!(r.clip_size().unwrap(), "2");
    let mut seen = vec![];
    let pasted = r
        .paste_games(b.clone(), |done, total| seen.push((done, total)))
        .unwrap();
    assert_eq!(
        (
            pasted.pasted.as_str(),
            pasted.skipped.as_str(),
            pasted.incomplete.as_str(),
            pasted.entry_count.as_str(),
            pasted.cancelled
        ),
        ("2", "0", "0", "2", false)
    );
    assert_eq!(seen.last(), Some(&(2, 2)));
    assert_eq!(whites(&r, &b), ["Charlie, P", "Delta, P"]);
    // Moves and the comment came along.
    let doc = r.load_entry(b.clone(), "1".into(), "l".into()).unwrap();
    let game = doc.game.unwrap();
    assert_eq!(game.nodes.len(), 3);
    assert!(
        game.nodes
            .iter()
            .any(|n| n.comments.iter().any(|c| c.contains("A comment")))
    );

    // Pasting into a read-only database or the source itself is refused.
    assert_eq!(
        r.paste_games(a.clone(), |_, _| {}).unwrap_err().code,
        "sameDatabase"
    );
    r.copy_rows(b.clone(), vec![range(1, 3)]).unwrap();
    assert_eq!(
        r.paste_games(b.clone(), |_, _| {}).unwrap_err().code,
        "sameDatabase"
    );
}

#[test]
fn delete_marks_the_selection_and_undeletes_it_when_all_are_deleted() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let a = source(&r, d.path());
    assert_eq!(r.delete_rows(a.clone(), vec![range(1, 3)]).unwrap(), "2");
    let deleted = |r: &Registry| -> Vec<bool> {
        r.read_rows(a.clone(), "1".into(), 200, "r".into())
            .unwrap()
            .entries
            .iter()
            .map(|e| e.deleted)
            .collect()
    };
    assert_eq!(deleted(&r), [true, true, false, false]);
    // A mixed selection is deleted; an all-deleted one is undeleted.
    assert_eq!(r.delete_rows(a.clone(), vec![range(2, 4)]).unwrap(), "1");
    assert_eq!(deleted(&r), [true, true, true, false]);
    assert_eq!(
        r.delete_rows(a.clone(), vec![range(1, 2), range(3, 4)])
            .unwrap(),
        "2"
    );
    assert_eq!(deleted(&r), [false, true, false, false]);
    assert_eq!(
        r.delete_rows(a, vec![range(0, 2)]).unwrap_err().code,
        "bounds"
    );
}

#[test]
fn a_paste_keeps_the_target_opening_tree_current() {
    use chess_workbench::trees::{Line, TreeStatus};
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("data/search-boosters"));
    let a = source(&r, d.path());
    // The target: six games of 1.d4 d5 with a tree.
    let b = r
        .create_database(d.path().join("B.2cbh"), "c".into())
        .unwrap()
        .database_id;
    let pgn = d.path().join("d4.pgn");
    std::fs::write(
        &pgn,
        "[Result \"1/2-1/2\"]\n\n1. d4 d5 1/2-1/2\n\n".repeat(6),
    )
    .unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..6 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, b.clone(), "0".into(), false).unwrap();
    }
    assert!(r.create_opening_tree(b.clone(), |_, _, _| {}).unwrap());
    let start = || {
        r.opening_tree(
            b.clone(),
            Line {
                start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".into(),
                moves: vec![],
            },
            "t".into(),
        )
        .unwrap()
    };
    assert_eq!(start().games, 6);
    // Paste the source's four 1.e4 games: the tree counts them at once.
    r.copy_rows(a, vec![range(1, 5)]).unwrap();
    let pasted = r.paste_games(b.clone(), |_, _| {}).unwrap();
    assert_eq!(pasted.pasted, "4");
    assert!(!pasted.tree_stale);
    let after = start();
    assert_eq!(after.status, TreeStatus::Tree);
    assert_eq!(after.games, 10);
    let e4 = after.moves.iter().find(|m| m.san == "e4").unwrap();
    assert_eq!(e4.games, 4);
}
