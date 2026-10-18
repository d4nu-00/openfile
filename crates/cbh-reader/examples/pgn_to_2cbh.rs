//! Imports a PGN file into a new 2CBH database, for example the showcase
//! database the README's screenshots are taken from.
//!
//! Usage: pgn_to_2cbh <games.pgn> <new.2cbh>
use cbh_reader::v2::write::Writer;
use chess_core::{GameLimits, parse_pgn};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [pgn, out] = args.as_slice() else {
        return Err("usage: pgn_to_2cbh <games.pgn> <new.2cbh>".into());
    };
    let games = parse_pgn(&std::fs::read(pgn)?, &GameLimits::default())?;
    let mut w = Writer::create(out)?;
    w.append_all(&games)?;
    println!("{} games written to {}", games.len(), out.display());
    Ok(())
}
