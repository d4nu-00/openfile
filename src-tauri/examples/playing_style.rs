//! Read-only timing/validation: playing_style <db> <data folder> <exact name>
//! <limit> [sample.pgn]. JSON report goes to stdout; timings go to stderr.
use cbh_reader::{Database, ReadLimits, name_list::NameKind};
use openfile::{playing_style::StyleParams, registry::Registry};
use std::{io::Write, path::Path, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "playing_style <db> <data folder> <exact name> <limit> [sample.pgn]";
    let path = args.next().ok_or(usage)?;
    let data = args.next().ok_or(usage)?;
    let name = args.next().ok_or(usage)?;
    let limit = args.next().ok_or(usage)?.parse()?;
    let export = args.next();
    let r = Registry::default();
    r.set_index_dir(Path::new(&data).join("search-boosters"));
    let db = r
        .open_database(path.clone().into(), "open".into())
        .map_err(|e| e.message)?
        .database_id;
    let identity_started = Instant::now();
    r.name_list(db.clone(), NameKind::Players, |_, _| {})
        .map_err(|e| e.message)?;
    let row = r
        .find_name(db.clone(), NameKind::Players, name.clone())
        .map_err(|e| e.message)?;
    let nearby = r
        .read_names(db.clone(), NameKind::Players, row, 12, "names".into())
        .map_err(|e| e.message)?
        .rows;
    eprintln!(
        "Name lookup (list/find/read): {:.3}s",
        identity_started.elapsed().as_secs_f64()
    );
    for p in &nearby {
        eprintln!("Player {}: {:?}", p.id, p.name);
    }
    let player = nearby
        .iter()
        .find(|p| p.name.as_deref() == Some(name.as_str()))
        .ok_or("No exact player identity at that name")?
        .id;
    let started = Instant::now();
    let mut last_stage = String::new();
    let mut stage_started = Instant::now();
    let report = r
        .playing_style(
            db.clone(),
            "timing".into(),
            &StyleParams {
                player_ids: vec![player],
                sample_limit: limit,
                ..Default::default()
            },
            |p| {
                if p.stage != last_stage {
                    if !last_stage.is_empty() {
                        eprintln!(
                            "{}: {:.3}s",
                            last_stage,
                            stage_started.elapsed().as_secs_f64()
                        );
                    }
                    last_stage = p.stage;
                    stage_started = Instant::now();
                }
            },
        )
        .map_err(|e| e.message)?
        .ok_or("cancelled")?;
    eprintln!(
        "{}: {:.3}s; total {:.3}s; selected {}, analysed {}",
        last_stage,
        stage_started.elapsed().as_secs_f64(),
        started.elapsed().as_secs_f64(),
        report.selected,
        report.analysed
    );
    if let Some(export) = export {
        r.validate_playing_style(db.clone(), "timing".into())
            .map_err(|e| e.message)?;
        let mut reader = Database::open(Path::new(&path), ReadLimits::default())?;
        let mut text = String::new();
        for entry in &report.games {
            let mut game = reader
                .load(entry.ordinal.parse()?)?
                .game
                .ok_or("Sample game unavailable")?;
            game.tags
                .insert("OpenFileEntry".into(), entry.ordinal.clone());
            game.tags
                .insert("OpenFilePlayerWhite".into(), entry.white.to_string());
            text.push_str(&chess_core::write_pgn(&game)?);
            text.push_str("\n\n");
        }
        reader.verify_sources()?;
        r.validate_playing_style(db, "timing".into())
            .map_err(|e| e.message)?;
        openfile::save::reject_source_destination(Path::new(&export), reader.source_paths())
            .map_err(|e| e.message)?;
        openfile::save::atomic_pgn(Path::new(&export), &text).map_err(|e| e.message)?;
    }
    serde_json::to_writer_pretty(std::io::stdout(), &report)?;
    std::io::stdout().write_all(b"\n")?;
    Ok(())
}
