use cbh_reader::{
    Database, ReadLimits,
    maintain::{Compare, Decision, DoubleOptions},
    v2::{compact, write::Writer},
};
use chess_core::{Game, GameLimits};
use std::{path::Path, sync::atomic::AtomicBool};

fn pgn(text: &str) -> Game {
    chess_core::parse_pgn(text.as_bytes(), &GameLimits::default())
        .unwrap()
        .remove(0)
}

fn game(white: &str, moves: &str) -> Game {
    pgn(&format!(
        "[White \"{white}\"]\n[Black \"Other, O\"]\n[Date \"2020.01.01\"]\n[Result \"1-0\"]\n\n{moves} 1-0"
    ))
}

fn summaries(path: &Path) -> Vec<(String, bool, usize)> {
    let mut db = Database::open(path, ReadLimits::default()).unwrap();
    (1..=db.info().entry_count)
        .map(|o| {
            let e = db.load(o).unwrap();
            (
                e.summary.white.unwrap_or_default(),
                e.summary.deleted,
                e.game.map_or(0, |g| g.nodes.len()),
            )
        })
        .collect()
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn compaction_removes_deleted_entries_and_keeps_the_rest_byte_for_byte() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Db.2cbh");
    let mut w = Writer::create(&path).unwrap();
    let names = ["Alpha, A", "Bravo, B", "Charlie, C", "Delta, D", "Echo, E"];
    for (i, n) in names.iter().enumerate() {
        let moves = [
            "1. e4 {Good} e5 2. Nf3",
            "1. d4",
            "1. c4 c5",
            "1. Nf3 (1. g3) d5",
            "1. b3",
        ][i];
        w.append(&game(n, moves)).unwrap();
    }
    // A replaced game leaves its old blocks unreferenced.
    w.replace(4, &game("Delta, D", "1. Nf3 (1. g3) d5 2. g3"))
        .unwrap();
    w.set_deleted(2, true).unwrap();
    w.set_deleted(5, true).unwrap();
    drop(w);
    std::fs::write(d.path().join("Db.2lcd"), b"index").unwrap();
    let before = summaries(&path);
    let size_before = compact::family_bytes(&path);

    // Cancelled: nothing changes.
    let none = compact::compact(&path, &AtomicBool::new(true), |_, _| {}).unwrap();
    assert!(none.cancelled);
    assert_eq!(summaries(&path), before);

    let mut steps = vec![];
    let report =
        compact::compact(&path, &AtomicBool::new(false), |d, t| steps.push((d, t))).unwrap();
    assert_eq!(
        (report.kept, report.removed, report.cancelled),
        (3, 2, false)
    );
    assert_eq!(report.renumbered, vec![0, 1, 0, 2, 3, 0]);
    assert_eq!(steps.last(), Some(&(5, 5)));
    assert!(report.bytes_after < size_before);
    let kept: Vec<_> = before.iter().filter(|e| !e.1).cloned().collect();
    assert_eq!(summaries(&path), kept);
    // Comments and variations survive, and ChessBase's index is gone.
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let alpha = db.load(1).unwrap().game.unwrap();
    assert_eq!(alpha.nodes[1].comments, vec!["Good".to_string()]);
    assert_eq!(
        files(d.path()),
        vec!["Db.2cba", "Db.2cbg", "Db.2cbh", "Db.2lid"]
    );
    // A database without deleted entries compacts to the same games.
    let again = compact::compact(&path, &AtomicBool::new(false), |_, _| {}).unwrap();
    assert_eq!((again.kept, again.removed), (3, 0));
    assert_eq!(summaries(&path), kept);
    // A leftover backup is never overwritten.
    std::fs::write(d.path().join("Db.compact-backup.2cbh"), b"x").unwrap();
    assert!(compact::compact(&path, &AtomicBool::new(false), |_, _| {}).is_err());
}

#[test]
fn integrity_reports_damaged_blocks() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Db.2cbh");
    let mut w = Writer::create(&path).unwrap();
    for n in ["Alpha, A", "Bravo, B", "Charlie, C"] {
        w.append(&game(n, "1. e4 e5")).unwrap();
    }
    drop(w);
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    let clean = db.check_integrity(&AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(
        (clean.checked, clean.games, clean.problems.len()),
        (3, 3, 0)
    );
    drop(db);
    // Damage the second game's move block: its record points at it.
    let record = std::fs::read(&path).unwrap();
    let offset = u64::from_le_bytes(record[2 * 192 + 8..2 * 192 + 16].try_into().unwrap());
    let mut moves = std::fs::read(path.with_extension("2cbg")).unwrap();
    moves[offset as usize + 30] ^= 0xff;
    std::fs::write(path.with_extension("2cbg"), moves).unwrap();
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    let report = db.check_integrity(&AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(report.problems.len(), 1);
    assert_eq!(report.problems[0].ordinal, 2);
    assert_eq!(report.problems[0].code, "damagedMoves");
    assert_eq!(report.counts.get("damagedMoves"), Some(&1));
}

#[test]
fn doubles_are_found_and_the_better_game_is_kept() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Db.2cbh");
    let mut w = Writer::create(&path).unwrap();
    let line = "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6";
    let mut rated = game("Kasparov, Garry", line);
    rated.tags.insert("WhiteElo".into(), "2800".into());
    let mut annotated = game("Kasparov, G", line);
    annotated.nodes[2].comments.push("The Ruy Lopez".into());
    for g in [
        game("Kasparov, Garry", line),       // 1: plain
        rated,                               // 2: same, with an Elo
        annotated,                           // 3: same game, annotated
        game("Kasparov, Garry", "1. d4 d5"), // 4: another game
        game("Karpov, Anatoly", line),       // 5: other players
    ] {
        w.append(&g).unwrap();
    }
    drop(w);
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    let cancel = AtomicBool::new(false);
    // Similar names ("kasp"): 1, 2 and 3 are doubles; the annotated game
    // is kept, then the rated one over the plain one.
    let r = db
        .find_doubles(&DoubleOptions::default(), &cancel, |_, _| {})
        .unwrap();
    assert_eq!(r.checked, 5);
    let marked: Vec<u64> = r.pairs.iter().map(|p| p.0).collect();
    assert_eq!(marked, vec![1, 2]);
    assert!(r.pairs.iter().all(|p| p.1 == 3 || p.1 == 2));
    // Exact names: only 1 and 2 are the same; 2 has more data.
    let exact = DoubleOptions {
        names: Compare::Exact,
        ..Default::default()
    };
    let r = db.find_doubles(&exact, &cancel, |_, _| {}).unwrap();
    assert_eq!(r.pairs, vec![(1, 2)]);
    // Always the second game.
    let second = DoubleOptions {
        names: Compare::Exact,
        decision: Decision::Second,
        ..Default::default()
    };
    assert_eq!(
        db.find_doubles(&second, &cancel, |_, _| {}).unwrap().pairs,
        vec![(2, 1)]
    );
    // A minimum length leaves shorter games out: the line has 3 moves.
    for (min_moves, pairs) in [(3, 2), (4, 0)] {
        let long = DoubleOptions {
            min_moves,
            ..Default::default()
        };
        assert_eq!(
            db.find_doubles(&long, &cancel, |_, _| {})
                .unwrap()
                .pairs
                .len(),
            pairs
        );
    }
}
