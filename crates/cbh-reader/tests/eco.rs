use cbh_reader::{
    Database, ReadLimits,
    eco::EcoOptions,
    v2::write::{Writer, pack_eco},
};
use chess_core::GameLimits;
use std::sync::atomic::AtomicBool;

fn pgn(text: &str) -> chess_core::Game {
    chess_core::parse_pgn(text.as_bytes(), &GameLimits::default())
        .unwrap()
        .remove(0)
}

fn ecos(path: &std::path::Path) -> Vec<Option<String>> {
    let mut db = Database::open(path, ReadLimits::default()).unwrap();
    (1..=db.info().entry_count)
        .map(|o| db.load(o).unwrap().summary.eco)
        .collect()
}

#[test]
fn set_eco_codes_classifies_main_lines_and_keeps_sub_codes() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Eco.2cbh");
    let mut w = Writer::create(&path).unwrap();
    // 1: a Petrov with its code; 2: a Najdorf without one; 3: a Queen's
    // Gambit with a wrong code; 4: a set-up; 5: a Petrov reached through a
    // variation's main line.
    w.append(&pgn("[ECO \"C42\"]\n\n1. e4 e5 2. Nf3 Nf6 *"))
        .unwrap();
    w.append(&pgn(
        "1. e4 c5 2. Nf3 d6 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6 *",
    ))
    .unwrap();
    w.append(&pgn("[ECO \"A00\"]\n\n1. d4 d5 2. c4 e6 3. Nc3 Nf6 *"))
        .unwrap();
    w.append(&pgn(
        "[SetUp \"1\"]\n[FEN \"4k3/8/8/8/8/8/4P3/4K3 w - - 0 1\"]\n\n1. e4 *",
    ))
    .unwrap();
    w.append(&pgn("1. e4 (1. d4) e5 2. Nf3 Nf6 *")).unwrap();
    // Game 1's code gets a ChessBase sub-classification (low seven bits).
    w.set_eco_many(&[(1, pack_eco("C42") | 5)]).unwrap();
    drop(w);

    let db = Database::open(&path, ReadLimits::default()).unwrap();
    let r = db
        .classify_eco(EcoOptions::default(), &AtomicBool::new(false), |_, _| {})
        .unwrap();
    assert_eq!(r.games, 5);
    assert_eq!(r.unchanged, 1);
    assert_eq!(r.unclassified, 1);
    assert_eq!(
        r.changes,
        vec![
            (2, pack_eco("B90")),
            (3, pack_eco("D35")),
            (5, pack_eco("C42"))
        ]
    );
    assert_eq!((r.had_code, r.agreed), (2, 1));
    // Only games without a code: game 3 keeps its (wrong) code.
    let only = db
        .classify_eco(
            EcoOptions { only_missing: true },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
    assert_eq!(only.kept, 1);
    assert_eq!(only.changes.len(), 2);
    drop(db);

    let mut w = Writer::open(&path).unwrap();
    w.set_eco_many(&r.changes).unwrap();
    // A word beyond E99 is refused before anything is written.
    assert!(w.set_eco_many(&[(2, 501 << 7)]).is_err());
    drop(w);
    assert_eq!(
        ecos(&path),
        [Some("C42"), Some("B90"), Some("D35"), None, Some("C42")].map(|e| e.map(String::from))
    );
    // Game 1 kept its sub-classification (record 1, bytes 128-129).
    let bytes = std::fs::read(&path).unwrap();
    let raw = u16::from_le_bytes([bytes[192 + 128], bytes[192 + 129]]);
    assert_eq!(raw, pack_eco("C42") | 5);
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    // A cancelled run reports nothing.
    let cancelled = db
        .classify_eco(EcoOptions::default(), &AtomicBool::new(true), |_, _| {})
        .unwrap();
    assert!(cancelled.cancelled);
    assert!(cancelled.changes.is_empty());
}
