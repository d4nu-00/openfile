//! Converts entries of a database into a new 2CBH database, as the
//! workbench's Convert to 2CBH does, and compares the space estimate with
//! the files written.
//!
//! Usage: convert --path <source> --out <new.2cbh> [--from N] [--count N]
use cbh_reader::{
    Database, ReadLimits,
    v2::{convert, write::Writer},
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
    let out = PathBuf::from(value("--out").ok_or("--out is required")?);
    let mut source = Database::open(&path, ReadLimits::default())?;
    let entries = source.info().entry_count;
    let from: u64 = value("--from").map_or(Ok(1), |v| v.parse())?;
    let count: u64 = value("--count").map_or(Ok(entries), |v| v.parse())?;
    let count = count.min(entries.saturating_sub(from - 1));
    let estimate = convert::estimate(&source, count);
    let mut writer = Writer::create(&out)?;
    let started = Instant::now();
    let report = convert::convert(
        &mut source,
        from..from + count,
        &mut writer,
        &AtomicBool::new(false),
        |_| {},
    )?;
    drop(writer);
    let seconds = started.elapsed().as_secs_f64();
    println!("{report:?}");
    println!(
        "{count} entries in {seconds:.1} s, {:.2} ms per entry",
        seconds * 1000.0 / count.max(1) as f64
    );
    let mut total = 0;
    for ext in ["2cbh", "2cbg", "2cba", "2lid"] {
        let len = std::fs::metadata(out.with_extension(ext))?.len();
        total += len;
        println!("{ext}: {len} bytes");
    }
    let lid = std::fs::metadata(out.with_extension("2lid"))?.len();
    println!(
        "written {total} bytes, estimate {estimate} bytes ({:.2}x); name groups {}",
        estimate as f64 / total as f64,
        (lid - 184) / 4234
    );
    let size = |ext: &str| {
        std::fs::metadata(path.with_extension(ext)).map_or(0, |m| m.len()) * count / entries.max(1)
    };
    println!(
        "source share: moves {} annotations {}",
        size("cbg"),
        size("cba")
    );
    Ok(())
}
