use chess_workbench::registry::Registry;

/// A 2CBH database with five games by White's name: Echo, Delta, Charlie,
/// Bravo, Alpha, in entry order; the odd entries are wins.
fn database(r: &Registry, dir: &std::path::Path) -> String {
    r.set_index_dir(dir.join("search-boosters"));
    let db = r.create_database(dir.join("L.2cbh"), "c".into()).unwrap();
    let pgn = dir.join("g.pgn");
    let mut text = String::new();
    for (i, white) in ["Echo", "Delta", "Charlie", "Bravo", "Alpha"]
        .iter()
        .enumerate()
    {
        let result = if i % 2 == 0 { "1-0" } else { "0-1" };
        text += &format!(
            "[White \"{white}, Player\"]\n[Black \"Other, One\"]\n[Result \"{result}\"]\n\n1. e4 {result}\n\n"
        );
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
    db.database_id
}

fn rows(r: &Registry, db: &str, start: &str, limit: usize) -> Vec<String> {
    r.read_rows(db.into(), start.into(), limit, "r".into())
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.ordinal)
        .collect()
}

#[test]
fn rows_follow_the_search_and_the_sort() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = database(&r, d.path());
    assert_eq!(rows(&r, &db, "1", 200), ["1", "2", "3", "4", "5"]);
    assert_eq!(rows(&r, &db, "4", 200), ["4", "5"]);
    assert_eq!(
        r.find_row(db.clone(), "4".into()).unwrap().as_deref(),
        Some("4")
    );

    let sorted = r
        .sort_list(
            db.clone(),
            Some(cbh_reader::sort::SortColumn::White),
            false,
            |_, _, _| {},
        )
        .unwrap();
    assert_eq!((sorted.rows.as_str(), sorted.cancelled), ("5", false));
    assert_eq!(rows(&r, &db, "1", 200), ["5", "4", "3", "2", "1"]);
    let page = r.read_rows(db.clone(), "2".into(), 2, "p".into()).unwrap();
    assert_eq!(page.next.as_deref(), Some("4"));
    assert_eq!(
        r.find_row(db.clone(), "5".into()).unwrap().as_deref(),
        Some("1")
    );
    // The sort order is the app's own file.
    let orders = d.path().join("sort-orders");
    assert_eq!(std::fs::read_dir(&orders).unwrap().count(), 1);

    // A search is shown in the sort's order.
    let filter = serde_json::from_str(r#"{"results":["1-0"]}"#).unwrap();
    let found = r.search_games(db.clone(), filter, "s".into()).unwrap();
    assert_eq!(found.hits, "3");
    assert_eq!(rows(&r, &db, "1", 200), ["5", "3", "1"]);
    assert_eq!(
        r.find_row(db.clone(), "1".into()).unwrap().as_deref(),
        Some("3")
    );
    assert_eq!(r.find_row(db.clone(), "2".into()).unwrap(), None);

    // Descending, then entry order again.
    r.sort_list(
        db.clone(),
        Some(cbh_reader::sort::SortColumn::White),
        true,
        |_, _, _| {},
    )
    .unwrap();
    assert_eq!(rows(&r, &db, "1", 200), ["1", "3", "5"]);
    r.clear_search(db.clone()).unwrap();
    assert_eq!(rows(&r, &db, "1", 200), ["1", "2", "3", "4", "5"]);
    let back = r.sort_list(db.clone(), None, false, |_, _, _| {}).unwrap();
    assert_eq!(back.column, None);
    assert_eq!(rows(&r, &db, "1", 3), ["1", "2", "3"]);
}

#[test]
fn saving_drops_the_sort_and_bounds_are_checked() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = database(&r, d.path());
    r.sort_list(
        db.clone(),
        Some(cbh_reader::sort::SortColumn::White),
        false,
        |_, _, _| {},
    )
    .unwrap();
    r.set_deleted(db.clone(), "2".into(), true).unwrap();
    // The changed database is shown in entry order until it is sorted again.
    assert_eq!(rows(&r, &db, "1", 200), ["1", "2", "3", "4", "5"]);
    let again = r
        .sort_list(
            db.clone(),
            Some(cbh_reader::sort::SortColumn::White),
            false,
            |_, _, _| {},
        )
        .unwrap();
    assert_eq!(again.rows, "5");
    assert_eq!(rows(&r, &db, "1", 200), ["5", "4", "3", "2", "1"]);
    for (start, limit) in [("0", 1), ("1", 0), ("1", 201)] {
        assert_eq!(
            r.read_rows(db.clone(), start.into(), limit, "r".into())
                .unwrap_err()
                .code,
            "bounds"
        );
    }
    assert!(r.read_rows(db.clone(), "6".into(), 1, "r".into()).is_err());
    r.close_database(db.clone()).unwrap();
    assert!(r.read_rows(db, "1".into(), 1, "r".into()).is_err());
}
