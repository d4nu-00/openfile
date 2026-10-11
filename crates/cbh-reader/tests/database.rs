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
fn existing_annotation_companion_is_never_available() {
    let d = family(true);
    fs::write(d.path().join("Synthetic.cba"), [0]).unwrap();
    assert_eq!(
        open(&d).load(1).unwrap().annotations_status,
        DataStatus::NotDecoded
    );
}
