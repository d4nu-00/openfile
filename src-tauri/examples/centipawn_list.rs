//! Times Centipawn Analysis of a 2CBH database's games through the app: the
//! database is opened for editing, so use a scratch copy.
//!   centipawn_list <scratch.2cbh> <engine> <seconds> <first> <count>
use openfile::{centipawn::CentipawnParams, clipboard::RowRange, registry::Registry};
use std::{sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "centipawn_list <scratch.2cbh> <engine> <seconds> <first> <count>";
    let path = args.next().ok_or(usage)?;
    let engine = args.next().ok_or(usage)?;
    let seconds: u32 = args.next().ok_or(usage)?.parse()?;
    let first: u64 = args.next().ok_or(usage)?.parse()?;
    let count: u64 = args.next().ok_or(usage)?.parse()?;
    let r = Registry::default();
    let db = r
        .open_database_with(path.into(), "open".into(), true)
        .map_err(|e| e.message)?
        .database_id;
    let started = Instant::now();
    let report = r
        .centipawn_list(
            &db,
            &[RowRange {
                start: first.to_string(),
                end: (first + count).to_string(),
            }],
            &CentipawnParams {
                engine: engine.into(),
                options: Default::default(),
                seconds,
                every_move: false,
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .map_err(|e| e.message)?;
    println!(
        "{} games: {} analysed, {} replaced, {} skipped in {:.1} s; {} players",
        report.games,
        report.analysed,
        report.replaced,
        report.skipped_count,
        started.elapsed().as_secs_f64(),
        report.players.len()
    );
    for s in &report.skipped {
        println!("skipped {}: {}", s.ordinal, s.reason);
    }
    // The statistics without names: values, weights and the derived columns.
    for p in &report.players {
        println!(
            "player: elo {} perform {:?} points {:.1} games {} weighted av {:?} values {:?} weights {:?}",
            p.elo,
            p.perform,
            p.points,
            p.games.len(),
            p.weighted_average.map(|v| (v * 100.0).round() / 100.0),
            p.games.iter().map(|g| g.value).collect::<Vec<_>>(),
            p.games.iter().map(|g| g.weight).collect::<Vec<_>>(),
        );
    }
    println!(
        "csv: {} lines, header {}",
        report.csv.lines().count(),
        report.csv.lines().next().unwrap_or("")
    );
    Ok(())
}
