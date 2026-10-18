//! Times a player dossier through the app, on a database opened read-only,
//! using the app's data folder for its player list. Prints counts only.
//!   player_dossier <db> <data folder> <"Last, First">
use cbh_reader::name_list::NameKind;
use openfile::registry::Registry;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "player_dossier <db> <data folder> <name>";
    let path = args.next().ok_or(usage)?;
    let data = args.next().ok_or(usage)?;
    let name = args.next().ok_or(usage)?;
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
    let d = r
        .player_dossier(db, player, |_, _, _| {})
        .map_err(|e| e.message)?
        .ok_or("cancelled")?;
    println!(
        "{:.2} s: {} games, {} years, bands {}, lines {}+{}, medals {}, mates {}/{} of {} read, annotated {}",
        started.elapsed().as_secs_f64(),
        d.games,
        d.years.len(),
        d.rating_bands.len(),
        d.repertoire_white.len(),
        d.repertoire_black.len(),
        d.medal_count,
        d.mates_given_count,
        d.mates_received_count,
        d.mates_read,
        d.annotated_count
    );
    if let Some(g) = d.mates_given.first().or(d.medal_games.first()) {
        println!("a listed game: entry {}", g.ordinal);
    }
    Ok(())
}
