//! Runs a game-data search and prints the hit count, timing and first hits.
//!
//! Usage: search --path <file.cbh|file.2cbh> --filter '<GameFilter JSON>'
//! Add --within-filter '<GameFilter JSON>' to time refinement of an initial result.
use cbh_reader::{Database, ReadLimits, search::GameFilter};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

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
        first.push(format!(
            "{o}: {} - {} {} {} {}",
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
