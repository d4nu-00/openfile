//! Runs a game-data search and prints the hit count, timing and first hits.
//!
//! Usage: search --path <file.cbh|file.2cbh> --filter '<GameFilter JSON>'
//! Add --within-filter '<GameFilter JSON>' to time refinement of an initial result.
use cbh_reader::{
    Database, ReadLimits,
    search::{GameFilter, Tally, final_position, mating_pieces},
};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

/// The involved piece kinds as letters, e.g. "Q,N,K".
fn piece_letters(t: &Tally) -> String {
    let kinds = [
        (t.queens, "Q"),
        (t.rooks, "R"),
        (t.bishops, "B"),
        (t.knights, "N"),
        (t.pawns, "P"),
        (t.kings, "K"),
    ];
    kinds
        .iter()
        .flat_map(|&(n, letter)| std::iter::repeat_n(letter, n as usize))
        .collect::<Vec<_>>()
        .join(",")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let path = PathBuf::from(value("--path").ok_or("--path is required")?);
    let filter: GameFilter =
        serde_json::from_str(value("--filter").ok_or("--filter is required")?)?;
    let mut db = Database::open(&path, ReadLimits::default())?;
    let selection = value("--within-filter")
        .map(|json| -> Result<_, Box<dyn std::error::Error>> {
            let filter: GameFilter = serde_json::from_str(json)?;
            Ok(db
                .search(&filter, 2_000_000, &AtomicBool::new(false), |_| {})?
                .ordinals)
        })
        .transpose()?;
    let started = Instant::now();
    let result = db.search_selected(
        &filter,
        selection.as_deref(),
        if filter.material.is_some() {
            100_000
        } else {
            2_000_000
        },
        &AtomicBool::new(false),
        |_, _| {},
    )?;
    let seconds = started.elapsed().as_secs_f64();
    let mut first = vec![];
    for &o in result.ordinals.iter().take(5) {
        let e = &db.page(o, 1)?.entries[0];
        // For a game that ends in mate, the pieces involved in the mate.
        let mate = db
            .load(o)?
            .game
            .and_then(|game| mating_pieces(&final_position(&game).ok()?))
            .map(|t| format!(" mate by {}", piece_letters(&t)))
            .unwrap_or_default();
        first.push(format!(
            "{o}: {} - {} {} {} {}{mate}",
            e.white.as_deref().unwrap_or("?"),
            e.black.as_deref().unwrap_or("?"),
            e.result.as_deref().unwrap_or("?"),
            e.event.as_deref().unwrap_or("?"),
            e.date.as_deref().unwrap_or("?")
        ));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "hits": result.ordinals.len(),
            "scanned": result.scanned,
            "truncated": result.truncated,
            "seconds": seconds,
            "first": first,
        }))?
    );
    Ok(())
}
