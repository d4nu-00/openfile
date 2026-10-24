use chess_core::{Game, GameLimits, parse_pgn, write_pgn};
use std::collections::BTreeMap;

fn game(pgn: &str) -> Game {
    parse_pgn(pgn.as_bytes(), &GameLimits::default())
        .unwrap()
        .remove(0)
}

/// The id of the node reached by these SAN moves, following any line.
fn find(g: &Game, sans: &[&str]) -> u32 {
    let mut node = 0;
    for san in sans {
        node = *g.nodes[node as usize]
            .children
            .iter()
            .find(|&&c| g.san_at(c).unwrap().as_deref() == Some(*san))
            .unwrap_or_else(|| panic!("no {san}"));
    }
    node
}

fn moves(g: &Game) -> String {
    let pgn = write_pgn(g).unwrap();
    pgn.split("\n\n").nth(1).unwrap().trim().to_string()
}

const TREE: &str =
    "[Result \"*\"]\n\n1. e4 e5 (1... c5 2. Nf3 (2. Nc3 Nc6) 2... d6) 2. Nf3 Nc6 3. Bb5 *\n";

#[test]
fn deletes_the_variation_of_a_move() {
    let mut g = game(TREE);
    // From inside the nested 2.Nc3 line: only that line goes.
    let nc6 = find(&g, &["e4", "c5", "Nc3", "Nc6"]);
    let selected = g.delete_variation(nc6).unwrap();
    assert_eq!(selected, find(&g, &["e4", "c5"]));
    assert_eq!(
        moves(&g),
        "1. e4 e5 (1... c5 2. Nf3 d6) 2. Nf3 Nc6 3. Bb5 *"
    );
    // From the Sicilian: the whole variation goes.
    let d6 = find(&g, &["e4", "c5", "Nf3", "d6"]);
    assert_eq!(g.delete_variation(d6).unwrap(), find(&g, &["e4"]));
    assert_eq!(moves(&g), "1. e4 e5 2. Nf3 Nc6 3. Bb5 *");
    // The main line is not a variation.
    let bb5 = find(&g, &["e4", "e5", "Nf3", "Nc6", "Bb5"]);
    assert!(g.delete_variation(bb5).is_err());
    // Ids stay dense and consistent.
    for (i, n) in g.nodes.iter().enumerate() {
        assert_eq!(n.id as usize, i);
        for &c in &n.children {
            assert_eq!(g.nodes[c as usize].parent, Some(n.id));
        }
    }
}

#[test]
fn promotes_a_variation_one_level() {
    let mut g = game(TREE);
    let nc3 = find(&g, &["e4", "c5", "Nc3"]);
    g.promote_variation(nc3).unwrap();
    assert_eq!(
        moves(&g),
        "1. e4 e5 (1... c5 2. Nc3 (2. Nf3 d6) 2... Nc6) 2. Nf3 Nc6 3. Bb5 *"
    );
    let c5 = find(&g, &["e4", "c5"]);
    g.promote_variation(c5).unwrap();
    assert_eq!(
        moves(&g),
        "1. e4 c5 (1... e5 2. Nf3 Nc6 3. Bb5) 2. Nc3 (2. Nf3 d6) 2... Nc6 *"
    );
}

#[test]
fn cuts_moves_before_and_after() {
    let mut g = game(TREE);
    let nf3 = find(&g, &["e4", "e5", "Nf3"]);
    assert_eq!(g.delete_remaining(nf3).unwrap(), nf3);
    assert_eq!(
        moves(&g),
        "1. e4 e5 (1... c5 2. Nf3 (2. Nc3 Nc6) 2... d6) 2. Nf3 *"
    );

    let mut g = game(TREE);
    let e5 = find(&g, &["e4", "e5"]);
    assert_eq!(g.delete_previous(e5).unwrap(), 0);
    assert_eq!(
        g.start_fen,
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
    );
    assert_eq!(g.tags["SetUp"], "1");
    assert_eq!(moves(&g), "2. Nf3 Nc6 3. Bb5 *");
}

#[test]
fn edits_game_data_with_checks() {
    let mut g = Game::initial();
    let mut tags: BTreeMap<String, String> = [
        ("White", "Carlsen, Magnus"),
        ("Black", "Caruana, Fabiano"),
        ("WhiteElo", "2830"),
        ("Date", "2024.04.??"),
        ("Result", "1/2-1/2"),
        ("ECO", "C65"),
        ("Event", ""),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect();
    g.set_tags(tags.clone()).unwrap();
    assert_eq!(g.tags["White"], "Carlsen, Magnus");
    // Required tags get their defaults; empty optional ones go.
    assert_eq!(g.tags["Event"], "?");
    assert_eq!(g.tags["Site"], "?");
    assert!(write_pgn(&g).unwrap().contains("[WhiteElo \"2830\"]"));
    for (key, value) in [
        ("Result", "2-0"),
        ("WhiteElo", "strong"),
        ("Date", "2024-04-01"),
        ("Bad Key", "x"),
        ("Annotator", "two\nlines"),
    ] {
        tags.insert(key.into(), value.into());
        let before = g.clone();
        assert!(g.set_tags(tags.clone()).is_err(), "{key}");
        assert_eq!(g, before);
        tags.remove(key);
    }
    // The start position's tags cannot be changed by hand.
    tags.insert("FEN".into(), "8/8/8/8/8/8/8/8 w - - 0 1".into());
    g.set_tags(tags).unwrap();
    assert!(!g.tags.contains_key("FEN"));
}

#[test]
fn text_before_moves() {
    let mut g = game(TREE);
    let e4 = find(&g, &["e4"]);
    g.set_pre_comments(e4, vec!["The king's pawn".into()])
        .unwrap();
    assert!(g.set_pre_comments(e4, vec!["{".into()]).is_err());
    assert!(g.set_pre_comments(0, vec!["x".into()]).is_err());
    assert_eq!(g.nodes[e4 as usize].pre_comments, ["The king's pawn"]);
}

#[test]
fn marks_are_set_checked_and_written() {
    use chess_core::{ArrowMark, SquareMark};
    let mut g = game("1. e4 e5 *\n");
    let sq = |c, s: &str| SquareMark {
        color: c,
        square: s.into(),
    };
    let ar = |c, f: &str, t: &str| ArrowMark {
        color: c,
        from: f.into(),
        to: t.into(),
    };
    // A square or an arrow given twice keeps its last colour.
    g.set_marks(
        1,
        vec![sq('G', "e4"), sq('R', "d5"), sq('Y', "e4")],
        vec![ar('G', "g1", "f3"), ar('R', "g1", "f3")],
    )
    .unwrap();
    assert_eq!(g.nodes[1].squares, [sq('R', "d5"), sq('Y', "e4")]);
    assert_eq!(g.nodes[1].arrows, [ar('R', "g1", "f3")]);
    assert!(moves(&g).contains("[%csl Rd5,Ye4]"));
    assert!(moves(&g).contains("[%cal Rg1f3]"));
    // The start position can be marked too.
    g.set_marks(0, vec![sq('B', "a1")], vec![]).unwrap();
    assert_eq!(g.nodes[0].squares, [sq('B', "a1")]);
    // Unknown colours, squares and zero-length arrows are refused.
    for (squares, arrows) in [
        (vec![sq('X', "e4")], vec![]),
        (vec![sq('G', "i9")], vec![]),
        (vec![], vec![ar('G', "e2", "e2")]),
    ] {
        assert!(g.set_marks(2, squares, arrows).is_err());
    }
    assert!(g.set_marks(9, vec![], vec![]).is_err());
    g.clear_marks();
    assert!(
        g.nodes
            .iter()
            .all(|n| n.squares.is_empty() && n.arrows.is_empty())
    );
}

#[test]
fn engine_lines_follow_existing_moves_and_change_nothing_when_illegal() {
    let mut g = game(TREE);
    let line = |m: &[&str]| m.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // 1.e4 e5 exists; 2.Bc4 is new beside 2.Nf3, so it becomes a variation.
    let e5 = find(&g, &["e4", "e5"]);
    let last = g
        .append_line(
            0,
            &line(&["e2e4", "e7e5", "f1c4", "g8f6"]),
            &GameLimits::default(),
        )
        .unwrap();
    assert_eq!(g.nodes[e5 as usize].children.len(), 2);
    assert_eq!(find(&g, &["e4", "e5", "Bc4", "Nf6"]), last);
    // An illegal move anywhere leaves the game as it was.
    let before = g.clone();
    assert!(
        g.append_line(0, &line(&["d2d4", "e7e5", "e1e8"]), &GameLimits::default())
            .is_err()
    );
    assert_eq!(g, before);
    assert!(g.append_line(0, &[], &GameLimits::default()).is_err());
}

#[test]
fn insert_into_game_puts_a_variation_before_the_moves_it_replaced() {
    // 2. Nf3 Nc6 is replaced by 2. h3 h6 2... then the old moves follow.
    let mut g = game("[Result \"*\"]\n\n1. e4 e5 2. Nf3 (2. h3 h6) 2... Nc6 (2... d6) 3. Bb5 *\n");
    let h6 = find(&g, &["e4", "e5", "h3", "h6"]);
    let last = g.insert_variation(h6).unwrap();
    assert_eq!(moves(&g), "1. e4 e5 2. h3 h6 3. Nf3 Nc6 (3... d6) 4. Bb5 *");
    assert_eq!(g.san_at(last).unwrap().as_deref(), Some("h6"));
    // Every child has a higher id than its parent.
    assert!(g.nodes.iter().all(|n| n.parent.is_none_or(|p| p < n.id)));
    // An odd number of moves, or moves that no longer fit, are refused.
    let mut odd = game("[Result \"*\"]\n\n1. e4 e5 2. Nf3 (2. h3) 2... Nc6 *\n");
    let h3 = find(&odd, &["e4", "e5", "h3"]);
    let before = odd.clone();
    assert!(odd.insert_variation(h3).is_err());
    assert_eq!(odd, before);
    let mut blocked = game("[Result \"*\"]\n\n1. e4 e5 2. Nf3 (2. Nc3 Nf6) 2... Nc6 *\n");
    let nf6 = find(&blocked, &["e4", "e5", "Nc3", "Nf6"]);
    // 3. Nf3 is still legal, but 3... Nc6 is fine too: this one fits.
    assert!(blocked.insert_variation(nf6).is_ok());
    let mut illegal = game("[Result \"*\"]\n\n1. e4 e5 2. Nf3 (2. Qh5 Nc6) 2... Nc6 3. Bb5 *\n");
    let nc6 = find(&illegal, &["e4", "e5", "Qh5", "Nc6"]);
    // After 2. Qh5 Nc6, 3. Nf3 Nc6 would play a knight from b8 twice.
    let before = illegal.clone();
    assert!(illegal.insert_variation(nc6).is_err());
    assert_eq!(illegal, before);
    // The main line has no variation to insert.
    let mut main = game(TREE);
    assert!(main.insert_variation(find(&main, &["e4"])).is_err());
}

#[test]
fn commentary_is_deleted_by_kind() {
    use chess_core::edit::Commentary;
    let pgn = "[Result \"*\"]\n\n{Intro} 1. e4 $1 {Best by test} {[%clk 0:05:00] fast} e5 {+0.35/22} (1... c5 {Sicilian}) 2. Nf3 {[%eval 0.3] [%emt 0:00:02]} *\n";
    let mut text = game(pgn);
    let nf3 = find(&text, &["e4", "e5", "Nf3"]);
    assert_eq!(text.delete_commentary(Commentary::Text, nf3).unwrap(), nf3);
    // The move time and evaluation are not text: they stay.
    assert_eq!(
        moves(&text),
        "1. e4 $1 1... e5 (1... c5) 2. Nf3 {[%emt 0:00:02] [%eval 0.30]} *"
    );

    let mut evals = game(pgn);
    evals
        .delete_commentary(Commentary::EvaluationAndTime, nf3)
        .unwrap();
    assert_eq!(
        moves(&evals),
        "{Intro} 1. e4 $1 {Best by test} {fast} 1... e5 (1... c5 {Sicilian}) 2. Nf3 *"
    );

    let mut all = game(pgn);
    let c5 = find(&all, &["e4", "c5"]);
    // The selected move was in a variation: its ancestor e4 is selected.
    let kept = all.delete_commentary(Commentary::All, c5).unwrap();
    assert_eq!(moves(&all), "1. e4 e5 2. Nf3 *");
    assert_eq!(all.san_at(kept).unwrap().as_deref(), Some("e4"));
}

#[test]
fn null_moves_are_entered_outside_check() {
    let mut g = game("[Result \"*\"]\n\n1. e4 e5 *\n");
    let e5 = find(&g, &["e4", "e5"]);
    let null = g.append_uci(e5, "0000", &GameLimits::default()).unwrap();
    assert_eq!(g.san_at(null).unwrap().as_deref(), Some("--"));
    let mut check = game("[Result \"*\"]\n\n1. e4 f5 2. Qh5+ *\n");
    let qh5 = find(&check, &["e4", "f5", "Qh5+"]);
    assert!(
        check
            .append_uci(qh5, "0000", &GameLimits::default())
            .is_err()
    );
}

#[test]
fn move_extras_are_set_cleared_and_kept_in_pgn() {
    use chess_core::edit::ExtraEdit;
    let mut g = game("[Result \"*\"]\n\n1. e4 e5 2. Nf3 *\n");
    let nf3 = find(&g, &["e4", "e5", "Nf3"]);
    g.set_extra(nf3, ExtraEdit::Medal { bits: 8 }).unwrap();
    g.set_extra(nf3, ExtraEdit::PiecePath { on: true }).unwrap();
    g.set_extra(
        nf3,
        ExtraEdit::MoveTime {
            time: Some(0x0001_0500),
        },
    )
    .unwrap();
    let x = g.nodes[nf3 as usize].extras.as_ref().unwrap();
    // f3 is square code 5 × 8 + 2 + 1.
    assert_eq!(x.piece_path, vec![3, 43]);
    assert_eq!(x.move_time_text().as_deref(), Some("0:01:05"));
    // The start has no move: no piece path or time there.
    assert!(g.set_extra(0, ExtraEdit::PiecePath { on: true }).is_err());
    assert!(
        g.set_extra(
            nf3,
            ExtraEdit::MoveTime {
                time: Some(0x0000_4000)
            }
        )
        .is_err()
    );
    g.set_extra(nf3, ExtraEdit::Medal { bits: 0 }).unwrap();
    g.set_extra(nf3, ExtraEdit::PiecePath { on: false })
        .unwrap();
    g.set_extra(nf3, ExtraEdit::MoveTime { time: None })
        .unwrap();
    assert!(g.nodes[nf3 as usize].extras.is_none());
    // PGN keeps move times and evaluations.
    let pgn = "[Result \"*\"]\n\n1. e4 {[%emt 0:00:44] [%eval -1.25,18] Good} e5 {[%eval #-3]} *\n";
    let read = game(pgn);
    let e4 = &read.nodes[find(&read, &["e4"]) as usize];
    assert_eq!(e4.comments, ["Good"]);
    let x = e4.extras.as_ref().unwrap();
    assert_eq!(x.move_time_text().as_deref(), Some("0:00:44"));
    assert_eq!(x.evaluation_fields(), Some((-125, 0, 18)));
    let written = chess_core::write_pgn(&read).unwrap();
    assert!(
        written.contains("[%emt 0:00:44] [%eval -1.25,18]"),
        "{written}"
    );
    assert!(written.contains("[%eval #-3]"), "{written}");
}
