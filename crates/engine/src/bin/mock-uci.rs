//! Deterministic test engine. Never distributed as a chess-playing engine.
//!
//! Mode `legal` (also as a copy named `mock-uci-legal`) analyses any
//! position: it offers `MultiPV`, and line *k*
//! is the *k*-th legal move in UCI order, scored 30 − 20 (*k* − 1)
//! centipawns, followed by the first legal reply. A `searchmoves` list limits
//! the lines to its moves. Mode `silent` offers `MultiPV` but never answers a
//! search before it is stopped.
use shakmaty::{CastlingMode, Chess, Position, fen::Fen, uci::UciMove};
use std::{
    io::{self, BufRead, Write},
    time::Duration,
};
fn emit(line: &str) {
    let mut out = io::stdout().lock();
    writeln!(out, "{line}").unwrap();
    out.flush().unwrap();
}
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    // Without arguments, a copy named `mock-uci-<mode>` runs that mode.
    let named = std::env::current_exe().ok().and_then(|p| {
        p.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("mock-uci-"))
            .map(str::to_string)
    });
    let mode = args
        .first()
        .and_then(|s| s.to_str())
        .or(named.as_deref())
        .unwrap_or("normal");
    if mode == "hold-pipes" {
        std::thread::sleep(Duration::from_secs(5));
        return;
    }
    if mode == "descendant-pipes" {
        let helper = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("hold-pipes")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap();
        std::fs::write(&args[2], helper.id().to_string()).unwrap();
        // Model a wrapper that exits before its helper closes inherited pipes.
        drop(helper);
    }
    let mut log = args.get(1).map(|path| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap()
    });
    let mut marker = 111;
    let mut searching = false;
    let mut lines = 1usize;
    let mut position = Chess::default();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if let Some(log) = &mut log {
            writeln!(log, "{line}").unwrap();
            log.flush().unwrap();
        }
        match line.as_str() {
            "uci" if mode != "handshake-timeout" => {
                emit("id name Workbench mock");
                emit("id author Workbench tests");
                emit("option name Threads type spin default 1 min 1 max 64");
                emit(
                    "option name Playing Style type combo default Normal Play var Normal Play var Very Solid",
                );
                emit("option name Book File type string default <empty>");
                if mode == "chess960" {
                    emit("option name UCI_Chess960 type check default false");
                }
                if mode == "legal" || mode == "silent" {
                    emit("option name MultiPV type spin default 1 min 1 max 500");
                }
                if mode == "syzygy" {
                    emit("option name SyzygyPath type string default <empty>");
                }
                emit("uciok");
            }
            "isready" => {
                emit("readyok");
                if mode == "blocked-input" {
                    loop {
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            }
            _ if line == "go infinite" || line.starts_with("go infinite searchmoves ") => {
                if mode == "process-death" {
                    std::process::exit(7);
                }
                searching = true;
                if mode == "silent" {
                    continue;
                }
                if mode == "stderr-saturation" {
                    io::stderr().write_all(&vec![b'x'; 200_000]).unwrap();
                    io::stderr().flush().unwrap();
                }
                if mode == "malformed" {
                    emit("info depth banana");
                }
                if mode == "overlong" {
                    emit(&"x".repeat(17_000));
                }
                if mode == "legal" {
                    let mut moves: Vec<String> = position
                        .legal_moves()
                        .iter()
                        .map(|m| UciMove::from_standard(*m).to_string())
                        .collect();
                    if let Some(only) = line.strip_prefix("go infinite searchmoves ") {
                        let only: Vec<&str> = only.split_whitespace().collect();
                        moves.retain(|m| only.contains(&m.as_str()));
                    }
                    moves.sort();
                    for (k, uci) in moves.iter().take(lines).enumerate() {
                        let mut pv = uci.clone();
                        let m = uci.parse::<UciMove>().unwrap().to_move(&position).unwrap();
                        let mut next = position.clone();
                        next.play_unchecked(m);
                        let mut replies: Vec<String> = next
                            .legal_moves()
                            .iter()
                            .map(|m| UciMove::from_standard(*m).to_string())
                            .collect();
                        replies.sort();
                        if let Some(r) = replies.first() {
                            pv = format!("{pv} {r}");
                        }
                        emit(&format!(
                            "info depth 10 multipv {} nodes {marker} score cp {} pv {pv}",
                            k + 1,
                            30 - 20 * k as i32
                        ));
                    }
                    continue;
                }
                if mode == "info-flood" {
                    for n in 0..10_000 {
                        emit(&format!(
                            "info depth 1 multipv 1 nodes {n} score cp 17 pv e7e5"
                        ));
                    }
                }
                emit(&format!(
                    "info depth 12 multipv 1 nodes {marker} score cp 17 pv e7e5"
                ));
            }
            "stop" if searching && mode != "ignored-stop" => {
                if mode == "late-info-before-bestmove" {
                    emit(&format!("info depth 13 nodes {marker} score cp 99 pv e7e5"));
                    std::thread::sleep(Duration::from_millis(20));
                }
                emit("bestmove e7e5");
                searching = false;
            }
            "quit" if mode != "ignored-quit" => return,
            _ if line.starts_with("position ") => {
                marker = if line.contains("d2d4") { 222 } else { 111 };
                if mode == "legal" {
                    position = parse_position(&line);
                }
            }
            _ if line.starts_with("setoption name MultiPV value ") => {
                lines = line[29..].trim().parse().unwrap_or(1);
            }
            _ => {}
        }
    }
    if mode == "ignored-quit" {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

/// The position of a `position startpos|fen … [moves …]` command.
fn parse_position(line: &str) -> Chess {
    let rest = &line["position ".len()..];
    let (setup, moves) = match rest.split_once(" moves ") {
        Some((s, m)) => (s, m),
        None => (rest, ""),
    };
    let mut position = match setup.strip_prefix("fen ") {
        Some(fen) => fen
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap(),
        None => Chess::default(),
    };
    for uci in moves.split_whitespace() {
        let m = uci.parse::<UciMove>().unwrap().to_move(&position).unwrap();
        position.play_unchecked(m);
    }
    position
}
