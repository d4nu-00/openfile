use openfile::registry::Registry;
use std::sync::atomic::AtomicBool;

fn document(r: &Registry, d: &tempfile::TempDir, name: &str, pgn: &str) -> String {
    let path = d.path().join(format!("{name}.pgn"));
    std::fs::write(&path, pgn).unwrap();
    let c = r.open_pgn(path, name.into()).unwrap();
    r.load_pgn_game(c.collection_id, 0, name.into())
        .unwrap()
        .document_id
}

#[test]
fn the_first_move_no_other_game_reaches_is_the_novelty() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let db = r
        .create_database(d.path().join("Ref"), "create".into())
        .unwrap()
        .database_id;
    for (i, pgn) in [
        "[White \"A\"]\n[Black \"B\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 *\n",
        "[White \"C\"]\n[Black \"D\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 *\n",
    ]
    .iter()
    .enumerate()
    {
        let doc = document(&r, &d, &format!("g{i}"), pgn);
        r.save_game(doc.clone(), db.clone(), "0".into(), false)
            .unwrap();
        r.close_document(doc).unwrap();
    }
    let new = document(&r, &d, "new", "1. e4 e5 2. Nf3 Nf6 3. Nxe5 *\n");
    let report = r.find_novelty(&new, &db, &AtomicBool::new(false)).unwrap();
    // 2...Nf6 (ply 4) is new; both games reached 2.Nf3.
    let novelty = report.novelty.unwrap();
    assert_eq!(novelty.ply, 4);
    assert_eq!(report.known_ply, 3);
    assert_eq!(report.total, 2);
    assert_eq!(report.games.len(), 2);
    // One question per ply up to the novelty.
    assert_eq!(report.queries, 5);

    // A game of the database: its own entry does not count.
    let own = r
        .load_entry(db.clone(), "1".into(), "own".into())
        .unwrap()
        .document_id
        .unwrap();
    let report = r.find_novelty(&own, &db, &AtomicBool::new(false)).unwrap();
    assert_eq!(report.novelty.unwrap().ply, 5);
    assert_eq!(report.total, 1);

    // A line every move of which another game played has no novelty.
    let known = document(&r, &d, "known", "1. e4 e5 2. Nf3 Nc6 *\n");
    let report = r
        .find_novelty(&known, &db, &AtomicBool::new(false))
        .unwrap();
    assert!(report.novelty.is_none());
    assert_eq!(report.known_ply, 4);

    // Stopped at once: nothing is found.
    let stopped = r.find_novelty(&new, &db, &AtomicBool::new(true)).unwrap();
    assert!(stopped.cancelled && stopped.novelty.is_none());
}
