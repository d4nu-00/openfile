use cbh_reader::{
    companions::{CompanionSet, choose_candidate},
    entities::EntityTables,
};
use std::path::Path;
fn table(path: &Path, stride: usize, start: usize, count: u32) {
    let mut bytes = vec![0; start + stride * count as usize];
    bytes[0..4].copy_from_slice(&count.to_le_bytes());
    bytes[8..12].copy_from_slice(&1234567890u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&((stride - 9) as u32).to_le_bytes());
    bytes[24] = (start - 28) as u8;
    std::fs::write(path, bytes).unwrap();
}
#[test]
fn discovers_mixed_case_extensions_and_missing_moves() {
    let dir = tempfile::tempdir().unwrap();
    let h = dir.path().join("Case Test.CBH");
    std::fs::write(&h, []).unwrap();
    std::fs::write(dir.path().join("case test.CBP"), []).unwrap();
    let files = CompanionSet::discover(&h).unwrap();
    assert!(files.get("cbp").is_some());
    assert!(files.get("cbg").is_none());
}
#[test]
fn duplicate_folded_names_are_ambiguous() {
    assert!(choose_candidate("demo", "cbp", &["Demo.cbp", "demo.CBP"]).is_err());
    assert_eq!(
        choose_candidate("demo", "cbp", &["other.cbp", "DEMO.CBP"]).unwrap(),
        Some("DEMO.CBP")
    );
}
#[test]
fn reads_both_entity_header_versions_and_unicode_names() {
    for start in [28, 32] {
        let dir = tempfile::tempdir().unwrap();
        let h = dir.path().join("Synthetic.cbh");
        std::fs::write(&h, []).unwrap();
        let path = dir.path().join("Synthetic.cbp");
        table(&path, 67, start, 1);
        let mut bytes = std::fs::read(&path).unwrap();
        let name = "García".as_bytes();
        bytes[start + 9..start + 9 + name.len()].copy_from_slice(name);
        bytes[start + 39..start + 42].copy_from_slice(b"Ada");
        std::fs::write(&path, bytes).unwrap();
        let t = dir.path().join("Synthetic.cbt");
        table(&t, 99, start, 1);
        let mut bytes = std::fs::read(&t).unwrap();
        bytes[start + 9..start + 14].copy_from_slice(b"Event");
        bytes[start + 49..start + 53].copy_from_slice(b"Site");
        std::fs::write(&t, bytes).unwrap();
        let files = CompanionSet::discover(&h).unwrap();
        let mut tables = EntityTables::open(&files, 8 * 1024 * 1024).unwrap();
        assert_eq!(
            tables.player(0).unwrap().text.as_deref(),
            Some("García, Ada")
        );
        assert_eq!(
            tables.tournament(0).unwrap().event.text.as_deref(),
            Some("Event")
        );
        assert!(tables.player(1).is_err());
    }
}
#[test]
fn invalid_utf8_and_full_width_strings_are_diagnosed() {
    let dir = tempfile::tempdir().unwrap();
    let h = dir.path().join("Synthetic.cbh");
    std::fs::write(&h, []).unwrap();
    let p = dir.path().join("Synthetic.cbp");
    table(&p, 67, 32, 2);
    let mut bytes = std::fs::read(&p).unwrap();
    bytes[41] = 0xff;
    bytes[32 + 67 + 9..32 + 67 + 39].fill(b'X');
    std::fs::write(&p, bytes).unwrap();
    let mut tables = EntityTables::open(&CompanionSet::discover(&h).unwrap(), 100).unwrap();
    assert!(!tables.player(0).unwrap().diagnostics.is_empty());
    let row = tables.player(1).unwrap();
    assert_eq!(row.text.as_deref(), Some("XXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"));
    assert!(!row.diagnostics.is_empty());
    assert!(tables.cached_bytes() <= 100);
}
#[test]
fn missing_names_are_unknown_and_cache_stays_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let h = dir.path().join("Synthetic.cbh");
    std::fs::write(&h, []).unwrap();
    let files = CompanionSet::discover(&h).unwrap();
    let mut tables = EntityTables::open(&files, 500).unwrap();
    assert!(tables.player(0).unwrap().text.is_none());
    table(&dir.path().join("Synthetic.cbp"), 67, 32, 100);
    let mut tables = EntityTables::open(&CompanionSet::discover(&h).unwrap(), 500).unwrap();
    for id in 0..100 {
        tables.player(id).unwrap();
        assert!(tables.cached_bytes() <= 500);
    }
}
#[test]
fn corrupt_entity_stride_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let h = dir.path().join("Synthetic.cbh");
    std::fs::write(&h, []).unwrap();
    table(&dir.path().join("Synthetic.cbp"), 65, 32, 1);
    assert!(EntityTables::open(&CompanionSet::discover(&h).unwrap(), 1000).is_err());
}
