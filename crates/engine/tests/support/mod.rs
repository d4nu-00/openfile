use engine::{AnalysisRequest, EngineEvent, EnginePayload, EngineSession, EngineSpec};
use std::time::{Duration, Instant};
pub fn spec(mode: &str) -> EngineSpec {
    EngineSpec {
        executable: env!("CARGO_BIN_EXE_mock-uci").into(),
        arguments: vec![mode.into()],
    }
}
pub fn mock_session(mode: &str) -> EngineSession {
    EngineSession::spawn(spec(mode), Duration::from_secs(1)).unwrap()
}
pub fn request(id: &str, uci: &str) -> AnalysisRequest {
    AnalysisRequest {
        request_id: id.into(),
        search_moves: Vec::new(),
        start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".into(),
        history: vec![uci.into()],
        root_turn: "black".into(),
    }
}
pub fn events_until_current_info(s: &mut EngineSession, id: &str) -> Vec<EngineEvent> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut events = vec![];
    while Instant::now() < deadline {
        if let Some(e) = s.recv_event(Duration::from_millis(50)).unwrap() {
            let done = e.request_id == id && matches!(e.payload, EnginePayload::Info(_));
            events.push(e);
            if done {
                return events;
            }
        }
    }
    panic!("No current info received");
}
pub fn contains_old_marker(e: &EngineEvent) -> bool {
    matches!(&e.payload,EnginePayload::Info(i) if i.nodes==Some(111))
}
