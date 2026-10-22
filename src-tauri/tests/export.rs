use openfile::{clipboard::RowRange, registry::Registry};

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

fn files(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn whites(text: &str) -> Vec<String> {
    chess_core::parse_pgn(text.as_bytes(), &chess_core::GameLimits::default())
        .unwrap()
        .iter()
        .map(|g| g.tags["White"].clone())
        .collect()
}

#[test]
fn exports_the_selected_rows_in_list_order() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let a = source(&r, d.path());
    // Rows 2-3 of the list sorted by White: Charlie, Delta.
    r.sort_list(
        a.clone(),
        Some(cbh_reader::sort::SortColumn::White),
        false,
        |_, _, _| {},
    )
    .unwrap();
    let out = d.path().join("out.pgn");
    let before = files(d.path());
    let mut seen = vec![];
    let done = r
        .export_rows_pgn(a.clone(), vec![range(2, 4)], out.clone(), |done, total| {
            seen.push((done, total))
        })
        .unwrap();
    assert_eq!(
        (
            done.written.as_str(),
            done.skipped.as_str(),
            done.stripped.as_str(),
            done.cancelled
        ),
        ("2", "0", "0", false)
    );
    assert_eq!(done.destination, out.to_string_lossy());
    assert_eq!(seen.last(), Some(&(2, 2)));
    assert_eq!(
        whites(&std::fs::read_to_string(&out).unwrap()),
        ["Charlie, P", "Delta, P"]
    );
    // Only the target appeared beside the database.
    let mut expected = before;
    expected.push("out.pgn".into());
    expected.sort();
    assert_eq!(files(d.path()), expected);
}

#[test]
fn refuses_database_files_and_empty_selections() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let a = source(&r, d.path());
    let code = |e: openfile::commands::AppError| e.code;
    let to = d.path().join("A.2cbh");
    let before = std::fs::read(&to).unwrap();
    assert_eq!(
        code(
            r.export_rows_pgn(a.clone(), vec![range(1, 3)], to.clone(), |_, _| {})
                .unwrap_err()
        ),
        "sourceDestination"
    );
    assert_eq!(std::fs::read(&to).unwrap(), before);
    assert_eq!(
        code(
            r.export_rows_pgn(
                a.clone(),
                vec![range(0, 2)],
                d.path().join("x.pgn"),
                |_, _| {}
            )
            .unwrap_err()
        ),
        "bounds"
    );
    assert!(!d.path().join("x.pgn").exists());
}

#[test]
fn a_cancelled_export_leaves_no_file() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let a = source(&r, d.path());
    let out = d.path().join("out.pgn");
    let before = files(d.path());
    let done = r
        .export_rows_pgn(a.clone(), vec![range(1, 5)], out.clone(), |_, _| {
            r.cancel_export(a.clone()).unwrap();
        })
        .unwrap();
    assert!(done.cancelled);
    assert_eq!(done.written, "1");
    assert!(!out.exists());
    assert_eq!(files(d.path()), before);
}
