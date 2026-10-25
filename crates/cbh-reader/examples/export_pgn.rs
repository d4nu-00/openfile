//! Searches a database and writes the first hits as PGN.
//!
//! Usage: export_pgn --path <file.cbh|file.2cbh> --filter '<GameFilter JSON>'
//!   --limit N --out file.pgn
use cbh_reader::{Database, ReadLimits, pgn_export::write_games, search::GameFilter};
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
    sync::atomic::AtomicBool,
    time::Instant,
};

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
    let limit: usize = value("--limit").ok_or("--limit is required")?.parse()?;
    let out = PathBuf::from(value("--out").ok_or("--out is required")?);
    let mut db = Database::open(&path, ReadLimits::default())?;
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let result = db.search(&filter, limit, &cancel, |_| {})?;
    let searched = started.elapsed().as_secs_f64();
    let mut file = BufWriter::new(std::fs::File::create(&out)?);
    let summary = write_games(&mut db, &result.ordinals, &mut file, &cancel, |_, _| {})?;
    file.flush()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "hits": result.ordinals.len(),
            "truncated": result.truncated,
            "written": summary.written,
            "skipped": summary.skipped,
            "stripped": summary.stripped,
            "searchSeconds": searched,
            "totalSeconds": started.elapsed().as_secs_f64(),
        }))?
    );
    Ok(())
}
