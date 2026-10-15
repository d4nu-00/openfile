use chess_workbench::{registry::Registry, trees::Line};

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// A new 2CBH database holding `games` (PGN texts); returns its id.
fn database(r: &Registry, d: &tempfile::TempDir, games: &[String]) -> String {
    let path = d.path().join("all.pgn");
    std::fs::write(&path, games.join("\n\n")).unwrap();
    let c = r.open_pgn(path, "o".into()).unwrap();
    let db = r
        .create_database(d.path().join("Report"), "create".into())
        .unwrap();
    for i in 0..games.len() {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, format!("l{i}"))
            .unwrap()
            .document_id;
        r.save_game(doc.clone(), db.database_id.clone(), "0".into(), false)
            .unwrap();
        r.close_document(doc).unwrap();
    }
    db.database_id
}

fn game(
    white: &str,
    welo: u16,
    black: &str,
    belo: u16,
    year: u16,
    result: &str,
    moves: &str,
) -> String {
    format!(
        "[White \"{white}\"]\n[Black \"{black}\"]\n[WhiteElo \"{welo}\"]\n[BlackElo \"{belo}\"]\n[Date \"{year}.01.01\"]\n[Result \"{result}\"]\n\n{moves} {result}"
    )
}

fn after_e4() -> Line {
    Line {
        start_fen: START.into(),
        moves: vec!["e2e4".into()],
    }
}

#[test]
fn the_report_covers_history_players_and_moves() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("data/search-boosters"));
    let mut games = vec![];
    // 40 Sicilians by a strong player, 20 French games by others, spread
    // over 1990-2019, and two games of another opening.
    for i in 0..40u16 {
        games.push(game(
            "Other, O",
            2300,
            "Strong, S",
            2700,
            1990 + i % 30,
            if i % 4 == 0 { "1-0" } else { "0-1" },
            "1. e4 c5 2. Nf3 d6 3. d4",
        ));
    }
    for i in 0..20u16 {
        games.push(game(
            &format!("Club {i}, C"),
            2000,
            "Frenchman, F",
            2400,
            2000 + i,
            "1/2-1/2",
            "1. e4 e6 2. d4 d5",
        ));
    }
    games.push(game("Q, Q", 2000, "P, P", 2000, 1950, "1-0", "1. d4 d5"));
    games.push(game("Q, Q", 2000, "P, P", 2000, 1951, "1-0", "1. d4 d5"));
    let db = database(&r, &d, &games);
    let mut stages = vec![];
    let report = r
        .opening_report(db.clone(), after_e4(), |s, _, _| stages.push(s.to_string()))
        .unwrap();
    assert!(!report.cancelled);
    assert_eq!(report.games, 60);
    assert_eq!(report.eco.as_deref(), Some("B00"));
    assert_eq!(report.line, ["e4"]);
    assert!(!report.white_to_move);
    assert_eq!(
        (report.white_wins, report.draws, report.black_wins),
        (10, 20, 30)
    );
    // History: no game is both annotated and strong, so the latest game.
    assert_eq!(
        report.earliest.as_ref().unwrap().date.as_deref(),
        Some("1990.01.01")
    );
    assert_eq!(
        report.latest.as_ref().unwrap().date.as_deref(),
        Some("2019.01.01")
    );
    assert!(report.latest_annotated.is_none() && report.latest_strong.is_none());
    assert!(report.listed.is_empty());
    assert_eq!(report.years.first(), Some(&(1990, 2)));
    // 60 games: buckets of 5 years; the database's 1950s games are not
    // the position's.
    assert_eq!(
        report.fashion.first().map(|b| (b.from, b.to)),
        Some((1990, 1994))
    );
    // With under 100 database games per bucket, no index is given.
    assert!(report.fashion.iter().all(|b| b.index.is_none()));
    assert_eq!(report.fashion.iter().map(|b| b.games).sum::<u32>(), 60);
    // Players: the 2700 player as Black; 20 club players are not notable.
    assert_eq!(report.strong_black.len(), 1);
    assert_eq!(report.strong_black[0].name, "Strong, S");
    assert_eq!(report.strong_black[0].score, 75.0);
    assert!(report.strong_white.is_empty());
    let notable: Vec<&str> = report.notable.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(notable, ["Other, O", "Frenchman, F"]);
    // Moves from the games read (no tree): c5 in 40 games, e6 in 20.
    assert!(!report.moves_from_tree);
    assert_eq!(report.read, 60);
    let moves: Vec<(&str, u64)> = report
        .moves
        .iter()
        .map(|m| (m.san.as_str(), m.games))
        .collect();
    assert_eq!(moves, [("c5", 40), ("e6", 20)]);
    assert_eq!(report.moves[0].played_by, ["Strong, S"]);
    assert_eq!(report.moves[0].score, Some(75.0));
    assert_eq!(report.moves[1].draws, Some(100.0));
    assert_eq!(report.main_line, ["c5", "Nf3", "d6", "d4"]);
    assert!(stages.iter().any(|s| s == "Counting the database's years"));

    // With an opening tree the move rows cover every game.
    r.create_opening_tree(db.clone(), |_, _, _| {}).unwrap();
    let with_tree = r
        .opening_report(db.clone(), after_e4(), |_, _, _| {})
        .unwrap();
    assert!(with_tree.moves_from_tree);
    assert_eq!(with_tree.moves[0].games, 40);
    assert!(with_tree.moves[0].performance.is_some());

    // Few games are listed, as in ChessBase.
    let d4 = r
        .opening_report(
            db,
            Line {
                start_fen: START.into(),
                moves: vec!["d2d4".into()],
            },
            |_, _, _| {},
        )
        .unwrap();
    assert_eq!(d4.listed.len(), 2);
    assert!(d4.fashion.is_empty());
}
