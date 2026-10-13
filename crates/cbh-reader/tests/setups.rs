mod support;
use cbh_reader::{ReadLimits, moves::decode_classic};
use support::encoder::{finish, primary, two_byte};

// Independently authored occupancy codes. File-major square index a1=0,
// a8=7, b1=8. Occupied marker + four piece bits, then zero pad to 192 bits.
fn setup(turn_ep: u8, rights: u8, number: u8, pieces: &[(usize, u8)], moves: &[&str]) -> Vec<u8> {
    let mut bytes = vec![0, turn_ep, rights, number];
    let mut bits = vec![];
    for square in 0..64 {
        match pieces.iter().find(|p| p.0 == square) {
            None => bits.push(0),
            Some((_, code)) => {
                bits.push(1);
                for shift in (0..4).rev() {
                    bits.push((code >> shift) & 1);
                }
            }
        }
    }
    assert!(bits.len() <= 192);
    bits.resize(192, 0);
    bytes.extend(
        bits.chunks(8)
            .map(|b| b.iter().fold(0u8, |v, bit| (v << 1) | bit)),
    );
    let mut count = 0;
    for uci in moves {
        two_byte(&mut bytes, &mut count, uci);
    }
    let mut record = finish(bytes, count);
    record[0] = 0x40;
    record
}
#[test]
fn setup_promotes_and_preserves_move_number() {
    let g = decode_classic(
        &setup(0, 0, 17, &[(32, 1), (39, 9), (6, 6)], &["a7a8n"]),
        &ReadLimits::default(),
    )
    .unwrap();
    assert_eq!(g.start_fen, "4k3/P7/8/8/8/8/8/4K3 w - - 0 17");
    assert_eq!(g.nodes[1].uci.as_deref(), Some("a7a8n"));
    assert_eq!(
        g.position_at(1).unwrap().fen,
        "N3k3/8/8/8/8/8/8/4K3 b - - 0 17"
    );
}
#[test]
fn setup_black_turn_and_castling() {
    let g = decode_classic(
        &setup(0x10, 8, 3, &[(32, 1), (39, 9), (63, 13)], &["e8g8"]),
        &ReadLimits::default(),
    )
    .unwrap();
    assert_eq!(g.start_fen, "4k2r/8/8/8/8/8/8/4K3 b k - 0 3");
    assert_eq!(
        g.position_at(1).unwrap().fen,
        "5rk1/8/8/8/8/8/8/4K3 w - - 1 4"
    );
}
#[test]
fn en_passant_zero_is_absent_and_eight_is_h_file() {
    let pieces = [(32, 1), (39, 9), (52, 6), (60, 14)]; // g5, h5
    let g = decode_classic(&setup(8, 0, 1, &pieces, &["g5h6"]), &ReadLimits::default()).unwrap();
    assert_eq!(g.start_fen, "4k3/8/8/6Pp/8/8/8/4K3 w - h6 0 1");
    assert!(decode_classic(&setup(0, 0, 1, &pieces, &["g5h6"]), &ReadLimits::default()).is_err());
}
#[test]
fn invalid_and_truncated_setups_fail() {
    assert!(decode_classic(&setup(0, 0, 1, &[(32, 1)], &[]), &ReadLimits::default()).is_err());
    assert!(
        decode_classic(
            &setup(9, 0, 1, &[(32, 1), (39, 9)], &[]),
            &ReadLimits::default()
        )
        .is_err()
    );
    assert!(decode_classic(&[0x40, 0, 0, 5, 0x0c], &ReadLimits::default()).is_err());
    let mut r = setup(0, 0, 1, &[(32, 1), (39, 9)], &[]);
    r[4] = 2;
    assert_eq!(
        decode_classic(&r, &ReadLimits::default())
            .unwrap_err()
            .code(),
        "unsupportedSetup"
    );
}
fn control(body: &mut Vec<u8>, count: u8, token: u8) {
    body.push(token.wrapping_add(count));
}
fn branches() -> Vec<u8> {
    let mut b = vec![];
    let mut c = 0;
    two_byte(&mut b, &mut c, "e2e4");
    control(&mut b, c, 0xdc); // save after e4; main e5/Nf3 first
    two_byte(&mut b, &mut c, "e7e5");
    two_byte(&mut b, &mut c, "g1f3");
    control(&mut b, c, 0x0c); // restore after e4, global counter=3
    two_byte(&mut b, &mut c, "c7c5");
    control(&mut b, c, 0xdc);
    two_byte(&mut b, &mut c, "g1f3");
    control(&mut b, c, 0x0c);
    two_byte(&mut b, &mut c, "b1c3");
    finish(b, c)
}
#[test]
fn branches_have_exact_ordered_pgn() {
    let g = decode_classic(&branches(), &ReadLimits::default()).unwrap();
    let pgn = chess_core::write_pgn(&g).unwrap();
    assert_eq!(
        pgn.split("\n\n").nth(1).unwrap().trim(),
        "1. e4 e5 (1... c5 2. Nf3 (2. Nc3)) 2. Nf3 *"
    );
    // e4, push, e5, pop, c5, root end. All literal bytes annotated.
    let literal = [0, 0, 0, 10, 0xff, 0xdd, 0x00, 0x0e, 0xdc, 0x0f];
    let g = decode_classic(&literal, &ReadLimits::default()).unwrap();
    assert_eq!(g.nodes[1].children.len(), 2);
    assert_eq!(g.nodes[3].uci.as_deref(), Some("c7c5"));
}
#[test]
fn null_and_filler_follow_global_counter() {
    let mut b = vec![];
    let mut c = 0;
    control(&mut b, c, 0x9f);
    primary(&mut b, &mut c, 0xaa);
    two_byte(&mut b, &mut c, "e7e5");
    let g = decode_classic(&finish(b, c), &ReadLimits::default()).unwrap();
    assert_eq!(g.nodes[1].uci.as_deref(), Some("0000"));
    assert_eq!(g.nodes[2].uci.as_deref(), Some("e7e5"));
}
#[test]
fn terminators_and_limits_are_checked() {
    for r in [
        vec![0, 0, 0, 6, 0x0c, 0x0c],
        vec![0, 0, 0, 6, 0xdc, 0x0c],
        vec![0, 0, 0, 5, 0x9f],
    ] {
        assert!(decode_classic(&r, &ReadLimits::default()).is_err());
    }
    let mut limits = ReadLimits::default();
    limits.game.max_variation_depth = 0;
    assert_eq!(
        decode_classic(&branches(), &limits).unwrap_err().code(),
        "limitExceeded"
    );
    limits = ReadLimits::default();
    limits.game.max_nodes = 2;
    assert!(decode_classic(&branches(), &limits).is_err());
    limits = ReadLimits::default();
    limits.max_game_bytes = 5;
    assert_eq!(
        decode_classic(&branches(), &limits).unwrap_err().code(),
        "limitExceeded"
    );
}
#[test]
fn unsupported_flags_are_not_empty_games() {
    for flag in [0x04, 0x80, 0x02, 0x0a, 0x41] {
        assert_eq!(
            decode_classic(&[flag, 0, 0, 5, 0x0c], &ReadLimits::default())
                .unwrap_err()
                .code(),
            "unsupportedEncoding"
        );
    }
}
#[test]
fn setup_byte_zero_one_and_impossible_en_passant_are_accepted() {
    // Byte 0 is 1 in every Mega Database 2025 setup.
    let mut r = setup(0, 0, 1, &[(32, 1), (39, 9)], &[]);
    r[4] = 1;
    let g = decode_classic(&r, &ReadLimits::default()).unwrap();
    assert_eq!(g.start_fen, "4k3/8/8/8/8/8/8/4K3 w - - 0 1");
    // En-passant file c with no black pawn on c5 is dropped; move 0 reads as 1.
    let g = decode_classic(
        &setup(3, 0, 0, &[(32, 1), (39, 9)], &[]),
        &ReadLimits::default(),
    )
    .unwrap();
    assert_eq!(g.start_fen, "4k3/8/8/8/8/8/8/4K3 w - - 0 1");
}
