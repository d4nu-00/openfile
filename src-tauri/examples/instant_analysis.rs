//! Times Instant Analysis through the app on games of a database opened
//! read-only, with a UCI engine, then again from the evaluation cache. It also
//! compares with a profile the game has stored.
//!   instant_analysis <db> <engine> <level 1-6> <entry>...
use openfile::{centipawn::CentipawnParams, evaluation_profile::InstantParams, registry::Registry};
use std::{sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "instant_analysis <db> <engine> <level> <entry>...";
    let path = args.next().ok_or(usage)?;
    let engine = args.next().ok_or(usage)?;
    let level: u32 = args.next().ok_or(usage)?.parse()?;
    let r = Registry::default();
    let db = r
        .open_database(path.into(), "open".into())
        .map_err(|e| e.message)?
        .database_id;
    for entry in args {
        let loaded = r
            .load_entry(db.clone(), entry.clone(), "load".into())
            .map_err(|e| e.message)?;
        let doc = loaded.document_id.clone().ok_or("no game")?;
        let stored = loaded.game.as_ref().map_or(0, |g| g.profile.len());
        let params = InstantParams {
            engine: engine.clone().into(),
            options: Default::default(),
            level,
            endless: false,
        };
        let cancel = AtomicBool::new(false);
        let t = Instant::now();
        let mut updates = 0;
        let first = r
            .instant_analysis(&doc, &params, &cancel, |_| updates += 1)
            .map_err(|e| e.message)?;
        let fresh = t.elapsed().as_secs_f64();
        let t = Instant::now();
        r.instant_analysis(&doc, &params, &cancel, |_| {})
            .map_err(|e| e.message)?;
        let cached = t.elapsed().as_secs_f64();
        // Centipawn Analysis at 0 s asks 200 ms per position: longer than
        // instant analysis, so it evaluates afresh; instant analysis after it
        // then reuses its positions.
        let t = Instant::now();
        r.centipawn_analysis(
            &doc,
            &CentipawnParams {
                engine: engine.clone().into(),
                options: Default::default(),
                seconds: 0,
                every_move: false,
            },
            &cancel,
            |_, _| {},
        )
        .map_err(|e| e.message)?;
        let centipawn = t.elapsed().as_secs_f64();
        println!(
            "entry {entry}: {} positions, {} evaluated, {fresh:.1} s fresh ({updates} updates), {cached:.2} s again, Centipawn {centipawn:.1} s; stored profile {stored} entries",
            first.total,
            first.profile.len(),
        );
    }
    Ok(())
}
