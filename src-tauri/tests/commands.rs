use chess_workbench::{commands::DiagnosticDto, registry::Registry};
fn family() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let mut h = vec![0u8; 92];
    h[..6].copy_from_slice(&[0, 0, 0x2c, 0, 0x2e, 1]);
    h[9] = 2;
    h[46] = 1;
    h[73] = 2;
    std::fs::write(d.path().join("test.cbh"), h).unwrap();
    std::fs::write(d.path().join("test.cbg"), [0, 0, 0, 7, 0xff, 0, 0x0e]).unwrap();
    d
}
#[test]
fn diagnostic_offsets_are_decimal_strings_on_wire() {
    let d = DiagnosticDto::from(cbh_reader::Diagnostic {
        code: "bounds".into(),
        message: "outside file".into(),
        file: None,
        offset: Some(9_007_199_254_740_993),
    });
    assert_eq!(
        serde_json::to_value(d).unwrap()["offset"],
        "9007199254740993"
    );
}
#[test]
fn handles_are_independent_and_working_copy_survives_database_close() {
    let r = Registry::default();
    let d = family();
    let path = d.path().join("test.cbh");
    let a = r.open_database(path.clone(), "a".into()).unwrap();
    let b = r.open_database(path, "b".into()).unwrap();
    assert_ne!(a.database_id, b.database_id);
    let loaded = r
        .load_entry(a.database_id.clone(), "1".into(), "c".into())
        .unwrap();
    let id = loaded.document_id.unwrap();
    r.close_database(a.database_id.clone()).unwrap();
    assert!(
        r.read_page(a.database_id, "1".into(), 1, "x".into())
            .is_err()
    );
    assert!(
        r.read_page(b.database_id, "1".into(), 1, "x".into())
            .is_ok()
    );
    assert_eq!(
        r.position_at(id.clone(), 2, "p".into()).unwrap().turn,
        "white"
    );
    r.close_document(id.clone()).unwrap();
    assert!(r.position_at(id, 0, "p".into()).is_err());
    assert!(r.close_document("unknown".into()).is_err());
}
#[test]
fn decimal_ordinals_never_pass_through_float_and_dtos_match_contract() {
    let r = Registry::default();
    let d = family();
    let db = r
        .open_database(d.path().join("test.cbh"), "open".into())
        .unwrap();
    for bad in [
        "-1",
        "1.0",
        " 1",
        "1e3",
        "",
        "18446744073709551616",
        "9007199254740993",
    ] {
        assert!(
            r.read_page(db.database_id.clone(), bad.into(), 1, "r".into())
                .is_err()
        );
    }
    let p = r
        .read_page(db.database_id.clone(), "1".into(), 1, "page".into())
        .unwrap();
    let json = serde_json::to_value(p).unwrap();
    assert_eq!(json["entries"][0]["ordinal"], "1");
    assert_eq!(json["requestId"], "page");
    let g = r
        .load_entry(db.database_id, "1".into(), "game".into())
        .unwrap();
    let json = serde_json::to_value(g).unwrap();
    assert_eq!(json["game"]["nodes"][1]["san"], "e4");
    assert!(json["game"]["startFen"].is_string());
}
#[test]
fn registry_budgets_reject_excess_handles() {
    let r = Registry::default();
    let d = family();
    for _ in 0..4 {
        r.open_database(d.path().join("test.cbh"), "r".into())
            .unwrap();
    }
    assert_eq!(
        r.open_database(d.path().join("test.cbh"), "r".into())
            .unwrap_err()
            .code,
        "limitExceeded"
    );
    let r = Registry::default();
    let db = r
        .open_database(d.path().join("test.cbh"), "r".into())
        .unwrap();
    for _ in 0..10 {
        r.load_entry(db.database_id.clone(), "1".into(), "r".into())
            .unwrap();
    }
    assert_eq!(
        r.load_entry(db.database_id, "1".into(), "r".into())
            .unwrap_err()
            .code,
        "limitExceeded"
    );
}
