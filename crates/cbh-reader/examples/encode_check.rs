//! Re-encodes every game of a 2CBH family and compares the result with the
//! stored 2CBG payload byte for byte.
//!
//! Usage: encode_check --new <file.2cbh> [--limit N]
//! Prints a JSON summary. Files are opened read-only.
use cbh_reader::{
    ReadLimits,
    source::SourceFile,
    v2::{block, encode, moves, record},
};
use std::{collections::BTreeMap, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let path = PathBuf::from(value("--new").ok_or("--new is required")?);
    let limit: u64 = value("--limit")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(u64::MAX);
    let header = SourceFile::open(&path)?;
    let moves_file = SourceFile::open(&path.with_extension("2cbg"))?;
    let head = header.read_exact_range(0, record::RECORD)?;
    let count = record::NewHeader::parse(&head, header.len())?.entry_count;
    let limits = ReadLimits::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut failures = vec![];
    for ordinal in 1..=count.min(limit) {
        let raw = record::parse_record(
            &header.read_exact_range(ordinal * record::RECORD as u64, record::RECORD)?,
            ordinal,
        )?;
        let Some(offset) = raw.moves_offset else {
            continue;
        };
        if raw.kind != cbh_reader::EntryKind::Game {
            continue;
        }
        let stored = block::read_block(&moves_file, offset, limits.max_game_bytes)?;
        let decoded = match moves::decode_moves(&stored.payload, &limits) {
            Ok(d) => d,
            Err(e) => {
                *counts.entry(format!("decode:{}", e.code())).or_default() += 1;
                continue;
            }
        };
        match encode::encode_moves(&decoded.game) {
            Ok(bytes) if bytes == stored.payload => {
                *counts.entry("identical".into()).or_default() += 1
            }
            Ok(bytes) => {
                *counts.entry("different".into()).or_default() += 1;
                if failures.len() < 20 {
                    let at = bytes.iter().zip(&stored.payload).position(|(a, b)| a != b);
                    if let Some(i) = at.filter(|_| std::env::var_os("ENCODE_DEBUG").is_some()) {
                        let word = |b: &[u8]| u16::from_le_bytes([b[i & !1], b[(i & !1) + 1]]);
                        let (mine, theirs) = (word(&bytes), word(&stored.payload));
                        let words = |b: &[u8]| -> Vec<String> {
                            b.chunks(2)
                                .map(|c| format!("{:04x}", u16::from_le_bytes([c[0], c[1]])))
                                .collect()
                        };
                        eprintln!("mine   {}", words(&bytes).join(" "));
                        eprintln!("stored {}", words(&stored.payload).join(" "));
                        eprintln!(
                            "{ordinal}: wrote {mine:#06x} {:?}, stored {theirs:#06x} {:?}",
                            moves::table().get(mine as usize),
                            moves::table().get(theirs as usize)
                        );
                    }
                    failures.push(format!(
                        "{ordinal}: {} vs {} bytes, first difference at {at:?}",
                        bytes.len(),
                        stored.payload.len()
                    ));
                }
            }
            Err(e) => {
                *counts.entry(format!("encode:{}", e.code())).or_default() += 1;
                if failures.len() < 20 {
                    failures.push(format!("{ordinal}: {e}"));
                }
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({ "counts": counts, "failures": failures })
        )?
    );
    Ok(())
}
