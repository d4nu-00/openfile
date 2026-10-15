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
    // Unencoded, unknown kinds, Chess960 without a set-up position.
    for flag in [0x80, 0x02, 0x03, 0x08, 0x0a, 0x10, 0x20] {
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

#[test]
fn setup_impossible_castling_rights_are_dropped_one_by_one() {
    // Kings on g1 and g8 with K and k set (bits 2 and 8), as in classic Mega
    // entries 64687 and 96531: both rights are impossible and dropped.
    let g = decode_classic(
        &setup(
            0,
            0x0a,
            17,
            &[(48, 1), (40, 5), (55, 9), (63, 13)],
            &["g1g2"],
        ),
        &ReadLimits::default(),
    )
    .unwrap();
    assert_eq!(g.start_fen, "6kr/8/8/8/8/8/8/5RK1 w - - 0 17");
    assert_eq!(g.nodes[1].uci.as_deref(), Some("g1g2"));
    // A possible right is kept beside an impossible one: White's K stays
    // and castles, Black's k (king on g8) goes.
    let g = decode_classic(
        &setup(
            0,
            0x0a,
            5,
            &[(32, 1), (56, 5), (55, 9), (63, 13)],
            &["e1g1"],
        ),
        &ReadLimits::default(),
    )
    .unwrap();
    assert_eq!(g.start_fen, "6kr/8/8/8/8/8/8/4K2R w K - 0 5");
    assert_eq!(
        g.position_at(1).unwrap().fen,
        "6kr/8/8/8/8/8/8/5RK1 b - - 1 5"
    );
}

mod kinds {
    use cbh_reader::{
        ReadLimits,
        moves::{decode_classic, tables},
    };

    /// The byte a record in `order` holds for standard byte `std` after
    /// `count` moves.
    fn remap(order: &[u8; 256], std: u8, count: u8) -> u8 {
        let index = tables::STANDARD_ORDER
            .iter()
            .position(|&b| b == std)
            .unwrap() as u8;
        order[index.wrapping_add(count) as usize]
    }
    fn record(flags: u8, body: Vec<u8>) -> Vec<u8> {
        let mut r = ((body.len() + 4) as u32).to_be_bytes().to_vec();
        r[0] = flags;
        r.extend(body);
        r
    }
    /// A two-byte move in a remapped record: 0x29, then the two
    /// deobfuscation-table bytes of the word.
    fn coordinate(order: &[u8; 256], out: &mut Vec<u8>, count: &mut u8, word: u16) {
        let enc = |b: u8| tables::DEOBFUSCATE.iter().position(|&d| d == b).unwrap() as u8;
        let [hi, lo] = word.to_be_bytes();
        out.push(remap(order, 0x29, *count));
        out.push(remap(order, enc(hi), *count));
        out.push(remap(order, enc(lo), *count));
        *count += 1;
    }
    const G1: u16 = 6 * 8;
    const E7: u16 = 4 * 8 + 6;
    const E5: u16 = 4 * 8 + 4;

    #[test]
    fn chess960_records_castle_with_their_rooks() {
        // White: K b1, R a1, R h1, P b2; Black: k b8, r a8, r h8, p e7.
        let mut r = super::setup(
            0,
            0x0f,
            1,
            &[
                (8, 1),
                (0, 5),
                (56, 5),
                (9, 6),
                (15, 9),
                (7, 13),
                (63, 13),
                (38, 14),
            ],
            &[],
        );
        r.truncate(32);
        r[0] = 0x4a;
        // Kings b1, b8; rooks h1, a1, h8, a8; position number 0x0123.
        r.extend([8, 15, 56, 0, 63, 7, 0x01, 0x23]);
        let mut count = 0;
        // O-O as the king's castling square g1 twice, then 1...e5.
        coordinate(&tables::CHESS960_ORDER, &mut r, &mut count, G1 | G1 << 6);
        coordinate(&tables::CHESS960_ORDER, &mut r, &mut count, E7 | E5 << 6);
        r.push(remap(&tables::CHESS960_ORDER, 0x0c, count));
        let len = r.len() as u32;
        r[1..4].copy_from_slice(&len.to_be_bytes()[1..]);
        let g = decode_classic(&r, &ReadLimits::default()).unwrap();
        assert_eq!(g.tags.get("Variant").map(String::as_str), Some("Chess960"));
        assert_eq!(g.start_fen, "rk5r/4p3/8/8/8/8/1P6/RK5R w KQkq - 0 1");
        assert_eq!(g.nodes[1].uci.as_deref(), Some("b1h1"));
        assert_eq!(g.san_at(1).unwrap().as_deref(), Some("O-O"));
        assert_eq!(
            g.position_at(2).unwrap().fen.split(' ').next(),
            Some("rk5r/8/8/4p3/8/8/1P6/R4RK1")
        );
        // Castling squares that do not hold the rooks are refused.
        let mut bad = r.clone();
        bad[34] = 48;
        assert!(decode_classic(&bad, &ReadLimits::default()).is_err());
    }

    #[test]
    fn kind4_records_use_their_byte_order() {
        // 1.e4 e5: the pawn tokens 0xff in standard bytes.
        let body = vec![
            remap(&tables::KIND4_ORDER, 0xff, 0),
            remap(&tables::KIND4_ORDER, 0xff, 1),
            remap(&tables::KIND4_ORDER, 0x0c, 2),
        ];
        let g = decode_classic(&record(0x04, body), &ReadLimits::default()).unwrap();
        let ucis: Vec<_> = g.nodes[1..]
            .iter()
            .map(|n| n.uci.clone().unwrap())
            .collect();
        assert_eq!(ucis, ["e2e4", "e7e5"]);
    }

    #[test]
    fn two_byte_word_records() {
        let e2e4: u16 = (4 * 8 + 1) | (4 * 8 + 3) << 6;
        let e7e5: u16 = E7 | E5 << 6;
        let words = [e2e4, e7e5 | 0x4000];
        // Kind 5: the byte at (value + counter) in its order.
        let mut body = vec![];
        for (count, w) in words.iter().enumerate() {
            for b in w.to_be_bytes() {
                body.push(tables::KIND5_ORDER[b.wrapping_add(count as u8) as usize]);
            }
        }
        let g = decode_classic(&record(0x05, body.clone()), &ReadLimits::default()).unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.nodes[2].uci.as_deref(), Some("e7e5"));
        // A word with variations is refused rather than guessed.
        let mut with = body.clone();
        with[0] = tables::KIND5_ORDER[(e2e4.to_be_bytes()[0] | 0x80) as usize];
        assert_eq!(
            decode_classic(&record(0x05, with), &ReadLimits::default())
                .unwrap_err()
                .code(),
            "unsupportedEncoding"
        );
        // Kind 1: the order's byte at the value, plus the counter; its
        // upper bits are not read.
        let mut body = vec![];
        for (count, w) in [e2e4 | 0x2000, e7e5].iter().enumerate() {
            for b in w.to_be_bytes() {
                body.push(tables::KIND1_ORDER[b as usize].wrapping_add(count as u8));
            }
        }
        let g = decode_classic(&record(0x01, body), &ReadLimits::default()).unwrap();
        assert_eq!(g.nodes[1].uci.as_deref(), Some("e2e4"));
        // A record of just the header is a game without moves.
        let empty = decode_classic(&[0x01, 0, 0, 4], &ReadLimits::default()).unwrap();
        assert_eq!(empty.nodes.len(), 1);
    }
}
