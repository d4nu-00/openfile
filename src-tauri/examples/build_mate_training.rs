//! Builds mate-in-N training databases from the games a search finds in a
//! database, with the same builder as the app's Build Mate Training. The
//! source is only read.
//!   build_mate_training --path <db> --filter '<GameFilter JSON>' --engine <exe>
//!     --millis 800 --min 1 --max 5 --per 20 [--combinations "Q+N,R+K"]
//!     [--pool-minors] --out <folder> --name <name>
//! `--engine` is needed only for depths above 2.
use cbh_reader::{Database, ReadLimits, search::GameFilter};
use engine::EngineSpec;
use openfile::mate_training::{
    build::BuildOptions,
    run::{Control, MateTrainingResponse, collect, write_databases},
    verify::{CombinedVerifier, EngineVerifier},
};
use std::{
    path::PathBuf,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

const USAGE: &str = "build_mate_training --path <db> --filter '<JSON>' [--engine <exe>] \
    [--millis 800] [--min 1] [--max 5] [--per 20] [--combinations \"Q+N,R+K\"] \
    [--pool-minors] --out <folder> --name <name>";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (mut path, mut filter, mut engine, mut out, mut name) = (None, None, None, None, None);
    let (mut millis, mut min, mut max, mut per) = (800u64, 1u8, 5u8, 20u32);
    let (mut combinations, mut pool_minors) = (None, false);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(USAGE);
        match flag.as_str() {
            "--path" => path = Some(PathBuf::from(value()?)),
            "--filter" => filter = Some(value()?),
            "--engine" => engine = Some(PathBuf::from(value()?)),
            "--millis" => millis = value()?.parse()?,
            "--min" => min = value()?.parse()?,
            "--max" => max = value()?.parse()?,
            "--per" => per = value()?.parse()?,
            "--combinations" => {
                combinations = Some(
                    value()?
                        .split(',')
                        .map(|c| c.trim().to_string())
                        .collect::<Vec<_>>(),
                )
            }
            "--pool-minors" => pool_minors = true,
            "--out" => out = Some(PathBuf::from(value()?)),
            "--name" => name = Some(value()?),
            _ => return Err(USAGE.into()),
        }
    }
    let (path, out, name) = (path.ok_or(USAGE)?, out.ok_or(USAGE)?, name.ok_or(USAGE)?);
    let filter: GameFilter = match filter {
        Some(json) => serde_json::from_str(&json)?,
        None => GameFilter::default(),
    };
    let options = BuildOptions {
        min_depth: min,
        max_depth: max,
        per,
        combinations,
        pool_minors,
    };
    options.check().map_err(|e| e.message)?;
    let mut db = Database::open(&path, ReadLimits::default())?;
    let started = Instant::now();
    let found = db.search(&filter, usize::MAX, &AtomicBool::new(false), |_| {})?;
    println!(
        "{:.1} s: {} of {} games match",
        started.elapsed().as_secs_f64(),
        found.ordinals.len(),
        found.scanned
    );
    let mut verifier = match (max > 2, engine) {
        (true, Some(executable)) => CombinedVerifier::new(
            EngineVerifier::spawn(
                EngineSpec {
                    executable,
                    arguments: vec![],
                },
                millis,
            )
            .map_err(|e| e.message)?,
        ),
        (true, None) => return Err("--engine is needed for depths above 2".into()),
        (false, _) => CombinedVerifier::solver_only(),
    };
    let build = Instant::now();
    let mut last = Instant::now();
    let (collected, outcome) = collect(
        &mut db,
        &found.ordinals,
        &options,
        &mut verifier,
        &Control::default(),
        |done, total, found| {
            if last.elapsed() >= Duration::from_secs(10) {
                last = Instant::now();
                let puzzles: Vec<usize> = (1..=5).map(|d| found.puzzles(d).len()).collect();
                println!(
                    "{:>7.1} s: {done}/{total} games, puzzles {puzzles:?}, {} chains",
                    build.elapsed().as_secs_f64(),
                    found.chains().len()
                );
            }
        },
    )
    .map_err(|e| e.message)?;
    let ran = build.elapsed();
    let databases = write_databases(&collected, &options, &out, &name, db.source_paths())
        .map_err(|e| e.message)?;
    let response =
        MateTrainingResponse::new(&collected, outcome, found.ordinals.len() as u64, databases);
    print_report(&response, min, max, ran);
    let mut lengths = [0u32; 6];
    for chain in collected.chains() {
        lengths[chain.steps.len()] += 1;
    }
    println!(
        "chains by length: {}",
        (2..=5)
            .map(|n| format!("{n}: {}", lengths[n]))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

fn print_report(r: &MateTrainingResponse, min: u8, max: u8, ran: Duration) {
    println!(
        "\nbuilt in {:.1} s from {} of {} games{}",
        ran.as_secs_f64(),
        r.scanned,
        r.total,
        if r.complete { " (every quota met)" } else { "" }
    );
    print!("{:<14}", "combination");
    for d in min..=max {
        print!("{:>9}", format!("mate {d}"));
    }
    println!("{:>9}", "chains");
    for c in &r.combinations {
        print!("{:<14}", c.combination);
        for d in min..=max {
            print!("{:>9}", c.depths[usize::from(d) - 1]);
        }
        println!("{:>9}", c.chains);
    }
    print!("{:<14}", "total");
    for d in min..=max {
        print!("{:>9}", r.puzzles[usize::from(d) - 1]);
    }
    println!("{:>9}", r.chains);
    println!("games of other combinations: {}", r.filtered_out);
    for x in &r.rejections {
        println!("rejected {}: {}", x.kind, x.count);
    }
    for d in &r.databases {
        println!("wrote {} ({} games)", d.path, d.games);
    }
}
