//! Times a player's statistics: the scan for the player's games, then the
//! scoring.
//!
//! Usage: player_stats --path <db> --dir <folder> --player "Last, First"
//! The player list is built in `--dir` when it is not there.
use cbh_reader::{
    Database, ReadLimits,
    name_list::{NameKind, NameList},
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
    println!(
        "{}: id {}, {} games in the list",
        db.player_name(found.id)?.unwrap_or_default(),
        found.id,
        found.games
    );

    let t = Instant::now();
    let scanned = db
        .player_games(found.id, workers, &cancel, |_, _| {})?
        .ok_or("cancelled")?;
    let scan = t.elapsed();
    let t = Instant::now();
    let stats = db.player_statistics(found.id, &scanned)?;
    let score = t.elapsed();
    println!(
        "scan ({workers} threads): {:.3} s, scoring and names: {:.3} s",
        scan.as_secs_f64(),
        score.as_secs_f64()
    );
    println!(
        "{} games ({} deleted, {} unscored), {}–{}; {} openings, {} opponents, {} tournaments, {} years",
        stats.found,
        stats.deleted,
        stats.unscored,
        stats.first_year,
        stats.last_year,
        stats.openings.len(),
        stats.opponents.len(),
        stats.tournaments.len(),
        stats.years.len()
    );
    for (label, s) in [
        ("White", stats.white),
        ("Black", stats.black),
        ("Total", stats.total),
    ] {
        println!(
            "{label}: {} games, +{} ={} -{}, {}%, Elo-Av {:?}, Perf {:?}",
            s.games,
            s.wins,
            s.draws,
            s.losses,
            s.percent.unwrap_or(0),
            s.average_elo,
            s.performance
        );
    }
    Ok(())
}
