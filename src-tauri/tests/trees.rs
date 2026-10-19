use cbh_reader::tree::ReferenceFilter;
use openfile::{
    registry::Registry,
    trees::{Line, ReferenceSource, TreeStatus},
};

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn line(moves: &[&str]) -> Line {
    Line {
        start_fen: START.into(),
        moves: moves.iter().map(|m| m.to_string()).collect(),
    }
}

/// Saves each PGN game into the database.
fn save_all(r: &Registry, d: &tempfile::TempDir, database_id: &str, pgn: &str) {
    let path = d.path().join("g.pgn");
    std::fs::write(&path, pgn).unwrap();
    let c = r.open_pgn(path, "o".into()).unwrap();
    for i in 0..c.games.len() {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, database_id.into(), "0".into(), false)
            .unwrap();
    }
}

#[test]
fn tree_answers_and_follows_saves() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("data/search-boosters"));
    let db = r
        .create_database(d.path().join("T.2cbh"), "c".into())
        .unwrap();
    assert!(!db.tree);
    let id = db.database_id.clone();
    let mut pgn = String::new();
    for i in 0..6 {
        let (result, reply) = if i % 2 == 0 {
            ("1-0", "e5")
        } else {
            ("0-1", "c5")
        };
        pgn += &format!(
            "[WhiteElo \"2600\"]\n[BlackElo \"2500\"]\n[Date \"20{i:02}.01.01\"]\n[Result \"{result}\"]\n\n1. e4 {reply} 2. Nf3 {result}\n\n"
        );
    }
    pgn += "[Result \"1/2-1/2\"]\n\n1. d4 d5 1/2-1/2\n\n";
    save_all(&r, &d, &id, &pgn);

    let none = r.opening_tree(id.clone(), line(&[]), "t".into()).unwrap();
    assert_eq!(none.status, TreeStatus::NoTree);
    let mut stages = vec![];
    assert!(
        r.create_opening_tree(id.clone(), |stage, done, total| stages
            .push((stage, done, total)))
            .unwrap()
    );
    assert!(!stages.is_empty());
    assert!(r.has_tree(&id).unwrap());
    // The tree lives in the app's data folder, beside the boosters.
    assert_eq!(
        std::fs::read_dir(d.path().join("data/opening-trees"))
            .unwrap()
            .count(),
        1
    );

    let start = r.opening_tree(id.clone(), line(&[]), "t".into()).unwrap();
    assert_eq!(start.status, TreeStatus::Tree);
    assert_eq!((start.games, start.ended), (7, 0));
    let e4 = &start.moves[0];
    assert_eq!((e4.san.as_str(), e4.games), ("e4", 6));
    assert_eq!(e4.score, Some(50.0));
    assert_eq!((e4.average_elo, e4.performance), (Some(2600), Some(2500)));
    assert_eq!((e4.first_year, e4.last_year), (Some(2000), Some(2005)));
    assert!((e4.share - 600.0 / 7.0).abs() < 1e-9);
    let compared = r
        .reference_statistics(
            id.clone(),
            line(&[]),
            "stats".into(),
            ReferenceFilter {
                elo_min: Some(2500),
                year_min: Some(2003),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        (
            compared.full_games,
            compared.filtered_games,
            compared.scanned_games
        ),
        (7, 3, 7)
    );
    assert_eq!((compared.after_elo, compared.after_year), (6, 3));
    assert_eq!(compared.missing_elo, 1);
    assert!(compared.complete && compared.full_complete);
    assert_eq!(
        (
            compared.moves[0].san.as_str(),
            compared.moves[0].full,
            compared.moves[0].filtered
        ),
        ("e4", 6, 3)
    );
    let good = r
        .reference_statistics(
            id.clone(),
            line(&[]),
            "good".into(),
            ReferenceFilter {
                good_games: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(good.filtered_games, 0); // Every synthetic game is under 40 plies.

    // 1.e4 e5 has three games, fewer than the tree stores: followed.
    let e5 = r
        .opening_tree(id.clone(), line(&["e2e4", "e7e5"]), "t".into())
        .unwrap();
    assert_eq!(e5.status, TreeStatus::Line);
    assert_eq!((e5.games, e5.ended), (3, 0));
    assert_eq!(e5.moves[0].san, "Nf3");
    let end = r
        .opening_tree(id.clone(), line(&["e2e4", "e7e5", "g1f3"]), "t".into())
        .unwrap();
    assert_eq!((end.games, end.ended, end.moves.len()), (3, 3, 0));

    // Reference search: from the tree, nodes found page by page.
    let found = r
        .reference_search(id.clone(), line(&["e2e4", "c7c5"]), "r".into())
        .unwrap();
    assert_eq!(
        (found.hits.as_str(), found.source),
        ("3", ReferenceSource::Line)
    );
    let page = r
        .read_reference_page(id.clone(), "1".into(), 10, "p".into())
        .unwrap();
    let nodes: Vec<_> = page
        .entries
        .iter()
        .map(|e| (e.ordinal.as_str(), e.found_node))
        .collect();
    assert_eq!(nodes, [("2", Some(2)), ("4", Some(2)), ("6", Some(2))]);
    // A position off the tree falls back to Find Position.
    let search = r
        .reference_search(
            id.clone(),
            Line {
                start_fen: "8/8/8/4k3/8/8/8/4K3 w - - 0 1".into(),
                moves: vec![],
            },
            "r".into(),
        )
        .unwrap();
    assert_eq!(
        (search.hits.as_str(), search.source),
        ("0", ReferenceSource::Search)
    );
    // On a large database without a search booster, that fallback would
    // read every game, so it runs only when asked for.
    r.set_reference_auto_limit(5);
    let off_tree = Line {
        start_fen: "8/8/8/4k3/8/8/8/4K3 w - - 0 1".into(),
        moves: vec![],
    };
    let skipped = r
        .reference_search(id.clone(), off_tree.clone(), "r".into())
        .unwrap();
    assert_eq!(
        (skipped.hits.as_str(), skipped.source),
        ("0", ReferenceSource::NotSearched)
    );
    let asked = r
        .reference_search_with(id.clone(), off_tree, "r".into(), true)
        .unwrap();
    assert_eq!(asked.source, ReferenceSource::Search);
    // The tree still answers by itself.
    let tree = r
        .reference_search(id.clone(), line(&["e2e4"]), "r".into())
        .unwrap();
    assert_eq!(tree.source, ReferenceSource::Tree);
    r.set_reference_auto_limit(0);

    // A save is recorded in the tree's journal: no rebuild needed.
    save_all(&r, &d, &id, "[Result \"1-0\"]\n\n1. e4 c5 2. Nc3 1-0\n\n");
    assert!(r.has_tree(&id).unwrap());
    let sicilian = r
        .opening_tree(id.clone(), line(&["e2e4", "c7c5"]), "t".into())
        .unwrap();
    let rows: Vec<_> = sicilian
        .moves
        .iter()
        .map(|m| (m.san.as_str(), m.games))
        .collect();
    assert_eq!(rows, [("Nf3", 3), ("Nc3", 1)]);

    r.delete_opening_tree(id.clone()).unwrap();
    assert!(!r.has_tree(&id).unwrap());
}

#[test]
fn filtered_tree_counts_only_matching_games_at_a_transposition() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = r
        .create_database(d.path().join("F.2cbh"), "c".into())
        .unwrap();
    let id = db.database_id;
    save_all(
        &r,
        &d,
        &id,
        "[Result \"1-0\"]\n\n1. Nf3 d5 2. d4 Nf6 3. c4 1-0\n\n[Result \"0-1\"]\n\n1. d4 d5 2. Nf3 Nf6 3. e3 0-1\n\n[Result \"1-0\"]\n\n1. e4 e5 1-0\n\n",
    );
    assert_eq!(
        r.filtered_opening_tree(id.clone(), line(&[]), "missing".into())
            .unwrap_err()
            .code,
        "noSearch"
    );
    r.show_entries(id.clone(), vec!["1".into(), "2".into()], "s".into())
        .unwrap();
    let at = r
        .filtered_opening_tree(
            id.clone(),
            line(&["d2d4", "d7d5", "g1f3", "g8f6"]),
            "f".into(),
        )
        .unwrap();
    assert_eq!((at.games, at.ended), (2, 0));
    assert_eq!((at.sampled_hits, at.total_hits), (Some(2), Some(2)));
    assert_eq!(at.moves.len(), 2);
    let mut moves: Vec<_> = at.moves.iter().map(|m| m.san.as_str()).collect();
    moves.sort_unstable();
    assert_eq!(moves, ["c4", "e3"]);

    r.show_entries(id.clone(), vec![], "s".into()).unwrap();
    let empty = r.filtered_opening_tree(id, line(&[]), "f".into()).unwrap();
    assert_eq!((empty.games, empty.moves.len()), (0, 0));
}

#[test]
fn filtered_tree_reports_when_a_large_result_is_sampled() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = r
        .create_database(d.path().join("L.2cbh"), "c".into())
        .unwrap();
    let id = db.database_id;
    save_all(&r, &d, &id, "[Result \"1-0\"]\n\n1. e4 1-0\n\n");
    // Repeated entry ids make the boundary cheap to set up; the regular
    // search path supplies distinct ids in actual use.
    r.show_entries(id.clone(), vec!["1".into(); 50_001], "s".into())
        .unwrap();
    let response = r.filtered_opening_tree(id, line(&[]), "f".into()).unwrap();
    assert_eq!(
        (response.sampled_hits, response.total_hits),
        (Some(50_000), Some(50_001))
    );
    assert_eq!(response.games, 50_000);
}
