//! Copies games into a new 2CBH database the way Paste does (load, then
//! append in batches of 200) and checks them back.
//!
//! Usage: copy_games --path <source> --out <new.2cbh> --from N --count N
use cbh_reader::{DataStatus, Database, ReadLimits, v2::write::Writer};
use std::{path::PathBuf, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let path = PathBuf::from(value("--path").ok_or("--path is required")?);
    let out = PathBuf::from(value("--out").ok_or("--out is required")?);
    let from: u64 = value("--from").ok_or("--from is required")?.parse()?;
    let count: u64 = value("--count").ok_or("--count is required")?.parse()?;
    let mut source = Database::open(&path, ReadLimits::default())?;
    let mut writer = Writer::create(&out)?;
    let started = Instant::now();
    let (mut pasted, mut skipped, mut partial, mut omitted) = (0, 0, 0, 0);
    let ordinals: Vec<u64> = (from..from + count).collect();
    let mut copied = vec![];
    let (mut load_s, mut write_s) = (0.0, 0.0);
    for chunk in ordinals.chunks(200) {
        let t = Instant::now();
        let mut games = vec![];
        for &o in chunk {
            let loaded = source.load(o)?;
            match loaded.game {
                Some(g) => {
                    if !matches!(
                        loaded.annotations_status,
                        DataStatus::Available | DataStatus::Missing
                    ) {
                        partial += 1;
                    }
                    copied.push((o, g.nodes.len()));
                    games.push(g);
                }
                None => skipped += 1,
            }
        }
        load_s += t.elapsed().as_secs_f64();
        let t = Instant::now();
        let written = writer.append_all(&games)?;
        write_s += t.elapsed().as_secs_f64();
        pasted += written.len();
        omitted += written.iter().filter(|(_, o)| !o.notes.is_empty()).count();
    }
    drop(writer);
    println!("loading {load_s:.2}s, writing {write_s:.2}s");
    let seconds = started.elapsed().as_secs_f64();
    // Check every copy: names, result and the number of move-tree nodes.
    let mut target = Database::open(&out, ReadLimits::default())?;
    let mut mismatched = 0;
    for (i, (o, nodes)) in copied.iter().enumerate() {
        let a = source.load(*o)?;
        let b = target.load(i as u64 + 1)?;
        let same = a.summary.white == b.summary.white
            && a.summary.black == b.summary.black
            && a.summary.event == b.summary.event
            && a.summary.result == b.summary.result
            && b.game.as_ref().map(|g| g.nodes.len()) == Some(*nodes);
        if !same {
            mismatched += 1;
            eprintln!(
                "entry {o}: {:?} {:?} {:?} {:?} nodes {nodes} | copy {:?} {:?} {:?} {:?} nodes {:?}",
                a.summary.white,
                a.summary.black,
                a.summary.event,
                a.summary.result,
                b.summary.white,
                b.summary.black,
                b.summary.event,
                b.summary.result,
                b.game.as_ref().map(|g| g.nodes.len())
            );
        }
    }
    println!(
        "pasted {pasted}, skipped {skipped}, not fully decoded {partial}, with omitted annotations {omitted}, in {seconds:.2}s ({:.2} ms/game); checked back: {mismatched} differ; target {} bytes",
        seconds * 1000.0 / pasted.max(1) as f64,
        std::fs::metadata(&out)?.len()
    );
    Ok(())
}
