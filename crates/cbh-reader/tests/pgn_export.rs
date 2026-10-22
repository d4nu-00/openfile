use cbh_reader::{Database, ReadLimits, pgn_export::write_games, v2::write::Writer};
use chess_core::{
    Game, GameLimits,
    training::{Answer, Block, Text, Training, TrainingKind},
};
use std::sync::atomic::AtomicBool;

fn pgn(text: &str) -> Game {
    chess_core::parse_pgn(text.as_bytes(), &GameLimits::default())
        .unwrap()
        .remove(0)
}

fn text(t: &str) -> Block {
    Block {
        texts: vec![Text {
            language: 0,
            index: 0,
            text: t.into(),
        }],
        ..Default::default()
    }
}

/// Three games; the first has a critical mark and a training question that
/// PGN cannot hold.
fn db() -> (tempfile::TempDir, Database) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Export.2cbh");
    let mut first =
        pgn("[White \"Alpha, A\"]\n[Black \"B\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 1-0");
    first.nodes[1].critical = Some(2);
    first.nodes[2].training = Some(Box::new(Training {
        kind: TrainingKind::MultipleChoice,
        seconds: 60,
        score: 5,
        question: text("Which plan?"),
        wrong: text("No."),
        help: Default::default(),
        answers: vec![Answer {
            points: 5,
            choice: text("Attack"),
            ..Default::default()
        }],
    }));
    let second = pgn("[White \"Beta, B\"]\n[Black \"B\"]\n[Result \"0-1\"]\n\n1. d4 d5 0-1");
    let third =
        pgn("[White \"Gamma, C\"]\n[Black \"B\"]\n[Result \"1/2-1/2\"]\n\n1. c4 c5 1/2-1/2");
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&[first, second, third]).unwrap();
    drop(w);
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    // The question is stored; only the export leaves it out.
    let stored = db.load(1).unwrap().game.unwrap();
    assert!(stored.nodes[2].training.is_some());
    (d, db)
}

fn export(db: &mut Database, ordinals: &[u64]) -> (String, cbh_reader::pgn_export::ExportSummary) {
    let mut out = vec![];
    let summary = write_games(db, ordinals, &mut out, &AtomicBool::new(false), |_, _| {}).unwrap();
    (String::from_utf8(out).unwrap(), summary)
}

#[test]
fn writes_the_listed_games_in_the_given_order() {
    let (_d, mut db) = db();
    let (text, summary) = export(&mut db, &[3, 2]);
    let games = chess_core::parse_pgn(text.as_bytes(), &GameLimits::default()).unwrap();
    let results: Vec<_> = games.iter().map(|g| g.tags["Result"].as_str()).collect();
    assert_eq!(results, ["1/2-1/2", "0-1"]);
    assert_eq!(
        (summary.written, summary.skipped, summary.stripped),
        (2, 0, 0)
    );
    assert!(!summary.cancelled);
    // Each game is followed by a blank line.
    assert!(text.ends_with("\n\n"));
}

#[test]
fn counts_games_whose_annotations_pgn_cannot_hold() {
    let (_d, mut db) = db();
    let (text, summary) = export(&mut db, &[1, 2]);
    assert_eq!(
        (summary.written, summary.skipped, summary.stripped),
        (2, 0, 1)
    );
    let games = chess_core::parse_pgn(text.as_bytes(), &GameLimits::default()).unwrap();
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].tags["White"], "Alpha, A");
}

#[test]
fn reports_progress_and_stops_when_cancelled() {
    let (_d, mut db) = db();
    let mut seen = vec![];
    let mut out = vec![];
    write_games(
        &mut db,
        &[1, 2, 3],
        &mut out,
        &AtomicBool::new(false),
        |d, t| seen.push((d, t)),
    )
    .unwrap();
    assert_eq!(seen, [(1, 3), (2, 3), (3, 3)]);

    let mut out = vec![];
    let summary = write_games(
        &mut db,
        &[1, 2, 3],
        &mut out,
        &AtomicBool::new(true),
        |_, _| {},
    )
    .unwrap();
    assert!(summary.cancelled);
    assert_eq!(summary.written, 0);
    assert!(out.is_empty());
}
