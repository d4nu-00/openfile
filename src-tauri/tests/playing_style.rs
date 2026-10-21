use cbh_reader::{Database, ReadLimits, v2::write::Writer};
use chess_core::{GameLimits, parse_pgn};
use openfile::{
    playing_style::{StyleParams, StyleReport},
    registry::Registry,
};
fn fixture(text: &str) -> (tempfile::TempDir, Registry, String, u32) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Style.2cbh");
    let games = parse_pgn(text.as_bytes(), &GameLimits::default()).unwrap();
    let mut w = Writer::create(&path).unwrap();
    w.append_all(&games).unwrap();
    drop(w);
    let mut reader = Database::open(&path, ReadLimits::default()).unwrap();
    let id = reader.page(1, 1).unwrap().entries[0].white_id.unwrap();
    drop(reader);
    let r = Registry::default();
    let db = r.open_database(path, "o".into()).unwrap().database_id;
    (d, r, db, id)
}
fn metric(r: &StyleReport, id: &str) -> (u64, u64) {
    let m = r.metrics.iter().find(|m| m.id == id).unwrap();
    (m.numerator, m.denominator)
}
#[test]
fn counts_recorded_checks_captures_both_colours_and_unknown_metadata() {
    let (_d, r, db, id) = fixture(
        r#"[White "Alpha"]
[Black "Other"]

1. e4 e5 2. Qh5 Nc6 3. Bc4 Nf6 4. Qxf7# *

[White "Other"]
[Black "Alpha"]

1. f3 e5 2. g4 Qh4# *"#,
    );
    let p = StyleParams {
        player_ids: vec![id],
        ..Default::default()
    };
    let report = r
        .playing_style(db, "one".into(), &p, |_| {})
        .unwrap()
        .unwrap();
    assert_eq!(
        (report.analysed, report.white_games, report.black_games),
        (2, 1, 1)
    );
    assert_eq!(metric(&report, "checks"), (2, 6));
    assert_eq!(metric(&report, "captures"), (1, 6));
    assert_eq!(metric(&report, "earlyQueen"), (2, 2));
    assert_eq!(metric(&report, "kingsideCastle"), (0, 0));
    assert_eq!(metric(&report, "endgame"), (0, 2));
    assert_eq!(report.unknown_result, 2);
    assert_eq!(report.unknown_opponent_elo, 2);
    let checks = report.metrics.iter().find(|m| m.id == "checks").unwrap();
    assert_eq!((checks.white.numerator, checks.black.numerator), (1, 1));
    assert!((checks.value.unwrap() - 100.0 / 3.0).abs() < 0.00001);
    for e in &checks.evidence {
        assert!(e.node > 0);
        assert!(e.san.ends_with('#'));
    }
}
#[test]
fn cancelled_and_obsolete_jobs_cannot_publish_and_close_cancels() {
    let (_d, r, db, id) = fixture(
        r#"[White "Alpha"]
[Black "Other"]

1. e4 e5 *"#,
    );
    let p = StyleParams {
        player_ids: vec![id],
        ..Default::default()
    };
    let mut fired = false;
    let out = r
        .playing_style(db.clone(), "old".into(), &p, |_| {
            if !fired {
                fired = true;
                r.cancel_playing_style(db.clone(), "old".into()).unwrap();
            }
        })
        .unwrap();
    assert!(out.is_none());
    let mut nested = false;
    let out = r
        .playing_style(db.clone(), "older".into(), &p, |_| {
            if !nested {
                nested = true;
                assert!(
                    r.playing_style(db.clone(), "newer".into(), &p, |_| {})
                        .unwrap()
                        .is_some()
                );
            }
        })
        .unwrap();
    assert!(out.is_none());
    let mut closed = false;
    let out = r
        .playing_style(db.clone(), "close".into(), &p, |_| {
            if !closed {
                closed = true;
                r.close_database(db.clone()).unwrap();
            }
        })
        .unwrap();
    assert!(out.is_none());
}
#[test]
fn sustained_deficit_is_observed_on_the_reply_even_if_the_record_ends_there() {
    let (_d, r, db, id) = fixture(
        r#"[White "Alpha"]
[Black "Other"]

1. e4 e5 2. Qh5 Nc6 3. Qxe5+ Nxe5 4. Nf3 Nc6 *"#,
    );
    let report = r
        .playing_style(
            db,
            "deficit".into(),
            &StyleParams {
                player_ids: vec![id],
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(metric(&report, "sustainedDeficit"), (1, 1));
    assert_eq!(
        report
            .metrics
            .iter()
            .find(|m| m.id == "sustainedDeficit")
            .unwrap()
            .evidence[0]
            .ply,
        8
    );
}
#[test]
fn verifies_sources_before_publishing_and_again_before_evidence_navigation() {
    use std::io::Write;
    let (d, r, db, id) = fixture(
        r#"[White "Alpha"]
[Black "Other"]

1. e4 e5 *"#,
    );
    let p = StyleParams {
        player_ids: vec![id],
        ..Default::default()
    };
    r.playing_style(db.clone(), "visible".into(), &p, |_| {})
        .unwrap()
        .unwrap();
    r.validate_playing_style(db.clone(), "visible".into())
        .unwrap();
    assert!(
        r.validate_playing_style(db.clone(), "other".into())
            .is_err()
    );
    std::fs::OpenOptions::new()
        .append(true)
        .open(d.path().join("Style.2cbh"))
        .unwrap()
        .write_all(&[0])
        .unwrap();
    assert_eq!(
        r.validate_playing_style(db.clone(), "visible".into())
            .unwrap_err()
            .code,
        "sourceChanged"
    );
    // A fresh handle sees the changed header. Mutate it again at the final callback.
    let out = r.playing_style(db, "publish".into(), &p, |event| {
        if event.stage == "replay" && event.done == event.total {
            std::fs::OpenOptions::new()
                .append(true)
                .open(d.path().join("Style.2cbh"))
                .unwrap()
                .write_all(&[0])
                .unwrap();
        }
    });
    assert_eq!(out.unwrap_err().code, "sourceChanged");
}
#[test]
fn keeps_primary_identity_first_when_alias_has_a_smaller_id() {
    let (d, r, db, first) = fixture(
        r#"[White "Alias"]
[Black "Primary"]

1. e4 e5 *"#,
    );
    let mut reader = Database::open(&d.path().join("Style.2cbh"), ReadLimits::default()).unwrap();
    let primary = reader.page(1, 1).unwrap().entries[0].black_id.unwrap();
    assert!(primary > first);
    let report = r
        .playing_style(
            db,
            "aliases".into(),
            &StyleParams {
                player_ids: vec![primary, first, primary],
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        report.players.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![primary, first]
    );
    assert_eq!(report.criteria.player_ids, vec![primary, first]);
}
#[test]
fn castling_windows_and_opposite_sides_use_observable_denominators() {
    let (_d, r, db, id) = fixture(
        r#"[White "Alpha"]
[Black "Other"]

1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. O-O Nf6 *

[White "Alpha"]
[Black "Other"]

1. d4 d5 2. Nc3 Nf6 3. Bf4 e6 4. Qd2 Be7 5. O-O-O O-O *

[White "Other"]
[Black "Alpha"]

1. e4 e5 *"#,
    );
    let report = r
        .playing_style(
            db,
            "castles".into(),
            &StyleParams {
                player_ids: vec![id],
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(metric(&report, "kingsideCastle"), (1, 2));
    assert_eq!(metric(&report, "queensideCastle"), (1, 2));
    assert_eq!(metric(&report, "oppositeCastle"), (1, 1));
    let m = report
        .metrics
        .iter()
        .find(|m| m.id == "queensideCastle")
        .unwrap();
    assert_eq!(m.black.denominator, 0);
}
#[test]
fn capped_games_do_not_supply_absence_denominators_and_variations_do_not_count() {
    let mut text = String::from("[White \"Alpha\"]\n[Black \"Other\"]\n\n");
    for _ in 0..101 {
        text.push_str("Nf3 Nf6 Ng1 Ng8 ");
    }
    text.push('*');
    text.push_str("\n\n[White \"Alpha\"]\n[Black \"Other\"]\n\n1. e4 (1. d4 d5) e5 *");
    let (_d, r, db, id) = fixture(&text);
    let report = r
        .playing_style(
            db,
            "cut".into(),
            &StyleParams {
                player_ids: vec![id],
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(report.cut_games, 1);
    assert_eq!(report.games.iter().find(|g| g.cut).unwrap().plies, 400);
    assert_eq!(metric(&report, "checks"), (0, 201));
    assert_eq!(metric(&report, "endgame"), (0, 1));
    assert_eq!(metric(&report, "sustainedDeficit"), (0, 1));
    assert_eq!(metric(&report, "imbalance20"), (0, 1));
    assert_eq!(metric(&report, "queenless30"), (0, 1));
    assert_eq!(metric(&report, "earlyQueen"), (0, 1));
}
#[test]
fn handles_empty_sample_setup_and_en_passant_and_bounds_evidence() {
    let mut text = String::new();
    for _ in 0..12 {
        text.push_str("[White \"Alpha\"]\n[Black \"Other\"]\n\n1. e4 a5 2. e5 d5 3. exd6 *\n\n");
    }
    text.push_str("[White \"Alpha\"]\n[Black \"Other\"]\n[SetUp \"1\"]\n[FEN \"4k3/8/8/8/8/8/P7/4K3 w - - 0 1\"]\n\n1. a4 *");
    let (_d, r, db, id) = fixture(&text);
    let p = StyleParams {
        player_ids: vec![id],
        ..Default::default()
    };
    let report = r
        .playing_style(db.clone(), "ep".into(), &p, |_| {})
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            report.selected,
            report.analysed,
            report.skipped_setup,
            report.skipped
        ),
        (13, 12, 1, 1)
    );
    assert_eq!(metric(&report, "captures"), (12, 36));
    assert_eq!(
        report
            .metrics
            .iter()
            .find(|m| m.id == "captures")
            .unwrap()
            .evidence
            .len(),
        8
    );
    let empty = r
        .playing_style(
            db,
            "empty".into(),
            &StyleParams {
                year_from: Some(2026),
                ..p
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(empty.analysed, 0);
    assert!(
        empty
            .metrics
            .iter()
            .all(|m| m.value.is_none() && m.status == "none")
    );
}
#[test]
fn corrupt_unsupported_partial_and_null_move_records_are_counted_and_excluded() {
    use cbh_reader::v2::block::{MARKER, checksum};
    let text = "[White \"Alpha\"]\n[Black \"Other\"]\n\n1. e4 *\n\n".repeat(5);
    let (d, r, db, id) = fixture(&text);
    r.close_database(db).unwrap();
    let path = d.path().join("Style.2cbh");
    let moves = d.path().join("Style.2cbg");
    let mut header = std::fs::read(&path).unwrap();
    let mut stream = std::fs::read(&moves).unwrap();
    // New frames: a damaged move, an unsupported start, uninterpreted move flag,
    // and a syntactically supported null move. The fifth stays valid.
    for (ordinal, words) in [
        (1, vec![0xfffc_u16, 0xb02d, 0xffff]),
        (2, vec![0xfff0, 0xffff]),
        (3, vec![0xfffc, 0xad3f, 0xfffe, 0xffff]),
        (4, vec![0xfffc, 0xfffa, 0xffff]),
    ] {
        let payload: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let offset = stream.len() as u64;
        let padding = (8 - (payload.len() + 34) % 8) % 8;
        let total = (34 + payload.len() + padding) as u64;
        stream.extend(MARKER.to_le_bytes());
        stream.extend((payload.len() as u32).to_le_bytes());
        stream.extend((padding as u32).to_le_bytes());
        stream.extend(checksum(&payload).to_le_bytes());
        stream.extend(1u16.to_le_bytes());
        stream.extend(payload);
        stream.extend(vec![0; padding]);
        stream.extend(total.to_le_bytes());
        header[ordinal * 192 + 8..ordinal * 192 + 16].copy_from_slice(&offset.to_le_bytes());
    }
    let len = stream.len() as u64;
    stream[..8].copy_from_slice(&len.to_le_bytes());
    std::fs::write(&moves, stream).unwrap();
    std::fs::write(&path, header).unwrap();
    let db = r.open_database(path, "reopen".into()).unwrap().database_id;
    let report = r
        .playing_style(
            db,
            "bad".into(),
            &StyleParams {
                player_ids: vec![id],
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            report.analysed,
            report.skipped,
            report.skipped_unreadable,
            report.skipped_unsupported,
            report.skipped_partial
        ),
        (1, 4, 2, 1, 1)
    );
    assert_eq!(metric(&report, "captures"), (0, 1));
}
#[test]
fn queenless_imbalanced_endgames_and_material_deficits_have_exact_positions() {
    // Deterministically generated legal capture sequence, kept explicit so its
    // piece inventories and event positions can be independently reviewed.
    let (d, r, db, id) = fixture(
        r#"[White "Alpha"]
[Black "Other"]

1. g3 Nh6 2. b4 f6 3. a4 c6 4. Nf3 e6 5. b5 cxb5 6. axb5 d5 7. Rxa7 Rxa7 8. e4 dxe4 9. Ba3 Qxd2+ 10. Kxd2 Bxa3 11. Nxa3 Rxa3 12. c3 Rxc3 13. Kxc3 exf3 14. Qxf3 Kd8 15. Qxf6+ gxf6 16. Kb2 Nd7 17. Be2 Rf8 18. Bh5 Ke7 19. Kc1 Nb6 20. Kc2 f5 21. Be2 Rf6 22. Kb1 e5 23. Rc1 Na4 24. Rxc8 *"#,
    );
    let p = StyleParams {
        player_ids: vec![id],
        ..Default::default()
    };
    let report = r
        .playing_style(db.clone(), "material".into(), &p, |_| {})
        .unwrap()
        .unwrap();
    for key in ["queenless30", "imbalance20", "sustainedDeficit", "endgame"] {
        assert_eq!(metric(&report, key), (1, 1), "{key}");
    }
    for (key, ply) in [
        ("queenless30", 30),
        ("imbalance20", 40),
        ("sustainedDeficit", 16),
        ("endgame", 47),
    ] {
        let m = report.metrics.iter().find(|m| m.id == key).unwrap();
        assert_eq!(m.evidence[0].ply, ply, "{key}");
    }
    let mut reader = Database::open(&d.path().join("Style.2cbh"), ReadLimits::default()).unwrap();
    let black = reader.page(1, 1).unwrap().entries[0].black_id.unwrap();
    let report = r
        .playing_style(
            db,
            "materialBlack".into(),
            &StyleParams {
                player_ids: vec![black],
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(metric(&report, "endgame"), (1, 1));
    assert_eq!(metric(&report, "queenless30"), (1, 1));
    assert_eq!(report.black_games, 1);
}

#[test]
fn partial_date_range_follows_packed_date_order_instead_of_question_mark_sorting() {
    let (_d, r, db, id) = fixture(
        r#"[White "Alpha"]
[Black "Other"]
[Date "2025.??.??"]

1. e4 *

[White "Alpha"]
[Black "Other"]
[Date "2025.06.01"]

1. e4 *"#,
    );
    let report = r
        .playing_style(
            db,
            "dates".into(),
            &StyleParams {
                player_ids: vec![id],
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(report.first_date.as_deref(), Some("2025.??.??"));
    assert_eq!(report.last_date.as_deref(), Some("2025.06.01"));
    assert_eq!(
        report
            .games
            .iter()
            .map(|g| g.ordinal.as_str())
            .collect::<Vec<_>>(),
        vec!["2", "1"]
    );
}

#[test]
fn endgame_and_deficit_events_observed_before_the_cap_remain_positive_evidence() {
    let mut text = String::from(
        "[White \"Alpha\"]\n[Black \"Other\"]\n\n1. g3 Nh6 2. b4 f6 3. a4 c6 4. Nf3 e6 5. b5 cxb5 6. axb5 d5 7. Rxa7 Rxa7 8. e4 dxe4 9. Ba3 Qxd2+ 10. Kxd2 Bxa3 11. Nxa3 Rxa3 12. c3 Rxc3 13. Kxc3 exf3 14. Qxf3 Kd8 15. Qxf6+ gxf6 16. Kb2 Nd7 17. Be2 Rf8 18. Bh5 Ke7 19. Kc1 Nb6 20. Kc2 f5 21. Be2 Rf6 22. Kb1 e5 23. Rc1 Na4 24. Rxc8 Nb6 ",
    );
    for _ in 0..100 {
        text.push_str("Bf1 Na4 Be2 Nb6 ");
    }
    text.push('*');
    let (_d, r, db, id) = fixture(&text);
    let report = r
        .playing_style(
            db,
            "positiveCut".into(),
            &StyleParams {
                player_ids: vec![id],
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(report.cut_games, 1);
    assert_eq!(metric(&report, "endgame"), (1, 1));
    assert_eq!(metric(&report, "sustainedDeficit"), (1, 1));
}
