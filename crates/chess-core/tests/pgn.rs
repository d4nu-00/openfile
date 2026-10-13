use chess_core::{Game, GameLimits, parse_pgn, write_pgn};

#[test]
fn pgn_preserves_ordered_alternatives_and_comments() {
    let bytes = b"[Event \"Synthetic\"]\n[Result \"*\"]\n\n1. e4 {centre} e5 (1... c5 $1) 2. Nf3 *";
    let limits = GameLimits::default();
    let g = parse_pgn(bytes, &limits).unwrap().remove(0);
    let e4 = g.nodes[0].children[0];
    assert_eq!(g.nodes[e4 as usize].comments, vec!["centre"]);
    let replies = &g.nodes[e4 as usize].children;
    assert_eq!(replies.len(), 2);
    assert_eq!(g.nodes[replies[0] as usize].uci.as_deref(), Some("e7e5"));
    assert_eq!(g.nodes[replies[1] as usize].uci.as_deref(), Some("c7c5"));
    assert_eq!(g.nodes[replies[1] as usize].nags, vec![1]);
    let text = write_pgn(&g).unwrap();
    assert_eq!(parse_pgn(text.as_bytes(), &limits).unwrap()[0], g);
}
#[test]
fn multigame_and_escaped_tags_round_trip() {
    let bytes = br#"[Event "Quote\" and \\ path"]
[Result "1-0"]

{start} 1. e4 1-0

[Event "Second"]
[Result "*"]

1. d4 *"#;
    let games = parse_pgn(bytes, &GameLimits::default()).unwrap();
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].tags["Event"], "Quote\" and \\ path");
    assert_eq!(games[0].nodes[0].comments, vec!["start"]);
    for g in games {
        assert_eq!(
            parse_pgn(write_pgn(&g).unwrap().as_bytes(), &GameLimits::default()).unwrap()[0],
            g
        );
    }
}
#[test]
fn black_to_move_setup_is_preserved() {
    let bytes =
        b"[SetUp \"1\"]\n[FEN \"4k3/8/8/8/8/8/8/R3K3 b Q - 0 17\"]\n[Result \"*\"]\n\n17... Ke7 *";
    let g = parse_pgn(bytes, &GameLimits::default()).unwrap().remove(0);
    assert_eq!(g.nodes[1].uci.as_deref(), Some("e8e7"));
    let text = write_pgn(&g).unwrap();
    assert!(text.contains("17... Ke7"));
    assert_eq!(
        parse_pgn(text.as_bytes(), &GameLimits::default()).unwrap()[0],
        g
    );
}
#[test]
fn unrepresentable_comment_keeps_previous_comment() {
    let mut g = Game::initial();
    g.set_comments(0, vec!["keep me".into()]).unwrap();
    assert!(g.set_comments(0, vec!["brace }".into()]).is_err());
    assert_eq!(g.nodes[0].comments, vec!["keep me"]);
}
#[test]
fn rejects_illegal_san_unbalanced_variation_and_conflicting_result() {
    for text in ["1. e5 *", "1. e4 (1. d4 *", "[Result \"1-0\"]\n\n1. e4 0-1"] {
        assert!(
            parse_pgn(text.as_bytes(), &GameLimits::default()).is_err(),
            "{text}"
        );
    }
}
#[test]
fn caps_bytes_nodes_and_variation_depth() {
    let limits = GameLimits {
        max_input_bytes: 4,
        ..GameLimits::default()
    };
    assert!(parse_pgn(b"1. e4 *", &limits).is_err());
    let limits = GameLimits {
        max_nodes: 2,
        ..GameLimits::default()
    };
    assert!(parse_pgn(b"1. e4 e5 *", &limits).is_err());
    let limits = GameLimits {
        max_variation_depth: 1,
        ..GameLimits::default()
    };
    assert!(parse_pgn(b"1. e4 (1. d4 d5 (1... Nf6)) *", &limits).is_err());
}
#[test]
fn validates_comment_and_nag_node_ids() {
    let mut g = Game::initial();
    assert!(g.set_comments(8, vec!["bad".into()]).is_err());
    assert!(g.set_nags(8, vec![1]).is_err());
    g.set_nags(0, vec![1, 2]).unwrap();
    assert_eq!(g.nodes[0].nags, vec![1, 2]);
}
#[test]
fn nested_branches_round_trip_and_retain_order() {
    let g = parse_pgn(
        b"1. e4 e5 (1... c5 2. Nf3 (2. Nc3)) 2. Nf3 *",
        &GameLimits::default(),
    )
    .unwrap()
    .remove(0);
    let text = write_pgn(&g).unwrap();
    assert_eq!(
        parse_pgn(text.as_bytes(), &GameLimits::default()).unwrap()[0],
        g
    );
    assert_eq!(g.nodes.len(), 7);
}
#[test]
fn long_comment_stays_one_comment() {
    let text = "x".repeat(20_000);
    let input = format!("1. e4 {{{text}}} *");
    let g = parse_pgn(input.as_bytes(), &GameLimits::default())
        .unwrap()
        .remove(0);
    assert_eq!(g.nodes[1].comments, vec![text]);
}
#[test]
fn caps_collection_count() {
    let input = "[Event \"Synthetic\"]\n\n*\n\n".repeat(2_001);
    assert!(parse_pgn(input.as_bytes(), &GameLimits::default()).is_err());
}
#[test]
fn a_created_setup_game_gets_pgn_setup_tags() {
    let g = Game::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    assert_eq!(g.tags.get("SetUp").map(String::as_str), Some("1"));
    assert_eq!(g.tags.get("FEN"), Some(&g.start_fen));
}
#[test]
fn repeated_move_occurrences_preserve_annotations_and_mainline() {
    let input = b"1. e4 $1 {main} (1. e4 $2 {variation} c5) e5 *";
    let game = chess_core::parse_pgn(input, &chess_core::GameLimits::default())
        .unwrap()
        .remove(0);
    assert_eq!(game.nodes[0].children.len(), 2);
    let main = game.nodes[0].children[0] as usize;
    let variation = game.nodes[0].children[1] as usize;
    assert_eq!(game.nodes[main].uci, game.nodes[variation].uci);
    assert_eq!(game.nodes[main].comments, ["main"]);
    assert_eq!(game.nodes[main].nags, [1]);
    assert_eq!(game.nodes[variation].comments, ["variation"]);
    assert_eq!(game.nodes[variation].nags, [2]);
    assert_eq!(
        game.nodes[game.nodes[main].children[0] as usize]
            .uci
            .as_deref(),
        Some("e7e5")
    );
    assert_eq!(
        game.nodes[game.nodes[variation].children[0] as usize]
            .uci
            .as_deref(),
        Some("c7c5")
    );
    let text = chess_core::write_pgn(&game).unwrap();
    let reopened = chess_core::parse_pgn(text.as_bytes(), &chess_core::GameLimits::default())
        .unwrap()
        .remove(0);
    assert_eq!(reopened, game);
}
#[test]
fn marks_and_text_before_moves_round_trip() {
    let bytes = b"[Event \"Marks\"]\n[Result \"*\"]\n\n1. e4 {[%csl Gd4,Re5] [%cal Gg1f3] centre} e5 ({idea} 1... c5 {[%cal Yb8c6]}) 2. Nf3 {[%cal Xa1a2] kept} *";
    let limits = GameLimits::default();
    let g = parse_pgn(bytes, &limits).unwrap().remove(0);
    let e4 = &g.nodes[g.nodes[0].children[0] as usize];
    assert_eq!(e4.comments, vec!["centre"]);
    assert_eq!(e4.squares.len(), 2);
    assert_eq!(
        (e4.squares[1].color, e4.squares[1].square.as_str()),
        ('R', "e5")
    );
    assert_eq!(
        (
            e4.arrows[0].color,
            e4.arrows[0].from.as_str(),
            e4.arrows[0].to.as_str()
        ),
        ('G', "g1", "f3")
    );
    let c5 = &g.nodes[e4.children[1] as usize];
    assert_eq!(c5.pre_comments, vec!["idea"]);
    assert!(c5.comments.is_empty());
    assert_eq!(c5.arrows[0].color, 'Y');
    let e5 = &g.nodes[e4.children[0] as usize];
    let nf3 = &g.nodes[e5.children[0] as usize];
    assert_eq!(nf3.comments, vec!["[%cal Xa1a2] kept"]);
    assert!(nf3.arrows.is_empty());
    let text = write_pgn(&g).unwrap();
    assert!(text.contains("{idea} 1... c5"), "{text}");
    assert_eq!(parse_pgn(text.as_bytes(), &limits).unwrap()[0], g);
}
