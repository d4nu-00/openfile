use openfile::{
    analysis_jobs::{JobKind, JobStatus, RunParams},
    clipboard::RowRange,
    registry::Registry,
};

/// List rows `start..end` (1-based, end exclusive).
fn rows(start: u64, end: u64) -> Vec<RowRange> {
    vec![RowRange {
        start: start.to_string(),
        end: end.to_string(),
    }]
}

/// The mock engine copied as `mock-uci-legal` (see `mock-uci.rs`).
fn legal_engine(d: &tempfile::TempDir) -> std::path::PathBuf {
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let name = format!("mock-uci{}", std::env::consts::EXE_SUFFIX);
    let mock = [deps.join(&name), deps.parent().unwrap().join(&name)]
        .into_iter()
        .find(|p| p.exists())
        .expect("mock-uci is missing: run `cargo test --workspace`");
    let copy = d
        .path()
        .join(format!("mock-uci-legal{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(mock, &copy).unwrap();
    copy
}

/// A new 2CBH database with `games` saved into it; returns its id.
fn database(r: &Registry, d: &tempfile::TempDir, games: &[&str]) -> String {
    let db = r
        .create_database(d.path().join("Jobs"), "create".into())
        .unwrap();
    for (i, pgn) in games.iter().enumerate() {
        let path = d.path().join(format!("g{i}.pgn"));
        std::fs::write(&path, pgn).unwrap();
        let c = r.open_pgn(path, format!("o{i}")).unwrap();
        let doc = r
            .load_pgn_game(c.collection_id, 0, format!("l{i}"))
            .unwrap()
            .document_id;
        r.save_game(doc.clone(), db.database_id.clone(), "0".into(), false)
            .unwrap();
        r.close_document(doc).unwrap();
    }
    db.database_id
}

fn run(r: &Registry, engine: std::path::PathBuf) -> openfile::analysis_jobs::RunReport {
    r.run_analysis_jobs(
        &RunParams {
            engine,
            options: Default::default(),
            // 15 s of job time take 15 ms.
            millis_per_second: Some(1),
        },
        |_| {},
    )
    .unwrap()
}

#[test]
fn jobs_analyse_each_game_and_replace_it() {
    let d = tempfile::tempdir().unwrap();
    let engine = legal_engine(&d);
    let r = Registry::default();
    let db = database(
        &r,
        &d,
        &[
            "[White \"A\"]\n[Black \"B\"]\n\n1. e4 e5 *\n",
            "[White \"C\"]\n[Black \"D\"]\n\n1. d4 d5 2. c4 *\n",
        ],
    );
    let (view, left_out) = r
        .add_analysis_jobs(
            db.clone(),
            rows(1, 3),
            JobKind::Analysis {
                lines: 2,
                seconds: 15,
            },
        )
        .unwrap();
    assert_eq!(left_out, 0);
    assert_eq!(view.jobs.len(), 2);
    assert_eq!(view.jobs[0].label, "A – B");
    // The final position of each main line.
    assert_eq!(view.jobs[1].moves, ["d2d4", "d7d5", "c2c4"]);
    let mut steps = 0;
    let report = r
        .run_analysis_jobs(
            &RunParams {
                engine: engine.clone(),
                options: Default::default(),
                millis_per_second: Some(1),
            },
            |_| steps += 1,
        )
        .unwrap();
    assert!(!report.stopped);
    assert_eq!((report.done, report.skipped, report.failed), (2, 0, 0));
    assert!(steps >= 4);
    assert!(report.jobs.iter().all(|j| j.status == JobStatus::Done));
    // Game 1 now continues with the engine's two lines from 1...e5.
    let g = r.load_entry(db.clone(), "1".into(), "l".into()).unwrap();
    let game = g.game.unwrap();
    let after = &game.nodes[2];
    assert_eq!(after.children.len(), 2);
    let first = &game.nodes[after.children[0] as usize];
    assert_eq!(first.uci.as_deref(), Some("a2a3"));
    // There is nothing left to run.
    assert!(
        r.run_analysis_jobs(
            &RunParams {
                engine,
                options: Default::default(),
                millis_per_second: Some(1),
            },
            |_| {},
        )
        .is_err()
    );
}

#[test]
fn open_and_changed_games_are_skipped_and_deep_jobs_run() {
    let d = tempfile::tempdir().unwrap();
    let engine = legal_engine(&d);
    let r = Registry::default();
    let db = database(&r, &d, &["1. e4 e5 *\n", "1. Nf3 *\n", "1. c4 *\n"]);
    // A job from the board: game 1, after 1.e4.
    let loaded = r.load_entry(db.clone(), "1".into(), "l".into()).unwrap();
    let doc = loaded.document_id.unwrap();
    let view = r
        .add_analysis_job(doc.clone(), 1, JobKind::Deep { seconds: 30 })
        .unwrap();
    assert_eq!(view.jobs[0].moves, ["e2e4"]);
    r.add_analysis_jobs(db.clone(), rows(2, 4), JobKind::Deep { seconds: 30 })
        .unwrap();
    // Game 2 changes after its job was added.
    let g2 = r.load_entry(db.clone(), "2".into(), "l2".into()).unwrap();
    let doc2 = g2.document_id.unwrap();
    r.edit_document(
        doc2.clone(),
        serde_json::from_str(r#"{"kind":"appendMove","parent":1,"uci":"d7d5"}"#).unwrap(),
        "0".into(),
    )
    .unwrap();
    r.replace_game(doc2.clone(), "1".into(), false).unwrap();
    r.close_document(doc2).unwrap();
    // Game 1 is still open in a tab.
    let report = run(&r, engine.clone());
    assert_eq!((report.done, report.skipped, report.failed), (1, 2, 0));
    assert_eq!(
        report.jobs[0].message.as_deref(),
        Some("The game is open in a tab")
    );
    assert_eq!(
        report.jobs[1].message.as_deref(),
        Some("The game changed since the job was added")
    );
    // Game 3's deep analysis went in.
    let g3 = r.load_entry(db.clone(), "3".into(), "l3".into()).unwrap();
    assert!(g3.game.unwrap().nodes.len() > 2);
    // Closing the tab and restarting the first job lets it run.
    r.close_document(doc).unwrap();
    r.edit_analysis_job(report.jobs[0].id, "restart").unwrap();
    let again = run(&r, engine);
    assert_eq!(again.done, 1);
    let g1 = r.load_entry(db, "1".into(), "l4".into()).unwrap();
    // 1...e5 is still the main line; the analysis added variations to it.
    let game = g1.game.unwrap();
    assert_eq!(
        game.nodes[game.nodes[1].children[0] as usize]
            .uci
            .as_deref(),
        Some("e7e5")
    );
    assert!(game.nodes[1].children.len() > 1);
}

#[test]
fn the_queue_is_bounded_and_needs_an_editable_database() {
    let d = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let db = database(&r, &d, &["1. e4 *\n"]);
    let kind = JobKind::Analysis {
        lines: 1,
        seconds: 15,
    };
    // The reference app's minimum times.
    assert!(
        r.add_analysis_jobs(
            db.clone(),
            rows(1, 2),
            JobKind::Analysis {
                lines: 1,
                seconds: 5
            }
        )
        .is_err()
    );
    assert!(
        r.add_analysis_jobs(db.clone(), rows(1, 2), JobKind::Deep { seconds: 20 })
            .is_err()
    );
    // At most 1,000 jobs: fill the queue, then one more is refused.
    for _ in 0..1_000 {
        r.add_analysis_jobs(db.clone(), rows(1, 2), kind.clone())
            .unwrap();
    }
    assert!(
        r.add_analysis_jobs(db.clone(), rows(1, 2), kind.clone())
            .is_err()
    );
    r.clear_analysis_jobs().unwrap();
    let (view, _) = r
        .add_analysis_jobs(db.clone(), rows(1, 2), kind.clone())
        .unwrap();
    let id = view.jobs[0].id;
    let view = r.edit_analysis_job(id, "skip").unwrap();
    assert_eq!(view.jobs[0].status, JobStatus::Skipped);
    let view = r.edit_analysis_job(id, "remove").unwrap();
    assert!(view.jobs.is_empty());
    assert!(r.clear_analysis_jobs().unwrap().jobs.is_empty());
    // A read-only database takes no jobs.
    r.close_database(db).unwrap();
    let ro = r
        .open_database(d.path().join("Jobs.2cbh"), "ro".into())
        .unwrap();
    assert!(
        r.add_analysis_jobs(ro.database_id, rows(1, 2), kind)
            .is_err()
    );
}
