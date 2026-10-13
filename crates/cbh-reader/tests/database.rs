use cbh_reader::{ClassicDatabase, DataStatus, EntryKind, ReadLimits};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
};
fn family(moves: bool) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let mut h = vec![0; 46 * 4];
    h[..6].copy_from_slice(&[0, 0, 0x2c, 0, 0x2e, 1]);
    h[9] = 4;
    for i in 1..4 {
        let r = &mut h[i * 46..(i + 1) * 46];
        r[0] = 1;
        r[4] = if i == 2 { 7 } else { 0 };
        r[27] = 2;
        r[29] = 1;
    }
    h[46] |= 0x80;
    h[138] = 3;
    h[138 + 9..138 + 12].fill(0xff);
    fs::write(d.path().join("Synthetic.cbh"), h).unwrap();
    if moves {
        fs::write(
            d.path().join("Synthetic.cbg"),
            [0, 0, 0, 7, 0xff, 0, 0x0e, 0, 0, 0, 5, 0xff],
        )
        .unwrap();
    }
    for (ext, stride) in [("cbp", 67), ("cbt", 99)] {
        let mut t = vec![0u8; 28 + stride];
        t[0..4].copy_from_slice(&1u32.to_le_bytes());
        t[8..12].copy_from_slice(&1234567890u32.to_le_bytes());
        t[12..16].copy_from_slice(&((stride - 9) as u32).to_le_bytes());
        t[37..42].copy_from_slice(b"Alpha");
        fs::write(d.path().join(format!("Synthetic.{ext}")), t).unwrap();
    }
    d
}
fn open(d: &tempfile::TempDir) -> ClassicDatabase {
    ClassicDatabase::open(&d.path().join("Synthetic.cbh"), ReadLimits::default()).unwrap()
}
#[test]
fn named_family_loads_and_marks_deletion_and_annotations() {
    let d = family(true);
    let mut db = open(&d);
    assert_eq!(db.info().entry_count, 3);
    let p = db.page(1, 200).unwrap();
    assert_eq!(p.entries.len(), 3);
    assert_eq!(p.next, None);
    assert!(p.entries[0].deleted);
    assert_eq!(p.entries[0].white.as_deref(), Some("Alpha"));
    assert_eq!(p.entries[2].kind, EntryKind::Text);
    assert!(p.entries[2].white.is_none());
    let g = db.load(1).unwrap();
    assert_eq!(g.moves_status, DataStatus::Available);
    assert_eq!(g.annotations_status, DataStatus::Missing);
    let g = g.game.unwrap();
    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.tags["White"], "Alpha");
    assert_eq!(g.tags["Result"], "1-0");
}
#[test]
fn cbh_without_moves_remains_browsable() {
    let d = family(false);
    let mut db = open(&d);
    assert_eq!(db.page(1, 200).unwrap().entries.len(), 3);
    let g = db.load(1).unwrap();
    assert!(g.game.is_none());
    assert_eq!(g.moves_status, DataStatus::Missing);
}
#[test]
fn corrupt_selected_stream_does_not_break_other_entries() {
    let d = family(true);
    let mut db = open(&d);
    assert_eq!(db.load(2).unwrap().moves_status, DataStatus::Corrupt);
    assert_eq!(db.load(1).unwrap().moves_status, DataStatus::Available);
    assert_eq!(db.load(3).unwrap().moves_status, DataStatus::Unsupported);
}
#[test]
fn paging_bounds_and_large_sparse_header_do_not_scan() {
    let d = family(false);
    let mut db = open(&d);
    for (start, limit) in [(0, 1), (1, 0), (1, 201), (u64::MAX, 1), (4, 1)] {
        assert!(db.page(start, limit).is_err());
    }
    assert_eq!(db.page(1, 2).unwrap().next, Some(3));
    assert_eq!(db.page(3, 2).unwrap().entries.len(), 1);
    let path = d.path().join("Synthetic.cbh");
    let mut f = fs::OpenOptions::new().write(true).open(&path).unwrap();
    let count = 1_000_001u32;
    f.set_len(count as u64 * 46).unwrap();
    f.seek(SeekFrom::Start(7)).unwrap();
    f.write_all(&count.to_be_bytes()[1..]).unwrap();
    drop(f);
    let mut db = open(&d);
    db.page(1, 200).unwrap();
    assert!(db.bytes_read() < 15000);
}
#[test]
fn source_mutation_is_an_error_and_bad_name_table_is_partial() {
    let d = family(true);
    fs::write(d.path().join("Synthetic.cbp"), [0]).unwrap();
    let mut db = open(&d);
    let g = db.load(1).unwrap();
    assert!(g.summary.white.is_none());
    assert!(g.game.is_some());
    fs::write(d.path().join("Synthetic.cbh"), [0]).unwrap();
    assert_eq!(db.page(1, 1).unwrap_err().code(), "sourceChanged");
}
#[test]
fn classic_annotations_load_and_bad_records_are_corrupt() {
    let d = family(true);
    let cbh = d.path().join("Synthetic.cbh");
    // Without an annotation offset the game has no annotations.
    fs::write(d.path().join("Synthetic.cba"), [0; 26]).unwrap();
    assert_eq!(
        open(&d).load(1).unwrap().annotations_status,
        DataStatus::Missing
    );
    // Game 1 points at a CBA record at offset 26: text after the first move
    // (position 0, English) and a green square on e4 after the second move.
    let mut record = vec![0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    let mut body = vec![0, 0, 0, 2, 0, 13, 0, b'*'];
    body.extend(b"Hello");
    body.extend([0, 0, 1, 4, 0, 8, 2, 4 * 8 + 3 + 1]);
    record.extend(((14 + body.len()) as u32).to_be_bytes());
    record.extend(&body);
    let mut cba = vec![0u8; 26];
    cba.extend(&record);
    fs::write(d.path().join("Synthetic.cba"), &cba).unwrap();
    let mut h = fs::read(&cbh).unwrap();
    h[46 + 5..46 + 9].copy_from_slice(&26u32.to_be_bytes());
    fs::write(&cbh, &h).unwrap();
    let g = open(&d).load(1).unwrap();
    assert_eq!(g.annotations_status, DataStatus::Available);
    let game = g.game.unwrap();
    assert_eq!(game.nodes[1].comments, vec!["Hello"]);
    assert_eq!(game.nodes[2].squares[0].square, "e4");
    assert_eq!(game.nodes[2].squares[0].color, 'G');
    // A record length that runs past the game's annotation block is corrupt;
    // the moves still load.
    cba[26 + 14 + 5] = 200;
    fs::write(d.path().join("Synthetic.cba"), &cba).unwrap();
    let g = open(&d).load(1).unwrap();
    assert_eq!(g.annotations_status, DataStatus::Corrupt);
    assert!(g.game.is_some());
}
#[test]
fn classic_annotators_sources_and_teams_resolve() {
    let d = family(true);
    let table = |ext: &str, stride: usize, fields: &[(usize, &[u8])]| {
        let mut t = vec![0u8; 28 + stride];
        t[0..4].copy_from_slice(&1u32.to_le_bytes());
        t[8..12].copy_from_slice(&1234567890u32.to_le_bytes());
        t[12..16].copy_from_slice(&((stride - 9) as u32).to_le_bytes());
        for (at, bytes) in fields {
            t[28 + at..28 + at + bytes.len()].copy_from_slice(bytes);
        }
        fs::write(d.path().join(format!("Synthetic.{ext}")), t).unwrap();
    };
    let date = ((1985u32 << 9) | (3 << 5) | 1).to_le_bytes();
    table("cbc", 62, &[(9, b"Lasker,Emanuel")]);
    table(
        "cbs",
        68,
        &[(9, b"CBM 001"), (34, b"ChessBase"), (50, &date)],
    );
    table("cbe", 72, &[(9, b"Metropolitan")]);
    // CBJ: LE32 version, record size and count; BE32 team ids from byte 32.
    let mut j = vec![0u8; 32 + 3 * 120];
    j[0..4].copy_from_slice(&11u32.to_le_bytes());
    j[4..8].copy_from_slice(&120u32.to_le_bytes());
    j[8..12].copy_from_slice(&3u32.to_le_bytes());
    j[32 + 4..32 + 8].fill(0xff);
    j[152..160].fill(0xff);
    fs::write(d.path().join("Synthetic.cbj"), j).unwrap();
    let mut db = open(&d);
    let e = &db.page(1, 3).unwrap().entries;
    assert_eq!(e[0].annotator.as_deref(), Some("Lasker,Emanuel"));
    assert_eq!(e[0].source.as_deref(), Some("CBM 001"));
    assert_eq!(e[0].source_publisher.as_deref(), Some("ChessBase"));
    assert_eq!(e[0].source_date.as_deref(), Some("1985.03.01"));
    assert_eq!(e[0].white_team.as_deref(), Some("Metropolitan"));
    assert_eq!(e[0].black_team, None);
    assert_eq!(e[1].white_team, None);
    let g = db.load(1).unwrap().game.unwrap();
    assert_eq!(g.tags["Annotator"], "Lasker,Emanuel");
    assert_eq!(g.tags["SourceDate"], "1985.03.01");
    // A damaged CBJ leaves the teams unknown and is reported.
    fs::write(d.path().join("Synthetic.cbj"), [0u8; 40]).unwrap();
    let mut db = open(&d);
    assert!(
        db.info()
            .diagnostics
            .iter()
            .any(|d| d.message.contains("cbj"))
    );
    assert_eq!(db.page(1, 1).unwrap().entries[0].white_team, None);
}
#[test]
fn classic_search_resolves_names_from_the_tables() {
    use cbh_reader::search::{GameFilter, PlayerQuery};
    let d = family(true);
    let mut db = open(&d);
    let filter = GameFilter {
        white: PlayerQuery {
            last: "alp".into(),
            first: String::new(),
        },
        tournament: "alpha".into(),
        ..Default::default()
    };
    let found = db
        .search(
            &filter,
            10,
            &std::sync::atomic::AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
    // Entries 1 and 2 are games naming player 0 and tournament 0; 3 is text.
    assert_eq!(found.ordinals, [1, 2]);
    assert_eq!(found.scanned, 3);
}
