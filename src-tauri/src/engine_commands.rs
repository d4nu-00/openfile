use crate::{commands::AppError, registry::Registry};
use engine::{
    AnalysisInfo, AnalysisRequest, EngineEvent, EnginePayload, EngineSession, EngineSpec,
};
use serde::Serialize;
use shakmaty::{san::SanPlus, uci::UciMove};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<String>,
    pub multipv: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<engine::Score>,
    pub root_turn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_pv: Option<Vec<String>>,
    pub diagnostics: Vec<String>,
}
pub fn render_info(info: AnalysisInfo, fen: &str, turn: &str) -> InfoDto {
    let mut dto = InfoDto {
        depth: info.depth,
        nodes: info.nodes.map(|n| n.to_string()),
        multipv: info.multipv,
        score: info.score,
        root_turn: turn.into(),
        pv: info.pv.as_ref().map(|_| vec![]),
        raw_pv: info.pv,
        diagnostics: vec![],
    };
    let mut position = match chess_core::model::parse_position(fen) {
        Ok(p) => p,
        Err(e) => {
            dto.diagnostics.push(e.to_string());
            return dto;
        }
    };
    for uci in dto.raw_pv.iter().flatten() {
        let result = (|| -> Result<String, AppError> {
            if uci == "0000" {
                position = chess_core::model::apply_uci(position.clone(), uci)?;
                return Ok("--".into());
            }
            let mv = uci
                .parse::<UciMove>()
                .map_err(|_| AppError::new("engine", "Invalid UCI move"))?
                .to_move(&position)
                .map_err(|_| AppError::new("engine", "Illegal PV move"))?;
            let san = SanPlus::from_move(position.clone(), mv).to_string();
            position = chess_core::model::apply_uci(position.clone(), uci)?;
            Ok(san)
        })();
        match result {
            Ok(san) => dto.pv.as_mut().unwrap().push(san),
            Err(e) => {
                dto.diagnostics
                    .push(format!("Illegal PV move {uci}: {}", e.message));
                break;
            }
        }
    }
    dto
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PayloadDto {
    Info {
        #[serde(flatten)]
        info: InfoDto,
    },
    BestMove {
        best: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        ponder: Option<String>,
    },
    Error {
        message: String,
    },
    Status {
        message: String,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineEventDto {
    pub engine_id: String,
    pub request_id: String,
    pub payload: PayloadDto,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineResponse {
    pub engine_id: String,
    pub identity: BTreeMap<String, String>,
    pub options: Vec<engine::EngineOption>,
    pub status: String,
}
type Sink = Arc<dyn Fn(EngineEventDto) -> bool + Send + Sync>;
struct Context {
    fen: String,
    turn: String,
    sink: Sink,
}
struct Running {
    session: EngineSession,
    contexts: HashMap<String, Context>,
    order: VecDeque<String>,
    current_document: Option<String>,
    current_request: String,
    failed: bool,
}
struct Active {
    inner: Arc<Mutex<Running>>,
    stopped: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}
/// Kibitzers running at once.
pub const MAX_KIBITZERS: usize = 4;
#[derive(Default)]
pub struct EngineManager {
    /// Running engines (kibitzers), by engine id.
    active: Mutex<HashMap<String, Active>>,
    lifecycle: Mutex<()>,
    sequence: AtomicU64,
    closed: AtomicBool,
}
fn error(e: impl std::fmt::Display) -> AppError {
    AppError::new("engine", e.to_string())
}
fn stop_worker(active: Active) -> Result<(), AppError> {
    active.stopped.store(true, Ordering::Release);
    let result = active
        .inner
        .lock()
        .map_err(error)
        .and_then(|mut r| r.session.shutdown().map_err(error));
    let _ = active.worker.join();
    result
}
fn deliver(r: &Running, id: &str, event: EngineEvent) -> bool {
    let Some(context) = r.contexts.get(&event.request_id) else {
        return true;
    };
    let payload = match event.payload {
        EnginePayload::Info(info) => PayloadDto::Info {
            info: render_info(info, &context.fen, &context.turn),
        },
        EnginePayload::BestMove { best, ponder } => PayloadDto::BestMove { best, ponder },
        EnginePayload::Error { message } => PayloadDto::Error { message },
        EnginePayload::Status { message } => PayloadDto::Status { message },
    };
    (context.sink)(EngineEventDto {
        engine_id: id.into(),
        request_id: event.request_id,
        payload,
    })
}
impl EngineManager {
    pub fn begin_choice(&self) -> Result<u64, AppError> {
        self.sequence
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map(|n| n + 1)
            .map_err(|_| AppError::new("limitExceeded", "Engine handle space exhausted"))
    }
    /// Starts the engine at `path` as one more kibitzer.
    pub fn choose(&self, path: PathBuf, ticket: u64) -> Result<EngineResponse, AppError> {
        let _lifecycle = self.lifecycle.lock().map_err(error)?;
        if self.closed.load(Ordering::Acquire) {
            return Err(AppError::new("staleSelection", "The app is closing"));
        }
        if self.active.lock().map_err(error)?.len() >= MAX_KIBITZERS {
            return Err(AppError::new(
                "limitExceeded",
                "Remove a kibitzer before adding another",
            ));
        }
        let mut session = EngineSession::spawn(
            EngineSpec {
                executable: path,
                arguments: vec![],
            },
            Duration::from_secs(5),
        )
        .map_err(error)?;
        if self.closed.load(Ordering::Acquire) {
            let _ = session.shutdown();
            return Err(AppError::new("staleSelection", "The app is closing"));
        }
        for option in session.options() {
            if let Some(value) = &option.default {
                standard_option(&option.name, value)?;
            }
        }
        crate::tablebases::configure_engine(&mut session);
        let id = format!("engine-{ticket}");
        let response = EngineResponse {
            engine_id: id.clone(),
            identity: session.identity().clone(),
            options: session.options().to_vec(),
            status: "ready".into(),
        };
        let inner = Arc::new(Mutex::new(Running {
            session,
            contexts: HashMap::new(),
            order: VecDeque::new(),
            current_document: None,
            current_request: String::new(),
            failed: false,
        }));
        let stopped = Arc::new(AtomicBool::new(false));
        let flag = stopped.clone();
        let running = inner.clone();
        let engine_id = id.clone();
        let worker = thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                let Ok(mut r) = running.lock() else {
                    break;
                };
                if r.failed {
                    drop(r);
                    thread::sleep(Duration::from_millis(25));
                    continue;
                }
                match r.session.recv_event(Duration::from_millis(25)) {
                    Ok(Some(event)) => {
                        let current = event.request_id == r.current_request;
                        if !deliver(&r, &engine_id, event) && current {
                            let _ = r.session.shutdown();
                            r.failed = true;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let request_id = r.current_request.clone();
                        let _ = deliver(
                            &r,
                            &engine_id,
                            EngineEvent {
                                request_id,
                                payload: EnginePayload::Error {
                                    message: e.to_string(),
                                },
                            },
                        );
                        let _ = r.session.shutdown();
                        r.failed = true;
                    }
                }
                drop(r);
                thread::sleep(Duration::from_millis(1));
            }
        });
        let next = Active {
            inner,
            stopped,
            worker,
        };
        self.active.lock().map_err(error)?.insert(id, next);
        Ok(response)
    }
    fn running(&self, id: &str) -> Result<Arc<Mutex<Running>>, AppError> {
        self.active
            .lock()
            .map_err(error)?
            .get(id)
            .map(|a| a.inner.clone())
            .ok_or_else(|| AppError::new("invalidHandle", "Engine is closed or unknown"))
    }
    pub fn start(
        &self,
        id: &str,
        document_id: String,
        node: u32,
        request_id: String,
        registry: &Registry,
        sink: Sink,
    ) -> Result<(), AppError> {
        let (start_fen, history, fen, root_turn) =
            registry.analysis_position(&document_id, node)?;
        let inner = self.running(id)?;
        let mut r = inner.lock().map_err(error)?;
        if r.failed {
            return Err(error("Engine unavailable; choose an engine again"));
        }
        r.contexts.insert(
            request_id.clone(),
            Context {
                fen,
                turn: root_turn.clone(),
                sink,
            },
        );
        r.order.push_back(request_id.clone());
        while r.order.len() > 128 {
            if let Some(old) = r.order.pop_front() {
                r.contexts.remove(&old);
            }
        }
        r.session
            .analyse(AnalysisRequest {
                request_id: request_id.clone(),
                start_fen,
                history,
                root_turn,
            })
            .map_err(error)?;
        r.current_document = Some(document_id);
        r.current_request = request_id;
        Ok(())
    }
    pub fn stop(&self, id: &str) -> Result<(), AppError> {
        let inner = self.running(id)?;
        let mut r = inner.lock().map_err(error)?;
        r.current_document = None;
        if r.failed {
            return Ok(());
        }
        r.session.stop().map_err(error)
    }
    pub fn set_option(&self, id: &str, name: &str, value: &str) -> Result<(), AppError> {
        standard_option(name, value)?;
        let inner = self.running(id)?;
        let mut r = inner.lock().map_err(error)?;
        if r.failed {
            return Err(error("Engine unavailable"));
        }
        r.session.set_option(name, value).map_err(error)
    }
    /// Stops every kibitzer analysing document `id`.
    pub fn stop_document(&self, id: &str) -> Result<(), AppError> {
        let running: Vec<_> = self
            .active
            .lock()
            .map_err(error)?
            .values()
            .map(|a| a.inner.clone())
            .collect();
        for inner in running {
            let mut r = inner.lock().map_err(error)?;
            if r.current_document.as_deref() == Some(id) {
                r.current_document = None;
                if !r.failed {
                    r.session.stop().map_err(error)?;
                }
            }
        }
        Ok(())
    }
    pub fn close(&self, id: &str) -> Result<(), AppError> {
        let _lifecycle = self.lifecycle.lock().map_err(error)?;
        let active = self
            .active
            .lock()
            .map_err(error)?
            .remove(id)
            .ok_or_else(|| AppError::new("invalidHandle", "Engine is closed or unknown"))?;
        stop_worker(active)
    }
    pub fn shutdown(&self) -> Result<(), AppError> {
        self.closed.store(true, Ordering::Release);
        let _lifecycle = self.lifecycle.lock().map_err(error)?;
        let all: Vec<_> = self
            .active
            .lock()
            .map_err(error)?
            .drain()
            .map(|(_, a)| a)
            .collect();
        let mut result = Ok(());
        for active in all {
            if let Err(e) = stop_worker(active) {
                result = Err(e);
            }
        }
        result
    }
}
impl Drop for EngineManager {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(error)?
}
#[tauri::command]
pub async fn choose_engine(
    state: tauri::State<'_, Arc<EngineManager>>,
    path: PathBuf,
) -> Result<EngineResponse, AppError> {
    let ticket = state.begin_choice()?;
    let s = state.inner().clone();
    blocking(move || s.choose(path, ticket)).await
}
/// An engine's name and author, from the UCI handshake, for Engine
/// Management. The engine is stopped again at once.
pub fn probe(path: PathBuf) -> Result<BTreeMap<String, String>, AppError> {
    let mut session = EngineSession::spawn(
        EngineSpec {
            executable: path,
            arguments: vec![],
        },
        Duration::from_secs(5),
    )
    .map_err(error)?;
    let identity = session.identity().clone();
    let _ = session.shutdown();
    Ok(identity)
}
#[tauri::command]
pub async fn probe_engine(path: PathBuf) -> Result<BTreeMap<String, String>, AppError> {
    blocking(move || probe(path)).await
}
#[tauri::command]
pub async fn set_engine_option(
    state: tauri::State<'_, Arc<EngineManager>>,
    engine_id: String,
    name: String,
    value: String,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    blocking(move || s.set_option(&engine_id, &name, &value)).await
}
#[tauri::command]
pub async fn start_analysis(
    state: tauri::State<'_, Arc<EngineManager>>,
    registry: tauri::State<'_, Arc<Registry>>,
    engine_id: String,
    document_id: String,
    node_id: u32,
    request_id: String,
    events: tauri::ipc::Channel<EngineEventDto>,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    let registry = registry.inner().clone();
    let sink = Arc::new(move |e| events.send(e).is_ok());
    blocking(move || {
        s.start(
            &engine_id,
            document_id,
            node_id,
            request_id,
            &registry,
            sink,
        )
    })
    .await
}
#[tauri::command]
pub async fn stop_analysis(
    state: tauri::State<'_, Arc<EngineManager>>,
    engine_id: String,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    blocking(move || s.stop(&engine_id)).await
}
#[tauri::command]
pub async fn close_engine(
    state: tauri::State<'_, Arc<EngineManager>>,
    engine_id: String,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    blocking(move || s.close(&engine_id)).await
}
pub fn standard_option(name: &str, value: &str) -> Result<(), AppError> {
    if (name.eq_ignore_ascii_case("UCI_Chess960") && value != "false")
        || (name.eq_ignore_ascii_case("UCI_Variant") && value != "chess")
    {
        return Err(AppError::new(
            "unsupported",
            "This workbench analyses standard chess only",
        ));
    }
    Ok(())
}
