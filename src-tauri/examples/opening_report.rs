//! Times an opening report on a database opened read-only, using the
//! app's data folder for its opening tree.
//!   opening_report <db> <data folder> <uci moves…>
use chess_workbench::{registry::Registry, trees::Line};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("opening_report <db> <data folder> <moves…>")?;
    let data = args.next().ok_or("missing data folder")?;
    let moves: Vec<String> = args.collect();
    let r = Registry::default();
    r.set_index_dir(std::path::Path::new(&data).join("search-boosters"));
    let db = r
        .open_database(path.into(), "open".into())
        .map_err(|e| e.message)?;
    let started = Instant::now();
    let report = r
        .opening_report(
            db.database_id,
            Line {
                start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".into(),
                moves,
            },
            |_, _, _| {},
        )
        .map_err(|e| e.message)?;
    println!(
        "{:.2} s: {} {:?} — {} games{}, +{} ={} -{}, read {}, moves from tree {}",
        started.elapsed().as_secs_f64(),
        report.eco.as_deref().unwrap_or("-"),
        report.name,
        report.games,
        if report.truncated { " (truncated)" } else { "" },
        report.white_wins,
        report.draws,
        report.black_wins,
        report.read,
        report.moves_from_tree
    );
    println!(
        "years {}..{}, fashion buckets {}",
        report.years.first().map_or(0, |y| y.0),
        report.years.last().map_or(0, |y| y.0),
        report.fashion.len()
    );
    println!(
        "strong as White: {}",
        report
            .strong_white
            .iter()
            .map(|p| format!("{} {}", p.name, p.best_elo.unwrap_or(0)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "notable: {}",
        report
            .notable
            .iter()
            .map(|p| format!("{} ({})", p.name, p.games))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for m in report.moves.iter().take(6) {
        println!(
            "  {} {} games {:.1}% score {:?} by {}",
            m.san,
            m.games,
            m.share,
            m.score.map(|s| (s * 10.0).round() / 10.0),
            m.played_by.join(", ")
        );
    }
    println!("main line: {}", report.main_line.join(" "));
    Ok(())
}
