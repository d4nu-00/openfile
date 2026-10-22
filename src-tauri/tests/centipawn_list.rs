use openfile::{centipawn::CentipawnParams, clipboard::RowRange, registry::Registry};
use std::sync::atomic::AtomicBool;

fn mock_engine() -> std::path::PathBuf {
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let name = format!("mock-uci{}", std::env::consts::EXE_SUFFIX);
    [deps.join(&name), deps.parent().unwrap().join(&name)]
        .into_iter()
        .find(|p| p.exists())
        .expect("mock-uci is missing: run `cargo test --workspace`")
}

const RUY: &str = "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 d6 8. c3 O-O 9. a3 Kh8 10. Kh1 Rb8 11. Qe2 Qd7";

/// A new 2CBH database with `games` saved into it; returns its id.
fn database(r: &Registry, d: &tempfile::TempDir, games: &[String]) -> String {
    let db = r
        .create_database(d.path().join("List"), "create".into())
        .unwrap();
    for (i, pgn) in games.iter().enumerate() {
        let path = d.path().join(format!("g{i}.pgn"));
        std::fs::write(&path, pgn).unwrap();
        let c = r.open_pgn(path, format!("o{i}")).unwrap();
        let doc = r
            .load_pgn_game(c.collection_id, 0, format!("l{i}"))
            .unwrap()
            .document_id;
        r.save_game(doc.clone(), db.database_id.clone(), "0".into(), false)
            .unwrap();
        r.close_document(doc).unwrap();
    }
    db.database_id
}

fn params() -> CentipawnParams {
    CentipawnParams {
        engine: mock_engine(),
        options: Default::default(),
        seconds: 0,
        every_move: false,
    }
}

fn all(n: u64) -> Vec<RowRange> {
    vec![RowRange {
        start: "1".into(),
        end: (n + 1).to_string(),
    }]
}

#[test]
fn list_analysis_replaces_each_game_and_keeps_player_statistics() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let game = |white: &str, black: &str, we: u16, be: u16, result: &str| {
        format!(
            "[White \"{white}\"]\n[Black \"{black}\"]\n[WhiteElo \"{we}\"]\n[BlackElo \"{be}\"]\n[Result \"{result}\"]\n\n{RUY} {result}\n"
        )
    };
    let db = database(
        &r,
        &d,
        &[
            game("Player, A", "Player, B", 2500, 2400, "1-0"),
            game("Player, B", "Player, A", 2400, 2500, "1/2-1/2"),
            "[White \"C\"]\n[Black \"D\"]\n\n1. e4 e5 *\n".into(),
        ],
    );
    let mut updates = 0;
    let report = r
        .centipawn_list(&db, &all(3), &params(), &AtomicBool::new(false), |_| {
            updates += 1
        })
        .unwrap();
    assert!(!report.cancelled && report.error.is_none());
    assert_eq!(
        (
            report.games,
            report.analysed,
            report.replaced,
            report.skipped_count
        ),
        (3, 2, 2, 1)
    );
    assert_eq!(report.skipped[0].ordinal, "3");
    assert!(updates > 2);
    // Every counted move loses 34 cp with the mock engine.
    assert_eq!(report.players.len(), 2);
    let a = &report
        .players
        .iter()
        .find(|p| p.name == "Player  A")
        .unwrap();
    assert_eq!(a.games.len(), 2);
    assert!(a.games.iter().all(|g| g.value == 34 && g.weight > 0));
    assert_eq!(a.points, 1.5);
    assert_eq!(a.perform, Some(2591));
    assert!(report.csv.starts_with(
        "Player Name,Elo,Perform,Points,BV-1,BV-2,Weighted Av,Opponent-1,Opponent-2\n"
    ));
    // The entry now holds the annotated game, with its evaluation profile.
    let loaded = r.load_entry(db.clone(), "1".into(), "x".into()).unwrap();
    let game = loaded.game.unwrap();
    assert!(game.nodes[0].comments[0].starts_with("Centipawn Analysis"));
    assert!(!game.profile.is_empty());

    // Stopped before it starts: nothing more is written.
    let stopped = r
        .centipawn_list(&db, &all(1), &params(), &AtomicBool::new(true), |_| {})
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!(stopped.replaced, 0);
}

#[test]
fn list_analysis_needs_a_database_opened_for_editing() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let path = d.path().join("g.pgn");
    std::fs::write(&path, format!("{RUY} *\n")).unwrap();
    let c = r.open_pgn(path, "o".into()).unwrap();
    assert!(
        r.centipawn_list(
            &c.collection_id,
            &all(1),
            &params(),
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err()
    );
}
