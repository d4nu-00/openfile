//! Cross-checks a 2CBH family against a classic CBH family entry by entry.
//!
//! Usage: compare_formats --classic <file.cbh> --new <file.2cbh> [--limit N]
//! Prints a JSON summary. Metadata, names, move trees and annotations are compared;
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
    let mut marks: BTreeMap<&str, usize> = BTreeMap::new();
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
        // Classic CBC stores annotators as "Last,First"; 2CBH uses a player
        // entry, shown as "Last, First".
        let annotator = |v: &Option<String>| v.as_ref().map(|v| v.replace(", ", ","));
        let (ca, na) = (annotator(&s.annotator), annotator(&t.annotator));
        for (field, x, y) in [
            ("white", &s.white, &t.white),
            ("black", &s.black, &t.black),
            ("event", &s.event, &t.event),
            ("site", &s.site, &t.site),
            ("eco", &s.eco, &t.eco),
            ("annotator", &ca, &na),
            ("source", &s.source, &t.source),
            ("sourcePublisher", &s.source_publisher, &t.source_publisher),
            ("sourceDate", &s.source_date, &t.source_date),
            ("whiteTeam", &s.white_team, &t.white_team),
            ("blackTeam", &s.black_team, &t.black_team),
        ] {
            if x.is_some() {
                bump(&format!("{field}Present"));
            }
            if x == y {
                bump(&format!("{field}Match"));
            } else {
                bump(&format!("{field}Mismatch"));
                if failures.len() < 50 || std::env::var_os("COMPARE_ALL").is_some() {
                    failures.push(format!("{ordinal}: {field} {x:?} vs {y:?}"));
                }
            }
        }
        match (&x.game, &y.game) {
            (Some(g), Some(h)) => {
                nodes += h.nodes.len() - 1;
                if tree(g) == tree(h) {
                    bump("treesMatch");
                    bump(&format!(
                        "annotations:{:?}/{:?}",
                        x.annotations_status, y.annotations_status
                    ));
                    let same = g.nodes.len() == h.nodes.len()
                        && g.nodes.iter().zip(&h.nodes).all(|(a, b)| {
                            a.uci == b.uci
                                && a.comments == b.comments
                                && a.pre_comments == b.pre_comments
                                && a.nags == b.nags
                                && a.squares == b.squares
                                && a.arrows == b.arrows
                        });
                    for n in &h.nodes {
                        *marks.entry("comments").or_default() += n.comments.len();
                        *marks.entry("preComments").or_default() += n.pre_comments.len();
                        *marks.entry("nags").or_default() += n.nags.len();
                        *marks.entry("squares").or_default() += n.squares.len();
                        *marks.entry("arrows").or_default() += n.arrows.len();
                    }
                    if same {
                        bump("annotationsMatch");
                    } else {
                        bump("annotationsDiffer");
                        if failures.len() < 50 || std::env::var_os("COMPARE_ALL").is_some() {
                            failures.push(format!(
                                "{ordinal}: annotations differ {:?} / {:?}",
                                x.diagnostics.iter().map(|d| &d.code).collect::<Vec<_>>(),
                                y.diagnostics.iter().map(|d| &d.code).collect::<Vec<_>>()
                            ));
                        }
                    }
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
    failures.truncate(if std::env::var_os("COMPARE_ALL").is_some() {
        usize::MAX
    } else {
        50
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "compared": limit.min(count),
            "newEntries": b.info().entry_count,
            "classicEntries": a.info().entry_count,
            "counts": counts,
            "newMoveNodes": nodes,
            "newDiagnostics": diagnostics,
            "newAnnotationTotals": marks,
            "firstFailures": failures,
            "maxNewLoadMs": slowest,
            "elapsedS": started.elapsed().as_secs_f64(),
            "newBytesRead": b.bytes_read(),
        }))?
    );
    Ok(())
}
