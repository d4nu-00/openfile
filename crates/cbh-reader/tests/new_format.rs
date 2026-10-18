use cbh_reader::{
    DataStatus, Database, DatabaseFormat, EntryKind, ReadLimits,
    v2::{block::checksum, moves::decode_moves},
};
use std::fs;

// Tokens observed for the test database entry 20 (research sample), paired
// with the moves an independent classic decoder produced for the same game.
const E2E4: u16 = 0xad3f;
const E7E5: u16 = 0xb02d;
const G1F3: u16 = 0x325f;
const SAMPLE: &[(u16, &str)] = &[
    (E2E4, "e2e4"),
    (E7E5, "e7e5"),
    (G1F3, "g1f3"),
    (0xafd0, "d7d6"),
    (0xae5a, "h2h3"),
    (0x8929, "g8f6"),
    (0xac84, "c2c3"),
    (0x8802, "f6e4"),
    (0x168d, "d1a4"),
];
// Placement tokens: base + kind * 64 + square, pawns on 48 squares of ranks 2-7.
const PLACE: u16 = 0xc02d;

fn bytes(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}
fn ucis(game: &chess_core::Game, ids: &[u32]) -> Vec<String> {
    ids.iter()
        .map(|&id| game.nodes[id as usize].uci.clone().unwrap())
        .collect()
}

#[test]
fn checksum_sums_eight_chunks_and_omits_the_tail() {
    let payload: Vec<u8> = (1..=17).collect();
    // Chunks of two bytes: 1+2, 3+4, ..., 15+16; byte 17 is not covered.
    assert_eq!(checksum(&payload), 0x03070b0f13171b1f);
    assert_eq!(checksum(&[0x10, 0x20, 0x30]), 0x1020300000000000);
    assert_eq!(checksum(&[0xff; 16]), 0xfefefefefefefefe);
}

#[test]
fn sample_mainline_decodes() {
    let mut words = vec![0xfffc];
    words.extend(SAMPLE.iter().map(|m| m.0));
    words.push(0xffff);
    let decoded = decode_moves(&bytes(&words), &ReadLimits::default()).unwrap();
    let expected: Vec<_> = SAMPLE.iter().map(|m| m.1.to_string()).collect();
    assert_eq!(ucis(&decoded.game, &decoded.stream_nodes), expected);
    assert!(decoded.diagnostics.is_empty());
}

#[test]
fn variations_attach_to_the_preceding_move() {
    // 1.e4 (variation marker) e5, end; alternative 1.Nf3, end.
    let words = [0xfffc, E2E4, 0xfffd, E7E5, 0xffff, G1F3, 0xffff];
    let g = decode_moves(&bytes(&words), &ReadLimits::default())
        .unwrap()
        .game;
    assert_eq!(ucis(&g, &g.nodes[0].children), ["e2e4", "g1f3"]);
    assert_eq!(ucis(&g, &g.nodes[1].children), ["e7e5"]);
}

#[test]
fn null_moves_and_flags_are_reported() {
    let words = [0xfffc, E2E4, 0xfffe, 0xfffa, G1F3, 0xffff];
    let d = decode_moves(&bytes(&words), &ReadLimits::default()).unwrap();
    assert_eq!(ucis(&d.game, &d.stream_nodes), ["e2e4", "0000", "g1f3"]);
    assert_eq!(d.diagnostics[0].code, "uninterpretedMoveFlag");
}

#[test]
fn setup_positions_place_pieces_then_play() {
    let words = [
        0xfffb,
        7,      // move number
        0x0001, // Black to move, no castling rights
        0,
        PLACE + 32,              // white king e1
        PLACE + 5 * 64 + 39,     // black king e8
        PLACE + 640 + 48 + 5,    // black pawn a7
        PLACE + 640 + 7 * 6 + 2, // white pawn h4
        0xfffc,
        0xffff,
    ];
    let g = decode_moves(&bytes(&words), &ReadLimits::default())
        .unwrap()
        .game;
    assert_eq!(g.start_fen, "4k3/p7/8/8/7P/8/8/4K3 b - - 0 7");
    let castling = [
        0xfffb,
        1,
        0x0f00,
        0,
        PLACE + 32,
        PLACE + 4 * 64,
        PLACE + 4 * 64 + 56,
        PLACE + 5 * 64 + 39,
        PLACE + 9 * 64 + 7,
        PLACE + 9 * 64 + 63,
        0xfffc,
        0xffff,
    ];
    let g = decode_moves(&bytes(&castling), &ReadLimits::default())
        .unwrap()
        .game;
    assert_eq!(g.start_fen, "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
}

#[test]
fn malformed_streams_fail_explicitly() {
    let limits = ReadLimits::default();
    let code = |words: &[u16]| decode_moves(&bytes(words), &limits).unwrap_err().code();
    assert_eq!(code(&[0xfff0, 0xffff]), "unsupportedEncoding");
    assert_eq!(code(&[0xfffc, 0xc000, 0xffff]), "unsupportedEncoding");
    assert_eq!(code(&[0xfffc, E7E5, 0xffff]), "corrupt");
    assert_eq!(code(&[0xfffc, E2E4]), "corrupt");
    assert_eq!(code(&[0xfffc, 0xfffd, 0xffff]), "corrupt");
    assert_eq!(code(&[0xfffc, 0xffff, 0xffff]), "corrupt");
    assert_eq!(code(&[0xfffb, 1, 0, 1, 0xfffc, 0xffff]), "unsupportedSetup");
    assert_eq!(
        code(&[0xfffb, 1, 0, 0, PLACE + 32, PLACE + 32, 0xfffc, 0xffff]),
        "corrupt"
    );
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    let padding = (8 - (payload.len() + 34) % 8) % 8;
    let total = (34 + payload.len() + padding) as u64;
    let mut b = 0x1122334455667788u64.to_le_bytes().to_vec();
    b.extend((payload.len() as u32).to_le_bytes());
    b.extend((padding as u32).to_le_bytes());
    b.extend(checksum(payload).to_le_bytes());
    b.extend(kind.to_le_bytes());
    b.extend(payload);
    b.extend(vec![0; padding]);
    b.extend(total.to_le_bytes());
    b
}

fn lid(records: &[(usize, u32, Vec<u8>)]) -> Vec<u8> {
    let slots = [64u32, 96, 64, 32, 32, 32];
    let mut h = vec![];
    for v in [184u32, 6] {
        h.extend(v.to_be_bytes());
    }
    for slot in slots {
        for v in [slot, 0, 2, u32::MAX, u32::MAX] {
            h.extend(v.to_be_bytes());
        }
    }
    for v in [u32::MAX, 1] {
        h.extend(v.to_be_bytes());
    }
    for kind in 0..6u32 {
        h.extend(kind.to_be_bytes());
        h.extend(1u32.to_be_bytes());
    }
    let group: u32 = slots.iter().sum();
    h.resize(184 + 2 * group as usize, 0);
    for (kind, id, payload) in records {
        let at = 184 + (id * group) as usize + slots[..*kind].iter().sum::<u32>() as usize;
        h[at..at + 4].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        h[at + 4..at + 4 + payload.len()].copy_from_slice(payload);
    }
    h
}
fn strings(values: &[&str]) -> Vec<u8> {
    let mut b = vec![];
    for v in values {
        b.extend((v.len() as u32).to_le_bytes());
        b.extend(v.as_bytes());
    }
    b.extend([0; 8]);
    b
}

fn family() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let mut words = vec![0xfffc];
    words.extend(SAMPLE.iter().map(|m| m.0));
    words.push(0xffff);
    let mut cbg = vec![0; 12];
    let game_at = cbg.len() as u64;
    cbg.extend(block(1, &bytes(&words)));
    let text_at = cbg.len() as u64;
    cbg.extend(block(0x1000, b"<html></html>"));
    let length = cbg.len() as u64;
    cbg[..8].copy_from_slice(&length.to_le_bytes());
    fs::write(d.path().join("Synthetic.2cbg"), cbg).unwrap();
    // 2CBA: text after the second move (position 1), English, then the end.
    let mut notes = 1i32.to_le_bytes().to_vec();
    notes.extend(1u32.to_le_bytes());
    notes.extend([2, 0, 0, 0, 0, 0, 5, 0, 0, 0]);
    notes.extend(b"Solid");
    notes.extend(i32::MAX.to_le_bytes());
    let mut cba = vec![0; 12];
    cba.extend(block(0x2000, &notes));
    fs::write(d.path().join("Synthetic.2cba"), cba).unwrap();

    let mut h = vec![0u8; 192 * 4];
    h[10..12].copy_from_slice(&192u16.to_le_bytes());
    h[16..24].copy_from_slice(&4u64.to_le_bytes());
    let game = |r: &mut [u8], flags: u16, white: u64| {
        r[..2].copy_from_slice(&flags.to_le_bytes());
        r[2..4].copy_from_slice(&0x0101u16.to_le_bytes());
        r[8..16].copy_from_slice(&game_at.to_le_bytes());
        r[16..24].copy_from_slice(&12u64.to_le_bytes());
        let ids = [
            (24, white),
            (32, 0),
            (40, 1),
            (48, 0),
            (56, 0),
            (64, 0),
            (72, 1),
        ];
        for (at, id) in ids {
            r[at..at + 8].copy_from_slice(&id.to_le_bytes());
        }
        r[88] = 1;
        r[90] = 3;
        r[92] = 2;
        r[96..98].copy_from_slice(&(0x1000u16 | 2700).to_le_bytes());
        r[128..130].copy_from_slice(&((102u16) << 7).to_le_bytes());
        r[188..192].copy_from_slice(&((2024u32 << 9) | (5 << 5) | 17).to_le_bytes());
    };
    game(&mut h[192..384], 1, 1);
    game(&mut h[384..576], 0x81, u64::MAX);
    let r = &mut h[576..768];
    r[..2].copy_from_slice(&3u16.to_le_bytes());
    r[2..4].copy_from_slice(&0x0301u16.to_le_bytes());
    r[8..16].copy_from_slice(&text_at.to_le_bytes());
    fs::write(d.path().join("Synthetic.2cbh"), h).unwrap();

    fs::write(
        d.path().join("Synthetic.2lid"),
        lid(&[
            (0, 0, strings(&["Zeta", "Ana"])),
            (0, 1, strings(&["Müller", "Jörg"])),
            (1, 1, strings(&["Göteborg", "Open"])),
            (2, 0, {
                let mut b = strings(&["EXT 2021", "Publisher"]);
                b.truncate(b.len() - 8);
                b.extend(((2020u32 << 9) | (10 << 5) | 23).to_le_bytes());
                b.extend(((2020u32 << 9) | (10 << 5) | 23).to_le_bytes());
                b.extend([1, 0, 1, 0]);
                b
            }),
            (4, 0, strings(&["North"])),
            (4, 1, strings(&["South"])),
        ]),
    )
    .unwrap();
    d
}

#[test]
fn new_family_pages_and_loads() {
    let d = family();
    let path = d.path().join("Synthetic.2cbh");
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    assert_eq!(db.info().format, DatabaseFormat::New);
    assert_eq!(db.info().entry_count, 3);
    let page = db.page(1, 10).unwrap();
    let e = &page.entries[0];
    assert_eq!(e.white.as_deref(), Some("Müller, Jörg"));
    assert_eq!(e.black.as_deref(), Some("Zeta, Ana"));
    assert_eq!(e.event.as_deref(), Some("Open"));
    assert_eq!(e.site.as_deref(), Some("Göteborg"));
    assert_eq!(e.date.as_deref(), Some("2024.05.17"));
    assert_eq!(e.result.as_deref(), Some("1/2-1/2"));
    assert_eq!(e.round.as_deref(), Some("3.2"));
    assert_eq!(e.white_elo, Some(2700));
    assert_eq!(e.eco.as_deref(), Some("B01"));
    // 2CBH annotator ids index the player list.
    assert_eq!(e.annotator_id, Some(0));
    assert_eq!(e.annotator.as_deref(), Some("Zeta, Ana"));
    assert_eq!(e.source.as_deref(), Some("EXT 2021"));
    assert_eq!(e.source_publisher.as_deref(), Some("Publisher"));
    assert_eq!(e.source_date.as_deref(), Some("2020.10.23"));
    assert_eq!(e.white_team.as_deref(), Some("North"));
    assert_eq!(e.black_team.as_deref(), Some("South"));
    assert!(!e.deleted);
    assert!(page.entries[1].deleted);
    assert_eq!(page.entries[1].white, None);
    assert_eq!(page.entries[2].kind, EntryKind::Text);

    let loaded = db.load(1).unwrap();
    assert_eq!(loaded.moves_status, DataStatus::Available);
    assert_eq!(loaded.annotations_status, DataStatus::Available);
    let game = loaded.game.unwrap();
    assert_eq!(game.nodes.len(), SAMPLE.len() + 1);
    assert_eq!(game.tags["White"], "Müller, Jörg");
    assert_eq!(game.tags["ECO"], "B01");
    assert_eq!(game.tags["Annotator"], "Zeta, Ana");
    assert_eq!(game.tags["WhiteTeam"], "North");
    assert_eq!(game.tags["SourceTitle"], "EXT 2021");
    assert_eq!(game.nodes[2].comments, ["Solid"]);
    assert_eq!(db.load(3).unwrap().moves_status, DataStatus::Unsupported);
    let lid = db
        .info()
        .companions
        .iter()
        .find(|c| c.extension == "2lid")
        .unwrap();
    assert_eq!(lid.status, DataStatus::Available);
}

#[test]
fn damaged_blocks_are_corrupt_not_partial() {
    let d = family();
    let cbg = d.path().join("Synthetic.2cbg");
    let mut bytes = fs::read(&cbg).unwrap();
    bytes[12 + 26] ^= 1; // first payload byte: checksum no longer matches
    fs::write(&cbg, bytes).unwrap();
    let mut db = Database::open(&d.path().join("Synthetic.2cbh"), ReadLimits::default()).unwrap();
    let loaded = db.load(1).unwrap();
    assert_eq!(loaded.moves_status, DataStatus::Corrupt);
    assert!(loaded.game.is_none());
}

#[test]
fn missing_name_file_is_reported() {
    let d = family();
    fs::remove_file(d.path().join("Synthetic.2lid")).unwrap();
    let mut db = Database::open(&d.path().join("Synthetic.2cbh"), ReadLimits::default()).unwrap();
    let e = &db.page(1, 1).unwrap().entries[0];
    assert_eq!(e.white, None);
    assert!(e.diagnostics.iter().any(|d| d.code == "missingNameTable"));
}
