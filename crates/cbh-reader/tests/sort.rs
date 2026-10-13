use cbh_reader::{
    Database, ReadLimits,
    sort::{SortColumn, SortOptions, SortOrder, collation_key},
    v2::write::Writer,
};
use chess_core::{GameLimits, parse_pgn};
use std::sync::atomic::AtomicBool;

const GAMES: &str = r#"[Event "Zurich"]
[Site "Zurich"]
[Date "2001.03.04"]
[Round "2"]
[White "Ánand, Viswanathan"]
[Black "Zeta, Ana"]
[Result "0-1"]
[WhiteElo "2700"]
[ECO "C42"]

1. e4 e5 0-1

[Event "Amsterdam"]
[Site "Amsterdam"]
[Date "1999.??.??"]
[Round "1"]
[White "Carlsen, Magnus"]
[Black "Anand, Viswanathan"]
[Result "1/2-1/2"]
[WhiteElo "2850"]
[BlackElo "2750"]
[ECO "B90"]

1. e4 c5 1/2-1/2

[Event "Zurich"]
[Site "Zurich"]
[Date "2001.03.01"]
[Round "1"]
[White "Bauer, Max"]
[Black "Carlsen, Magnus"]
[Result "1-0"]
[ECO "A00"]

1. a3 1-0

[Event "Amsterdam"]
[Site "Amsterdam"]
[Date "2010.01.01"]
[Round "1.2"]
[White "anand, viswanathan"]
[Black "Bauer, Max"]
[Result "1-0"]
[WhiteElo "2650"]
[ECO "C42"]

1. d4 1-0"#;

fn db() -> (tempfile::TempDir, Database) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Sort.2cbh");
    let games = parse_pgn(GAMES.as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    let db = Database::open(&path, ReadLimits::default()).unwrap();
    (d, db)
}

fn sorted(db: &Database, dir: &std::path::Path, column: SortColumn) -> Vec<u64> {
    let path = dir.join(format!("{}.cwsort", column.name()));
    let options = SortOptions {
        workers: 3,
        ..SortOptions::default()
    };
    assert!(
        SortOrder::build(
            db,
            column,
            &path,
            &options,
            &AtomicBool::new(false),
            |_, _, _| {}
        )
        .unwrap()
    );
    let mut order = SortOrder::open(&path, db, column).unwrap().unwrap();
    order.read(0, 100, false).unwrap()
}

#[test]
fn columns_sort_by_their_keys_and_ties_keep_entry_order() {
    let (d, db) = db();
    let dir = d.path().join("orders");
    // Names fold case and accents, so entries 1 and 4 tie and keep entry order.
    assert_eq!(sorted(&db, &dir, SortColumn::White), [1, 4, 3, 2]);
    assert_eq!(sorted(&db, &dir, SortColumn::Black), [2, 4, 3, 1]);
    // Unrated games first.
    assert_eq!(sorted(&db, &dir, SortColumn::WhiteElo), [3, 4, 1, 2]);
    assert_eq!(sorted(&db, &dir, SortColumn::Date), [2, 3, 1, 4]);
    // 1-0, then the draw, then 0-1.
    assert_eq!(sorted(&db, &dir, SortColumn::Result), [3, 4, 2, 1]);
    assert_eq!(sorted(&db, &dir, SortColumn::Eco), [3, 2, 1, 4]);
    // Tournaments by name, each in round order.
    assert_eq!(sorted(&db, &dir, SortColumn::Tournament), [2, 4, 3, 1]);
    assert_eq!(sorted(&db, &dir, SortColumn::Round), [2, 3, 4, 1]);
}

#[test]
fn reading_descending_places_and_arranging_a_search() {
    let (d, db) = db();
    let path = d.path().join("date.cwsort");
    SortOrder::build(
        &db,
        SortColumn::Date,
        &path,
        &SortOptions::default(),
        &AtomicBool::new(false),
        |_, _, _| {},
    )
    .unwrap();
    let mut order = SortOrder::open(&path, &db, SortColumn::Date)
        .unwrap()
        .unwrap();
    assert_eq!(order.len(), 4);
    assert_eq!(order.read(0, 10, true).unwrap(), [4, 1, 3, 2]);
    assert_eq!(order.read(1, 2, true).unwrap(), [1, 3]);
    assert_eq!(order.read(3, 5, false).unwrap(), [4]);
    assert!(order.read(4, 5, false).unwrap().is_empty());
    assert_eq!(order.place_of(1, false).unwrap(), Some(2));
    assert_eq!(order.place_of(1, true).unwrap(), Some(1));
    assert_eq!(order.place_of(9, false).unwrap(), None);
    // Indexes into the hits [4, 1, 2], in date order: 2 (1999), 1, 4.
    let arranged = order
        .arrange(&[4, 1, 2], false, &AtomicBool::new(false))
        .unwrap()
        .unwrap();
    assert_eq!(arranged, [2, 1, 0]);
    assert_eq!(
        order
            .arrange(&[4, 1, 2], false, &AtomicBool::new(true))
            .unwrap(),
        Some(vec![2, 1, 0]),
        "a list shorter than the check interval finishes"
    );
}

#[test]
fn a_changed_database_or_another_column_is_not_current() {
    let (d, db) = db();
    let path = d.path().join("white.cwsort");
    SortOrder::build(
        &db,
        SortColumn::White,
        &path,
        &SortOptions::default(),
        &AtomicBool::new(false),
        |_, _, _| {},
    )
    .unwrap();
    assert!(
        SortOrder::open(&path, &db, SortColumn::Black)
            .unwrap()
            .is_none()
    );
    assert!(
        SortOrder::open(&path, &db, SortColumn::White)
            .unwrap()
            .is_some()
    );
    let games = parse_pgn(GAMES.as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::open(&d.path().join("Sort.2cbh")).unwrap();
    w.append(&games[0]).unwrap();
    drop(w);
    let db = Database::open(&d.path().join("Sort.2cbh"), ReadLimits::default()).unwrap();
    assert!(
        SortOrder::open(&path, &db, SortColumn::White)
            .unwrap()
            .is_none()
    );
    // A missing file is simply absent.
    assert!(
        SortOrder::open(&d.path().join("none.cwsort"), &db, SortColumn::White)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_cancelled_build_leaves_nothing_and_large_databases_are_refused() {
    let (d, db) = db();
    let path = d.path().join("orders").join("eco.cwsort");
    let built = SortOrder::build(
        &db,
        SortColumn::Eco,
        &path,
        &SortOptions::default(),
        &AtomicBool::new(true),
        |_, _, _| {},
    )
    .unwrap();
    assert!(!built);
    assert!(!path.exists());
    let leftovers = std::fs::read_dir(path.parent().unwrap())
        .map(|r| r.count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0);
    let small = SortOptions {
        max_entries: 2,
        ..SortOptions::default()
    };
    let refused = SortOrder::build(
        &db,
        SortColumn::Eco,
        &path,
        &small,
        &AtomicBool::new(false),
        |_, _, _| {},
    );
    assert!(refused.is_err());
}

#[test]
fn collation_folds_case_and_common_accents() {
    assert_eq!(collation_key(" Ánand "), "anand");
    assert_eq!(collation_key("Müller"), "muller");
    assert_eq!(collation_key("Straße"), "strasse");
    assert_eq!(collation_key("Łódź"), "lodz");
}

mod classic {
    use super::*;
    use std::fs;

    /// Three games with ratings and dates, then a text entry.
    fn family() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let mut h = vec![0; 46 * 5];
        h[..6].copy_from_slice(&[0, 0, 0x2c, 0, 0x2e, 1]);
        h[9] = 5;
        let games = [(2400u16, 2001u32), (0, 1990), (2700, 2001)];
        for (i, (elo, year)) in games.iter().enumerate() {
            let r = &mut h[(i + 1) * 46..(i + 2) * 46];
            r[0] = 1;
            r[24..27].copy_from_slice(&(year << 9).to_be_bytes()[1..]);
            r[27] = 2;
            r[31..33].copy_from_slice(&elo.to_be_bytes());
        }
        h[4 * 46] = 3;
        fs::write(d.path().join("Synthetic.cbh"), h).unwrap();
        d
    }

    #[test]
    fn classic_records_sort_and_text_entries_come_first() {
        let d = family();
        let db = Database::open(&d.path().join("Synthetic.cbh"), ReadLimits::default()).unwrap();
        // The unrated game and the text entry both have key 0.
        assert_eq!(sorted(&db, d.path(), SortColumn::WhiteElo), [2, 4, 1, 3]);
        assert_eq!(sorted(&db, d.path(), SortColumn::Date), [4, 2, 1, 3]);
        // Without a player table every game has the unknown rank.
        assert_eq!(sorted(&db, d.path(), SortColumn::White), [1, 2, 3, 4]);
    }
}
