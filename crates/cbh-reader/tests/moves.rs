mod support;
use cbh_reader::{ReadLimits, moves::decode_classic};
use support::encoder;
fn line(g: &chess_core::Game) -> Vec<String> {
    let mut out = vec![];
    let mut parent = 0;
    while let Some(&n) = g.nodes[parent].children.first() {
        out.push(g.nodes[n as usize].uci.clone().unwrap());
        parent = n as usize;
    }
    out
}
#[test]
fn literal_e4_e5_decodes() {
    let g = decode_classic(&[0, 0, 0, 7, 0xff, 0x00, 0x0e], &ReadLimits::default()).unwrap();
    assert_eq!(line(&g), vec!["e2e4", "e7e5"]);
    assert_eq!(
        g.position_at(2).unwrap().fen,
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
    );
}
#[test]
fn literal_two_byte_e4_decodes() {
    let g = decode_classic(
        &[0, 0, 0, 8, 0x29, 0x47, 0xfc, 0x0d],
        &ReadLimits::default(),
    )
    .unwrap();
    assert_eq!(line(&g), vec!["e2e4"]);
}
#[test]
fn captures_renumber_surviving_knight() {
    let prefix = [
        "d2d4", "d7d5", "b1c3", "b8c6", "c1f4", "c8f5", "c3b5", "c6b4", "b5c7", "d8c7", "f4c7",
        "e7e6",
    ];
    let mut body = vec![];
    let mut count = 0;
    for u in prefix {
        encoder::two_byte(&mut body, &mut count, u);
    }
    // First white knight was captured; g1 knight now uses ordinal zero (fa).
    encoder::primary(&mut body, &mut count, 0xfa);
    let g = decode_classic(&encoder::finish(body, count), &ReadLimits::default()).unwrap();
    let mut expected = prefix.to_vec();
    expected.push("g1f3");
    assert_eq!(line(&g), expected);
}
#[test]
fn both_castling_directions_update_rook_positions() {
    let short = ["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5"];
    let long = [
        "d2d4", "d7d5", "b1c3", "b8c6", "c1f4", "c8f5", "d1d2", "d8d7",
    ];
    for (prefix, token, uci, rookmove) in [
        (short.as_slice(), 0x76, "e1g1", "f1e1"),
        (long.as_slice(), 0xb5, "e1c1", "d1e1"),
    ] {
        let mut body = vec![];
        let mut count = 0;
        for u in prefix {
            encoder::two_byte(&mut body, &mut count, u);
        }
        encoder::primary(&mut body, &mut count, token);
        encoder::two_byte(&mut body, &mut count, "a7a6");
        encoder::two_byte(&mut body, &mut count, rookmove);
        let g = decode_classic(&encoder::finish(body, count), &ReadLimits::default()).unwrap();
        let mut expected = prefix.to_vec();
        expected.extend([uci, "a7a6", rookmove]);
        assert_eq!(line(&g), expected);
    }
}
#[test]
fn en_passant_removes_captured_pawn_from_piece_ordinals() {
    let prefix = ["e2e4", "a7a6", "e4e5", "d7d5", "e5d6"];
    let g = decode_classic(&encoder::line(&prefix), &ReadLimits::default()).unwrap();
    assert_eq!(line(&g), prefix);
    let mut body = vec![];
    let mut count = 0;
    for u in prefix {
        encoder::two_byte(&mut body, &mut count, u);
    }
    // Captured black d-pawn must not be available for a one-square move.
    encoder::primary(&mut body, &mut count, 0xc5);
    assert!(decode_classic(&encoder::finish(body, count), &ReadLimits::default()).is_err());
}
#[test]
fn counter_wraps_after_256_moves() {
    let moves = ["g1f3", "g8f6", "f3g1", "f6g8"].repeat(70);
    let g = decode_classic(&encoder::line(&moves), &ReadLimits::default()).unwrap();
    assert_eq!(line(&g), moves);
    assert_eq!(g.nodes.len(), 281);
}
#[test]
fn rejects_truncated_payloads_invalid_source_and_illegal_move() {
    for record in [
        vec![0, 0, 0, 6, 0x29, 0x47],
        vec![0, 0, 0, 6, 0x39, 0x0d],
        vec![0, 0, 0, 6, 0x40, 0x0d],
        vec![0, 0, 0, 7, 0xff],
    ] {
        assert!(decode_classic(&record, &ReadLimits::default()).is_err());
    }
}
