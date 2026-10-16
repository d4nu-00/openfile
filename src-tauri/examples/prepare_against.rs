//! Times Prepare against White or Black through the app, on a database
//! opened read-only, using the app's data folder for its player list.
//!   prepare_against <db> <data folder> <"Last, First"> [black]
use cbh_reader::name_list::NameKind;
use chess_workbench::registry::Registry;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "prepare_against <db> <data folder> <name> [black]";
    let path = args.next().ok_or(usage)?;
    let data = args.next().ok_or(usage)?;
    let name = args.next().ok_or(usage)?;
    let white = args.next().as_deref() != Some("black");
    let r = Registry::default();
    r.set_index_dir(std::path::Path::new(&data).join("search-boosters"));
    let db = r
        .open_database(path.into(), "open".into())
        .map_err(|e| e.message)?
        .database_id;
    r.name_list(db.clone(), NameKind::Players, |_, _| {})
        .map_err(|e| e.message)?;
    let row = r
        .find_name(db.clone(), NameKind::Players, name)
        .map_err(|e| e.message)?;
    let player = r
        .read_names(db.clone(), NameKind::Players, row, 1, "n".into())
        .map_err(|e| e.message)?
        .rows
        .remove(0)
        .id;
    let started = Instant::now();
    let p = r
        .prepare_against(db, player, white, |_, _, _| {})
        .map_err(|e| e.message)?
        .ok_or("cancelled")?;
    println!(
        "{:.2} s: {} games, {} read, {} unreadable, {} lines, {} openings, recent rating {:?}",
        started.elapsed().as_secs_f64(),
        p.games,
        p.read,
        p.unreadable,
        p.repertoire.len(),
        p.openings.len(),
        p.recent_rating
    );
    Ok(())
}
