//! Times Centipawn Analysis through the app on games of a database opened
//! read-only, with a UCI engine.
//!   centipawn_analysis <db> <engine> <seconds> <entry>...
use openfile::{centipawn::CentipawnParams, registry::Registry};
use std::{sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "centipawn_analysis <db> <engine> <seconds> <entry>...";
    let path = args.next().ok_or(usage)?;
    let engine = args.next().ok_or(usage)?;
    let seconds: u32 = args.next().ok_or(usage)?.parse()?;
    let r = Registry::default();
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
        let params = CentipawnParams {
            engine: engine.clone().into(),
            options: Default::default(),
            seconds,
            every_move: false,
        };
        let started = Instant::now();
        let rep = r
            .centipawn_analysis(&doc, &params, &AtomicBool::new(false), |_, _| {})
            .map_err(|e| e.message)?;
        println!(
            "entry {entry}: {:.1} s, book {} plies, {} positions, loss W {:?} B {:?}, error W {:?} B {:?}, categories W {:?} B {:?}",
            started.elapsed().as_secs_f64(),
            rep.book_plies,
            rep.positions,
            rep.white.average_loss.map(|v| v.round()),
            rep.black.average_loss.map(|v| v.round()),
            rep.white.error_value.map(|v| (v * 100.0).round() / 100.0),
            rep.black.error_value.map(|v| (v * 100.0).round() / 100.0),
            rep.white.categories,
            rep.black.categories,
        );
    }
    Ok(())
}
