//! Classifies every game of a database with the workbench's ECO table and
//! compares with the stored codes; writes nothing.
//!   eco <db>
use cbh_reader::{Database, ReadLimits, eco::EcoOptions};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::args().nth(1).ok_or("eco <db>")?);
    let db = Database::open(&path, ReadLimits::default())?;
    let started = Instant::now();
    let r = db.classify_eco(EcoOptions::default(), &AtomicBool::new(false), |_, _| {})?;
    let pct = |a: u64, b: u64| {
        if b == 0 {
            0.0
        } else {
            100.0 * a as f64 / b as f64
        }
    };
    println!(
        "games {} in {:.2} s; same main code {} of {} coded games ({:.1} %); would change {}; unclassified {}; unreadable {}",
        r.games,
        started.elapsed().as_secs_f64(),
        r.agreed,
        r.had_code,
        pct(r.agreed, r.had_code),
        r.changes.len(),
        r.unclassified,
        r.unreadable
    );
    println!(
        "of the {} coded games given another code: same letter {} ({:.1} %), same letter and tens digit {} ({:.1} %)",
        r.had_code - r.agreed,
        r.same_letter,
        pct(r.same_letter, r.had_code - r.agreed),
        r.same_tens,
        pct(r.same_tens, r.had_code - r.agreed)
    );
    Ok(())
}
