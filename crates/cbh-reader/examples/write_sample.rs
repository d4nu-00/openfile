//! Copies a deterministic sample of games from one database into a new 2CBH
//! database, reads the copy back and compares it with the source.
//!
//! Usage: write_sample --source <file.cbh|file.2cbh> --out <new.2cbh>
//!        [--count N] [--stride S]
//!
//! Entries 1, 1+S, 1+2S, ... are copied until N games are written. The output
//! must not exist yet; the source is opened read-only. Afterwards a few
//! entries are replaced and marked deleted to exercise those operations.
//! Prints a JSON summary.
use cbh_reader::{
    DataStatus, Database, EntryKind, ReadLimits,
    source::SourceFile,
    v2::{block, record, write::Writer},
};
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

fn same_nodes(a: &chess_core::Game, b: &chess_core::Game) -> Result<(), String> {
    if a.start_fen != b.start_fen {
        return Err("start position".into());
    }
    if a.nodes.len() != b.nodes.len() {
        return Err(format!("{} vs {} nodes", a.nodes.len(), b.nodes.len()));
    }
    for (x, y) in a.nodes.iter().zip(&b.nodes) {
        let field = if x.uci != y.uci || x.children != y.children {
            "tree"
        } else if x.comments != y.comments {
            "comments"
        } else if x.pre_comments != y.pre_comments {
            "text before"
        } else if x.nags != y.nags {
            "nags"
        } else if x.squares != y.squares {
            "squares"
        } else if x.arrows != y.arrows {
            "arrows"
        } else {
            continue;
        };
        return Err(format!("{field} differ at node {}", x.id));
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let source = PathBuf::from(value("--source").ok_or("--source is required")?);
    let out = PathBuf::from(value("--out").ok_or("--out is required")?);
    let count: u64 = value("--count")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(2000);
    let stride: u64 = value("--stride")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(1);
    let started = Instant::now();
    let mut src = Database::open(&source, ReadLimits::default())?;
    let mut writer = Writer::create(&out)?;
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut failures: Vec<String> = vec![];
    // (source ordinal, copy ordinal, summary, game); written in batches.
    let mut copied = vec![];
    let mut pending: Vec<(u64, cbh_reader::EntrySummary, chess_core::Game)> = vec![];
    let mut flush = |pending: &mut Vec<(u64, cbh_reader::EntrySummary, chess_core::Game)>,
                     copied: &mut Vec<_>,
                     failures: &mut Vec<String>,
                     counts: &mut BTreeMap<String, u64>|
     -> Result<(), Box<dyn std::error::Error>> {
        let games: Vec<_> = pending.iter().map(|p| p.2.clone()).collect();
        let written = writer.append_all(&games)?;
        for ((from, summary, game), (to, omitted)) in pending.drain(..).zip(written) {
            if !omitted.notes.is_empty() {
                *counts.entry("omittedOnWrite".into()).or_default() += 1;
                if failures.len() < 40 {
                    failures.push(format!("{from}: omitted {:?}", omitted.notes));
                }
            }
            copied.push((from, to, summary, game));
        }
        Ok(())
    };
    let mut ordinal = 1;
    while ((copied.len() + pending.len()) as u64) < count && ordinal <= src.info().entry_count {
        let loaded = src.load(ordinal)?;
        if let (EntryKind::Game, Some(game)) = (&loaded.summary.kind, loaded.game) {
            if loaded.annotations_status == DataStatus::Partial {
                *counts.entry("sourcePartialAnnotations".into()).or_default() += 1;
            }
            pending.push((ordinal, loaded.summary, game));
            if pending.len() == 500 {
                flush(&mut pending, &mut copied, &mut failures, &mut counts)?;
            }
        } else {
            *counts.entry("skippedNonGame".into()).or_default() += 1;
        }
        ordinal += stride;
    }
    flush(&mut pending, &mut copied, &mut failures, &mut counts)?;
    drop(flush);
    let write_s = started.elapsed().as_secs_f64();
    let mut bump = |k: &str| *counts.entry(k.to_string()).or_default() += 1;

    // Read the copy back through the ordinary reader.
    let mut copy = Database::open(&out, ReadLimits::default())?;
    let source_moves = (src.info().format == cbh_reader::DatabaseFormat::New)
        .then(|| SourceFile::open(&source.with_extension("2cbg")))
        .transpose()?;
    let (src_header, out_header, out_moves) = (
        SourceFile::open(&source)?,
        SourceFile::open(&out)?,
        SourceFile::open(&out.with_extension("2cbg"))?,
    );
    for (from, to, s, game) in &copied {
        let back = copy.load(*to)?;
        let t = &back.summary;
        for (field, x, y) in [
            ("white", &s.white, &t.white),
            ("black", &s.black, &t.black),
            ("event", &s.event, &t.event),
            ("site", &s.site, &t.site),
            ("date", &s.date, &t.date),
            ("round", &s.round, &t.round),
            ("eco", &s.eco, &t.eco),
            ("source", &s.source, &t.source),
            ("sourcePublisher", &s.source_publisher, &t.source_publisher),
            ("sourceDate", &s.source_date, &t.source_date),
        ] {
            if x != y {
                bump(&format!("{field}Mismatch"));
                if failures.len() < 40 {
                    failures.push(format!("{from}->{to}: {field} {x:?} vs {y:?}"));
                }
            }
        }
        let norm = |v: &Option<String>| v.as_ref().map(|v| v.replace(", ", ","));
        if norm(&s.annotator) != norm(&t.annotator) {
            bump("annotatorMismatch");
        }
        let pgn = |c: Option<u8>| c.and_then(cbh_reader::header::pgn_result);
        if pgn(s.result_code) != pgn(t.result_code)
            || s.white_elo != t.white_elo
            || s.black_elo != t.black_elo
        {
            bump("resultOrEloMismatch");
            if failures.len() < 40 {
                failures.push(format!(
                    "{from}->{to}: result/elo {:?} {:?}",
                    s.result, t.result
                ));
            }
        }
        match &back.game {
            Some(read) => match same_nodes(game, read) {
                Ok(()) => bump("gamesMatch"),
                Err(e) => {
                    bump("gamesDiffer");
                    if failures.len() < 40 {
                        failures.push(format!("{from}->{to}: {e}"));
                    }
                }
            },
            None => {
                bump("notReadBack");
                failures.push(format!("{from}->{to}: {:?}", back.diagnostics));
            }
        }
        if back.annotations_status == DataStatus::Partial {
            bump("copyPartialAnnotations");
        }
        // Byte-compare the 2CBG payload with the source's, when it has one.
        if let Some(moves) = &source_moves {
            let a = record::parse_record(
                &src_header.read_exact_range(from * record::RECORD as u64, record::RECORD)?,
                *from,
            )?;
            let b = record::parse_record(
                &out_header.read_exact_range(to * record::RECORD as u64, record::RECORD)?,
                *to,
            )?;
            let limit = 1 << 24;
            let x = block::read_block(moves, a.moves_offset.unwrap(), limit)?;
            let y = block::read_block(&out_moves, b.moves_offset.unwrap(), limit)?;
            bump(if x.payload == y.payload {
                "movesBytesIdentical"
            } else {
                "movesBytesDiffer"
            });
        }
    }

    // Replace, delete and undelete a few entries, then read them back.
    let n = copied.len();
    if n >= 4 {
        let mut writer = Writer::open(&out)?;
        for i in [0, n / 2] {
            let (_, to, _, _) = &copied[i];
            let (_, _, _, donor) = &copied[n - 1 - i];
            writer.replace(*to, donor)?;
            writer.set_deleted(*to, true)?;
        }
        writer.set_deleted(copied[0].1, false)?;
        let mut copy = Database::open(&out, ReadLimits::default())?;
        for (i, deleted) in [(0, false), (n / 2, true)] {
            let back = copy.load(copied[i].1)?;
            let donor = &copied[n - 1 - i].3;
            let ok = back.summary.deleted == deleted
                && back
                    .game
                    .as_ref()
                    .is_some_and(|g| same_nodes(donor, g).is_ok());
            bump(if ok {
                "replaceMatches"
            } else {
                "replaceDiffers"
            });
        }
        if copy.info().entry_count != n as u64 {
            bump("countChangedByReplace");
        }
    }
    let sizes: BTreeMap<String, u64> = ["2cbh", "2cbg", "2cba", "2lid"]
        .iter()
        .map(|e| {
            let len = std::fs::metadata(out.with_extension(e)).map_or(0, |m| m.len());
            (e.to_string(), len)
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "copied": n,
            "lastSourceEntry": ordinal - stride,
            "counts": counts,
            "firstFailures": failures,
            "writeSeconds": write_s,
            "totalSeconds": started.elapsed().as_secs_f64(),
            "outputBytes": sizes,
            "outputEntries": copy.info().entry_count,
            "outputDiagnostics": copy.info().diagnostics.iter().map(|d| &d.code).collect::<Vec<_>>(),
        }))?
    );
    Ok(())
}
