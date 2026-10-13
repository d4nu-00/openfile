//! Builds the player and tournament lists, finds a name and prints a
//! tournament's crosstable.
//!
//! Usage: names --path <file.cbh|file.2cbh> --dir <folder> [--player <prefix>] [--tournament <prefix>]
use cbh_reader::{
    Database, ReadLimits,
    name_list::{NameKind, NameList},
};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let path = PathBuf::from(value("--path").ok_or("--path is required")?);
    let dir = PathBuf::from(value("--dir").ok_or("--dir is required")?);
    let mut db = Database::open(&path, ReadLimits::default())?;
    let cancel = AtomicBool::new(false);
    for (kind, prefix) in [
        (NameKind::Players, value("--player")),
        (NameKind::Tournaments, value("--tournament")),
    ] {
        let file = dir.join(format!("{}.cwnames", kind.name()));
        let started = Instant::now();
        NameList::build(&db, kind, &file, 8, &cancel, |_, _| {})?;
        let seconds = started.elapsed().as_secs_f64();
        let mut list = NameList::open(&file, &db, kind)?.ok_or("not current")?;
        println!(
            "{}: {} names in {seconds:.2}s, {} bytes",
            kind.name(),
            list.len(),
            std::fs::metadata(&file)?.len()
        );
        let Some(prefix) = prefix else { continue };
        let t = Instant::now();
        let at = list.find(&mut db, kind, prefix)?;
        println!(
            "  find {prefix:?}: row {at} in {:.1} ms",
            t.elapsed().as_secs_f64() * 1e3
        );
        for row in list.read(at, 5)? {
            let name = match kind {
                NameKind::Players => db.player_name(row.id)?.unwrap_or_default(),
                NameKind::Tournaments => {
                    let (e, s) = db.tournament_name(row.id)?;
                    format!("{} / {}", e.unwrap_or_default(), s.unwrap_or_default())
                }
            };
            println!(
                "  {name}: {} games, {}-{}",
                row.games, row.first_year, row.last_year
            );
        }
        if kind == NameKind::Tournaments {
            let id = list.read(at, 1)?[0].id;
            let t = Instant::now();
            let table = db.crosstable(id, None, &cancel)?.ok_or("cancelled")?;
            println!(
                "  crosstable in {:.2}s: {:?}, {} players, {} games, {} rounds, avg {:?}, cat {:?}",
                t.elapsed().as_secs_f64(),
                table.layout,
                table.players.len(),
                table.games,
                table.rounds,
                table.average_elo,
                table.category
            );
            for (i, p) in table.players.iter().take(12).enumerate() {
                let cells: Vec<String> = p
                    .cells
                    .iter()
                    .map(|c| c.iter().map(|g| g.mark.as_str()).collect::<String>())
                    .map(|s| if s.is_empty() { ".".into() } else { s })
                    .collect();
                println!(
                    "  {:>2} {:<28} {:>4} {:>4.1} SB {:>5.2} BH {:>5.1} perf {:?} | {}",
                    i + 1,
                    p.name.clone().unwrap_or_default(),
                    p.elo.unwrap_or(0),
                    p.points,
                    p.sonneborn_berger,
                    p.buchholz,
                    p.performance,
                    cells.join(" ")
                );
            }
        }
    }
    Ok(())
}
