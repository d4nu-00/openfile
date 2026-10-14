//! The tablebase folder goes to engines as `SyzygyPath`. A test file of its
//! own: the chosen folder is process-wide.
use chess_workbench::tablebases::{Tablebases, configure_engine};
use engine::{EngineSession, EngineSpec};
use std::time::Duration;

fn mock_engine() -> std::path::PathBuf {
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let name = format!("mock-uci{}", std::env::consts::EXE_SUFFIX);
    [deps.join(&name), deps.parent().unwrap().join(&name)]
        .into_iter()
        .find(|p| p.exists())
        .expect("mock-uci is missing: run `cargo test --workspace`")
}

fn session(mode: &str, log: &std::path::Path) -> EngineSession {
    EngineSession::spawn(
        EngineSpec {
            executable: mock_engine(),
            arguments: vec![mode.into(), log.as_os_str().into()],
        },
        Duration::from_secs(5),
    )
    .unwrap()
}

#[test]
fn engines_get_the_chosen_tablebase_folder() {
    let d = tempfile::tempdir().unwrap();
    let folder = d.path().join("syzygy tables");
    std::fs::create_dir(&folder).unwrap();
    let log = d.path().join("commands.log");
    // No folder chosen: nothing is set.
    let mut s = session("syzygy", &log);
    configure_engine(&mut s);
    s.shutdown().unwrap();
    assert!(
        !std::fs::read_to_string(&log)
            .unwrap()
            .contains("SyzygyPath")
    );
    let t = Tablebases::default();
    t.set_dir(Some(&folder)).unwrap();
    let mut s = session("syzygy", &log);
    configure_engine(&mut s);
    s.shutdown().unwrap();
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(
        text.contains(&format!(
            "setoption name SyzygyPath value {}",
            folder.display()
        )),
        "{text}"
    );
    // An engine without the option is left alone.
    let other = d.path().join("other.log");
    let mut s = session("normal", &other);
    configure_engine(&mut s);
    s.shutdown().unwrap();
    assert!(
        !std::fs::read_to_string(&other)
            .unwrap()
            .contains("SyzygyPath")
    );
}
