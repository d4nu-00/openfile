//! Writing the games of a database as PGN text (the list's Export PGN; see
//! docs/game-list.md).
use crate::{CbhError, Database};
use std::{
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
};

/// What an export did, in games.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportSummary {
    pub written: u64,
    /// Entries that are not games, whose moves could not be read, or that
    /// PGN could not write.
    pub skipped: u64,
    /// Games written without critical-position marks or training
    /// questions, which PGN cannot hold.
    pub stripped: u64,
    /// Stopped by `cancel`; what was written is partial.
    pub cancelled: bool,
}

/// Writes the games at `ordinals` to `out` in the given order, each followed
/// by a blank line. `progress` receives the games handled and the total
/// after each; `cancel` is checked between games.
pub fn write_games(
    db: &mut Database,
    ordinals: &[u64],
    out: &mut impl Write,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<ExportSummary, CbhError> {
    let total = ordinals.len() as u64;
    let mut summary = ExportSummary::default();
    for (done, &ordinal) in ordinals.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            break;
        }
        match db.load(ordinal)?.game {
            None => summary.skipped += 1,
            Some(mut game) => {
                let mut stripped = false;
                for node in &mut game.nodes {
                    stripped |= node.critical.take().is_some() | node.training.take().is_some();
                }
                match chess_core::write_pgn(&game) {
                    Ok(text) => {
                        out.write_all(text.as_bytes())?;
                        out.write_all(b"\n\n")?;
                        summary.written += 1;
                        summary.stripped += u64::from(stripped);
                    }
                    Err(_) => summary.skipped += 1,
                }
            }
        }
        progress(done as u64 + 1, total);
    }
    Ok(summary)
}
