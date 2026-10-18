//! Times Generate Repertoire and the Repertoire Report through the app, on a
//! database opened read-only, using the app's data folder for its player
//! list. The repertoire databases are written into `out`, which must exist.
//!   generate_repertoire <db> <data folder> <"Last, First"> <out> [report]
use cbh_reader::name_list::NameKind;
use openfile::registry::Registry;
use std::{sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "generate_repertoire <db> <data folder> <name> <out> [report]";
    let path = args.next().ok_or(usage)?;
    let data = args.next().ok_or(usage)?;
    let name = args.next().ok_or(usage)?;
    let out = std::path::PathBuf::from(args.next().ok_or(usage)?);
    let report = args.next().as_deref() == Some("report");
    let r = Registry::default();
    r.set_index_dir(std::path::Path::new(&data).join("search-boosters"));
    let db = r
        .open_database(path.into(), "open".into())
        .map_err(|e| e.message)?
        .database_id;
    let started = Instant::now();
    r.name_list(db.clone(), NameKind::Players, |_, _| {})
        .map_err(|e| e.message)?;
    let frequent = r.frequent_players(db.clone()).map_err(|e| e.message)?;
    println!(
        "{:.2} s: player list and {} frequent players",
        started.elapsed().as_secs_f64(),
        frequent.len()
    );
    let row = r
        .find_name(db.clone(), NameKind::Players, name)
        .map_err(|e| e.message)?;
    let player = r
        .read_names(db.clone(), NameKind::Players, row, 1, "n".into())
        .map_err(|e| e.message)?
        .rows
        .remove(0)
        .id;
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let g = r
        .generate_repertoire(db.clone(), vec![player], out, &cancel, |_, _, _| {})
        .map_err(|e| e.message)?
        .ok_or("cancelled")?;
    println!("{:.2} s: generated", started.elapsed().as_secs_f64());
    for s in &g.sides {
        println!(
            "  {}: {} games, {} read, {} repertoire games of {} merged, {} rare, {} unclassified, {} cut",
            if s.white { "White" } else { "Black" },
            s.games,
            s.read,
            s.repertoire_games,
            s.merged,
            s.rare,
            s.unclassified,
            s.cut
        );
    }
    if report {
        for s in &g.sides {
            let Some(p) = &s.path else { continue };
            let started = Instant::now();
            let rep = r
                .repertoire_report(db.clone(), std::path::Path::new(p), &cancel, |_, _, _| {})
                .map_err(|e| e.message)?;
            println!(
                "{:.2} s: report {}: {} positions, {} of {} games match, {} topics shown",
                started.elapsed().as_secs_f64(),
                if s.white { "White" } else { "Black" },
                rep.positions,
                rep.matched,
                rep.scanned,
                rep.topics.len()
            );
        }
    }
    Ok(())
}
