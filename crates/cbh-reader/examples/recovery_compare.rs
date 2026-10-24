//! Compare each retained recovery entry with a known original using its log.
use cbh_reader::{Database, ReadLimits};
use std::{
    io::{BufRead, BufReader},
    path::Path,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: recovery_compare KNOWN_ORIGINAL.2cbh RESULT_FOLDER".into());
    }
    let folder = Path::new(&args[2]);
    let mut original = Database::open(Path::new(&args[1]), ReadLimits::default())?;
    let mut recovered = Database::open(&folder.join("Recovered.2cbh"), ReadLimits::default())?;
    let mut compared = 0;
    for line in BufReader::new(std::fs::File::open(folder.join("recovery.jsonl"))?).lines() {
        let row: serde_json::Value = serde_json::from_str(&line?)?;
        if row["status"] != "recovered" {
            continue;
        }
        let old = row["entry"].as_u64().ok_or("missing entry")?;
        let new = row["recoveredEntry"]
            .as_u64()
            .ok_or("missing recovered entry")?;
        let a = original.load(old)?;
        let b = recovered.load(new)?;
        assert_eq!(a.game, b.game, "game {old} -> {new}");
        assert_eq!(a.summary.deleted, b.summary.deleted);
        assert_eq!(a.summary.medals, b.summary.medals);
        compared += 1;
    }
    let checked = recovered.check_integrity(&std::sync::atomic::AtomicBool::new(false), |_| {})?;
    assert!(checked.problems.is_empty());
    assert_eq!(compared, recovered.info().entry_count);
    println!(
        "{compared} recovered games match complete decoded move trees, represented annotations, metadata, results, deleted marks and medals; integrity clean"
    );
    Ok(())
}
