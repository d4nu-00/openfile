use crate::{
    AnalysisInfo, EngineError, EngineOption, UciResponse, parse_line, position_command,
    validate_option,
};
use std::{
    collections::{BTreeMap, VecDeque},
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Condvar, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const QUEUE_LIMIT: usize = 256;
const EVENT_LIMIT: usize = 128;
const STOP_TIMEOUT: Duration = Duration::from_secs(2);
const QUIT_TIMEOUT: Duration = Duration::from_secs(1);
#[derive(Clone, Debug)]
pub struct EngineSpec {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
}
#[derive(Clone, Debug)]
pub struct AnalysisRequest {
    pub request_id: String,
    pub start_fen: String,
    pub history: Vec<String>,
    pub root_turn: String,
}
#[derive(Debug)]
pub struct EngineEvent {
    pub request_id: String,
    pub payload: EnginePayload,
}
#[derive(Debug)]
pub enum EnginePayload {
    Info(AnalysisInfo),
    BestMove {
        best: String,
        ponder: Option<String>,
    },
    Error {
        message: String,
    },
    Status {
        message: String,
    },
}
enum Raw {
    Response(UciResponse),
    Diagnostic(String),
    Eof,
}
impl Raw {
    fn is_info(&self) -> bool {
        matches!(self, Self::Response(UciResponse::Info(_)))
    }
}
#[derive(Default)]
struct QueueState {
    items: VecDeque<Raw>,
    fatal: Option<String>,
    closed: bool,
}
#[derive(Default)]
struct OutputQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
}
impl OutputQueue {
    fn push(&self, mut item: Raw) -> bool {
        let Ok(mut q) = self.state.lock() else {
            return false;
        };
        if q.closed {
            return false;
        }
        if let Raw::Response(UciResponse::Info(info)) = &mut item {
            // A control response can separate positions before attribution.
            let index = q
                .items
                .iter()
                .enumerate()
                .rev()
                .take_while(|(_, raw)| {
                    matches!(
                        raw,
                        Raw::Response(UciResponse::Info(_)) | Raw::Diagnostic(_)
                    )
                })
                .find_map(|(index, raw)| match raw {
                    Raw::Response(UciResponse::Info(old)) if old.multipv == info.multipv => {
                        Some(index)
                    }
                    _ => None,
                });
            if let Some(index) = index
                && let Some(Raw::Response(UciResponse::Info(previous))) = q.items.remove(index)
            {
                info.inherit_missing(&previous);
            }
        }
        if q.items.len() >= QUEUE_LIMIT {
            if let Some(index) = q.items.iter().position(Raw::is_info) {
                q.items.remove(index);
            } else if item.is_info() {
                return true;
            } else {
                q.fatal = Some("Engine control output exceeded the queue budget".into());
                q.closed = true;
                self.ready.notify_all();
                return false;
            }
        }
        q.items.push_back(item);
        self.ready.notify_one();
        true
    }
    fn pop(&self, timeout: Duration) -> Result<Option<Raw>, EngineError> {
        let mut q = self
            .state
            .lock()
            .map_err(|_| EngineError::Protocol("Output queue unavailable".into()))?;
        if q.items.is_empty() && q.fatal.is_none() && !q.closed {
            (q, _) = self
                .ready
                .wait_timeout_while(q, timeout, |q| {
                    q.items.is_empty() && q.fatal.is_none() && !q.closed
                })
                .map_err(|_| EngineError::Protocol("Output queue unavailable".into()))?;
        }
        if let Some(item) = q.items.pop_front() {
            return Ok(Some(item));
        }
        if let Some(message) = q.fatal.take() {
            return Err(EngineError::Protocol(message));
        }
        Ok(None)
    }
    fn close(&self) {
        if let Ok(mut q) = self.state.lock() {
            q.closed = true;
            self.ready.notify_all();
        }
    }
}
fn stdout_reader(stdout: impl Read, queue: Arc<OutputQueue>, cancel: Arc<AtomicBool>) {
    let mut input = BufReader::new(stdout);
    let mut line = Vec::new();
    let mut overlong = false;
    loop {
        if cancel.load(Ordering::Acquire) {
            return;
        }
        let chunk = match input.fill_buf() {
            Ok(chunk) if !chunk.is_empty() => chunk,
            Ok(_) => {
                if !line.is_empty() && !overlong {
                    emit_line(&queue, &line);
                }
                queue.push(Raw::Eof);
                return;
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(e) => {
                queue.push(Raw::Diagnostic(format!("stdout: {e}")));
                queue.push(Raw::Eof);
                return;
            }
        };
        let consumed = chunk
            .iter()
            .position(|&b| b == b'\n')
            .map_or(chunk.len(), |n| n + 1);
        let complete = chunk[consumed - 1] == b'\n';
        let content = &chunk[..consumed - usize::from(complete)];
        if !overlong {
            if line.len() + content.len() > crate::protocol::MAX_LINE + 1 {
                overlong = true;
                line.clear();
                if !queue.push(Raw::Diagnostic(EngineError::LineLimit.to_string())) {
                    return;
                }
            } else {
                line.extend_from_slice(content);
            }
        }
        input.consume(consumed);
        if complete {
            if !overlong && !emit_line(&queue, &line) {
                return;
            }
            line.clear();
            overlong = false;
        }
    }
}
fn emit_line(queue: &OutputQueue, bytes: &[u8]) -> bool {
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    let response = match std::str::from_utf8(bytes) {
        Ok(line) => match parse_line(line) {
            Ok(Some(response)) => Raw::Response(response),
            Ok(None) => return true,
            Err(error) => Raw::Diagnostic(format!(
                "{error}: {}",
                line.chars().take(512).collect::<String>()
            )),
        },
        Err(_) => Raw::Diagnostic("Engine output is not UTF-8".into()),
    };
    queue.push(response)
}
fn stderr_reader(mut stderr: impl Read, tail: Arc<Mutex<VecDeque<u8>>>, cancel: Arc<AtomicBool>) {
    let mut bytes = [0u8; 4096];
    while !cancel.load(Ordering::Acquire) {
        let n = match stderr.read(&mut bytes) {
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(_) => return,
        };
        if n == 0 {
            break;
        }
        if let Ok(mut tail) = tail.lock() {
            tail.extend(&bytes[..n]);
            while tail.len() > 8192 {
                tail.pop_front();
            }
        }
    }
}
fn write_command(
    stdin: &mut impl Write,
    command: &str,
    cancel: &AtomicBool,
) -> std::io::Result<()> {
    let bytes = format!("{command}\n").into_bytes();
    let mut offset = 0;
    while offset < bytes.len() {
        if cancel.load(Ordering::Acquire) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Engine input cancelled",
            ));
        }
        match stdin.write(&bytes[offset..]) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(n) => offset += n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(Duration::from_millis(5))
            }
            Err(e) => return Err(e),
        }
    }
    stdin.flush()
}
struct PendingWrite {
    command: String,
    reply: mpsc::Sender<std::io::Result<()>>,
}
struct InputPipe {
    sender: mpsc::SyncSender<PendingWrite>,
    worker: JoinHandle<()>,
}
pub struct EngineSession {
    spec: EngineSpec,
    handshake_timeout: Duration,
    child: Option<Child>,
    tree: Option<crate::process::ProcessTree>,
    cancel: Arc<AtomicBool>,
    stdin: Option<InputPipe>,
    readers: Vec<JoinHandle<()>>,
    queue: Arc<OutputQueue>,
    stderr: Arc<Mutex<VecDeque<u8>>>,
    identity: BTreeMap<String, String>,
    options: Vec<EngineOption>,
    cached: BTreeMap<String, String>,
    active: Option<AnalysisRequest>,
    events: VecDeque<EngineEvent>,
    closed: bool,
}
impl EngineSession {
    pub fn spawn(spec: EngineSpec, timeout: Duration) -> Result<Self, EngineError> {
        crate::launch::check_program(&spec.executable)?;
        let mut s = Self {
            spec,
            handshake_timeout: timeout,
            child: None,
            tree: None,
            cancel: Arc::new(AtomicBool::new(false)),
            stdin: None,
            readers: vec![],
            queue: Arc::new(OutputQueue::default()),
            stderr: Arc::new(Mutex::new(VecDeque::new())),
            identity: BTreeMap::new(),
            options: vec![],
            cached: BTreeMap::new(),
            active: None,
            events: VecDeque::new(),
            closed: false,
        };
        if let Err(e) = s.launch() {
            return Err(crate::launch::explain_failure(&s.spec.executable, e));
        }
        Ok(s)
    }
    fn launch(&mut self) -> Result<(), EngineError> {
        self.cancel = Arc::new(AtomicBool::new(false));
        self.queue = Arc::new(OutputQueue::default());
        self.stderr = Arc::new(Mutex::new(VecDeque::new()));
        self.identity.clear();
        self.options.clear();
        let mut command = Command::new(&self.spec.executable);
        command
            .args(&self.spec.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (child, tree) = crate::process::ProcessTree::spawn(&mut command)?;
        self.child = Some(child);
        self.tree = Some(tree);
        let result = (|| {
            let child = self.child.as_mut().ok_or(EngineError::Exited)?;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| EngineError::Protocol("Missing stdin pipe".into()))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| EngineError::Protocol("Missing stdout pipe".into()))?;
            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| EngineError::Protocol("Missing stderr pipe".into()))?;
            crate::process::nonblocking(&stdin)?;
            crate::process::nonblocking(&stdout)?;
            crate::process::nonblocking(&stderr)?;
            let (sender, receiver) = mpsc::sync_channel::<PendingWrite>(1);
            let cancel = self.cancel.clone();
            let worker = thread::spawn(move || {
                while let Ok(write) = receiver.recv() {
                    let result = write_command(&mut stdin, &write.command, &cancel);
                    let failed = result.is_err();
                    let _ = write.reply.send(result);
                    if failed {
                        break;
                    }
                }
            });
            self.stdin = Some(InputPipe { sender, worker });
            let queue = self.queue.clone();
            let cancel = self.cancel.clone();
            self.readers
                .push(thread::spawn(move || stdout_reader(stdout, queue, cancel)));
            let tail = self.stderr.clone();
            let cancel = self.cancel.clone();
            self.readers
                .push(thread::spawn(move || stderr_reader(stderr, tail, cancel)));
            self.send("uci")?;
            self.wait_for(
                self.handshake_timeout,
                |r| matches!(r, UciResponse::UciOk),
                "uciok",
            )?;
            self.ready(self.handshake_timeout)
        })();
        if result.is_err() {
            let _ = self.terminate();
        }
        result
    }
    fn send(&mut self, command: &str) -> Result<(), EngineError> {
        self.send_until(command, self.handshake_timeout)
    }
    fn send_until(&mut self, command: &str, timeout: Duration) -> Result<(), EngineError> {
        if command.contains(['\r', '\n']) || command.len() > crate::protocol::MAX_LINE {
            return Err(EngineError::Protocol("Invalid command boundary".into()));
        }
        let (reply, result) = mpsc::channel();
        self.stdin
            .as_ref()
            .ok_or(EngineError::Exited)?
            .sender
            .try_send(PendingWrite {
                command: command.into(),
                reply,
            })
            .map_err(|_| EngineError::Protocol("Engine input queue unavailable".into()))?;
        let status = match result.recv_timeout(timeout) {
            Ok(result) => result.map_err(EngineError::Io),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(EngineError::Timeout("stdin write".into())),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(EngineError::Exited),
        };
        if status.is_err() {
            let _ = self.terminate();
        }
        status
    }
    fn enqueue(
        &mut self,
        mut payload: EnginePayload,
        request_id: String,
    ) -> Result<(), EngineError> {
        if let EnginePayload::Info(info) = &mut payload
            && let Some(index) = self.events.iter().position(|e| {
                e.request_id == request_id
                    && matches!(&e.payload,EnginePayload::Info(old) if old.multipv==info.multipv)
            })
            && let Some(EngineEvent {
                payload: EnginePayload::Info(previous),
                ..
            }) = self.events.remove(index)
        {
            info.inherit_missing(&previous);
        }
        if self.events.len() >= EVENT_LIMIT {
            if let Some(index) = self
                .events
                .iter()
                .position(|e| matches!(e.payload, EnginePayload::Info(_)))
            {
                self.events.remove(index);
            } else {
                return Err(EngineError::Protocol(
                    "Drain engine events before continuing".into(),
                ));
            }
        }
        self.events.push_back(EngineEvent {
            request_id,
            payload,
        });
        Ok(())
    }
    fn consume(&mut self, raw: Raw) -> Result<(), EngineError> {
        let request_id = self
            .active
            .as_ref()
            .map(|r| r.request_id.clone())
            .unwrap_or_default();
        match raw {
            Raw::Response(UciResponse::Id { field, value }) => {
                if self.identity.len() >= 32 && !self.identity.contains_key(&field) {
                    return Err(EngineError::Protocol("Too many identity fields".into()));
                }
                self.identity.insert(field, value);
            }
            Raw::Response(UciResponse::Option(option)) => {
                if self.options.len() >= 256 {
                    return Err(EngineError::Protocol("Too many engine options".into()));
                }
                if let Some(old) = self.options.iter_mut().find(|o| o.name == option.name) {
                    *old = option;
                } else {
                    self.options.push(option);
                }
            }
            Raw::Response(UciResponse::Info(info)) => {
                if self.active.is_some() {
                    self.enqueue(EnginePayload::Info(info), request_id)?;
                }
            }
            Raw::Response(UciResponse::BestMove { best, ponder }) => {
                if self.active.take().is_some() {
                    self.enqueue(EnginePayload::BestMove { best, ponder }, request_id)?;
                }
            }
            Raw::Diagnostic(message) => {
                self.enqueue(EnginePayload::Error { message }, request_id)?
            }
            Raw::Eof => return Err(EngineError::Exited),
            _ => {}
        }
        Ok(())
    }
    fn wait_for(
        &mut self,
        timeout: Duration,
        done: impl Fn(&UciResponse) -> bool,
        label: &str,
    ) -> Result<(), EngineError> {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(EngineError::Timeout(label.into()));
            }
            if let Some(raw) = self.queue.pop(deadline - now)? {
                let complete = matches!(&raw,Raw::Response(r) if done(r));
                self.consume(raw)?;
                if complete {
                    return Ok(());
                }
            }
        }
    }
    fn ready(&mut self, timeout: Duration) -> Result<(), EngineError> {
        self.send_until("isready", timeout)?;
        let result = self.wait_for(timeout, |r| matches!(r, UciResponse::ReadyOk), "readyok");
        if result.is_err() {
            let _ = self.terminate();
        }
        result
    }
    pub fn set_option(&mut self, name: &str, value: &str) -> Result<(), EngineError> {
        let option = self
            .options
            .iter()
            .find(|o| o.name == name)
            .ok_or_else(|| EngineError::Option("Unknown option".into()))?;
        let command = validate_option(option, value)?;
        if self.active.is_some() {
            return Err(EngineError::Option(
                "Stop analysis before changing options".into(),
            ));
        }
        if self.closed {
            return Err(EngineError::Exited);
        }
        self.send(&command)?;
        self.ready(self.handshake_timeout)?;
        self.cached.insert(name.into(), value.into());
        Ok(())
    }
    fn validate_request(request: &AnalysisRequest) -> Result<String, EngineError> {
        if request.request_id.is_empty()
            || request.request_id.len() > 128
            || request.request_id.contains(['\r', '\n'])
        {
            return Err(EngineError::Position("Invalid request ID".into()));
        }
        let command = position_command(&request.start_fen, &request.history)?;
        let mut pos = chess_core::model::parse_position(&request.start_fen)
            .map_err(|e| EngineError::Position(e.to_string()))?;
        for uci in &request.history {
            pos = chess_core::model::apply_uci(pos, uci)
                .map_err(|e| EngineError::Position(e.to_string()))?;
        }
        // Use the same trusted position projection as the document service.
        use shakmaty::Position;
        let turn = if pos.turn().is_white() {
            "white"
        } else {
            "black"
        };
        if request.root_turn != turn {
            return Err(EngineError::Position(
                "Root turn disagrees with validated position".into(),
            ));
        }
        Ok(command)
    }
    pub fn analyse(&mut self, request: AnalysisRequest) -> Result<(), EngineError> {
        let position = Self::validate_request(&request)?;
        if self.closed {
            return Err(EngineError::Exited);
        }
        // Chess960 positions (castling rooks off their standard squares) need
        // the engine's UCI_Chess960; other positions are sent as standard
        // chess, which plays the same.
        use shakmaty::Position;
        let chess960 = chess_core::model::parse_position(&request.start_fen)
            .is_ok_and(|p| p.castles().mode() == shakmaty::CastlingMode::Chess960);
        let option = self
            .options
            .iter()
            .find(|o| o.name.eq_ignore_ascii_case("UCI_Chess960"))
            .map(|o| o.name.clone());
        if chess960 && option.is_none() {
            return Err(EngineError::Option(
                "This engine does not play Chess960 (it has no UCI_Chess960 option)".into(),
            ));
        }
        self.stop()?;
        // Drain unsolicited idle output before assigning a new request ID.
        while let Some(raw) = self.queue.pop(Duration::ZERO)? {
            self.consume(raw)?;
        }
        if let Some(name) = option {
            let wanted = if chess960 { "true" } else { "false" };
            let current = self.cached.get(&name).map_or("false", String::as_str);
            if current != wanted {
                self.set_option(&name, wanted)?;
            }
        }
        self.ready(self.handshake_timeout)?;
        self.send(&position)?;
        self.active = Some(request);
        self.send("go infinite")?;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<(), EngineError> {
        if self.active.is_none() {
            return Ok(());
        }
        let old = self
            .active
            .as_ref()
            .map(|r| r.request_id.clone())
            .unwrap_or_default();
        let result = self.send("stop").and_then(|_| {
            self.wait_for(
                STOP_TIMEOUT,
                |r| matches!(r, UciResponse::BestMove { .. }),
                "bestmove after stop",
            )
        });
        if let Err(error) = result {
            self.active = None;
            self.terminate()?;
            self.enqueue(
                EnginePayload::Error {
                    message: format!("{error}; restarting engine"),
                },
                old,
            )?;
            self.launch()?;
            for (name, value) in self.cached.clone() {
                self.set_option(&name, &value)?;
            }
        }
        Ok(())
    }
    pub fn recv_event(&mut self, timeout: Duration) -> Result<Option<EngineEvent>, EngineError> {
        if let Some(e) = self.events.pop_front() {
            return Ok(Some(e));
        }
        let first = self.queue.pop(timeout)?;
        if let Some(raw) = first {
            self.consume(raw)?;
            // Attribution remains serial; bestmove clears active before later lines.
            for _ in 0..QUEUE_LIMIT {
                match self.queue.pop(Duration::ZERO)? {
                    Some(raw) => self.consume(raw)?,
                    None => break,
                }
            }
        }
        Ok(self.events.pop_front())
    }
    fn terminate(&mut self) -> Result<(), EngineError> {
        self.cancel.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_millis(250);
        let worker = self.stdin.take().map(|pipe| {
            drop(pipe.sender);
            pipe.worker
        });
        self.active = None;
        let mut error = None;
        if let Some(tree) = self.tree.take()
            && let Err(e) = tree.terminate()
        {
            error = Some(EngineError::Io(e));
        }
        if let Some(worker) = &worker {
            crate::process::cancel_blocked(worker);
        }
        for reader in &self.readers {
            crate::process::cancel_blocked(reader);
        }
        if let Some(mut child) = self.child.take() {
            let running = match child.try_wait() {
                Ok(status) => status.is_none(),
                Err(e) => {
                    error = Some(EngineError::Io(e));
                    true
                }
            };
            if running && let Err(e) = child.kill() {
                error = Some(EngineError::Io(e));
            }
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Ok(None) => {
                        error = Some(EngineError::Timeout("process reap".into()));
                        break;
                    }
                    Err(e) => {
                        error = Some(EngineError::Io(e));
                        break;
                    }
                }
            }
        }
        self.queue.close();
        for worker in worker.into_iter().chain(self.readers.drain(..)) {
            while !worker.is_finished() && Instant::now() < deadline {
                crate::process::cancel_blocked(&worker);
                thread::sleep(Duration::from_millis(5));
            }
            if worker.is_finished() {
                if worker.join().is_err() {
                    error = Some(EngineError::Protocol("Pipe worker failed".into()));
                }
            } else {
                error = Some(EngineError::Timeout("pipe worker cleanup".into()));
            }
        }
        if let Some(error) = error {
            return Err(error);
        }
        Ok(())
    }
    pub fn shutdown(&mut self) -> Result<(), EngineError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let deadline = Instant::now() + QUIT_TIMEOUT;
        if self.active.is_some() {
            let _ = self.send_until("stop", deadline.saturating_duration_since(Instant::now()));
        }
        let _ = self.send_until("quit", deadline.saturating_duration_since(Instant::now()));
        if let Some(child) = self.child.as_mut() {
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {}
                    Err(_) => break,
                }
                if Instant::now() >= deadline {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
        self.terminate()
    }
    pub fn process_id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }
    pub fn options(&self) -> &[EngineOption] {
        &self.options
    }
    pub fn identity(&self) -> &BTreeMap<String, String> {
        &self.identity
    }
    pub fn stderr_tail(&self) -> String {
        self.stderr
            .lock()
            .map(|s| String::from_utf8_lossy(&s.iter().copied().collect::<Vec<_>>()).into_owned())
            .unwrap_or_default()
    }
}
impl Drop for EngineSession {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
#[cfg(test)]
mod coalescing_tests {
    use super::*;
    fn info(line: &str) -> AnalysisInfo {
        let Some(UciResponse::Info(info)) = parse_line(line).unwrap() else {
            panic!()
        };
        info
    }
    #[test]
    fn raw_queue_retains_fields_across_partial_updates_and_respects_completion() {
        let full = info("info depth 12 nodes 111 score cp 25 pv e2e4");
        let queue = OutputQueue::default();
        queue.push(Raw::Response(UciResponse::Info(full.clone())));
        queue.push(Raw::Response(UciResponse::Info(info("info nodes 222"))));
        let mut last = None;
        while let Some(raw) = queue.pop(Duration::ZERO).unwrap() {
            if let Raw::Response(UciResponse::Info(info)) = raw {
                last = Some(info);
            }
        }
        let merged = last.as_ref().unwrap();
        assert_eq!(merged.nodes, Some(222));
        assert_eq!(merged.depth, full.depth);
        assert_eq!(merged.score, full.score);
        assert_eq!(merged.pv, full.pv);
        queue.push(Raw::Response(UciResponse::Info(full)));
        queue.push(Raw::Response(UciResponse::BestMove {
            best: "e2e4".into(),
            ponder: None,
        }));
        queue.push(Raw::Response(UciResponse::Info(info("info nodes 333"))));
        while let Some(raw) = queue.pop(Duration::ZERO).unwrap() {
            if let Raw::Response(UciResponse::Info(info)) = raw {
                last = Some(info);
            }
        }
        assert!(last.unwrap().score.is_none());
    }
    #[test]
    fn attributed_event_coalescing_keeps_partial_fields_within_the_same_request() {
        let mut session = EngineSession {
            spec: EngineSpec {
                executable: PathBuf::new(),
                arguments: vec![],
            },
            handshake_timeout: Duration::from_secs(1),
            child: None,
            tree: None,
            cancel: Arc::new(AtomicBool::new(false)),
            stdin: None,
            readers: vec![],
            queue: Arc::new(OutputQueue::default()),
            stderr: Arc::new(Mutex::new(VecDeque::new())),
            identity: BTreeMap::new(),
            options: vec![],
            cached: BTreeMap::new(),
            active: None,
            events: VecDeque::new(),
            closed: true,
        };
        let full = info("info depth 12 nodes 111 score cp 25 pv e2e4");
        session
            .enqueue(EnginePayload::Info(full.clone()), "old".into())
            .unwrap();
        session
            .enqueue(EnginePayload::Info(info("info nodes 222")), "old".into())
            .unwrap();
        session
            .enqueue(EnginePayload::Info(info("info nodes 333")), "new".into())
            .unwrap();
        let EnginePayload::Info(merged) =
            session.recv_event(Duration::ZERO).unwrap().unwrap().payload
        else {
            panic!()
        };
        assert_eq!(merged.nodes, Some(222));
        assert_eq!(merged.score, full.score);
        assert_eq!(merged.pv, full.pv);
        let EnginePayload::Info(new) = session.recv_event(Duration::ZERO).unwrap().unwrap().payload
        else {
            panic!()
        };
        assert!(new.score.is_none());
    }
}
