//! Builds a sort order and prints its timing and first entries.
//!
//! Usage: sort --path <file.cbh|file.2cbh> --column <white|date|…> --out <file>
use cbh_reader::{
    Database, ReadLimits,
    sort::{SortColumn, SortOptions, SortOrder},
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
    let name = value("--column").ok_or("--column is required")?;
    let column = SortColumn::ALL
        .into_iter()
        .find(|c| c.name() == name)
        .ok_or("unknown column")?;
    let out = PathBuf::from(value("--out").ok_or("--out is required")?);
    let mut db = Database::open(&path, ReadLimits::default())?;
    let started = Instant::now();
    let mut stages = vec![];
    let mut last = None;
    let built = SortOrder::build(
        &db,
        column,
        &out,
        &SortOptions::default(),
        &AtomicBool::new(false),
        |stage, _, _| {
            if last != Some(stage) {
                stages.push(format!(
                    "{stage:?} at {:.2}s",
                    started.elapsed().as_secs_f64()
                ));
                last = Some(stage);
            }
        },
    )?;
    let seconds = started.elapsed().as_secs_f64();
    let mut order = SortOrder::open(&out, &db, column)?.ok_or("not current")?;
    let mut first = vec![];
    for descending in [false, true] {
        for o in order.read(0, 3, descending)? {
            let e = &db.page(o, 1)?.entries[0];
            first.push(format!(
                "{o}: {} - {} {} {} {} {} {}",
                e.white.as_deref().unwrap_or("?"),
                e.black.as_deref().unwrap_or("?"),
                e.result.as_deref().unwrap_or("?"),
                e.event.as_deref().unwrap_or("?"),
                e.date.as_deref().unwrap_or("?"),
                e.white_elo.unwrap_or(0),
                e.round.as_deref().unwrap_or("")
            ));
        }
    }
    let probe = order.read(order.len() / 2, 1, false)?[0];
    let t = Instant::now();
    let place = order.place_of(probe, false)?;
    let place_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "built": built,
            "seconds": seconds,
            "stages": stages,
            "bytes": std::fs::metadata(&out)?.len(),
            "first": first,
            "placeOfMiddleMs": place_ms,
            "place": place,
        }))?
    );
    Ok(())
}
