use cbh_reader::name_list::NameKind;
use openfile::{registry::Registry, trees::Line};

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn line(moves: &[&str]) -> Line {
    serde_json::from_value(serde_json::json!({
        "startFen": START,
        "moves": moves,
    }))
    .unwrap()
}

#[test]
fn prepare_against_builds_the_player_s_tree_with_one_colour() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let db = r
        .create_database(d.path().join("P.2cbh"), "c".into())
        .unwrap()
        .database_id;
    let pgn = d.path().join("g.pgn");
    // Alpha has White in three games (1.e4 twice) and Black in one.
    std::fs::write(
        &pgn,
        "[Event \"Club\"]\n[Date \"2019.01.01\"]\n[White \"Alpha, A\"]\n[Black \"Bravo, B\"]\n[Result \"1-0\"]\n[WhiteElo \"2400\"]\n\n1. e4 c5 2. Nf3 1-0\n\n\
         [Event \"Club\"]\n[Date \"2020.01.01\"]\n[White \"Alpha, A\"]\n[Black \"Charlie, C\"]\n[Result \"1/2-1/2\"]\n[WhiteElo \"2500\"]\n\n1. e4 c5 2. Nf3 1/2-1/2\n\n\
         [Event \"Club\"]\n[Date \"2020.02.01\"]\n[White \"Alpha, A\"]\n[Black \"Bravo, B\"]\n[Result \"0-1\"]\n\n1. d4 d5 0-1\n\n\
         [Event \"Club\"]\n[Date \"2020.03.01\"]\n[White \"Bravo, B\"]\n[Black \"Alpha, A\"]\n[Result \"1-0\"]\n\n1. c4 1-0\n",
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
    r.name_list(db.clone(), NameKind::Players, |_, _| {})
        .unwrap();
    let alpha = r
        .read_names(db.clone(), NameKind::Players, "1".into(), 1, "n".into())
        .unwrap()
        .rows[0]
        .id;

    // Without a preparation the Book has nothing to show.
    let empty = r.prepared_tree(db.clone(), line(&[]), "t".into()).unwrap();
    assert_eq!(serde_json::to_value(&empty).unwrap()["status"], "noTree");

    let mut stages = vec![];
    let p = r
        .prepare_against(db.clone(), alpha, true, |stage, _, _| stages.push(stage))
        .unwrap()
        .unwrap();
    assert!(stages.contains(&"games") && stages.contains(&"lines"));
    assert_eq!(p.name.as_deref(), Some("Alpha, A"));
    assert!(p.white);
    assert_eq!((p.games, p.read, p.unreadable), (3, 3, 0));
    assert_eq!((p.score.games, p.score.points), (3, 1.5));
    assert_eq!(p.recent_rating, Some(2450));
    assert_eq!((p.first_year, p.last_year), (2019, 2020));
    assert_eq!(p.last_game.as_deref(), Some("3"));
    assert_eq!(p.latest.len(), 3);
    assert_eq!(p.latest[0].opponent.as_deref(), Some("Bravo, B"));
    // 1.e4 c5 2.Nf3 is played twice; 1.d4 once, so it is not a line.
    assert_eq!(p.repertoire.len(), 1);
    assert_eq!(p.repertoire[0].text, "1.e4 c5 2.Nf3");
    assert_eq!(p.repertoire[0].games, 2);
    assert_eq!(p.repertoire[0].score, Some(75.0));
    assert_eq!(p.repertoire[0].eco.as_deref(), Some("B27"));

    // The Book shows the player's moves.
    let tree = r.prepared_tree(db.clone(), line(&[]), "t".into()).unwrap();
    assert_eq!(tree.games, 3);
    assert_eq!(tree.moves[0].san, "e4");
    assert_eq!(tree.moves[0].games, 2);
    let after = r
        .prepared_tree(db.clone(), line(&["e2e4", "c7c5"]), "t".into())
        .unwrap();
    assert_eq!(after.moves.len(), 1);
    assert_eq!(after.moves[0].san, "Nf3");

    // Its games are listed, all or by position.
    let all = r
        .preparation_games(db.clone(), alpha, true, None, "q".into())
        .unwrap();
    assert_eq!(all.hits, "3");
    let d4 = r
        .preparation_games(db.clone(), alpha, true, Some(line(&["d2d4"])), "q".into())
        .unwrap();
    assert_eq!(d4.hits, "1");
    let rows = r.read_rows(db.clone(), "1".into(), 10, "p".into()).unwrap();
    assert_eq!(rows.entries[0].ordinal, "3");
    // Another colour or player is refused.
    assert!(
        r.preparation_games(db.clone(), alpha, false, None, "q".into())
            .is_err()
    );

    // With Black, Alpha has one game.
    let black = r
        .prepare_against(db.clone(), alpha, false, |_, _, _| {})
        .unwrap()
        .unwrap();
    assert_eq!((black.games, black.score.points), (1, 0.0));
    // A single game is still a line.
    assert_eq!(black.repertoire.len(), 1);
    assert_eq!(black.repertoire[0].text, "1.c4");

    // Closing it returns the Book to the database's tree, and changing the
    // database drops it too.
    r.close_preparation(db.clone()).unwrap();
    assert!(
        r.preparation_games(db.clone(), alpha, false, None, "q".into())
            .is_err()
    );
    r.prepare_against(db.clone(), alpha, true, |_, _, _| {})
        .unwrap()
        .unwrap();
    r.set_deleted(db.clone(), "3".into(), true).unwrap();
    assert!(
        r.preparation_games(db.clone(), alpha, true, None, "q".into())
            .is_err()
    );
}
