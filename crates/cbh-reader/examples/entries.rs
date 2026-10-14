//! Loads a list of entries and counts how they decode.
//!
//! Usage: entries --path <db> --list <file with one entry number per line>
//!        [--pgn <out.pgn>]
use cbh_reader::{Database, ReadLimits};
use std::{collections::BTreeMap, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let path = PathBuf::from(value("--path").ok_or("--path is required")?);
    let list = std::fs::read_to_string(value("--list").ok_or("--list is required")?)?;
    let mut db = Database::open(&path, ReadLimits::default())?;
    let mut statuses: BTreeMap<String, u32> = BTreeMap::new();
    let mut pgn = String::new();
    let mut nodes = 0usize;
    for line in list.lines().filter(|l| !l.trim().is_empty()) {
        let ordinal: u64 = line.trim().parse()?;
        let loaded = db.load(ordinal)?;
        let status = format!("{:?}", loaded.moves_status);
        let message = loaded
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join("; ");
        *statuses
            .entry(if message.is_empty() {
                status
            } else {
                format!("{status}: {message}")
            })
            .or_default() += 1;
        if let Some(game) = &loaded.game {
            nodes += game.nodes.len();
            if value("--pgn").is_some() {
                pgn += &chess_core::write_pgn(game)?;
                pgn.push('\n');
            }
        }
    }
    for (status, n) in &statuses {
        println!("{n:>6}  {status}");
    }
    println!("{nodes} nodes");
    if let Some(out) = value("--pgn") {
        std::fs::write(out, pgn)?;
    }
    Ok(())
}
