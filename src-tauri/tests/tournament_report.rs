use cbh_reader::name_list::NameKind;
use chess_workbench::registry::Registry;

#[test]
fn tournament_report_lists_rounds_standings_and_results() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let db = r
        .create_database(d.path().join("T.2cbh"), "c".into())
        .unwrap()
        .database_id;
    let pgn = d.path().join("g.pgn");
    // Three players, two rounds, and one game without a round.
    std::fs::write(
        &pgn,
        "[Event \"Club\"]\n[Date \"2020.01.11\"]\n[Round \"2\"]\n[White \"Alpha, A\"]\n[Black \"Charlie, C\"]\n[Result \"1/2-1/2\"]\n\n1. d4 d5 2. c4 1/2-1/2\n\n\
         [Event \"Club\"]\n[Date \"2020.01.10\"]\n[Round \"1\"]\n[White \"Bravo, B\"]\n[Black \"Alpha, A\"]\n[Result \"0-1\"]\n\n1. e4 e5 0-1\n\n\
         [Event \"Club\"]\n[Date \"2020.01.12\"]\n[Round \"1\"]\n[White \"Charlie, C\"]\n[Black \"Bravo, B\"]\n[Result \"1-0\"]\n\n1. c4 1-0\n\n\
         [Event \"Club\"]\n[White \"Bravo, B\"]\n[Black \"Charlie, C\"]\n[Result \"1-0\"]\n\n1. f4 1-0\n",
    )
    .unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..4 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.clone(), "0".into(), false).unwrap();
    }
    r.name_list(db.clone(), NameKind::Tournaments, |_, _| {})
        .unwrap();
    let club = r
        .read_names(db.clone(), NameKind::Tournaments, "1".into(), 1, "n".into())
        .unwrap()
        .rows[0]
        .id;

    let report = r.tournament_report(db.clone(), club).unwrap();
    assert_eq!(report.table.players.len(), 3);
    let name = |row: usize| report.table.players[row].name.clone().unwrap();
    let rounds: Vec<u16> = report.rounds.iter().map(|r| r.round).collect();
    // Numbered rounds in order, then the game without a round.
    assert_eq!(rounds, [1, 2, 0]);
    let first = &report.rounds[0];
    assert_eq!(first.date.as_deref(), Some("2020.01.10"));
    assert_eq!(first.games.len(), 2);
    assert_eq!(name(first.games[0].white), "Bravo, B");
    assert_eq!(name(first.games[0].black), "Alpha, A");
    assert_eq!(first.games[0].result, "0-1");
    // After round 1 Alpha and Charlie lead with a point each.
    let leaders: Vec<(String, f64)> = first
        .standings
        .iter()
        .map(|s| (name(s.row), s.points))
        .collect();
    assert_eq!(leaders[0].1, 1.0);
    assert_eq!(leaders[1].1, 1.0);
    assert_eq!(leaders[2], ("Bravo, B".into(), 0.0));
    // After round 2 Alpha and Charlie have 1½; ties keep the crosstable's
    // order.
    let second = &report.rounds[1];
    let top: Vec<f64> = second.standings.iter().map(|s| s.points).collect();
    assert_eq!(top, [1.5, 1.5, 0.0]);
    assert!(second.standings[0].row < second.standings[1].row);
    assert_eq!(
        (
            report.stats.games,
            report.stats.white_wins,
            report.stats.draws,
            report.stats.black_wins
        ),
        (4, 2, 1, 1)
    );
    assert_eq!(report.stats.white_score, Some(62.5));
    assert_eq!(report.stats.average_moves, Some(1));

    // A round's games are shown as the list.
    let shown = r
        .show_entries(
            db.clone(),
            first.games.iter().map(|g| g.ordinal.to_string()).collect(),
            "q".into(),
        )
        .unwrap();
    assert_eq!(shown.hits, "2");
    let rows = r.read_rows(db.clone(), "1".into(), 10, "p".into()).unwrap();
    assert_eq!(rows.entries.len(), 2);
    assert!(
        r.show_entries(db.clone(), vec!["99".into()], "q".into())
            .is_err()
    );
}
