use cbh_reader::{Database, ReadLimits, v2::write::Writer};
use chess_core::{GameLimits, parse_pgn};
use openfile::{
    playing_style::{StyleParams, StyleReport},
    registry::Registry,
    style_dimensions::NativeDimension,
};

const FIVE_MOVES: &str = "1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. d3 d6 5. Nc3 Nf6";

fn game(white: &str, black: &str, result: &str, tags: &str, moves: &str) -> String {
    format!(
        "[White \"{white}\"]\n[Black \"{black}\"]\n[Result \"{result}\"]\n{tags}\n{moves} {result}\n\n"
    )
}

fn fixture(text: &str) -> (tempfile::TempDir, Registry, String, u32) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Style.2cbh");
    let games = parse_pgn(text.as_bytes(), &GameLimits::default()).unwrap();
    let mut writer = Writer::create(&path).unwrap();
    writer.append_all(&games).unwrap();
    drop(writer);
    let mut reader = Database::open(&path, ReadLimits::default()).unwrap();
    let player = reader.page(1, 1).unwrap().entries[0].white_id.unwrap();
    let registry = Registry::default();
    let database = registry
        .open_database(path, "open".into())
        .unwrap()
        .database_id;
    (d, registry, database, player)
}

fn params(player: u32) -> StyleParams {
    StyleParams {
        player_ids: vec![player],
        game_types: 15,
        ..Default::default()
    }
}

fn dimension<'a>(report: &'a StyleReport, id: &str) -> &'a NativeDimension {
    report
        .dimensions
        .iter()
        .find(|dimension| dimension.id == id)
        .unwrap()
}

fn close(actual: Option<f64>, expected: f64) {
    assert!((actual.expect("calculated native value") - expected).abs() < 1e-10);
}

fn assert_eight_dimensions_and_no_proxy_values(report: &StyleReport) {
    let expected = [
        ("score", "Score"),
        ("theory", "Theory"),
        ("decidedGames", "Decided Games"),
        ("tenacity", "Tenacity"),
        ("aggressiveness", "Aggressiveness"),
        ("risk", "Risk"),
        ("positionalPlay", "Positional Play"),
        ("endgameAffinity", "Endgame Affinity"),
    ];
    assert_eq!(report.dimensions.len(), expected.len());
    for (dimension, (id, label)) in report.dimensions.iter().zip(expected) {
        assert_eq!(dimension.id, id);
        assert_eq!(dimension.label, label);
        assert!(dimension.grade.is_none());
        assert!(dimension.grade_reason.contains("uninitialized"));
        if id != "score" && id != "decidedGames" {
            assert!(
                dimension.raw_score.is_none(),
                "{id} cannot use a descriptive proxy"
            );
            assert!(dimension.normalized.is_none());
            assert!(dimension.unavailable_reason.is_some());
            assert!(dimension.evidence.is_empty());
        }
    }
    let wire = serde_json::to_value(report).unwrap();
    assert!(wire.get("dimensions").is_some());
    assert!(wire.get("metrics").is_none());
}

fn assert_source_bound_evidence(
    d: &tempfile::TempDir,
    registry: &Registry,
    database: &str,
    report: &StyleReport,
) {
    let mut reader = Database::open(&d.path().join("Style.2cbh"), ReadLimits::default()).unwrap();
    for id in ["score", "decidedGames"] {
        let evidence = &dimension(report, id).evidence;
        assert!(evidence.len() <= 7);
        for item in evidence {
            let entry = report
                .games
                .iter()
                .find(|game| game.ordinal == item.ordinal)
                .unwrap();
            let source = reader
                .load(item.ordinal.parse().unwrap())
                .unwrap()
                .game
                .unwrap();
            let mut end = 0;
            for _ in 0..400 {
                let Some(&next) = source.nodes[end as usize].children.first() else {
                    break;
                };
                end = next;
            }
            assert_eq!(
                item.node, end,
                "result evidence uses the last legally replayed main-line node"
            );
            assert_eq!(item.ply, entry.plies);
            assert_eq!(source.path_to(item.node).unwrap().len(), item.ply as usize);
            assert_eq!(
                source.san_at(item.node).unwrap().as_deref(),
                Some(item.san.as_str())
            );
            let loaded = registry
                .load_style_entry(
                    database.into(),
                    report.job_id.clone(),
                    item.ordinal.clone(),
                    "evidence".into(),
                )
                .unwrap();
            let document = loaded.document_id.unwrap();
            registry
                .position_at(document.clone(), item.node, "position".into())
                .unwrap();
            registry.close_document(document).unwrap();
        }
    }
}

#[test]
fn native_result_math_uses_both_colours_and_retains_source_bound_evidence() {
    let mut text = String::new();
    for result in ["1-0", "1/2-1/2", "0-1"] {
        text.push_str(&game("Alpha", "Other", result, "", FIVE_MOVES));
    }
    for result in ["0-1", "1/2-1/2"] {
        text.push_str(&game("Other", "Alpha", result, "", FIVE_MOVES));
    }
    let (d, registry, database, player) = fixture(&text);
    let report = registry
        .playing_style(database.clone(), "results".into(), &params(player), |_| {})
        .unwrap()
        .unwrap();
    assert_eq!(
        (report.analysed, report.white_games, report.black_games),
        (5, 3, 2)
    );
    // White: 1.5 points / 3 games; Black: 1.5 / 2.
    close(dimension(&report, "score").raw_score, 0.2545454545454545);
    close(dimension(&report, "score").normalized, 70.18181818181819);
    // Draw fractions 1/3 and 1/2; raw = 2.5 - 1/.543.
    close(
        dimension(&report, "decidedGames").raw_score,
        0.6583793738489871,
    );
    close(
        dimension(&report, "decidedGames").normalized,
        77.22529158993247,
    );
    let tenacity = dimension(&report, "tenacity");
    let short = tenacity
        .inputs
        .iter()
        .find(|i| i.label == "Short draws (stored length below 26)")
        .unwrap();
    close(short.numerator, 2.0);
    assert_eq!(short.denominator, Some(5));
    close(short.value, 0.4);
    close(short.reference, 0.13);
    let length = tenacity
        .inputs
        .iter()
        .find(|i| i.label == "Mean stored draw length")
        .unwrap();
    close(length.numerator, 10.0);
    assert_eq!(length.denominator, Some(2));
    close(length.value, 5.0);
    close(length.reference, 40.1);
    assert!(
        tenacity.raw_score.is_none(),
        "known draw components cannot replace the missing native exchange terms"
    );
    assert_eq!(dimension(&report, "score").status, "available");
    assert_eq!(report.unknown_result, 0);
    assert_eq!(report.unknown_opponent_elo, 5);
    assert_eight_dimensions_and_no_proxy_values(&report);
    assert!(!dimension(&report, "score").evidence.is_empty());
    assert!(!dimension(&report, "decidedGames").evidence.is_empty());
    assert_source_bound_evidence(&d, &registry, &database, &report);
}

#[test]
fn canonical_forfeits_and_unknown_results_follow_native_all_game_denominators() {
    let text = game("Alpha", "Other", "*", "", FIVE_MOVES).repeat(2)
        + &game("Other", "Alpha", "*", "", FIVE_MOVES).repeat(2);
    let (d, registry, database, player) = fixture(&text);
    registry.close_database(database).unwrap();
    let path = d.path().join("Style.2cbh");
    let mut header = std::fs::read(&path).unwrap();
    // 6 White forfeit win; 5 forfeit draw; 4 Black forfeit win; 3 unknown.
    for (ordinal, code) in [(1, 6), (2, 5), (3, 4), (4, 3)] {
        header[ordinal * 192 + 88] = code;
    }
    std::fs::write(&path, header).unwrap();
    let database = registry
        .open_database(path, "reopen".into())
        .unwrap()
        .database_id;
    let report = registry
        .playing_style(database.clone(), "forfeits".into(), &params(player), |_| {})
        .unwrap()
        .unwrap();
    assert_eq!((report.analysed, report.unknown_result), (4, 1));
    // White scores .75; Black scores .5 because the unknown game remains counted.
    close(dimension(&report, "score").raw_score, 0.34545454545454546);
    close(dimension(&report, "score").normalized, 73.81818181818181);
    close(
        dimension(&report, "decidedGames").normalized,
        107.91896869244935,
    );
    for id in ["score", "decidedGames"] {
        let dim = dimension(&report, id);
        assert_eq!(dim.status, "partial");
        assert!(dim.unavailable_reason.as_ref().unwrap().contains("unknown"));
        assert!(dim.inputs.iter().any(|input| input.label.contains("Black")
            && input.label.contains("unknown")
            && input.numerator == Some(1.0)
            && input.denominator == Some(2)));
    }
    assert_source_bound_evidence(&d, &registry, &database, &report);
}

#[test]
fn cancelled_and_obsolete_jobs_cannot_publish_and_close_cancels() {
    let (_d, registry, database, player) = fixture(&game("Alpha", "Other", "1-0", "", FIVE_MOVES));
    let params = params(player);
    let mut fired = false;
    let out = registry
        .playing_style(database.clone(), "old".into(), &params, |_| {
            if !fired {
                fired = true;
                registry
                    .cancel_playing_style(database.clone(), "old".into())
                    .unwrap();
            }
        })
        .unwrap();
    assert!(out.is_none());
    let mut nested = false;
    let out = registry
        .playing_style(database.clone(), "older".into(), &params, |_| {
            if !nested {
                nested = true;
                assert!(
                    registry
                        .playing_style(database.clone(), "newer".into(), &params, |_| {})
                        .unwrap()
                        .is_some()
                );
            }
        })
        .unwrap();
    assert!(out.is_none());
    // An obsolete cancellation must not invalidate the newer report.
    registry
        .cancel_playing_style(database.clone(), "older".into())
        .unwrap();
    registry
        .validate_playing_style(database.clone(), "newer".into())
        .unwrap();
    let mut closed = false;
    let out = registry
        .playing_style(database.clone(), "close".into(), &params, |_| {
            if !closed {
                closed = true;
                registry.close_database(database.clone()).unwrap();
            }
        })
        .unwrap();
    assert!(out.is_none());
}

#[test]
fn verifies_sources_before_publication_and_before_source_bound_navigation() {
    use std::io::Write;
    let (d, registry, database, player) = fixture(&game("Alpha", "Other", "1-0", "", FIVE_MOVES));
    let params = params(player);
    registry
        .playing_style(database.clone(), "visible".into(), &params, |_| {})
        .unwrap()
        .unwrap();
    registry
        .validate_playing_style(database.clone(), "visible".into())
        .unwrap();
    assert!(
        registry
            .validate_playing_style(database.clone(), "other".into())
            .is_err()
    );
    std::fs::OpenOptions::new()
        .append(true)
        .open(d.path().join("Style.2cbh"))
        .unwrap()
        .write_all(&[0])
        .unwrap();
    assert_eq!(
        registry
            .load_style_entry(
                database.clone(),
                "visible".into(),
                "1".into(),
                "stale".into()
            )
            .unwrap_err()
            .code,
        "sourceChanged"
    );
    let out = registry.playing_style(database, "publish".into(), &params, |event| {
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
fn alias_union_preserves_primary_identity_counts_once_and_excludes_deleted_games() {
    let text = game("Alias", "Primary", "1-0", "", FIVE_MOVES)
        + &game("Primary", "Other", "0-1", "", FIVE_MOVES);
    let (d, registry, database, alias) = fixture(&text);
    registry.close_database(database).unwrap();
    let path = d.path().join("Style.2cbh");
    let mut writer = Writer::open(&path).unwrap();
    writer.set_deleted(2, true).unwrap();
    drop(writer);
    let mut reader = Database::open(&path, ReadLimits::default()).unwrap();
    let primary = reader.page(1, 1).unwrap().entries[0].black_id.unwrap();
    assert!(primary > alias);
    let database = registry
        .open_database(path, "reopen".into())
        .unwrap()
        .database_id;
    let report = registry
        .playing_style(
            database,
            "aliases".into(),
            &StyleParams {
                player_ids: vec![primary, alias, primary],
                game_types: 15,
                ..Default::default()
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        report.players.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![primary, alias]
    );
    assert_eq!(report.criteria.player_ids, vec![primary, alias]);
    assert_eq!(
        (
            report.eligible,
            report.deleted,
            report.analysed,
            report.white_games
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(report.games[0].ordinal, "1");
}

#[test]
fn native_minimum_length_is_applied_before_latest_sample_limiting() {
    let text = game(
        "Alpha",
        "Other",
        "1-0",
        "[Date \"2020.01.01\"]\n",
        FIVE_MOVES,
    ) + &game(
        "Alpha",
        "Other",
        "0-1",
        "[Date \"2027.01.01\"]\n",
        "1. e4 e5",
    );
    let (_d, registry, database, player) = fixture(&text);
    let report = registry
        .playing_style(
            database,
            "minimum".into(),
            &StyleParams {
                sample_limit: 1,
                ..params(player)
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            report.too_short,
            report.eligible,
            report.selected,
            report.analysed
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(report.games[0].ordinal, "1");
}

#[test]
fn long_recorded_results_survive_replay_caps_and_variations_do_not_change_evidence_path() {
    let mut text = String::from("[White \"Alpha\"]\n[Black \"Other\"]\n[Result \"1-0\"]\n\n");
    for _ in 0..101 {
        text.push_str("Nf3 Nf6 Ng1 Ng8 ");
    }
    text.push_str("1-0\n\n");
    text.push_str(&game(
        "Other",
        "Alpha",
        "0-1",
        "",
        "1. e4 (1. d4 d5) e5 2. Nf3 Nc6 3. Bc4 Bc5 4. d3 d6 5. Nc3 Nf6",
    ));
    let (d, registry, database, player) = fixture(&text);
    let report = registry
        .playing_style(database.clone(), "cut".into(), &params(player), |_| {})
        .unwrap()
        .unwrap();
    assert_eq!((report.analysed, report.cut_games), (2, 1));
    assert_eq!(
        report.games.iter().find(|game| game.cut).unwrap().plies,
        400
    );
    assert_eq!(
        report.games.iter().find(|game| !game.cut).unwrap().plies,
        10
    );
    close(dimension(&report, "score").raw_score, 1.8);
    close(dimension(&report, "score").normalized, 132.0);
    assert_eight_dimensions_and_no_proxy_values(&report);
    assert_source_bound_evidence(&d, &registry, &database, &report);
}

#[test]
fn legal_castling_and_promotions_are_replayed_without_substituting_positional_scores() {
    let text = game(
        "Alpha",
        "Other",
        "1-0",
        "",
        "1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. O-O Nf6 5. d3 d6",
    ) + &game(
        "Other",
        "Alpha",
        "1/2-1/2",
        "",
        "1. d4 d5 2. Nc3 Nf6 3. Bf4 e6 4. Qd2 Be7 5. O-O-O O-O",
    ) + &game(
        "Alpha",
        "Other",
        "0-1",
        "",
        "1. a4 h5 2. a5 h4 3. a6 h3 4. axb7 hxg2 5. bxa8=Q gxh1=Q",
    );
    let (d, registry, database, player) = fixture(&text);
    let report = registry
        .playing_style(database.clone(), "special".into(), &params(player), |_| {})
        .unwrap()
        .unwrap();
    assert_eq!((report.analysed, report.skipped), (3, 0));
    assert!(report.games.iter().all(|game| game.plies == 10));
    assert_eight_dimensions_and_no_proxy_values(&report);
    assert_source_bound_evidence(&d, &registry, &database, &report);
}

#[test]
fn excludes_setup_handles_en_passant_bounds_evidence_and_reports_an_empty_filtered_sample() {
    let mut text = game(
        "Alpha",
        "Other",
        "1-0",
        "",
        "1. e4 a5 2. e5 d5 3. exd6 Nc6 4. Nf3 exd6 5. Bc4 Nf6",
    )
    .repeat(12);
    text.push_str(&game(
        "Alpha",
        "Other",
        "1-0",
        "[SetUp \"1\"]\n[FEN \"4k3/8/8/8/8/8/P7/4K3 w - - 0 1\"]\n",
        "1. a4 Ke7 2. a5 Kd6 3. a6 Ke5 4. a7 Kd4 5. a8=Q Kc3",
    ));
    let (d, registry, database, player) = fixture(&text);
    let params = params(player);
    let report = registry
        .playing_style(database.clone(), "ep".into(), &params, |_| {})
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
    assert_eq!(dimension(&report, "score").evidence.len(), 7);
    assert_eq!(dimension(&report, "decidedGames").evidence.len(), 7);
    assert_source_bound_evidence(&d, &registry, &database, &report);
    let empty = registry
        .playing_style(
            database,
            "empty".into(),
            &StyleParams {
                year_from: Some(2026),
                ..params
            },
            |_| {},
        )
        .unwrap()
        .unwrap();
    assert_eq!(empty.analysed, 0);
    assert!(
        empty
            .dimensions
            .iter()
            .all(|dimension| dimension.raw_score.is_none()
                && dimension.normalized.is_none()
                && dimension.status == "insufficient")
    );
    assert_eight_dimensions_and_no_proxy_values(&empty);
}

#[test]
fn corrupt_unsupported_partial_and_null_move_records_are_counted_and_excluded() {
    use cbh_reader::v2::block::{MARKER, checksum};
    let text = game("Alpha", "Other", "1-0", "", FIVE_MOVES).repeat(5);
    let (d, registry, database, player) = fixture(&text);
    registry.close_database(database).unwrap();
    let path = d.path().join("Style.2cbh");
    let moves = d.path().join("Style.2cbg");
    let mut header = std::fs::read(&path).unwrap();
    let mut stream = std::fs::read(&moves).unwrap();
    for (ordinal, words) in [
        (1, vec![0xfffc_u16, 0xb02d, 0xffff]),
        (2, vec![0xfff0, 0xffff]),
        (3, vec![0xfffc, 0xad3f, 0xfffe, 0xffff]),
        (4, vec![0xfffc, 0xfffa, 0xffff]),
    ] {
        let payload: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
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
    let database = registry
        .open_database(path, "reopen".into())
        .unwrap()
        .database_id;
    let report = registry
        .playing_style(database.clone(), "bad".into(), &params(player), |_| {})
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
    // Failed move records must not contaminate the surviving game's result inputs.
    assert_eq!(dimension(&report, "score").inputs[0].denominator, Some(1));
    assert_source_bound_evidence(&d, &registry, &database, &report);
}

#[test]
fn partial_date_range_follows_packed_date_order_instead_of_question_mark_sorting() {
    let text = game(
        "Alpha",
        "Other",
        "1-0",
        "[Date \"2025.??.??\"]\n",
        FIVE_MOVES,
    ) + &game(
        "Alpha",
        "Other",
        "1/2-1/2",
        "[Date \"2025.06.01\"]\n",
        FIVE_MOVES,
    );
    let (_d, registry, database, player) = fixture(&text);
    let report = registry
        .playing_style(database, "dates".into(), &params(player), |_| {})
        .unwrap()
        .unwrap();
    assert_eq!(report.first_date.as_deref(), Some("2025.??.??"));
    assert_eq!(report.last_date.as_deref(), Some("2025.06.01"));
    assert_eq!(
        report
            .games
            .iter()
            .map(|game| game.ordinal.as_str())
            .collect::<Vec<_>>(),
        vec!["2", "1"]
    );
}
