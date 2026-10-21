use cbh_reader::{Database, ReadLimits, style_selection::StyleCriteria, v2::write::Writer};
use chess_core::{GameLimits, parse_pgn};
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn filters_alias_union_before_latest_bounded_sample_and_counts_unknowns() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Style.2cbh");
    let games = parse_pgn(
        br#"[White "Alpha"]
[Black "Other"]
[Date "2024.01.01"]
[BlackElo "2400"]
1. e4 *

[White "Other"]
[Black "Alias"]
[Date "2025.01.01"]
[WhiteElo "2300"]
1. e4 *

[White "Alpha"]
[Black "Other"]
[Date "2026.01.01"]
[BlackElo "1000"]
1. e4 *

[White "Alias"]
[Black "Alpha"]
[Date "2025.01.01"]
[BlackElo "2500"]
1. e4 *

[White "Alpha"]
[Black "Other"]
1. e4 *

[White "Alpha"]
[Black "Other"]
[Date "2027.01.01"]
1. e4 *
"#,
        &GameLimits::default(),
    )
    .unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    w.set_deleted(6, true).unwrap();
    drop(w);
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let a = db.page(1, 1).unwrap().entries[0].white_id.unwrap();
    let b = db.page(2, 1).unwrap().entries[0].black_id.unwrap();
    let p = StyleCriteria {
        player_ids: vec![a, b, a],
        sample_limit: 1,
        min_opponent_elo: Some(2000),
        ..Default::default()
    };
    let s = db
        .select_style_games(&p, &AtomicBool::new(false), |_, _| {})
        .unwrap()
        .unwrap();
    assert_eq!((s.eligible, s.deleted, s.filtered), (3, 1, 2));
    assert_eq!(
        (s.unknown_date, s.unknown_opponent_elo, s.unknown_result),
        (1, 1, 5)
    );
    assert_eq!(s.games.len(), 1);
    assert_eq!(s.games[0].ordinal, 4);
    // The alias-vs-primary entry is counted once, with White precedence.
    assert!(s.games[0].white);
    let stop = AtomicBool::new(false);
    assert!(
        db.select_style_games(&p, &stop, |_, _| stop.store(true, Ordering::Relaxed))
            .unwrap()
            .is_none()
    );
}

#[test]
fn rejects_invalid_limits_and_uses_unknown_dates_after_known_dates() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("Empty.2cbh");
    drop(Writer::create(&p).unwrap());
    let db = Database::open(&p, ReadLimits::default()).unwrap();
    for params in [
        StyleCriteria::default(),
        StyleCriteria {
            player_ids: vec![1],
            sample_limit: 0,
            ..Default::default()
        },
        StyleCriteria {
            player_ids: vec![1],
            sample_limit: 2001,
            ..Default::default()
        },
        StyleCriteria {
            player_ids: vec![1],
            year_from: Some(2026),
            year_to: Some(2020),
            ..Default::default()
        },
    ] {
        assert!(
            db.select_style_games(&params, &AtomicBool::new(false), |_, _| {})
                .is_err()
        );
    }
}
#[test]
fn opponent_rating_range_is_validated() {
    for rating in [0, 4001, u16::MAX] {
        assert!(
            StyleCriteria {
                player_ids: vec![1],
                min_opponent_elo: Some(rating),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    for rating in [1, 4000] {
        assert!(
            StyleCriteria {
                player_ids: vec![1],
                min_opponent_elo: Some(rating),
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
    }
}
#[test]
fn scans_matches_beyond_the_existing_player_statistics_cap_before_filtering() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Large.2cbh");
    let games = parse_pgn(
        b"[White \"Alpha\"]\n[Black \"Other\"]\n[Date \"2020.01.01\"]\n\n1. e4 *",
        &GameLimits::default(),
    )
    .unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    let original = std::fs::read(&path).unwrap();
    let record = &original[192..384];
    let n = 100_002usize;
    let mut bytes = Vec::with_capacity((n + 1) * 192);
    bytes.extend_from_slice(&original[..192]);
    for _ in 0..n {
        bytes.extend_from_slice(record);
    }
    bytes[16..24].copy_from_slice(&((n + 1) as u64).to_le_bytes());
    bytes[n * 192 + 188..n * 192 + 192]
        .copy_from_slice(&((2026u32 << 9) | (1 << 5) | 1).to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let id = db.page(1, 1).unwrap().entries[0].white_id.unwrap();
    let s = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![id],
                sample_limit: 1,
                year_from: Some(2025),
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!((s.eligible, s.filtered), (1, 100_001));
    assert_eq!(s.games[0].ordinal, n as u64);
}
#[test]
fn invalid_calendar_dates_are_unknown_and_do_not_win_the_latest_sample() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Dates.2cbh");
    let games=parse_pgn(b"[White \"Alpha\"]\n[Black \"Other\"]\n[Date \"2024.01.01\"]\n\n1. e4 *\n\n[White \"Alpha\"]\n[Black \"Other\"]\n[Date \"2025.01.01\"]\n\n1. e4 *",&GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[2 * 192 + 188..2 * 192 + 192]
        .copy_from_slice(&((2026u32 << 9) | (13 << 5) | 1).to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let id = db.page(1, 1).unwrap().entries[0].white_id.unwrap();
    let s = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![id],
                sample_limit: 1,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(s.unknown_date, 1);
    assert_eq!(s.games[0].ordinal, 1);
}
