use openfile::registry::Registry;

#[test]
fn searches_page_through_hits_and_clear() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = r
        .create_database(d.path().join("S.2cbh"), "c".into())
        .unwrap();
    let pgn = d.path().join("g.pgn");
    let mut text = String::new();
    for i in 0..5 {
        let white = if i % 2 == 0 {
            "Even, Player"
        } else {
            "Odd, Player"
        };
        text += &format!("[White \"{white}\"]\n[Result \"1-0\"]\n\n1. e4 1-0\n\n");
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
    let filter = serde_json::from_str(r#"{"white":{"last":"even"}}"#).unwrap();
    let found = r
        .search_games(db.database_id.clone(), filter, "s".into(), |_, _| {})
        .unwrap();
    assert_eq!((found.hits.as_str(), found.scanned.as_str()), ("3", "5"));
    let page = r
        .read_search_page(db.database_id.clone(), "1".into(), 2, "p".into())
        .unwrap();
    let ordinals: Vec<_> = page.entries.iter().map(|e| e.ordinal.as_str()).collect();
    assert_eq!(ordinals, ["1", "3"]);
    assert_eq!(page.next.as_deref(), Some("3"));
    let rest = r
        .read_search_page(db.database_id.clone(), "3".into(), 2, "p".into())
        .unwrap();
    assert_eq!(rest.entries[0].ordinal, "5");
    assert_eq!(rest.next, None);
    r.clear_search(db.database_id.clone()).unwrap();
    assert_eq!(
        r.read_search_page(db.database_id, "1".into(), 2, "p".into())
            .unwrap_err()
            .code,
        "noSearch"
    );
}

#[test]
fn position_search_uses_the_booster_and_reports_nodes() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("boosters"));
    let db = r
        .create_database(d.path().join("P.2cbh"), "c".into())
        .unwrap();
    assert!(!db.booster);
    let pgn = d.path().join("g.pgn");
    std::fs::write(
        &pgn,
        "1. e4 e5 2. Nf3 *\n\n1. d4 d5 *\n\n1. Nf3 e5 2. e4 *\n",
    )
    .unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..3 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.database_id.clone(), "0".into(), false)
            .unwrap();
    }
    let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2";
    let plain = r
        .find_position(db.database_id.clone(), fen.into(), "f".into(), |_, _| {})
        .unwrap();
    assert!(!plain.indexed);
    assert_eq!(plain.hits, "2");
    let mut seen = vec![];
    assert!(
        r.create_search_booster(db.database_id.clone(), |done, total| seen
            .push((done, total)))
            .unwrap()
    );
    assert_eq!(seen.last(), Some(&(3, 3)));
    assert!(r.has_booster(&db.database_id).unwrap());
    let fast = r
        .find_position(db.database_id.clone(), fen.into(), "f".into(), |_, _| {})
        .unwrap();
    assert!(fast.indexed);
    assert_eq!(fast.hits, "2");
    let page = r
        .read_search_page(db.database_id.clone(), "1".into(), 10, "p".into())
        .unwrap();
    let found: Vec<_> = page
        .entries
        .iter()
        .map(|e| (e.ordinal.as_str(), e.found_node))
        .collect();
    assert_eq!(found, [("1", Some(3)), ("3", Some(3))]);
    // A pattern from the position tab: a white knight on f3 and a black
    // pawn on e5, in either colour (mirrored). It does not use the booster.
    let query: cbh_reader::pattern::PositionQuery =
        serde_json::from_str(r#"{"lookFor":[["f3","N"],["e5","p"]],"horizontal":true}"#).unwrap();
    let mut progress = vec![];
    let pattern = r
        .search_position(db.database_id.clone(), query, "q".into(), |d, t| {
            progress.push((d, t))
        })
        .unwrap();
    assert!(!pattern.indexed);
    assert_eq!(pattern.hits, "2");
    assert_eq!(progress.last(), Some(&(3, 3)));
    let page = r
        .read_search_page(db.database_id.clone(), "1".into(), 10, "p".into())
        .unwrap();
    let found: Vec<_> = page
        .entries
        .iter()
        .map(|e| (e.ordinal.as_str(), e.found_node))
        .collect();
    assert_eq!(found, [("1", Some(3)), ("3", Some(2))]);
    let empty: cbh_reader::pattern::PositionQuery = serde_json::from_str("{}").unwrap();
    assert_eq!(
        r.search_position(db.database_id.clone(), empty, "q".into(), |_, _| {})
            .unwrap_err()
            .code,
        "invalidQuery"
    );
    r.delete_search_booster(db.database_id.clone()).unwrap();
    assert!(!r.has_booster(&db.database_id).unwrap());
}

fn scoped_database(r: &Registry, dir: &std::path::Path) -> String {
    r.set_index_dir(dir.join("boosters"));
    let path = dir.join("Scope.2cbh");
    let games = chess_core::parse_pgn(b"[White \"Z\"]\n[Result \"1-0\"]\n\n1. e4 e5 1-0\n\n[White \"A\"]\n[Result \"0-1\"]\n\n1. d4 d5 0-1\n\n[White \"M\"]\n[Result \"1-0\"]\n\n1. e4 c5 1-0", &chess_core::GameLimits::default()).unwrap();
    let mut w = cbh_reader::v2::write::Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    r.open_database(path, "o".into()).unwrap().database_id
}
fn ordinals(r: &Registry, db: &str) -> Vec<String> {
    r.read_search_page(db.into(), "1".into(), 50, "p".into())
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.ordinal)
        .collect()
}

#[test]
fn scoped_search_intersects_unions_deduplicates_and_preserves_sort() {
    use openfile::searches::SearchScope;
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = scoped_database(&r, d.path());
    let f = || serde_json::from_str(r#"{"results":["1-0"]}"#).unwrap();
    assert_eq!(
        r.search_games_scoped(db.clone(), f(), SearchScope::Within, "s".into(), |_, _| {})
            .unwrap_err()
            .code,
        "noSearch"
    );
    r.show_entries(
        db.clone(),
        vec!["3".into(), "2".into(), "3".into()],
        "s".into(),
    )
    .unwrap();
    let mut progress = vec![];
    let found = r
        .search_games_scoped(db.clone(), f(), SearchScope::Within, "s".into(), |d, t| {
            progress.push((d, t))
        })
        .unwrap();
    assert_eq!(found.scanned, "2");
    assert_eq!(ordinals(&r, &db), ["3"]);
    assert_eq!(progress.last(), Some(&(2, 2)));
    r.sort_list(
        db.clone(),
        Some(cbh_reader::sort::SortColumn::White),
        false,
        |_, _, _| {},
    )
    .unwrap();
    let black = serde_json::from_str(r#"{"results":["0-1"]}"#).unwrap();
    r.search_games_scoped(db.clone(), black, SearchScope::Add, "s".into(), |_, _| {})
        .unwrap();
    assert_eq!(ordinals(&r, &db), ["2", "3"]);
    let rows = r.read_rows(db.clone(), "1".into(), 50, "p".into()).unwrap();
    assert_eq!(
        rows.entries
            .iter()
            .map(|e| e.ordinal.as_str())
            .collect::<Vec<_>>(),
        ["2", "3"]
    );
    r.search_games_scoped(db.clone(), f(), SearchScope::Add, "s".into(), |_, _| {})
        .unwrap();
    assert_eq!(ordinals(&r, &db), ["1", "2", "3"]);
    let rows = r.read_rows(db.clone(), "1".into(), 50, "p".into()).unwrap();
    assert_eq!(
        rows.entries
            .iter()
            .map(|e| e.ordinal.as_str())
            .collect::<Vec<_>>(),
        ["2", "3", "1"]
    );
}

#[test]
fn empty_refinement_cancel_errors_and_reset_leave_no_partial_publication() {
    use openfile::searches::SearchScope;
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = scoped_database(&r, d.path());
    r.show_entries(db.clone(), vec![], "s".into()).unwrap();
    let f = || cbh_reader::search::GameFilter::default();
    let found = r
        .search_games_scoped(db.clone(), f(), SearchScope::Within, "s".into(), |_, _| {})
        .unwrap();
    assert_eq!(found.hits, "0");
    assert_eq!(found.scanned, "0");
    r.show_entries(db.clone(), vec!["2".into()], "s".into())
        .unwrap();
    let bad = serde_json::from_str(r#"{"material":{"length":0}}"#).unwrap();
    assert!(
        r.search_games_scoped(db.clone(), bad, SearchScope::All, "s".into(), |_, _| {})
            .is_err()
    );
    assert_eq!(ordinals(&r, &db), ["2"]);
    let cancelled = r
        .search_games_scoped(db.clone(), f(), SearchScope::Add, "s".into(), |_, _| {
            r.cancel_search(db.clone()).unwrap();
        })
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(ordinals(&r, &db), ["2"]);
    let reset = r
        .search_games_scoped(db.clone(), f(), SearchScope::Within, "s".into(), |_, _| {
            r.clear_search(db.clone()).unwrap();
        })
        .unwrap();
    assert!(reset.cancelled);
    assert_eq!(
        r.read_search_page(db, "1".into(), 50, "p".into())
            .unwrap_err()
            .code,
        "noSearch"
    );
}

#[test]
fn position_scopes_keep_hit_nodes_and_header_refinement_preserves_them() {
    use openfile::searches::SearchScope;
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = scoped_database(&r, d.path());
    r.show_entries(db.clone(), vec!["2".into()], "s".into())
        .unwrap();
    let q = || serde_json::from_str(r#"{"lookFor":[["e4","P"]]}"#).unwrap();
    r.search_position_scoped(db.clone(), q(), SearchScope::Add, "s".into(), |_, _| {})
        .unwrap();
    let page = r
        .read_search_page(db.clone(), "1".into(), 10, "p".into())
        .unwrap();
    assert_eq!(
        page.entries
            .iter()
            .map(|e| (e.ordinal.as_str(), e.found_node))
            .collect::<Vec<_>>(),
        [("1", Some(1)), ("2", Some(0)), ("3", Some(1))]
    );
    let f = serde_json::from_str(r#"{"results":["1-0"]}"#).unwrap();
    r.search_games_scoped(db.clone(), f, SearchScope::Within, "s".into(), |_, _| {})
        .unwrap();
    let page = r
        .read_search_page(db.clone(), "1".into(), 10, "p".into())
        .unwrap();
    assert_eq!(
        page.entries
            .iter()
            .map(|e| (e.ordinal.as_str(), e.found_node))
            .collect::<Vec<_>>(),
        [("1", Some(1)), ("3", Some(1))]
    );
    let mut progress = vec![];
    let found = r
        .search_position_scoped(db.clone(), q(), SearchScope::Within, "s".into(), |d, t| {
            progress.push((d, t))
        })
        .unwrap();
    assert_eq!(found.scanned, "2");
    assert_eq!(progress.last(), Some(&(2, 2)));
}

#[test]
fn superseded_search_and_closed_database_never_publish() {
    use openfile::searches::SearchScope;
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = scoped_database(&r, d.path());
    let mut replaced = false;
    let result = r
        .search_games_scoped(
            db.clone(),
            Default::default(),
            SearchScope::All,
            "old".into(),
            |_, _| {
                if !replaced {
                    replaced = true;
                    let f = serde_json::from_str(r#"{"results":["0-1"]}"#).unwrap();
                    r.search_games(db.clone(), f, "new".into(), |_, _| {})
                        .unwrap();
                }
            },
        )
        .unwrap();
    assert!(result.cancelled);
    assert_eq!(ordinals(&r, &db), ["2"]);
    let mut closed = false;
    let result = r
        .search_games_scoped(
            db.clone(),
            Default::default(),
            SearchScope::Within,
            "old".into(),
            |_, _| {
                if !closed {
                    closed = true;
                    r.close_database(db.clone()).unwrap();
                }
            },
        )
        .unwrap();
    assert!(result.cancelled);
}

#[test]
fn cancelled_booster_cannot_remove_the_newer_search_token() {
    use std::sync::mpsc::channel;
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = scoped_database(&r, d.path());
    let (boost_ready_tx, boost_ready_rx) = channel();
    let (boost_go_tx, boost_go_rx) = channel();
    let (search_ready_tx, search_ready_rx) = channel();
    let (search_go_tx, search_go_rx) = channel();
    std::thread::scope(|scope| {
        let registry = &r;
        let id = db.clone();
        let booster = scope.spawn(move || {
            let mut first = Some(boost_ready_tx);
            registry.create_search_booster(id, |_, _| {
                if let Some(tx) = first.take() {
                    tx.send(()).unwrap();
                    boost_go_rx.recv().unwrap();
                }
            })
        });
        boost_ready_rx.recv().unwrap();
        let registry = &r;
        let id = db.clone();
        let search = scope.spawn(move || {
            let mut first = Some(search_ready_tx);
            registry.search_games(id, Default::default(), "new".into(), |_, _| {
                if let Some(tx) = first.take() {
                    tx.send(()).unwrap();
                    search_go_rx.recv().unwrap();
                }
            })
        });
        search_ready_rx.recv().unwrap();
        boost_go_tx.send(()).unwrap();
        booster.join().unwrap().unwrap();
        search_go_tx.send(()).unwrap();
        assert!(!search.join().unwrap().unwrap().cancelled);
    });
    assert_eq!(ordinals(&r, &db), ["1", "2", "3"]);
}
