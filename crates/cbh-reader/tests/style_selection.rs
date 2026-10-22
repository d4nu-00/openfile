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
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *

[White "Other"]
[Black "Alias"]
[Date "2025.01.01"]
[WhiteElo "2300"]
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *

[White "Alpha"]
[Black "Other"]
[Date "2026.01.01"]
[BlackElo "1000"]
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *

[White "Alias"]
[Black "Alpha"]
[Date "2025.01.01"]
[BlackElo "2500"]
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *

[White "Alpha"]
[Black "Other"]
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *

[White "Alpha"]
[Black "Other"]
[Date "2027.01.01"]
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *
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
        game_types: 15,
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
    let mut db = Database::open(&p, ReadLimits::default()).unwrap();
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
        b"[White \"Alpha\"]\n[Black \"Other\"]\n[Date \"2020.01.01\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *",
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
                game_types: 15,
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
    let games=parse_pgn(b"[White \"Alpha\"]\n[Black \"Other\"]\n[Date \"2024.01.01\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *\n\n[White \"Alpha\"]\n[Black \"Other\"]\n[Date \"2025.01.01\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *",&GameLimits::default()).unwrap();
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
                game_types: 15,
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

#[test]
fn native_stored_length_filter_precedes_the_recent_sample_limit() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Length.2cbh");
    let games = parse_pgn(
        br#"[White "Alpha"]
[Black "Other"]
[Date "2024.01.01"]
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 1-0

[White "Alpha"]
[Black "Other"]
[Date "2025.01.01"]
1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 1/2-1/2
"#,
        &GameLimits::default(),
    )
    .unwrap();
    let mut writer = Writer::create(&path).unwrap();
    writer.append_all(&games).unwrap();
    drop(writer);
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let player = db.page(1, 1).unwrap().entries[0].white_id.unwrap();
    let selected = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![player],
                game_types: 15,
                sample_limit: 1,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(selected.eligible, 1);
    assert_eq!(selected.too_short, 1);
    assert_eq!(selected.games[0].ordinal, 1);
    assert_eq!(selected.games[0].stored_moves, 5);
}

const CATEGORY_MOVES: &str = "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *";

fn category_fixture(cases: &[(&str, Option<u16>)]) -> (tempfile::TempDir, Database, u32) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Categories.2cbh");
    let mut text = String::new();
    for (index, (event, _)) in cases.iter().enumerate() {
        text.push_str(&format!("[White \"Alpha\"]\n[Black \"Other\"]\n[Event \"{event}\"]\n[Date \"{}.01.01\"]\n\n{CATEGORY_MOVES}\n\n", 2020 + index));
    }
    let games = parse_pgn(text.as_bytes(), &GameLimits::default()).unwrap();
    let mut writer = Writer::create(&path).unwrap();
    writer.append_all(&games).unwrap();
    drop(writer);
    let mut reader = Database::open(&path, ReadLimits::default()).unwrap();
    let entries = reader.page(1, cases.len()).unwrap().entries;
    let player = entries[0].white_id.unwrap();
    drop(reader);
    let lid = path.with_extension("2lid");
    let mut bytes = std::fs::read(&lid).unwrap();
    let head = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    let slots: Vec<usize> = (0..6)
        .map(|kind| {
            u32::from_be_bytes(bytes[8 + kind * 20..12 + kind * 20].try_into().unwrap()) as usize
        })
        .collect();
    let group: usize = slots.iter().sum();
    for (entry, (_, flags)) in entries.iter().zip(cases) {
        let record = head + entry.tournament_id.unwrap() as usize * group + slots[0];
        let mut at = record + 4;
        for _ in 0..2 {
            let length = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            at += 4 + length;
        }
        if let Some(flags) = flags {
            bytes[at + 4..at + 6].copy_from_slice(&flags.to_le_bytes());
        } else {
            // Legacy name-only records must not acquire flags from zero padding.
            bytes[record..record + 4].copy_from_slice(&((at - record - 4) as u32).to_le_bytes());
        }
    }
    std::fs::write(&lid, bytes).unwrap();
    (
        dir,
        Database::open(&path, ReadLimits::default()).unwrap(),
        player,
    )
}

#[test]
fn native_categories_use_verified_flags_names_and_simul_blitz_rapid_precedence() {
    let cases = [
        ("Classical", Some(0)),
        ("Unlabelled fast", Some(0x20)),
        ("Unlabelled medium", Some(0x40)),
        ("Unlabelled exhibition", Some(7)),
        ("Mixed RAPID BLITZ", Some(0x40)),
        ("Night Blitz sim tables", Some(0x20)),
        ("Bullet final", Some(0)),
        ("Rapid final", Some(0)),
        ("BLITZ legacy without flags", None),
        ("Classical type5", Some(5)),
        ("Simul begins", Some(0)),
        ("Simul heading", Some(0x20)),
    ];
    let (_dir, mut db, player) = category_fixture(&cases);
    let expected = [
        Some(4),
        Some(1),
        Some(2),
        Some(8),
        Some(1),
        Some(8),
        Some(1),
        Some(2),
        None,
        Some(4),
        Some(4),
        Some(1),
    ];
    for (mask, eligible) in [(1, 4), (2, 2), (4, 3), (8, 2), (15, 12)] {
        let report = db
            .select_style_games(
                &StyleCriteria {
                    player_ids: vec![player],
                    game_types: mask,
                    ..Default::default()
                },
                &AtomicBool::new(false),
                |_, _| {},
            )
            .unwrap()
            .unwrap();
        assert_eq!(report.eligible, eligible, "mask {mask}");
        assert_eq!(report.unknown_category, 1);
        for game in &report.games {
            assert_eq!(game.category, expected[game.ordinal as usize - 1]);
        }
    }
    // Default Long and latest-one limiting must happen after category filtering.
    let report = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![player],
                sample_limit: 1,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(report.games[0].ordinal, 11);
    assert_eq!(db.tournament_metadata(9).unwrap().flags, Some(0x103));
    assert_eq!(db.tournament_metadata(8).unwrap().flags, None);
}

#[test]
fn invalid_game_type_masks_are_rejected_and_serde_default_is_long() {
    for mask in [0, 16, 255] {
        assert!(
            StyleCriteria {
                player_ids: vec![1],
                game_types: mask,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    for mask in 1..=15 {
        assert!(
            StyleCriteria {
                player_ids: vec![1],
                game_types: mask,
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
    }
    let params: StyleCriteria =
        serde_json::from_value(serde_json::json!({"playerIds":[1]})).unwrap();
    assert_eq!(params.game_types, 4);
    assert_eq!(serde_json::to_value(params).unwrap()["gameTypes"], 4);
}

#[test]
fn unknown_categories_are_included_only_by_explicit_all_types_choice() {
    let (_dir, mut db, player) = category_fixture(&[("BLITZ missing metadata", None)]);
    let selected = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![player],
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            selected.eligible,
            selected.filtered,
            selected.unknown_category
        ),
        (0, 1, 1)
    );
    let selected = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![player],
                game_types: 15,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(selected.eligible, 1);
    assert_eq!(selected.games[0].category, None);
    let selected = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![player],
                game_types: 1,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        selected.eligible, 0,
        "a keyword cannot replace unavailable flags"
    );
}

#[test]
fn new_writer_tournament_metadata_has_an_explicit_date_and_long_flags() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Written.2cbh");
    let games=parse_pgn(format!("[Event \"Festival\"]\n[Date \"2025.01.01\"]\n[White \"Alpha\"]\n[Black \"Other\"]\n\n{CATEGORY_MOVES}").as_bytes(),&GameLimits::default()).unwrap();
    let mut writer = Writer::create(&path).unwrap();
    writer.append_all(&games).unwrap();
    drop(writer);
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let entry = db.page(1, 1).unwrap().entries.remove(0);
    let metadata = db
        .tournament_metadata(entry.tournament_id.unwrap())
        .unwrap();
    assert_eq!(metadata.flags, Some(0));
    let report = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![entry.white_id.unwrap()],
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!((report.eligible, report.unknown_category), (1, 0));
    assert_eq!(report.games[0].category, Some(4));
}

#[test]
fn cancellation_interrupts_a_header_batch_before_more_metadata_reads() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Cancel.2cbh");
    let games = parse_pgn(
        b"[White \"Alpha\"]\n[Black \"Other\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *",
        &GameLimits::default(),
    ).unwrap();
    let mut writer = Writer::create(&path).unwrap();
    writer.append_all(&games).unwrap();
    drop(writer);
    let original = std::fs::read(&path).unwrap();
    let mut bytes = original[..192].to_vec();
    for _ in 0..1024 {
        bytes.extend_from_slice(&original[192..384]);
    }
    bytes[16..24].copy_from_slice(&1025u64.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let id = db.page(1, 1).unwrap().entries[0].white_id.unwrap();
    let stop = AtomicBool::new(false);
    let mut scanned = Vec::new();
    let selected = db
        .select_style_games(
            &StyleCriteria {
                player_ids: vec![id],
                game_types: 15,
                ..Default::default()
            },
            &stop,
            |at, _| {
                scanned.push(at);
                if at == 256 {
                    stop.store(true, Ordering::Relaxed);
                }
            },
        )
        .unwrap();
    assert!(
        selected.is_none(),
        "Stop must interrupt the current header batch"
    );
    assert_eq!(scanned, vec![0, 256]);
}
