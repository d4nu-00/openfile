//! Disposable synthetic fixtures for actual desktop recovery acceptance.
use cbh_reader::{
    Database, ReadLimits,
    v2::write::{WRITTEN, Writer},
};
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(
        args.get(1)
            .ok_or("usage: recovery_fixture NEW_DIRECTORY [slow]")?,
    );
    std::fs::create_dir(root)?;
    let original = root.join("Original.2cbh");
    let mut writer = Writer::create(&original)?;
    let mut games = vec![];
    for i in 1..=80 {
        let result = ["1-0", "0-1", "1/2-1/2", "*"][i % 4];
        let text = format!(
            "[White \"Study {i}\"]\n[Black \"Practice\"]\n[Event \"Recovery acceptance\"]\n[Site \"Synthetic\"]\n[Date \"2026.10.22\"]\n[Round \"{}.1\"]\n[WhiteElo \"2200\"]\n[BlackElo \"2100\"]\n[Result \"{result}\"]\n\n1. e4 {{Centre [%csl Gd4] [%cal Ge2e4]}} e5 (1... c5 2. Nf3) 2. Nf3 Nc6 {result}",
            i % 9 + 1
        );
        games.push(chess_core::parse_pgn(text.as_bytes(), &Default::default())?.remove(0));
    }
    writer.append_all(&games)?;
    writer.set_deleted(20, true)?;
    drop(writer);
    let damaged = root.join("Damaged.2cbh");
    for ext in WRITTEN {
        std::fs::copy(original.with_extension(ext), damaged.with_extension(ext))?;
    }
    let mut records = std::fs::read(&damaged)?;
    for (entry, field, ext) in [(3, 8, "2cbg"), (7, 16, "2cba")] {
        let at =
            u64::from_le_bytes(records[entry * 192 + field..entry * 192 + field + 8].try_into()?)
                as usize;
        let path = damaged.with_extension(ext);
        let mut bytes = std::fs::read(&path)?;
        bytes[at + 26] ^= 1;
        std::fs::write(path, bytes)?;
    }
    records[11 * 192 + 24..11 * 192 + 32].copy_from_slice(&99999u64.to_le_bytes());
    records[80 * 192 + 2..80 * 192 + 4].copy_from_slice(&0x0301u16.to_le_bytes());
    records[16..24].copy_from_slice(&99999u64.to_le_bytes());
    std::fs::write(&damaged, records)?;
    if args.get(2).is_some_and(|s| s == "slow") {
        let mut writer = Writer::create(&root.join("Stop test.2cbh"))?;
        let batch = vec![games[0].clone(); 1000];
        for _ in 0..100 {
            writer.append_all(&batch)?;
        }
    }
    let mut db = Database::open(&original, ReadLimits::default())?;
    assert!(db.load(1)?.game.is_some());
    println!(
        "{}; expected recovered=76, corrupt=3, unsupported=1; excluded original entries 3,7,11,80",
        root.display()
    );
    Ok(())
}
