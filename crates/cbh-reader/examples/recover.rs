//! Read-only source recovery and a machine-readable summary for validation.
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: recover SOURCE.2cbh NEW_RESULT_FOLDER".into());
    }
    let start = Instant::now();
    let r = cbh_reader::v2::recover::recover(
        Path::new(&args[1]),
        Path::new(&args[2]),
        &AtomicBool::new(false),
        |phase, done, total| {
            if done == 0 || done == total {
                eprintln!("{phase}: {done}/{total}");
            }
        },
    )?;
    println!("{}", serde_json::to_string_pretty(&r)?);
    eprintln!("elapsed: {:.3}s", start.elapsed().as_secs_f64());
    Ok(())
}
