use cbh_reader::v2::write::Writer;
use chess_core::{GameLimits, parse_pgn};
use openfile::cloud::{CloudDownloads, status};

#[test]
fn a_database_on_disk_needs_no_download() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Local.2cbh");
    let games = parse_pgn(b"1. d4 d5 *", &GameLimits::default()).unwrap();
    Writer::create(&path).unwrap().append_all(&games).unwrap();
    let s = status(&path).unwrap();
    assert!(s.files.is_empty());
    assert_eq!(s.bytes, "0");
    let mut reports = vec![];
    CloudDownloads::default()
        .download(path, "r".into(), |p| reports.push(p))
        .unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!((reports[0].files_done, reports[0].files), (0, 0));
    // Stop for an unknown request is ignored.
    CloudDownloads::default().cancel("other");
}
