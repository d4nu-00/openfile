//! Cross-checks a 2CBH family against a classic CBH family entry by entry.
//!
//! Usage: compare_formats --classic <file.cbh> --new <file.2cbh> [--limit N]
//! Prints a JSON summary. Only metadata and decoded move trees are compared;
//! source files are opened read-only.
use cbh_reader::{DataStatus, Database, EntryKind, ReadLimits};
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

fn tree(game: &chess_core::Game) -> Vec<String> {
    // Every root-to-node path as UCI text, in child order.
    let mut out = vec![];
    let mut stack = vec![(0u32, String::new())];
    while let Some((id, prefix)) = stack.pop() {
        for &child in game.nodes[id as usize].children.iter().rev() {
            let path = format!(
                "{prefix} {}",
                game.nodes[child as usize].uci.as_deref().unwrap_or("?")
            );
            out.push(path.clone());
            stack.push((child, path));
        }
    }
    out.sort();
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        options.insert(key, args.next().ok_or("Missing argument value")?);
    }
    let classic = PathBuf::from(options.get("--classic").ok_or("--classic is required")?);
    let new = PathBuf::from(options.get("--new").ok_or("--new is required")?);
    let mut a = Database::open(&classic, ReadLimits::default())?;
    let mut b = Database::open(&new, ReadLimits::default())?;
    let count = b.info().entry_count.min(a.info().entry_count);
    let limit: u64 = options
        .get("--limit")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(count);
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut bump = |k: &str| *counts.entry(k.to_string()).or_default() += 1;
    let mut failures = vec![];
    let mut diagnostics: BTreeMap<String, u64> = BTreeMap::new();
    let (mut nodes, mut slowest) = (0usize, 0f64);
    let started = Instant::now();
    for ordinal in 1..=limit.min(count) {
        let x = a.load(ordinal)?;
        let t = Instant::now();
        let y = b.load(ordinal)?;
        slowest = slowest.max(t.elapsed().as_secs_f64() * 1000.);
        for d in y.diagnostics.iter().chain(&y.summary.diagnostics) {
            *diagnostics.entry(d.code.clone()).or_default() += 1;
        }
        let (s, t) = (&x.summary, &y.summary);
        let same_meta = s.kind == t.kind
            && s.date == t.date
            && s.result == t.result
            && s.round == t.round
            && s.white_elo == t.white_elo
            && s.black_elo == t.black_elo;
        if !same_meta {
            bump("metadataMismatch");
            failures.push(format!("{ordinal}: metadata differs"));
            continue;
        }
        if t.kind != EntryKind::Game {
            bump("textEntries");
            continue;
        }
        for (field, x, y) in [
            ("white", &s.white, &t.white),
            ("black", &s.black, &t.black),
            ("event", &s.event, &t.event),
            ("site", &s.site, &t.site),
            ("eco", &s.eco, &t.eco),
        ] {
            if x == y {
                bump(&format!("{field}Match"));
            } else {
                bump(&format!("{field}Mismatch"));
                if failures.len() < 50 {
                    failures.push(format!("{ordinal}: {field} {x:?} vs {y:?}"));
                }
            }
        }
        match (&x.game, &y.game) {
            (Some(g), Some(h)) => {
                nodes += h.nodes.len() - 1;
                if tree(g) == tree(h) {
                    bump("treesMatch");
                } else {
                    bump("treeMismatch");
                    failures.push(format!("{ordinal}: move tree differs"));
                }
            }
            (None, Some(_)) => {
                bump(&format!("onlyNew:{:?}", x.moves_status));
                failures.push(format!("{ordinal}: classic {:?}", x.diagnostics));
            }
            (Some(_), None) => {
                bump(&format!("onlyClassic:{:?}", y.moves_status));
                if y.moves_status == DataStatus::Corrupt {
                    failures.push(format!("{ordinal}: {:?}", y.diagnostics));
                }
            }
            (None, None) => bump(&format!(
                "neither:{:?}/{:?}",
                x.moves_status, y.moves_status
            )),
        }
    }
    failures.truncate(50);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "compared": limit.min(count),
            "newEntries": b.info().entry_count,
            "classicEntries": a.info().entry_count,
            "counts": counts,
            "newMoveNodes": nodes,
            "newDiagnostics": diagnostics,
            "firstFailures": failures,
            "maxNewLoadMs": slowest,
            "elapsedS": started.elapsed().as_secs_f64(),
            "newBytesRead": b.bytes_read(),
        }))?
    );
    Ok(())
}
