//! Runs the Error Report through the app on one player's games, on a
//! database opened read-only, with an engine of the user's. Prints counts,
//! timings and the estimate only.
//!   error_report <db> <data folder> <"Last, First"> <engine> <games> [slower]
use cbh_reader::{name_list::NameKind, player_stats::Group};
use openfile::{clipboard::RowRange, error_report::ErrorReportParams, registry::Registry};
use std::{sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "error_report <db> <data folder> <name> <engine> <games> [slower]";
    let path = args.next().ok_or(usage)?;
    let data = args.next().ok_or(usage)?;
    let name = args.next().ok_or(usage)?;
    let engine = args.next().ok_or(usage)?;
    let games: u32 = args.next().ok_or(usage)?.parse()?;
    let slower = args.next().as_deref() == Some("slower");
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
    r.player_statistics(db.clone(), player, |_, _| {})
        .map_err(|e| e.message)?;
    // The player's games become the list; the last `games` rows are taken.
    let listed = r
        .statistics_games(db.clone(), player, Group::All, "q".into())
        .map_err(|e| e.message)?;
    let hits: u64 = listed.hits.parse()?;
    let start = hits.saturating_sub(u64::from(games)) + 1;
    let ranges: Vec<RowRange> = serde_json::from_value(serde_json::json!([
        { "start": start.to_string(), "end": (hits + 1).to_string() }
    ]))?;
    let started = Instant::now();
    let report = r
        .error_report(
            db,
            &ErrorReportParams {
                engine: engine.into(),
                options: Default::default(),
                slower,
                player: Some(player),
                ranges,
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .map_err(|e| e.message)?;
    println!(
        "{:.1} s: {} games ({} skipped), {} moves at depth {}, average loss {:.1}, error value {:.1}, {:.0} plies/game",
        started.elapsed().as_secs_f64(),
        report.games,
        report.skipped,
        report.moves,
        report.depth,
        report.average_loss.unwrap_or(0.0),
        report.error_value.unwrap_or(0.0),
        report.plies_per_game.unwrap_or(0.0)
    );
    println!(
        "Blunder Elo {:?}, rating {:?}, difference {:?}, severity {:?}",
        report.blunder_elo, report.rating, report.difference, report.severity
    );
    Ok(())
}
