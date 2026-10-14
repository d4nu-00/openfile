use cbh_reader::{name_list::NameKind, player_stats::Group};
use chess_workbench::registry::Registry;

#[test]
fn player_statistics_score_a_player_and_open_a_row_s_games() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let db = r
        .create_database(d.path().join("S.2cbh"), "c".into())
        .unwrap()
        .database_id;
    let pgn = d.path().join("g.pgn");
    std::fs::write(
        &pgn,
        "[Event \"Club\"]\n[Date \"2019.01.01\"]\n[White \"Bravo, B\"]\n[Black \"Alpha, A\"]\n[Result \"0-1\"]\n[ECO \"B90\"]\n\n1. e4 0-1\n\n\
         [Event \"Club\"]\n[Date \"2020.01.01\"]\n[White \"Alpha, A\"]\n[Black \"Charlie, C\"]\n[Result \"1/2-1/2\"]\n[ECO \"D02\"]\n\n1. d4 1/2-1/2\n\n\
         [Event \"Club\"]\n[Date \"2020.01.02\"]\n[White \"Bravo, B\"]\n[Black \"Charlie, C\"]\n[Result \"1-0\"]\n\n1. c4 1-0\n",
    )
    .unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..3 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.clone(), "0".into(), false).unwrap();
    }
    r.name_list(db.clone(), NameKind::Players, |_, _| {})
        .unwrap();
    let alpha = r
        .read_names(db.clone(), NameKind::Players, "1".into(), 1, "n".into())
        .unwrap()
        .rows[0]
        .id;
    // Rows can be opened only once the statistics are shown.
    assert_eq!(
        r.statistics_games(db.clone(), alpha, Group::All, "q".into())
            .unwrap_err()
            .code,
        "noStatistics"
    );
    let mut calls = 0;
    let s = r
        .player_statistics(db.clone(), alpha, |_, _| calls += 1)
        .unwrap()
        .unwrap();
    assert!(calls > 0);
    assert_eq!(s.name.as_deref(), Some("Alpha, A"));
    assert_eq!((s.total.games, s.total.points), (2, 1.5));
    assert_eq!((s.white.games, s.black.games), (1, 1));
    assert_eq!((s.first_year, s.last_year), (2019, 2020));
    assert_eq!(s.openings.len(), 2);
    assert_eq!(s.opponents.len(), 2);

    let hits = r
        .statistics_games(db.clone(), alpha, Group::Color { white: false }, "q".into())
        .unwrap();
    assert_eq!(hits.hits, "1");
    let rows = r.read_rows(db.clone(), "1".into(), 10, "p".into()).unwrap();
    assert_eq!(rows.entries.len(), 1);
    assert_eq!(rows.entries[0].ordinal, "1");
    // Another player's rows are refused.
    assert!(
        r.statistics_games(db.clone(), alpha + 1, Group::All, "q".into())
            .is_err()
    );

    // Changing the database drops the kept games.
    r.set_deleted(db.clone(), "3".into(), true).unwrap();
    assert!(
        r.statistics_games(db.clone(), alpha, Group::All, "q".into())
            .is_err()
    );
}
