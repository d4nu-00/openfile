use cbh_reader::name_list::NameKind;
use openfile::registry::Registry;
use std::sync::atomic::AtomicBool;

const NAJDORF: &str = "1. e4 c5 2. Nf3 d6 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6 6. Be3 e5";

fn game(white: &str, black: &str, result: &str, elo: u16, moves: &str) -> String {
    format!(
        "[Event \"Club\"]\n[Date \"2020.01.01\"]\n[White \"{white}\"]\n[Black \"{black}\"]\n\
         [Result \"{result}\"]\n[WhiteElo \"{elo}\"]\n[BlackElo \"{elo}\"]\n\n{moves} {result}\n\n"
    )
}

#[test]
fn generates_both_repertoires_and_reports_their_games() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(d.path().join("search-boosters"));
    let db = r
        .create_database(d.path().join("Mine.2cbh"), "c".into())
        .unwrap()
        .database_id;
    let pgn = d.path().join("g.pgn");
    let text = [
        // Alpha's English Attack twice as White, branching at move 7.
        game(
            "Alpha, A",
            "Bravo, B",
            "1-0",
            2400,
            &format!("{NAJDORF} 7. Nb3"),
        ),
        game(
            "Alpha, A",
            "Charlie, C",
            "1/2-1/2",
            2500,
            &format!("{NAJDORF} 7. Nf3"),
        ),
        // One Queen's Gambit: too rare for a repertoire game.
        game("Alpha, A", "Bravo, B", "0-1", 2400, "1. d4 d5 2. c4"),
        // The Ruy Lopez twice as Black.
        game(
            "Bravo, B",
            "Alpha, A",
            "1-0",
            2300,
            "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4",
        ),
        game(
            "Charlie, C",
            "Alpha, A",
            "0-1",
            2200,
            "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4",
        ),
        // Someone else's English Attack, for the report.
        game("Delta, D", "Echo, E", "1-0", 2700, NAJDORF),
    ]
    .concat();
    std::fs::write(&pgn, text).unwrap();
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..6 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.clone(), "0".into(), false).unwrap();
    }

    // The frequent players need the player list.
    assert!(r.frequent_players(db.clone()).is_err());
    r.name_list(db.clone(), NameKind::Players, |_, _| {})
        .unwrap();
    let players = r.frequent_players(db.clone()).unwrap();
    assert_eq!(players[0].name.as_deref(), Some("Alpha, A"));
    assert_eq!(players[0].games, 5);
    let alpha = players[0].id;

    let out = d.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let cancel = AtomicBool::new(false);
    assert!(
        r.generate_repertoire(db.clone(), vec![], out.clone(), &cancel, |_, _, _| {})
            .is_err()
    );
    let mut stages = vec![];
    let g = r
        .generate_repertoire(db.clone(), vec![alpha], out.clone(), &cancel, |s, _, _| {
            stages.push(s)
        })
        .unwrap()
        .unwrap();
    assert!(stages.contains(&"games") && stages.contains(&"lines"));
    assert_eq!(g.players, ["Alpha, A"]);
    let white = &g.sides[0];
    assert!(white.white);
    assert_eq!((white.games, white.read), (3, 3));
    assert_eq!(
        (white.repertoire_games, white.merged, white.rare),
        (1, 2, 1)
    );
    let black = &g.sides[1];
    assert_eq!(
        (black.games, black.repertoire_games, black.merged),
        (2, 1, 2)
    );
    let white_path = std::path::PathBuf::from(white.path.clone().unwrap());
    assert!(white_path.ends_with("Repertoire White.2cbh"));
    assert!(out.join("Repertoire Black.2cbh").exists());

    // The repertoire game: both games merged, 6.Be3 marked.
    let rep = r.open_database(white_path.clone(), "r".into()).unwrap();
    let page = r
        .read_page(rep.database_id.clone(), "1".into(), 10, "p".into())
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    let e = &page.entries[0];
    assert_eq!(e.white.as_deref(), Some("Sicilian Defense"));
    assert_eq!(
        e.black.as_deref(),
        Some("Najdorf Variation, English Attack")
    );
    assert_eq!(e.eco.as_deref(), Some("B90"));
    let mut rdb = cbh_reader::Database::open(&white_path, Default::default()).unwrap();
    let game = rdb.load(1).unwrap().game.unwrap();
    let marked: Vec<&str> = game
        .nodes
        .iter()
        .filter(|n| n.critical == Some(1))
        .filter_map(|n| n.uci.as_deref())
        .collect();
    assert_eq!(marked, ["c1e3"]);

    // A second run refuses to overwrite.
    let again = r.generate_repertoire(db.clone(), vec![alpha], out.clone(), &cancel, |_, _, _| {});
    assert_eq!(again.unwrap_err().code, "exists");

    // The report: three games reach the position after 6.Be3, the
    // strongest first.
    let report = r
        .repertoire_report(db.clone(), &white_path, &cancel, |_, _, _| {})
        .unwrap();
    assert_eq!(report.repertoire_games, 1);
    assert_eq!(report.positions, 1);
    assert_eq!(report.matched, "3");
    assert_eq!(report.topics.len(), 1);
    let t = &report.topics[0];
    assert_eq!(
        t.name,
        "Sicilian Defense (Najdorf Variation, English Attack)"
    );
    let p = &t.positions[0];
    assert_eq!(
        p.text,
        "1.e4 c5 2.Nf3 d6 3.d4 cxd4 4.Nxd4 Nf6 5.Nc3 a6 6.Be3"
    );
    assert_eq!(p.games, 3);
    assert_eq!(p.top[0].white.as_deref(), Some("Delta, D"));
    let listed = r
        .show_entries(db.clone(), p.ordinals.clone(), "s".into())
        .unwrap();
    assert_eq!(listed.hits, "3");
}
