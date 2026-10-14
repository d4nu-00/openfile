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
