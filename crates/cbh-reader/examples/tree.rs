//! Builds an opening tree and answers a line, printing timings.
//!
//! Usage: tree --path <db> --tree <file> [--build] [--plies N] [--line e2e4,e7e5]
//!        [--game N --ply K] [--verify N]
//!
//! --verify compares N lines, taken from games of the database, with a full
//! scan of every main line.
use cbh_reader::{
    Database, ReadLimits,
    tree::{BuildOptions, OpeningTree, pack_move, position_hash},
};
use shakmaty::{Chess, Position, uci::UciMove};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let path = PathBuf::from(value("--path").ok_or("--path is required")?);
    let out = PathBuf::from(value("--tree").ok_or("--tree is required")?);
    let db = Database::open(&path, ReadLimits::default())?;
    let cancel = AtomicBool::new(false);
    if args.iter().any(|a| a == "--build") {
        let mut options = BuildOptions::default();
        if let Some(p) = value("--plies") {
            options.plies = p.parse()?;
        }
        let started = Instant::now();
        let mut last = Instant::now();
        OpeningTree::build(&db, &out, &options, &cancel, |stage, done, total| {
            if last.elapsed().as_secs() >= 30 || (done == total) {
                eprintln!(
                    "{stage:?} {done}/{total} after {:.0} s",
                    started.elapsed().as_secs_f64()
                );
                last = Instant::now();
            }
        })?;
        println!(
            "built in {:.1} s, {} bytes",
            started.elapsed().as_secs_f64(),
            std::fs::metadata(&out)?.len()
        );
    }
    let mut tree = OpeningTree::open(&out, &db)?.ok_or("tree is missing or stale")?;
    println!(
        "{} games, {} positions, {} plies",
        tree.games(),
        tree.positions(),
        tree.plies()
    );
    if let Some(n) = value("--verify") {
        return verify(&db, &mut tree, n.parse()?);
    }
    let mut pos = Chess::default();
    let mut hashes = vec![position_hash(pos.board(), pos.turn())];
    let mut moves = vec![];
    for uci in value("--line")
        .map_or("", |v| v)
        .split(',')
        .filter(|s| !s.is_empty())
    {
        let m = UciMove::from_ascii(uci.as_bytes())?.to_move(&pos)?;
        moves.push(pack_move(&m).ok_or("bad move")?);
        pos.play_unchecked(m);
        hashes.push(position_hash(pos.board(), pos.turn()));
    }
    if let Some(ordinal) = value("--game") {
        // The line of a game of the database, up to --ply.
        let ply: usize = value("--ply").map_or(Ok(40), |p| p.parse())?;
        let line = db
            .main_line(ordinal.parse()?, ply)?
            .ok_or("no main line")?
            .positions;
        hashes = line.iter().map(|p| p.0).collect();
        moves = line[..line.len() - 1].iter().map(|p| p.2).collect();
    }
    let started = Instant::now();
    let answer = tree.query(&db, &hashes, &moves, 100_000, 5_000, &cancel)?;
    println!(
        "{:?}: {} games listed{} in {:.1} ms",
        answer.source,
        answer.games.len(),
        if answer.truncated { " (limit)" } else { "" },
        started.elapsed().as_secs_f64() * 1e3
    );
    for m in answer.moves.iter().take(12) {
        println!(
            "{:6} {:>8} {:>6} {:>6} {:>5} {:>5} {}-{}",
            m.uci().unwrap_or_else(|| "end".into()),
            m.games,
            m.score().map_or("".into(), |s| format!("{s:.1}%")),
            m.draw_rate().map_or("".into(), |s| format!("{s:.0}%")),
            m.average_elo().map_or("".into(), |e| e.to_string()),
            m.performance().map_or("".into(), |e| e.to_string()),
            m.year_min,
            m.year_max
        );
    }
    Ok(())
}

/// Checks answers for lines cut from the database's own games against a scan
/// of all main lines.
fn verify(db: &Database, tree: &mut OpeningTree, n: u64) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::{HashMap, HashSet};
    let count = db.info().entry_count;
    let started = Instant::now();
    let mut reach: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut lines = HashMap::new();
    for ordinal in 1..=count {
        if let Some(line) = db.main_line(ordinal, usize::MAX)?.map(|l| l.positions) {
            for (h, _, _) in &line {
                reach.entry(*h).or_default().insert(ordinal);
            }
            lines.insert(ordinal, line);
        }
    }
    println!(
        "scanned {} games in {:.1} s",
        lines.len(),
        started.elapsed().as_secs_f64()
    );
    let cancel = AtomicBool::new(false);
    let (mut same, mut missing, mut extra, mut too_many) = (0, 0, 0, 0);
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut checked = 0;
    while checked < n {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let Some(line) = lines.get(&(state % count + 1)) else {
            continue;
        };
        let cut = (state >> 32) as usize % line.len();
        let hashes: Vec<u64> = line[..=cut].iter().map(|p| p.0).collect();
        let moves: Vec<u16> = line[..cut].iter().map(|p| p.2).collect();
        checked += 1;
        let answer = match tree.query(db, &hashes, &moves, usize::MAX, 5_000, &cancel) {
            Ok(a) => a,
            Err(cbh_reader::CbhError::LimitExceeded) => {
                too_many += 1;
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let found: HashSet<u64> = answer.games.iter().map(|g| g.0).collect();
        let truth = &reach[hashes.last().unwrap()];
        let total: u64 = answer.moves.iter().map(|m| m.games).sum();
        assert_eq!(total, found.len() as u64, "statistics and games disagree");
        if !found.is_subset(truth) {
            extra += 1;
        }
        if found == *truth {
            same += 1;
        } else if found.is_subset(truth) {
            missing += 1;
            if missing <= 5 {
                println!(
                    "ply {cut} {:?}: {} of {} games",
                    answer.source,
                    found.len(),
                    truth.len()
                );
            }
        }
    }
    println!(
        "{checked} lines: {same} exact, {missing} missing games, {extra} with wrong games, {too_many} over the follow limit"
    );
    Ok(())
}
