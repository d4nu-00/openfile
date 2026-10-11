use cbh_reader::{ClassicDatabase, ReadLimits};
use std::{collections::HashMap, io::Write, path::PathBuf, time::Instant};
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut options = HashMap::new();
    while let Some(key) = args.next() {
        if ![
            "--path",
            "--page-start",
            "--page-limit",
            "--entry",
            "--pgn-out",
            "--json-out",
        ]
        .contains(&key.as_str())
            || options.contains_key(&key)
        {
            return Err(format!("Unknown or repeated argument: {key}").into());
        }
        let value = args.next().ok_or("Missing argument value")?;
        options.insert(key, value);
    }
    let path = PathBuf::from(options.get("--path").ok_or("--path is required")?);
    if options.contains_key("--pgn-out") && !options.contains_key("--entry") {
        return Err("--pgn-out requires --entry".into());
    }
    let started = Instant::now();
    let mut db = ClassicDatabase::open(&path, ReadLimits::default())?;
    let open_ms = started.elapsed().as_secs_f64() * 1000.;
    let start = options
        .get("--page-start")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(1);
    let limit = options
        .get("--page-limit")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(50);
    let started = Instant::now();
    let page = if db.info().entry_count == 0 {
        None
    } else {
        Some(db.page(start, limit)?)
    };
    let page_ms = started.elapsed().as_secs_f64() * 1000.;
    let started = Instant::now();
    let loaded = options
        .get("--entry")
        .map(|s| s.parse::<u64>())
        .transpose()?
        .map(|n| db.load(n))
        .transpose()?;
    let load_ms = started.elapsed().as_secs_f64() * 1000.;
    if let Some(output) = options.get("--pgn-out") {
        let game = loaded
            .as_ref()
            .and_then(|l| l.game.as_ref())
            .ok_or("Selected entry has no supported game")?;
        write_new(output, &chess_core::write_pgn(game)?)?;
    }
    let json = serde_json::to_string_pretty(
        &serde_json::json!({"info":db.info(),"page":page,"loaded":loaded,"metrics":{"openMs":open_ms,"pageMs":page_ms,"loadMs":load_ms,"bytesRead":db.bytes_read()}}),
    )?;
    if let Some(output) = options.get("--json-out") {
        write_new(output, &json)?;
    } else {
        println!("{json}");
    }
    Ok(())
}
fn write_new(path: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    let p = std::path::Path::new(path);
    let extension = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if [
        "cbh", "cbg", "cba", "cbp", "cbt", "cbc", "cbe", "cbs", "cbl",
    ]
    .contains(&extension.as_str())
    {
        return Err("Refusing a database companion output path".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(p)?;
    file.write_all(value.as_bytes())?;
    file.sync_all()?;
    Ok(())
}
