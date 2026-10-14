use chess_workbench::tablebases::{Outcome, TablebaseStatus, Tablebases};

/// A folder of real Syzygy files, never committed: `SYZYGY_DIR`, if set.
fn tables() -> Option<Tablebases> {
    let dir = std::env::var_os("SYZYGY_DIR")?;
    let t = Tablebases::default();
    assert!(t.set_dir(Some(dir.as_ref())).unwrap() > 0);
    Some(t)
}

#[test]
fn without_tables_positions_report_no_tables() {
    let t = Tablebases::default();
    let r = t
        .probe("8/8/8/4k3/8/8/8/4K2Q w - - 0 1", "r".into())
        .unwrap();
    assert_eq!(r.status, TablebaseStatus::NoTables);
    assert_eq!(r.pieces, 3);
    assert!(r.moves.is_empty());
    // A folder that does not exist is refused, and nothing is opened.
    assert!(t.set_dir(Some("/nonexistent/syzygy".as_ref())).is_err());
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(t.set_dir(Some(empty.path())).unwrap(), 0);
    let r = t
        .probe("8/8/8/4k3/8/8/8/4K2Q w - - 0 1", "r".into())
        .unwrap();
    assert_eq!(r.status, TablebaseStatus::NoTables);
    assert!(t.probe("not a fen", "r".into()).is_err());
}

#[test]
fn real_tables_rank_moves() {
    let Some(t) = tables() else {
        eprintln!("SYZYGY_DIR is not set: real tablebase probes skipped");
        return;
    };
    // K+Q v K: White wins. Winning moves come first, fastest first; a queen
    // move next to the black king that loses the queen only draws.
    let r = t
        .probe("8/8/8/4k3/8/8/8/4K2Q w - - 0 1", "r".into())
        .unwrap();
    assert_eq!(r.status, TablebaseStatus::Found);
    assert_eq!(r.outcome, Some(Outcome::Win));
    assert!(r.dtz.unwrap() > 0);
    assert_eq!(r.moves[0].outcome, Outcome::Win);
    let wins: Vec<_> = r
        .moves
        .iter()
        .filter(|m| m.outcome == Outcome::Win)
        .collect();
    assert!(
        wins.windows(2)
            .all(|w| w[0].dtz.unwrap().abs() <= w[1].dtz.unwrap().abs())
    );
    let blunder = r.moves.iter().find(|m| m.san == "Qe4+").unwrap();
    assert_eq!(blunder.outcome, Outcome::Draw);
    assert_eq!(r.moves.last().unwrap().outcome, Outcome::Draw);
    // K+P v K with the black king in front of the pawn: a draw.
    let draw = t
        .probe("4k3/8/8/8/8/8/4P3/4K3 b - - 0 1", "r".into())
        .unwrap();
    assert_eq!(draw.outcome, Some(Outcome::Draw));
    // Too many pieces for the tables, and castling rights.
    let start = t
        .probe(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r".into(),
        )
        .unwrap();
    assert_eq!(start.status, TablebaseStatus::TooManyPieces);
}

#[test]
fn annotating_a_game_marks_moves_that_give_away_the_result() {
    let game = |moves: &[&str]| {
        let mut g = chess_core::Game::from_fen("8/8/8/4k3/8/8/8/4K2Q w - - 0 1").unwrap();
        let mut id = 0;
        for m in moves {
            id = g
                .append_uci(id, m, &chess_core::GameLimits::default())
                .unwrap();
        }
        g
    };
    // Without tables nothing is added.
    let none = Tablebases::default().annotate(&game(&["h1e4"])).unwrap();
    assert_eq!(none.status, TablebaseStatus::NoTables);
    assert!(none.items.is_empty());
    let Some(t) = tables() else {
        eprintln!("SYZYGY_DIR is not set: real tablebase annotation skipped");
        return;
    };
    // 1.Qe4+?? Kxe4 throws the win away; 1.Qh5+ keeps it.
    let a = t.annotate(&game(&["h1e4", "e5e4"])).unwrap();
    assert_eq!(a.status, TablebaseStatus::Found);
    assert_eq!(a.probed, 3);
    assert_eq!(a.mistakes, 1);
    let item = &a.items[0];
    assert_eq!(item.node, 1);
    assert_eq!(item.nag, Some(2));
    assert_eq!(item.comment.as_deref(), Some("Tablebase: draw"));
    assert_eq!(item.line.len(), 1);
    assert_eq!(item.line_comment.as_deref(), Some("Tablebase: White wins"));
    let good = t.annotate(&game(&["h1h5", "e5e4"])).unwrap();
    assert_eq!(good.mistakes, 0);
    assert!(good.items.is_empty());
}
