use cbh_reader::{
    DataStatus,
    annotations::{Kind, Order, apply, decode_text, parse_classic, parse_new, positions},
};
use chess_core::{Game, GameLimits};

/// 1.e4 (1.d4) 1...e5: stream order e4, e5, d4; line order e4, d4, e5.
fn game() -> Game {
    let mut g = Game::initial();
    let limits = GameLimits::default();
    let e4 = g.append_uci(0, "e2e4", &limits).unwrap();
    g.append_uci(e4, "e7e5", &limits).unwrap();
    g.append_uci(0, "d2d4", &limits).unwrap();
    g
}

fn classic_record(game: u32, records: &[(i32, u8, &[u8])]) -> Vec<u8> {
    let mut body = vec![];
    for &(position, ty, data) in records {
        body.extend(&position.to_be_bytes()[1..]);
        body.push(ty);
        body.extend(((data.len() + 6) as u16).to_be_bytes());
        body.extend(data);
    }
    let mut out = game.to_be_bytes()[1..].to_vec();
    out.extend([0; 7]);
    out.extend(((14 + body.len()) as u32).to_be_bytes());
    out.extend(body);
    out
}

fn text_entry(ty: u16, language: u16, text: &str) -> Vec<u8> {
    let mut b = ty.to_le_bytes().to_vec();
    b.extend(0u16.to_le_bytes());
    b.extend(language.to_le_bytes());
    b.extend((text.len() as u32).to_le_bytes());
    b.extend(text.as_bytes());
    b
}
fn group(position: i32, entries: &[Vec<u8>]) -> Vec<u8> {
    let mut b = position.to_le_bytes().to_vec();
    b.extend((entries.len() as u32).to_le_bytes());
    for e in entries {
        b.extend(e);
    }
    b
}

#[test]
fn position_orders_differ_for_variations() {
    let g = game();
    assert_eq!(positions(&g, Order::Stream), [1, 2, 3]);
    assert_eq!(positions(&g, Order::Line), [1, 3, 2]);
}

#[test]
fn classic_records_map_in_stream_order() {
    let bytes = classic_record(
        5,
        &[
            (-1, 0x02, b"\x00\x00Intro"),
            (0, 0x03, &[1, 18]),
            (1, 0x82, b"\x00*Before e5"),
            (1, 0x04, &[2, 5 * 8 + 6 + 1, 4, 1]),
            (2, 0x05, &[7, 3 * 8 + 1 + 1, 3 * 8 + 3 + 1]),
        ],
    );
    let parsed = parse_classic(&bytes, 5).unwrap();
    assert!(parsed.diagnostics.is_empty());
    let mut g = game();
    let (status, notes) = apply(&mut g, parsed, Order::Stream);
    assert_eq!(status, DataStatus::Available, "{notes:?}");
    assert_eq!(g.nodes[0].comments, ["Intro"]);
    assert_eq!(g.nodes[1].nags, [1, 18]);
    assert_eq!(g.nodes[2].pre_comments, ["Before e5"]);
    let squares: Vec<_> = g.nodes[2]
        .squares
        .iter()
        .map(|m| (m.color, m.square.as_str()))
        .collect();
    assert_eq!(squares, [('G', "f7"), ('R', "a1")]);
    let a = &g.nodes[3].arrows[0];
    assert_eq!((a.color, a.from.as_str(), a.to.as_str()), ('B', "d2", "d4"));
}

#[test]
fn new_blocks_map_in_line_order() {
    let mut payload = group(0, &[vec![3, 0, 1, 0, 0]]);
    payload.extend(group(1, &[text_entry(0x02, 0, "After d4")]));
    let mut squares = 4u16.to_le_bytes().to_vec();
    squares.extend(2u32.to_le_bytes());
    squares.extend([3, 4 * 8 + 4 + 1]);
    payload.extend(group(2, &[squares]));
    payload.extend(i32::MAX.to_le_bytes());
    let parsed = parse_new(&payload).unwrap();
    assert_eq!(parsed.entries.len(), 3);
    let mut g = game();
    let (status, _) = apply(&mut g, parsed, Order::Line);
    assert_eq!(status, DataStatus::Available);
    assert_eq!(g.nodes[1].nags, [1]);
    assert_eq!(g.nodes[3].comments, ["After d4"]);
    assert_eq!(g.nodes[2].squares[0].square, "e5");
    assert_eq!(g.nodes[2].squares[0].color, 'Y');
}

#[test]
fn one_language_is_kept_with_neutral_text() {
    let payload = [
        group(
            0,
            &[
                text_entry(0x02, 1, "Deutsch"),
                text_entry(0x02, 0, "English"),
                text_entry(0x02, 7, "!!"),
            ],
        ),
        i32::MAX.to_le_bytes().to_vec(),
    ]
    .concat();
    let mut g = game();
    let (status, notes) = apply(&mut g, parse_new(&payload).unwrap(), Order::Line);
    assert_eq!(g.nodes[1].comments, ["English", "!!"]);
    assert_eq!(status, DataStatus::Partial);
    assert_eq!(notes[0].code, "otherLanguageText");
}

#[test]
fn unsupported_and_unsized_types_are_reported() {
    // A critical position (one byte) is kept on its move; a game quotation
    // stops parsing.
    let payload = [
        group(0, &[vec![0x18, 0, 2]]),
        group(1, &[vec![0x13, 0, 9, 9, 9]]),
        group(2, &[text_entry(0x02, 0, "lost")]),
    ]
    .concat();
    let parsed = parse_new(&payload).unwrap();
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].kind, Kind::Critical(2));
    assert_eq!(parsed.stopped_at, Some((1, 0x13)));
    let mut g = game();
    let (status, notes) = apply(&mut g, parsed, Order::Line);
    assert_eq!(status, DataStatus::Partial);
    assert_eq!(g.nodes[1].critical, Some(2));
    let codes: Vec<_> = notes.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["annotationsStopped"]);
}

#[test]
fn malformed_records_fail() {
    let mut bytes = classic_record(1, &[(0, 0x02, b"\x00*text")]);
    let at = bytes.len() - 6;
    bytes[at - 1] = 60; // record length beyond the block
    assert_eq!(parse_classic(&bytes, 1).unwrap_err().code(), "corrupt");
    let bytes = classic_record(1, &[(0, 0x04, &[2])]);
    assert_eq!(parse_classic(&bytes, 1).unwrap_err().code(), "corrupt");
    // Positions must increase in 2CBA.
    let payload = [group(1, &[]), group(0, &[])].concat();
    assert_eq!(parse_new(&payload).unwrap_err().code(), "corrupt");
    // Out-of-range positions are reported, not attached elsewhere.
    let bytes = classic_record(1, &[(9, 0x03, &[1])]);
    let mut g = game();
    let (status, notes) = apply(&mut g, parse_classic(&bytes, 1).unwrap(), Order::Stream);
    assert_eq!(status, DataStatus::Partial);
    assert_eq!(notes[0].code, "annotationOutOfRange");
}

#[test]
fn text_encodings() {
    assert_eq!(decode_text(b"caf\xe9 \x80"), "café €");
    assert_eq!(decode_text("Löwe".as_bytes()), "Löwe");
    assert_eq!(decode_text(b"Lehner, \x00garbage"), "Lehner, ");
}
