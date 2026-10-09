use chess_core::{Game, GameLimits};

#[test]
fn illegal_edit_keeps_tree_unchanged() {
    let mut g = Game::initial();
    let e4 = g.append_uci(0, "e2e4", &GameLimits::default()).unwrap();
    let before = g.clone();
    assert!(g.append_uci(e4, "e7e4", &GameLimits::default()).is_err());
    assert_eq!(g, before);
    assert_eq!(g.san_at(e4).unwrap().as_deref(), Some("e4"));
    assert_eq!(g.position_at(e4).unwrap().turn, "black");
}
#[test]
fn repeated_move_reuses_existing_child_and_new_move_is_sibling() {
    let mut g = Game::initial();
    let a = g.append_uci(0, "e2e4", &GameLimits::default()).unwrap();
    assert_eq!(g.append_uci(0, "e2e4", &GameLimits::default()).unwrap(), a);
    let b = g.append_uci(0, "d2d4", &GameLimits::default()).unwrap();
    assert_eq!(g.nodes[0].children, vec![a, b]);
    assert_eq!(g.nodes.len(), 3);
}
#[test]
fn rejects_invalid_position_and_node() {
    assert!(Game::from_fen("8/8/8/8/8/8/8/8 w - - 0 1").is_err());
    let mut g = Game::initial();
    assert!(g.append_uci(99, "e2e4", &GameLimits::default()).is_err());
    assert!(g.position_at(99).is_err());
}
#[test]
fn null_move_is_allowed_only_outside_check() {
    let mut g = Game::initial();
    let n = g.append_uci(0, "0000", &GameLimits::default()).unwrap();
    assert_eq!(g.position_at(n).unwrap().turn, "black");
    let mut checked = Game::from_fen("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1").unwrap();
    assert!(
        checked
            .append_uci(0, "0000", &GameLimits::default())
            .is_err()
    );
}
#[test]
fn node_budget_rejects_growth_without_rejecting_existing_move() {
    let mut g = Game::initial();
    let limits = GameLimits {
        max_nodes: 2,
        ..GameLimits::default()
    };
    let n = g.append_uci(0, "e2e4", &limits).unwrap();
    assert_eq!(g.append_uci(0, "e2e4", &limits).unwrap(), n);
    assert!(g.append_uci(n, "e7e5", &limits).is_err());
    assert_eq!(g.nodes.len(), 2);
}
#[test]
fn detects_cyclic_tree_before_replaying() {
    let mut g = Game::initial();
    let n = g.append_uci(0, "e2e4", &GameLimits::default()).unwrap();
    g.nodes[n as usize].parent = Some(n);
    assert!(g.position_at(n).is_err());
}
#[test]
fn promotion_and_castling_use_standard_uci() {
    let mut g = Game::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let n = g.append_uci(0, "a7a8n", &GameLimits::default()).unwrap();
    assert_eq!(g.san_at(n).unwrap().as_deref(), Some("a8=N"));
    let mut g = Game::from_fen("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1").unwrap();
    let n = g.append_uci(0, "e1g1", &GameLimits::default()).unwrap();
    assert_eq!(g.san_at(n).unwrap().as_deref(), Some("O-O"));
}
