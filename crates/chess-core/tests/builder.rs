use chess_core::{Game, GameBuilder, GameLimits};
#[test]
fn long_mainline_and_branches_replay_correctly() {
    let mut b = GameBuilder::new(Game::initial(), GameLimits::default()).unwrap();
    let mut current = 0;
    for _ in 0..300 {
        for u in ["g1f3", "g8f6", "f3g1", "f6g8"] {
            current = b.append_uci(current, u).unwrap();
        }
    }
    let alt = b.append_uci(0, "e2e4").unwrap();
    assert_eq!(b.game().nodes.len(), 1202);
    let g = b.finish();
    assert_eq!(g.position_at(current).unwrap().turn, "white");
    assert_eq!(g.position_at(alt).unwrap().turn, "black");
    assert_eq!(g.nodes[0].children, vec![1, alt]);
}
#[test]
fn builder_rejects_invalid_move_and_node_without_growth() {
    let mut b = GameBuilder::new(
        Game::initial(),
        GameLimits {
            max_nodes: 2,
            ..GameLimits::default()
        },
    )
    .unwrap();
    let e4 = b.append_uci(0, "e2e4").unwrap();
    assert!(b.append_uci(0, "e2e5").is_err());
    assert!(b.append_uci(99, "e2e4").is_err());
    assert!(b.append_uci(e4, "e7e5").is_err());
    assert_eq!(b.game().nodes.len(), 2);
}
