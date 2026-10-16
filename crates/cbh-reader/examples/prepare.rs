//! Times a preparation against a player: the scan for the player's games,
//! the reading of their main lines with one colour, and the repertoire.
//!
//! Usage: prepare --path <db> --dir <folder> --player "Last, First" [--black]
//! The player list is built in `--dir` when it is not there. Only counts and
//! timings are printed.
use cbh_reader::{
    Database, ReadLimits,
    name_list::{NameKind, NameList},
    prepare,
};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let path = PathBuf::from(value("--path").ok_or("--path is required")?);
    let dir = PathBuf::from(value("--dir").ok_or("--dir is required")?);
    let player = value("--player").ok_or("--player is required")?;
    let white = !args.iter().any(|a| a == "--black");
    let mut db = Database::open(&path, ReadLimits::default())?;
    let cancel = AtomicBool::new(false);
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));

    let names = dir.join("players.cwnames");
    if NameList::open(&names, &db, NameKind::Players)?.is_none() {
        NameList::build(&db, NameKind::Players, &names, workers, &cancel, |_, _| {})?;
    }
    let mut list = NameList::open(&names, &db, NameKind::Players)?.ok_or("no player list")?;
    let row = list.find(&mut db, NameKind::Players, player)?;
    let found = list.read(row, 1)?.pop().ok_or("player not found")?;

    let t = Instant::now();
    let games = db
        .player_games(found.id, workers, &cancel, |_, _| {})?
        .ok_or("cancelled")?;
    let scan = t.elapsed();
    let ordinals: Vec<u64> = games
        .games
        .iter()
        .filter(|g| g.white == white)
        .map(|g| g.ordinal)
        .collect();
    let t = Instant::now();
    let tree = db
        .player_tree(&ordinals, prepare::PLIES, workers, &cancel, |_, _| {})?
        .ok_or("cancelled")?;
    let read = t.elapsed();
    let t = Instant::now();
    let lines = tree.repertoire(white);
    let repertoire = t.elapsed();
    println!(
        "{} games with {}; {} read, {} unreadable",
        ordinals.len(),
        if white { "White" } else { "Black" },
        tree.games(),
        tree.unreadable
    );
    println!(
        "scan ({workers} threads): {:.3} s, main lines: {:.3} s, repertoire ({} lines): {:.4} s",
        scan.as_secs_f64(),
        read.as_secs_f64(),
        lines.len(),
        repertoire.as_secs_f64()
    );
    Ok(())
}
