//! Check Integrity, Find Double Games and Remove Deleted Games, timed.
//!
//! Usage:
//!   maintain integrity <db>
//!   maintain doubles <db> [exact]
//!   maintain compact <copy.2cbh> [mark-every N]   (changes the files!)
use cbh_reader::{
    Database, ReadLimits,
    maintain::{Compare, DoubleOptions},
    v2::{compact, write::Writer},
};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (what, path) = (args[0].as_str(), PathBuf::from(&args[1]));
    let cancel = AtomicBool::new(false);
    let t = Instant::now();
    match what {
        "integrity" => {
            let db = Database::open(&path, ReadLimits::default())?;
            let r = db.check_integrity(&cancel, |_| {})?;
            println!(
                "checked {} (games {}, texts {}, deleted {}) in {:.2} s; problems {:?}",
                r.checked,
                r.games,
                r.texts,
                r.deleted,
                t.elapsed().as_secs_f64(),
                r.counts
            );
            for p in r.problems.iter().take(5) {
                println!("  {} {} {}", p.ordinal, p.code, p.message);
            }
        }
        "doubles" => {
            let db = Database::open(&path, ReadLimits::default())?;
            let mut o = DoubleOptions::default();
            if args.get(2).map(String::as_str) == Some("exact") {
                o.names = Compare::Exact;
                o.moves = Compare::Exact;
            }
            let r = db.find_doubles(&o, &cancel, |_, _| {})?;
            println!(
                "checked {} in {:.2} s: {} doubles to mark, {} pairs kept",
                r.checked,
                t.elapsed().as_secs_f64(),
                r.pairs.len(),
                r.kept_both
            );
            let mut db = db;
            let step = (r.pairs.len() / 6).max(1);
            for p in r.pairs.iter().step_by(step) {
                for o in [p.0, p.1] {
                    let e = db.load(o)?;
                    let s = &e.summary;
                    println!(
                        "  {} {:?} - {:?} {:?} {:?} {:?} nodes {:?}",
                        o,
                        s.white,
                        s.black,
                        s.event,
                        s.date,
                        s.result,
                        e.game.map(|g| g.nodes.len())
                    );
                }
            }
        }
        "compact" => {
            if let Some(n) = args.get(3).and_then(|v| v.parse::<u64>().ok()) {
                let mut w = Writer::open(&path)?;
                let mut db = Database::open(&path, ReadLimits::default())?;
                let mut marks = vec![];
                for o in (n..=w.entry_count()).step_by(n as usize) {
                    if db.page(o, 1)?.entries[0].kind == cbh_reader::EntryKind::Game {
                        marks.push(o);
                    }
                }
                drop(db);
                w.set_deleted_many(&marks)?;
                println!("marked {} deleted", marks.len());
            }
            let r = compact::compact(&path, &cancel, |_, _| {})?;
            println!(
                "kept {} removed {} in {:.2} s; {} -> {} bytes",
                r.kept,
                r.removed,
                t.elapsed().as_secs_f64(),
                r.bytes_before,
                r.bytes_after
            );
            let db = Database::open(&path, ReadLimits::default())?;
            let c = db.check_integrity(&cancel, |_| {})?;
            println!("after: {} entries, problems {:?}", c.checked, c.counts);
        }
        _ => return Err("integrity, doubles or compact".into()),
    }
    Ok(())
}
