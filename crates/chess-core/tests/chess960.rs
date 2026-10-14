use chess_core::{
    GameBuilder, GameLimits,
    model::{Game, parse_position},
    parse_pgn, write_pgn,
};
use shakmaty::{CastlingMode, Color, Piece, Position, Role, Square};

/// The white king on b1 with rooks on a1 and h1, castling both ways; Black
/// as usual. Shredder-FEN letters name the rook files.
const START: &str = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/RK5R w AHah - 0 1";

#[test]
fn chess960_positions_castle_king_to_rook() {
    let pos = parse_position(START).unwrap();
    assert_eq!(pos.castles().mode(), CastlingMode::Chess960);
    // Standard positions stay standard.
    let standard =
        parse_position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
    assert_eq!(standard.castles().mode(), CastlingMode::Standard);
    let game = Game::from_fen(START).unwrap();
    assert!(game.is_chess960());
    assert_eq!(
        game.tags.get("Variant").map(String::as_str),
        Some("Chess960")
    );
    let mut b = GameBuilder::new(game, GameLimits::default()).unwrap();
    let legal = b.game().position_at(0).unwrap().legal_uci;
    // O-O is the king onto its h1 rook; O-O-O onto a1 (d1 must be free: it is).
    assert!(legal.contains(&"b1h1".to_string()), "{legal:?}");
    assert!(legal.contains(&"b1a1".to_string()), "{legal:?}");
    let id = b.append_uci(0, "b1h1").unwrap();
    let game = b.finish();
    assert_eq!(game.san_at(id).unwrap().as_deref(), Some("O-O"));
    let pos = game.chess_position(id).unwrap();
    let white = |role| {
        Some(Piece {
            color: Color::White,
            role,
        })
    };
    assert_eq!(pos.board().piece_at(Square::G1), white(Role::King));
    assert_eq!(pos.board().piece_at(Square::F1), white(Role::Rook));
    assert_eq!(pos.board().piece_at(Square::A1), white(Role::Rook));
}

#[test]
fn chess960_pgn_round_trips() {
    let pgn = format!(
        "[Variant \"Chess960\"]\n[SetUp \"1\"]\n[FEN \"{START}\"]\n\n1. O-O-O O-O 2. a3 Kh8 *\n"
    );
    let games = parse_pgn(pgn.as_bytes(), &GameLimits::default()).unwrap();
    let game = &games[0];
    assert!(game.is_chess960());
    let ucis: Vec<_> = game.nodes[1..]
        .iter()
        .map(|n| n.uci.clone().unwrap())
        .collect();
    // Black castles in the standard way, written king to rook as well.
    assert_eq!(ucis, ["b1a1", "e8h8", "a2a3", "g8h8"]);
    let out = write_pgn(game).unwrap();
    assert!(out.contains("[Variant \"Chess960\"]"), "{out}");
    assert!(out.contains("1. O-O-O O-O 2. a3 Kh8"), "{out}");
    let again = parse_pgn(out.as_bytes(), &GameLimits::default()).unwrap();
    assert_eq!(again[0].nodes, game.nodes);
}
