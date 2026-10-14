mod support;
use engine::{EngineError, EnginePayload, EngineSession};
use std::{
    fs,
    time::{Duration, Instant},
};
#[test]
fn handshake_options_and_structured_executable_paths() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("directory with spaces");
    fs::create_dir(&dir).unwrap();
    let file = dir.join(if cfg!(windows) {
        "mock engine.exe"
    } else {
        "mock engine ; argument"
    });
    fs::copy(env!("CARGO_BIN_EXE_mock-uci"), &file).unwrap();
    let mut spec = support::spec("normal");
    spec.executable = file;
    let mut s = EngineSession::spawn(spec, Duration::from_secs(1)).unwrap();
    assert_eq!(s.identity()["name"], "Workbench mock");
    assert_eq!(s.options().len(), 3);
    s.set_option("Threads", "4").unwrap();
    assert!(s.set_option("Threads", "0").is_err());
    assert!(s.set_option("missing", "1").is_err());
    s.analyse(support::request("first", "e2e4")).unwrap();
    let events = support::events_until_current_info(&mut s, "first");
    assert!(events.iter().any(support::contains_old_marker));
    s.stop().unwrap();
    s.shutdown().unwrap();
    assert!(s.process_id().is_none());
}
#[test]
fn delayed_old_info_is_not_assigned_to_new_position() {
    let mut s = support::mock_session("late-info-before-bestmove");
    s.analyse(support::request("old", "e2e4")).unwrap();
    s.analyse(support::request("new", "d2d4")).unwrap();
    let events = support::events_until_current_info(&mut s, "new");
    assert!(
        events
            .iter()
            .filter(|e| e.request_id == "new")
            .all(|e| !support::contains_old_marker(e))
    );
    assert!(
        events
            .iter()
            .any(|e| e.request_id == "old" && matches!(e.payload, EnginePayload::BestMove { .. }))
    );
    s.shutdown().unwrap();
}
#[test]
fn ignored_stop_restarts_and_reapplies_options_before_new_search() {
    let d = tempfile::tempdir().unwrap();
    let log = d.path().join("commands with spaces.log");
    let mut spec = support::spec("ignored-stop");
    spec.arguments.push(log.as_os_str().into());
    let mut s = EngineSession::spawn(spec, Duration::from_secs(1)).unwrap();
    s.set_option("Threads", "3").unwrap();
    let old = s.process_id();
    s.analyse(support::request("old", "e2e4")).unwrap();
    let start = Instant::now();
    s.analyse(support::request("new", "d2d4")).unwrap();
    assert!(start.elapsed() < Duration::from_secs(4));
    assert_ne!(s.process_id(), old);
    let events = support::events_until_current_info(&mut s, "new");
    assert!(
        events
            .iter()
            .filter(|e| e.request_id == "new")
            .all(|e| !support::contains_old_marker(e))
    );
    let text = fs::read_to_string(log).unwrap();
    assert_eq!(text.matches("setoption name Threads value 3").count(), 2);
    s.shutdown().unwrap();
}
#[test]
fn malformed_and_overlong_output_are_diagnostics_and_do_not_crash() {
    for mode in ["malformed", "overlong"] {
        let mut s = support::mock_session(mode);
        s.analyse(support::request("current", "e2e4")).unwrap();
        let events = support::events_until_current_info(&mut s, "current");
        assert!(
            events
                .iter()
                .any(|e| matches!(e.payload, EnginePayload::Error { .. }))
        );
        s.shutdown().unwrap();
    }
}
#[test]
fn stderr_and_info_saturation_do_not_block_completion() {
    for mode in ["stderr-saturation", "info-flood"] {
        let mut s = support::mock_session(mode);
        s.analyse(support::request("current", "e2e4")).unwrap();
        support::events_until_current_info(&mut s, "current");
        s.stop().unwrap();
        let mut best = false;
        for _ in 0..140 {
            match s.recv_event(Duration::from_millis(5)).unwrap() {
                Some(e) => best |= matches!(e.payload, EnginePayload::BestMove { .. }),
                None => break,
            }
        }
        assert!(best);
        assert!(s.stderr_tail().len() <= 8192);
        if mode == "stderr-saturation" {
            assert!(!s.stderr_tail().is_empty());
        }
        s.shutdown().unwrap();
    }
}
#[test]
fn process_death_is_reported_and_reaped() {
    let mut s = support::mock_session("process-death");
    s.analyse(support::request("dying", "e2e4")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut exited = false;
    while Instant::now() < deadline {
        match s.recv_event(Duration::from_millis(50)) {
            Err(EngineError::Exited) => {
                exited = true;
                break;
            }
            Ok(_) => {}
            Err(e) => panic!("{e}"),
        }
    }
    assert!(exited);
    s.shutdown().unwrap();
    assert!(s.process_id().is_none());
}
#[test]
fn handshake_timeout_and_ignored_quit_are_bounded() {
    let start = Instant::now();
    let result = EngineSession::spawn(
        support::spec("handshake-timeout"),
        Duration::from_millis(100),
    );
    assert!(matches!(result, Err(EngineError::Timeout(_))));
    assert!(start.elapsed() < Duration::from_secs(2));
    let mut s = support::mock_session("ignored-quit");
    let start = Instant::now();
    s.shutdown().unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(s.process_id().is_none());
    s.shutdown().unwrap();
}
#[test]
fn invalid_positions_and_wrong_turn_cannot_interrupt_a_search() {
    let mut s = support::mock_session("normal");
    s.analyse(support::request("current", "e2e4")).unwrap();
    let mut invalid = support::request("invalid", "d2d4");
    invalid.root_turn = "white".into();
    assert!(s.analyse(invalid).is_err());
    assert!(s.set_option("Threads", "2").is_err());
    let events = support::events_until_current_info(&mut s, "current");
    assert!(events.iter().all(|e| e.request_id == "current"));
    s.shutdown().unwrap();
}

#[test]
fn unresponsive_stdin_and_ready_timeout_leave_no_child() {
    let mut s =
        EngineSession::spawn(support::spec("blocked-input"), Duration::from_secs(1)).unwrap();
    let start = Instant::now();
    assert!(s.set_option("Book File", &"x".repeat(16000)).is_err());
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(s.process_id().is_none());
    s.shutdown().unwrap();
}
#[test]
fn shutdown_is_bounded_when_a_descendant_inherits_pipes_after_parent_exit() {
    let d = tempfile::tempdir().unwrap();
    let pid = d.path().join("helper.pid");
    let mut spec = support::spec("descendant-pipes");
    spec.arguments
        .push(d.path().join("commands.log").into_os_string());
    spec.arguments.push(pid.clone().into_os_string());
    let mut session = EngineSession::spawn(spec, Duration::from_secs(1)).unwrap();
    assert!(fs::read_to_string(&pid).unwrap().parse::<u32>().unwrap() > 0);
    let start = Instant::now();
    session.shutdown().unwrap();
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "Inherited pipe joins exceeded shutdown deadline"
    );
    assert!(session.process_id().is_none());
    {
        let helper = fs::read_to_string(pid).unwrap().parse().unwrap();
        let deadline = Instant::now() + Duration::from_millis(500);
        while helper_alive(helper) {
            assert!(
                Instant::now() < deadline,
                "Helper process remained alive after shutdown"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
#[cfg(unix)]
fn helper_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}
#[cfg(windows)]
fn helper_alive(pid: u32) -> bool {
    use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::*};
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return false;
    }
    let mut code = 0;
    let alive = unsafe { GetExitCodeProcess(process, &mut code) } != 0 && code == 259;
    unsafe {
        CloseHandle(process);
    }
    alive
}

#[test]
fn chess960_positions_switch_uci_chess960_and_need_the_option() {
    use engine::AnalysisRequest;
    let d = tempfile::tempdir().unwrap();
    let log = d.path().join("commands.log");
    let mut spec = support::spec("chess960");
    spec.arguments.push(log.as_os_str().into());
    let mut s = EngineSession::spawn(spec, Duration::from_secs(1)).unwrap();
    // The king on b1 castles with the h1 rook: Chess960 only.
    let fen = "rk5r/pppppppp/8/8/8/8/PPPPPPPP/RK5R w KQkq - 0 1";
    let request = |id: &str, start_fen: &str, history: Vec<String>, turn: &str| AnalysisRequest {
        request_id: id.into(),
        start_fen: start_fen.into(),
        history,
        root_turn: turn.into(),
    };
    s.analyse(request("c960", fen, vec!["b1h1".into()], "black"))
        .unwrap();
    s.stop().unwrap();
    s.analyse(support::request("std", "e2e4")).unwrap();
    s.stop().unwrap();
    s.shutdown().unwrap();
    let text = fs::read_to_string(&log).unwrap();
    let on = text.find("setoption name UCI_Chess960 value true").unwrap();
    let position = text
        .find(&format!("position fen {fen} moves b1h1"))
        .unwrap();
    let off = text
        .find("setoption name UCI_Chess960 value false")
        .unwrap();
    assert!(on < position && position < off, "{text}");

    // An engine without the option is refused for Chess960 positions.
    let mut plain = support::mock_session("normal");
    let err = plain
        .analyse(request("c960", fen, vec![], "white"))
        .unwrap_err();
    assert!(err.to_string().contains("Chess960"), "{err}");
    plain.shutdown().unwrap();
}
