//! Deterministic test engine. Never distributed as a chess-playing engine.
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
    let mode = args.first().and_then(|s| s.to_str()).unwrap_or("normal");
    let mut log = args.get(1).map(|path| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap()
    });
    let mut marker = 111;
    let mut searching = false;
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
            "go infinite" => {
                if mode == "process-death" {
                    std::process::exit(7);
                }
                searching = true;
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
