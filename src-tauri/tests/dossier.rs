use cbh_reader::{name_list::NameKind, player_stats::Group};
use openfile::registry::Registry;

#[test]
fn player_dossier_reports_statistics_lines_mates_medals_and_annotations() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let path = d.path().join("D.2cbh");
    let db = r
        .create_database(path.clone(), "c".into())
        .unwrap()
        .database_id;
    let pgn = d.path().join("g.pgn");
    // Alpha mates Bravo with Black (Fool's mate), is mated with White, wins
    // with White without mate, annotates one game and draws one.
    std::fs::write(
        &pgn,
        "[Event \"Club\"]\n[Date \"2019.01.01\"]\n[White \"Bravo, B\"]\n[Black \"Alpha, A\"]\n[Result \"0-1\"]\n[WhiteElo \"2500\"]\n\n1. f3 e5 2. g4 Qh4# 0-1\n\n\
         [Event \"Club\"]\n[Date \"2020.01.01\"]\n[White \"Alpha, A\"]\n[Black \"Charlie, C\"]\n[Result \"0-1\"]\n[BlackElo \"2400\"]\n\n1. f3 e5 2. g4 Qh4# 0-1\n\n\
         [Event \"Open\"]\n[Date \"2020.02.01\"]\n[White \"Alpha, A\"]\n[Black \"Bravo, B\"]\n[Result \"1-0\"]\n[Annotator \"Alpha, A\"]\n[BlackElo \"2500\"]\n\n1. e4 e5 2. Nf3 1-0\n\n\
         [Event \"Open\"]\n[Date \"2020.03.01\"]\n[White \"Alpha, A\"]\n[Black \"Charlie, C\"]\n[Result \"1/2-1/2\"]\n\n1. e4 e5 1/2-1/2\n",
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
    // Give entry 3 the Best game medal (bit 0), at 2CBH record bytes 130–131.
    r.close_database(db).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[3 * 192 + 130] = 1;
    std::fs::write(&path, bytes).unwrap();
    let db = r
        .open_database(path.clone(), "o".into())
        .unwrap()
        .database_id;
    r.name_list(db.clone(), NameKind::Players, |_, _| {})
        .unwrap();
    let alpha = r
        .read_names(db.clone(), NameKind::Players, "1".into(), 1, "n".into())
        .unwrap()
        .rows[0]
        .id;

    let mut stages = vec![];
    let dossier = r
        .player_dossier(db.clone(), alpha, |stage, _, _| stages.push(stage))
        .unwrap()
        .unwrap();
    assert!(
        ["games", "lines", "mates"]
            .iter()
            .all(|s| stages.contains(s))
    );
    assert_eq!(dossier.name.as_deref(), Some("Alpha, A"));
    assert_eq!(dossier.games, 4);
    assert_eq!((dossier.first_year, dossier.last_year), (2019, 2020));
    // With White: a loss, a win and a draw: (2 + 1) × 50 / 3 = 50 %, 5
    // below 55: band 2.
    assert_eq!(dossier.white.games, 3);
    assert_eq!(dossier.white.score, Some(50));
    assert_eq!(dossier.white.score_band, Some(2));
    assert_eq!(dossier.black.score, Some(100));
    assert_eq!(dossier.all.games, 4);
    assert_eq!(dossier.all.quick_draw_percent, Some(25));
    assert_eq!(dossier.rating_bands.len(), 2);
    // Mates by the main line's last position.
    assert_eq!(dossier.mates_read, 3);
    assert_eq!(dossier.mates_given_count, 1);
    assert_eq!(dossier.mates_given[0].ordinal, "1");
    assert_eq!(dossier.mates_received_count, 1);
    assert_eq!(
        dossier.mates_received[0].opponent.as_deref(),
        Some("Charlie, C")
    );
    assert_eq!(dossier.medal_count, 1);
    assert_eq!(dossier.medal_games[0].medals, ["Best game"]);
    assert_eq!(dossier.annotated_count, 1);
    assert_eq!(dossier.annotated[0].ordinal, "3");
    assert_eq!(dossier.repertoire_white[0].text, "1.e4 e5");
    assert_eq!(dossier.opponents.len(), 2);

    // The games are kept, so its rows list their games.
    let hits = r
        .statistics_games(db.clone(), alpha, Group::Color { white: false }, "q".into())
        .unwrap();
    assert_eq!(hits.hits, "1");
}
