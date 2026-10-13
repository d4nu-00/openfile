//! Times a crosstable found by scanning the entry index and through a sort
//! order by tournament.
//!
//! Usage: crosstable --path <db> --dir <folder> [--entry N]
//! The tournament is that of entry N (default 1). The sort order is built
//! in `--dir` when it is not there.
use cbh_reader::{
    Database, ReadLimits,
    sort::{SortColumn, SortOptions, SortOrder},
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
    let entry: u64 = value("--entry").map_or(Ok(1), |s| s.parse())?;
    let mut db = Database::open(&path, ReadLimits::default())?;
    let tournament = db
        .load(entry)?
        .summary
        .tournament_id
        .ok_or("the entry has no tournament")?;
    let order_path = dir.join("tournament.cwsort");
    let cancel = AtomicBool::new(false);
    if SortOrder::open(&order_path, &db, SortColumn::Tournament)?.is_none() {
        let t = Instant::now();
        SortOrder::build(
            &db,
            SortColumn::Tournament,
            &order_path,
            &SortOptions::default(),
            &cancel,
            |_, _, _| {},
        )?;
        println!("sort order built in {:.2} s", t.elapsed().as_secs_f64());
    }
    let mut order = SortOrder::open(&order_path, &db, SortColumn::Tournament)?.ok_or("no order")?;
    let t = Instant::now();
    let scanned = db.crosstable(tournament, None, &cancel)?;
    println!("scan: {:.3} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let sorted = db.crosstable_with(tournament, None, Some(&mut order), &cancel)?;
    println!("sort order: {:.3} s", t.elapsed().as_secs_f64());
    let players = sorted.as_ref().map_or(0, |c| c.players.len());
    println!(
        "tournament {tournament}: {players} players, same table: {}",
        scanned == sorted
    );
    Ok(())
}
