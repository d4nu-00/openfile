use openfile::{
    documents::Edit,
    registry::Registry,
    save::{atomic_pgn, atomic_pgn_with_writer},
};
use std::{fs, io::Write, path::PathBuf};
fn pgn(r: &Registry, d: &tempfile::TempDir) -> (String, String) {
    let path = d.path().join("synthetic.pgn");
    fs::write(&path,"[White \"Alpha\"]\n[Result \"*\"]\n\n1. e4 e5 *\n\n[White \"Second\"]\n[Result \"*\"]\n\n1. d4 d5 *").unwrap();
    let c = r.open_pgn(path, "open".into()).unwrap();
    assert_eq!(c.game_count, 2);
    let doc = r
        .load_pgn_game(c.collection_id.clone(), 0, "load".into())
        .unwrap();
    (c.collection_id, doc.document_id)
}
#[test]
fn failed_replacement_preserves_existing_destination() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("keep.pgn");
    fs::write(&path, "old contents").unwrap();
    let err = atomic_pgn_with_writer(&path, |writer| {
        writer.write_all(b"partial")?;
        Err(std::io::Error::other("simulated disk failure"))
    });
    assert!(err.is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "old contents");
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 1);
}
#[test]
fn atomic_replacement_writes_utf8_and_preserves_permissions() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("keep.pgn");
    fs::write(&path, "old").unwrap();
    let old = fs::metadata(&path).unwrap().permissions();
    atomic_pgn(&path, "{García} *").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "{García} *");
    assert_eq!(
        fs::metadata(path).unwrap().permissions().readonly(),
        old.readonly()
    );
}
#[test]
fn pgn_collection_copies_survive_close_and_edits_are_revisioned() {
    let r = Registry::default();
    let d = tempfile::tempdir().unwrap();
    let (c, id) = pgn(&r, &d);
    let second = r.load_pgn_game(c.clone(), 1, "second".into()).unwrap();
    assert_eq!(second.game.tags["White"], "Second");
    r.close_pgn(c.clone()).unwrap();
    assert!(r.load_pgn_game(c, 0, "bad".into()).is_err());
    let same = r
        .edit_document(
            id.clone(),
            Edit::AppendMove {
                parent: 0,
                uci: "e2e4".into(),
            },
            "0".into(),
        )
        .unwrap();
    assert_eq!(same.revision, "0");
    assert!(!same.dirty);
    let branch = r
        .edit_document(
            id.clone(),
            Edit::AppendMove {
                parent: 0,
                uci: "d2d4".into(),
            },
            "0".into(),
        )
        .unwrap();
    assert_eq!(branch.revision, "1");
    assert!(branch.dirty);
    assert_eq!(branch.game.nodes[0].children.len(), 2);
    assert_eq!(
        r.edit_document(
            id.clone(),
            Edit::SetComments {
                node: 1,
                comments: vec!["stale".into()]
            },
            "0".into()
        )
        .unwrap_err()
        .code,
        "staleRevision"
    );
    let comment = r
        .edit_document(
            id.clone(),
            Edit::SetComments {
                node: 1,
                comments: vec!["Centre".into()],
            },
            "1".into(),
        )
        .unwrap();
    assert_eq!(comment.revision, "2");
    assert!(
        r.edit_document(
            id.clone(),
            Edit::AppendMove {
                parent: 0,
                uci: "a1a8".into()
            },
            "2".into()
        )
        .is_err()
    );
    let path = d.path().join("saved.pgn");
    r.save_document(id.clone(), path.clone(), "2".into(), false)
        .unwrap();
    let text = fs::read(&path).unwrap();
    let games = chess_core::parse_pgn(&text, &chess_core::GameLimits::default()).unwrap();
    assert_eq!(games[0].nodes[1].comments, vec!["Centre"]);
}
#[test]
fn edits_during_save_remain_dirty_and_saved_revision_is_exact() {
    let r = Registry::default();
    let d = tempfile::tempdir().unwrap();
    let (_, id) = pgn(&r, &d);
    let path = d.path().join("snapshot.pgn");
    let result = r
        .save_document_with_writer(id.clone(), path.clone(), "0".into(), false, |p, text| {
            r.edit_document(
                id.clone(),
                Edit::SetComments {
                    node: 1,
                    comments: vec!["New edit".into()],
                },
                "0".into(),
            )
            .unwrap();
            atomic_pgn(p, text)
        })
        .unwrap();
    assert_eq!(result.saved_revision, "0");
    let current = r
        .edit_document(
            id,
            Edit::AppendMove {
                parent: 0,
                uci: "e2e4".into(),
            },
            "1".into(),
        )
        .unwrap();
    assert!(current.dirty);
    assert_eq!(current.revision, "1");
    assert!(!fs::read_to_string(path).unwrap().contains("New edit"));
}
fn cbh(r: &Registry, d: &tempfile::TempDir) -> String {
    let mut h = vec![0u8; 92];
    h[..6].copy_from_slice(&[0, 0, 0x2c, 0, 0x2e, 1]);
    h[9] = 2;
    h[46] = 1;
    h[73] = 2;
    // Annotations at CBA offset 1: one critical-position mark, which the
    // reader does not represent, so the export needs an acknowledgement.
    h[51..55].copy_from_slice(&1u32.to_be_bytes());
    fs::write(d.path().join("read-only.cbh"), h).unwrap();
    fs::write(d.path().join("read-only.cbg"), [0, 0, 0, 7, 0xff, 0, 0x0e]).unwrap();
    let mut cba = vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 21];
    cba.extend([0, 0, 0, 0x18, 0, 7, 2]);
    fs::write(d.path().join("read-only.cba"), cba).unwrap();
    let db = r
        .open_database(d.path().join("read-only.cbh"), "open".into())
        .unwrap();
    let doc = r
        .load_entry(db.database_id.clone(), "1".into(), "load".into())
        .unwrap();
    r.close_database(db.database_id).unwrap();
    doc.document_id.unwrap()
}
#[test]
fn cbh_exports_require_acknowledgement_and_refuse_source_aliases() {
    let r = Registry::default();
    let d = tempfile::tempdir().unwrap();
    let id = cbh(&r, &d);
    let out = d.path().join("export.pgn");
    assert_eq!(
        r.save_document(id.clone(), out.clone(), "0".into(), false)
            .unwrap_err()
            .code,
        "incompleteAnnotations"
    );
    r.save_document(id.clone(), out, "0".into(), true).unwrap();
    let source = d.path().join("read-only.cbh");
    let alias = d.path().join("alias.pgn");
    fs::hard_link(&source, &alias).unwrap();
    assert_eq!(
        r.save_document(id.clone(), alias, "0".into(), true)
            .unwrap_err()
            .code,
        "sourceDestination"
    );
    assert_eq!(
        r.save_document(id, d.path().join("read-only.CBG"), "0".into(), true)
            .unwrap_err()
            .code,
        "sourceDestination"
    );
    assert_eq!(fs::metadata(source).unwrap().len(), 92);
}
#[test]
fn bad_pgn_and_stale_save_do_not_replace_existing_state_or_files() {
    let r = Registry::default();
    let d = tempfile::tempdir().unwrap();
    let (_, id) = pgn(&r, &d);
    let bad = d.path().join("bad.pgn");
    fs::write(&bad, "1. e5 *").unwrap();
    assert!(r.open_pgn(bad, "bad".into()).is_err());
    let target = d.path().join("keep.pgn");
    fs::write(&target, "old").unwrap();
    assert_eq!(
        r.save_document(id, target.clone(), "1".into(), false)
            .unwrap_err()
            .code,
        "staleRevision"
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "old");
    assert!(
        atomic_pgn(
            &PathBuf::from("/nonexistent-workbench-test-directory/export.pgn"),
            "*"
        )
        .is_err()
    );
}
#[test]
fn source_alias_protection_includes_unparsed_database_companions() {
    let r = Registry::default();
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("read-only.cbgi"), b"private index").unwrap();
    let id = cbh(&r, &d);
    let alias = d.path().join("alias.pgn");
    fs::hard_link(d.path().join("read-only.cbgi"), &alias).unwrap();
    assert_eq!(
        r.save_document(id, alias.clone(), "0".into(), true)
            .unwrap_err()
            .code,
        "sourceDestination"
    );
    assert_eq!(fs::read(alias).unwrap(), b"private index");
}
#[test]
fn overlapping_saves_cannot_persist_an_older_snapshot_after_a_newer_save() {
    use std::{
        sync::{Arc, mpsc},
        time::Duration,
    };
    let r = Arc::new(Registry::default());
    let d = tempfile::tempdir().unwrap();
    let (_, id) = pgn(&r, &d);
    let path = d.path().join("ordered.pgn");
    let (started, ready) = mpsc::channel();
    let (release, delayed) = mpsc::channel();
    let old_registry = r.clone();
    let old_id = id.clone();
    let old_path = path.clone();
    let old = std::thread::spawn(move || {
        old_registry.save_document_with_writer(old_id, old_path, "0".into(), false, |p, text| {
            started.send(()).unwrap();
            delayed.recv_timeout(Duration::from_secs(3)).unwrap();
            atomic_pgn(p, text)
        })
    });
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    r.edit_document(
        id.clone(),
        Edit::SetComments {
            node: 1,
            comments: vec!["New revision".into()],
        },
        "0".into(),
    )
    .unwrap();
    let (persisted, completion) = mpsc::channel();
    let new_registry = r.clone();
    let new_id = id.clone();
    let new_path = path.clone();
    let new = std::thread::spawn(move || {
        new_registry.save_document_with_writer(new_id, new_path, "1".into(), false, |p, text| {
            atomic_pgn(p, text)?;
            persisted.send(()).unwrap();
            Ok(())
        })
    });
    let completed_out_of_order = completion.recv_timeout(Duration::from_millis(100)).is_ok();
    release.send(()).unwrap();
    old.join().unwrap().unwrap();
    new.join().unwrap().unwrap();
    assert!(
        !completed_out_of_order,
        "Newer save persisted while an older snapshot was still pending"
    );
    assert!(fs::read_to_string(path).unwrap().contains("New revision"));
    let document = r
        .edit_document(
            id,
            Edit::AppendMove {
                parent: 0,
                uci: "e2e4".into(),
            },
            "1".into(),
        )
        .unwrap();
    assert!(!document.dirty);
}
