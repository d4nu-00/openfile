use cbh_reader::{
    EntryKind,
    header::{ClassicHeader, parse_record},
    source::SourceFile,
};
fn header(count: u32) -> [u8; 46] {
    let mut h = [0; 46];
    h[..6].copy_from_slice(&[0, 0, 0x2c, 0, 0x2e, 1]);
    h[7..10].copy_from_slice(&count.to_be_bytes()[1..]);
    h
}
#[test]
fn count_excludes_header_and_rejects_incomplete_records() {
    assert_eq!(
        ClassicHeader::parse(&header(3), 138).unwrap().entry_count,
        2
    );
    assert!(ClassicHeader::parse(&header(3), 137).is_err());
    assert!(ClassicHeader::parse(&header(4), 138).is_err());
    assert!(ClassicHeader::parse(&header(0), 46).is_err());
}
#[test]
fn extended_count_requires_newer_version() {
    let mut h = header(0xffffff);
    h[36..44].copy_from_slice(&20_000_001u64.to_be_bytes());
    assert_eq!(
        ClassicHeader::parse(&h, 20_000_001 * 46)
            .unwrap()
            .entry_count,
        20_000_000
    );
    h[2] = 0x24;
    assert_eq!(
        ClassicHeader::parse(&h, 20_000_001 * 46)
            .unwrap()
            .entry_count,
        0xfffffe
    );
}
#[test]
fn unused_complete_records_are_warned_not_counted() {
    let h = ClassicHeader::parse(&header(2), 138).unwrap();
    assert_eq!(h.entry_count, 1);
    assert!(!h.diagnostics.is_empty());
}
#[test]
fn unknown_headers_are_rejected() {
    assert!(ClassicHeader::parse(&[0; 46], 46).is_err());
    assert!(ClassicHeader::parse(&header(2)[..45], 92).is_err());
}
#[test]
fn game_fields_follow_documented_endianness_and_rating_mask() {
    let mut r = [0; 46];
    r[0] = 0x81;
    r[1..5].copy_from_slice(&0x10203040u32.to_be_bytes());
    r[9..12].copy_from_slice(&[0x12, 0x34, 0x56]);
    let date = (2025u32 << 9) | (10 << 5) | 9;
    r[24..27].copy_from_slice(&date.to_be_bytes()[1..]);
    r[27] = 2;
    r[31..33].copy_from_slice(&(0x8000u16 | 2400).to_be_bytes());
    let r = parse_record(&r, 1).unwrap();
    assert_eq!(r.moves_offset, Some(0x10203040));
    assert_eq!(r.white_id, Some(0x123456));
    assert_eq!(r.date.as_deref(), Some("2025.10.09"));
    assert_eq!(r.result.as_deref(), Some("1-0"));
    assert_eq!(r.white_elo, Some(2400));
    assert!(r.deleted);
}
#[test]
fn text_record_does_not_expose_game_fields() {
    let mut r = [0xff; 46];
    r[0] = 3;
    let r = parse_record(&r, 1).unwrap();
    assert_eq!(r.kind, EntryKind::Text);
    assert!(r.white_id.is_none());
    assert!(r.black_id.is_none());
    assert!(r.moves_offset.is_none());
}
#[test]
fn forfeits_and_lines_use_chessbase_result_strings() {
    use cbh_reader::header::pgn_result;
    for (code, evaluation, shown, pgn) in [
        (0, 0, "0-1", "0-1"),
        (2, 0, "1-0", "1-0"),
        (3, 0, "Line", "*"),
        (3, 16, "±", "*"),
        (4, 0, "-:+", "0-1"),
        (5, 0, "=:=", "1/2-1/2"),
        (6, 0, "+:-", "1-0"),
        (7, 0, "0-0", "*"),
    ] {
        let mut r = [0; 46];
        r[0] = 1;
        r[27] = code;
        r[28] = evaluation;
        let r = parse_record(&r, 1).unwrap();
        assert_eq!(r.result.as_deref(), Some(shown));
        assert_eq!(pgn_result(code), Some(pgn));
    }
}
#[test]
fn unknown_result_and_invalid_date_remain_unknown() {
    let mut r = [0; 46];
    r[0] = 1;
    r[27] = 8;
    let date = (2025u32 << 9) | (4 << 5) | 31;
    r[24..27].copy_from_slice(&date.to_be_bytes()[1..]);
    let r = parse_record(&r, 1).unwrap();
    assert!(r.result.is_none());
    assert!(r.date.is_none());
    assert!(!r.diagnostics.is_empty());
}
#[test]
fn range_reads_are_bounded_and_truncation_is_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.cbh");
    std::fs::write(&path, b"abcdef").unwrap();
    let f = SourceFile::open(&path).unwrap();
    assert_eq!(f.read_exact_range(2, 3).unwrap(), b"cde");
    assert_eq!(f.bytes_read(), 3);
    assert!(f.read_exact_range(u64::MAX, 2).is_err());
    assert!(f.read_exact_range(0, 7).is_err());
    std::fs::write(&path, b"ab").unwrap();
    assert!(f.read_exact_range(0, 2).is_err());
}
#[test]
fn replaced_source_is_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.cbh");
    std::fs::write(&path, b"abcdef").unwrap();
    let f = SourceFile::open(&path).unwrap();
    std::fs::rename(&path, dir.path().join("old")).unwrap();
    std::fs::write(&path, b"ABCDEF").unwrap();
    assert!(f.read_exact_range(0, 2).is_err());
}
