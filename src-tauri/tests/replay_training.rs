use openfile::replay_training::{Progress, ReplayTrainingStore};

#[test]
fn progress_survives_a_new_store_instance() {
    let dir = tempfile::tempdir().unwrap();
    let first = ReplayTrainingStore::default();
    first.set_dir(dir.path().to_path_buf());
    first
        .record(
            "game/question".into(),
            Progress {
                attempts: 2,
                needs_retry: true,
                best_points: 3,
            },
        )
        .unwrap();
    let second = ReplayTrainingStore::default();
    second.set_dir(dir.path().to_path_buf());
    assert_eq!(second.load().unwrap()["game/question"].attempts, 2);
}

#[test]
fn corrupt_or_newer_progress_is_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let store = ReplayTrainingStore::default();
    store.set_dir(dir.path().to_path_buf());
    let path = dir.path().join("progress.json");
    std::fs::write(&path, r#"{"version":2,"items":{}}"#).unwrap();
    assert!(store.record("key".into(), Progress::default()).is_err());
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("\"version\":2")
    );
    std::fs::write(&path, "{").unwrap();
    assert!(store.load().is_err());
}
