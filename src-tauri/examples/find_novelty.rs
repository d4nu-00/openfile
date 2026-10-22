//! Times Find Novelty through the app on games of a database opened
//! read-only, against the same database (each game's own entry left out).
//!   find_novelty <db> <search-booster folder> <entry>...
//! The opening tree, when there is one, is read beside that folder.
use openfile::registry::Registry;
use std::{sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "find_novelty <db> <search-booster folder> <entry>...";
    let path = args.next().ok_or(usage)?;
    let r = Registry::default();
    r.set_index_dir(args.next().ok_or(usage)?.into());
    let db = r
        .open_database(path.into(), "open".into())
        .map_err(|e| e.message)?
        .database_id;
    for entry in args {
        let doc = r
            .load_entry(db.clone(), entry.clone(), "load".into())
            .map_err(|e| e.message)?
            .document_id
            .ok_or("no game")?;
        let t = Instant::now();
        let report = r
            .find_novelty(&doc, &db, &AtomicBool::new(false))
            .map_err(|e| e.message)?;
        println!(
            "entry {entry}: novelty {:?} known ply {} games {} queries {} not searched {} in {:.2} s",
            report.novelty.as_ref().map(|n| n.ply),
            report.known_ply,
            report.total,
            report.queries,
            report.not_searched,
            t.elapsed().as_secs_f64()
        );
        r.close_document(doc).map_err(|e| e.message)?;
    }
    Ok(())
}
