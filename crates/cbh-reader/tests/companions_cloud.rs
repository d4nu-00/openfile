use cbh_reader::{Database, ReadLimits, companions::not_downloaded, v2::write::Writer};
use chess_core::{GameLimits, parse_pgn};

#[test]
fn files_on_disk_need_no_download() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Local.2cbh");
    let games = parse_pgn(b"1. e4 e5 *", &GameLimits::default()).unwrap();
    Writer::create(&path).unwrap().append_all(&games).unwrap();
    assert_eq!(not_downloaded(&path).unwrap(), vec![]);
    Database::open(&path, ReadLimits::default()).unwrap();
    // Not a database: the same refusal as opening it.
    assert!(not_downloaded(&d.path().join("Local.txt")).is_err());
}
