//! Builds a position index and finds a position, printing timings.
//!
//! Usage: position --path <db> [--index <file>] [--build] [--fen <FEN>]
//!        [--max N]
use cbh_reader::{
    Database, ReadLimits,
    position::{PositionIndex, Target},
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
    let mut db = Database::open(&path, ReadLimits::default())?;
    let cancel = AtomicBool::new(false);
    let index_path = value("--index").map(PathBuf::from);
    if args.iter().any(|a| a == "--build") {
        let out = index_path.as_ref().ok_or("--build needs --index")?;
        let started = Instant::now();
        let mut last = Instant::now();
        PositionIndex::build(&db, out, &cancel, |done, total| {
            if last.elapsed().as_secs() >= 30 {
                eprintln!(
                    "{done}/{total} after {:.0} s",
                    started.elapsed().as_secs_f64()
                );
                last = Instant::now();
            }
        })?;
        println!(
            "built {} in {:.1} s, {} bytes",
            out.display(),
            started.elapsed().as_secs_f64(),
            std::fs::metadata(out)?.len()
        );
    }
    if let Some(fen) = value("--fen") {
        let target = Target::from_fen(fen)?;
        let mut index = match &index_path {
            Some(p) => PositionIndex::open(p, &db)?,
            None => None,
        };
        let max = value("--max")
            .map(|v| v.parse())
            .transpose()?
            .unwrap_or(1000);
        let started = Instant::now();
        let r = db.find_position(&target, index.as_mut(), max, &cancel, |_, _| {})?;
        println!(
            "{} hits ({}), {} replayed, {} scanned, indexed {}, {:.2} s, first {:?}",
            r.hits.len(),
            if r.truncated {
                "stopped at limit"
            } else {
                "complete"
            },
            r.replayed,
            r.scanned,
            r.indexed,
            started.elapsed().as_secs_f64(),
            &r.hits[..r.hits.len().min(5)]
        );
    }
    Ok(())
}
