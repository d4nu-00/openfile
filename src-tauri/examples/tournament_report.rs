//! Times a tournament report through the app, on a database opened
//! read-only, using the app's data folder for its tournament list and sort
//! orders. Prints counts only.
//!   tournament_report <db> <data folder> <tournament name prefix>
use cbh_reader::name_list::NameKind;
use chess_workbench::registry::Registry;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "tournament_report <db> <data folder> <name>";
    let path = args.next().ok_or(usage)?;
    let data = args.next().ok_or(usage)?;
    let name = args.next().ok_or(usage)?;
    let r = Registry::default();
    r.set_index_dir(std::path::Path::new(&data).join("search-boosters"));
    let db = r
        .open_database(path.into(), "open".into())
        .map_err(|e| e.message)?
        .database_id;
    r.name_list(db.clone(), NameKind::Tournaments, |_, _| {})
        .map_err(|e| e.message)?;
    let row = r
        .find_name(db.clone(), NameKind::Tournaments, name)
        .map_err(|e| e.message)?;
    let tournament = r
        .read_names(db.clone(), NameKind::Tournaments, row, 1, "n".into())
        .map_err(|e| e.message)?
        .rows
        .remove(0)
        .id;
    let started = Instant::now();
    let report = r.tournament_report(db, tournament).map_err(|e| e.message)?;
    println!(
        "{:.2} s: {} players, {} games, {} rounds ({} listed), {:?} layout",
        started.elapsed().as_secs_f64(),
        report.table.players.len(),
        report.stats.games,
        report.table.rounds,
        report.rounds.len(),
        report.table.layout
    );
    Ok(())
}
