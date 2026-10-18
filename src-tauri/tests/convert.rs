use openfile::{clipboard::RowRange, registry::Registry};

fn range(start: u64, end: u64) -> RowRange {
    serde_json::from_value(serde_json::json!({
        "start": start.to_string(),
        "end": end.to_string()
    }))
    .unwrap()
}

/// A database with four games by White's name; Charlie's is deleted.
fn source(r: &Registry, dir: &std::path::Path) -> String {
    let db = r.create_database(dir.join("A.2cbh"), "c".into()).unwrap();
    let pgn = dir.join("g.pgn");
    let mut text = String::new();
    for white in ["Echo", "Delta", "Charlie", "Bravo"] {
        text += &format!(
            "[White \"{white}, P\"]\n[Black \"Other, O\"]\n[Result \"1-0\"]\n\n1. e4 $1 {{A comment}} e5 1-0\n\n"
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
    r.set_deleted(db.database_id.clone(), "3".into(), true)
        .unwrap();
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

fn stamps(dir: &std::path::Path) -> Vec<(String, u64, std::time::SystemTime)> {
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.file_name().to_string_lossy().starts_with("A."))
        .map(|e| {
            let m = e.metadata().unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                m.len(),
                m.modified().unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn converts_a_database_or_a_selection_into_a_new_2cbh() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("indexes"));
    let a = source(&r, d.path());
    let before = stamps(d.path());

    let estimate = r
        .convert_estimate(a.clone(), None, d.path().join("All.2cbh"))
        .unwrap();
    assert_eq!(estimate.games, "4");
    assert!(estimate.bytes.parse::<u64>().unwrap() > 4 * 192);
    assert!(estimate.free.is_some());

    let mut seen = vec![];
    let all = r
        .convert_database(a.clone(), None, d.path().join("All"), |done, total| {
            seen.push((done, total))
        })
        .unwrap();
    assert_eq!(
        (all.report.written, all.report.deleted, all.report.cancelled),
        (4, 1, false)
    );
    assert_eq!(seen.first(), Some(&(0, 4)));
    assert_eq!(seen.last(), Some(&(4, 4)));
    let path = all.path.unwrap();
    assert!(path.ends_with("All.2cbh"));
    // No temporary files remain, and the source is untouched.
    assert!(!d.path().join("All.converting.2cbh").exists());
    assert_eq!(stamps(d.path()), before);

    let b = r
        .open_database_with(path.into(), "o".into(), false)
        .unwrap()
        .database_id;
    assert_eq!(
        rows(&r, &b),
        vec![
            ("Echo, P".into(), false),
            ("Delta, P".into(), false),
            ("Charlie, P".into(), true),
            ("Bravo, P".into(), false),
        ]
    );
    let game = r.load_entry(b.clone(), "1".into(), "g".into()).unwrap();
    let e4 = &game.game.as_ref().unwrap().nodes[1];
    assert_eq!((e4.nags.as_slice(), e4.comments.len()), (&[1u16][..], 1));

    // Rows 2–3 of the list: Delta and Charlie.
    let part = r
        .convert_database(
            a.clone(),
            Some(vec![range(2, 4)]),
            d.path().join("Part.2cbh"),
            |_, _| {},
        )
        .unwrap();
    assert_eq!(part.report.written, 2);
    let p = r
        .open_database_with(part.path.unwrap().into(), "o".into(), false)
        .unwrap()
        .database_id;
    assert_eq!(rows(&r, &p).len(), 2);

    // An existing family, or the source itself, is never replaced.
    for target in ["All.2cbh", "A.2cbh"] {
        assert!(
            r.convert_database(a.clone(), None, d.path().join(target), |_, _| {})
                .is_err()
        );
    }
    std::fs::write(d.path().join("Index.2lcd"), b"x").unwrap();
    assert_eq!(
        r.convert_database(a.clone(), None, d.path().join("Index.2cbh"), |_, _| {})
            .unwrap_err()
            .code,
        "exists"
    );
    assert_eq!(stamps(d.path()), before);
}
