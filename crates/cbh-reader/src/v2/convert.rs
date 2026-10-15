//! Converting games of any database into a 2CBH database (ChessBase's
//! "Convert to free 2CBH format", `CONVERT_TO_FREE_2CBH`). Each game is
//! loaded from the source and appended in batches; see docs/conversion.md.
use super::write::Writer;
use crate::{CbhError, DataStatus, Database, EntryKind};
use std::sync::atomic::{AtomicBool, Ordering};

/// Games appended per write; each batch is synced once.
pub const BATCH: usize = 200;

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertReport {
    /// Entries looked at.
    pub done: u64,
    /// Games written.
    pub written: u64,
    /// Of those, games kept marked deleted.
    pub deleted: u64,
    /// Games written without some annotations: not decoded, or not
    /// storable in 2CBH.
    pub incomplete: u64,
    /// Text entries, which the writer cannot store yet.
    pub texts: u64,
    /// Chess960 games, which 2CBH cannot hold yet.
    pub chess960: u64,
    /// Entries whose moves could not be read.
    pub unreadable: u64,
    pub cancelled: bool,
}

/// Appends the entries `ordinals` of `source` to `writer`, in order.
/// Stops between batches when `cancel` is set; `progress` gets the entries
/// done after each batch.
pub fn convert(
    source: &mut Database,
    ordinals: impl IntoIterator<Item = u64>,
    writer: &mut Writer,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&ConvertReport),
) -> Result<ConvertReport, CbhError> {
    let mut report = ConvertReport::default();
    let mut ordinals = ordinals.into_iter().peekable();
    while ordinals.peek().is_some() {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let mut games = Vec::with_capacity(BATCH);
        let mut deleted = Vec::with_capacity(BATCH);
        let mut partial = Vec::with_capacity(BATCH);
        for ordinal in ordinals.by_ref().take(BATCH) {
            report.done += 1;
            let loaded = source.load(ordinal)?;
            match (loaded.summary.kind, loaded.game) {
                (EntryKind::Text, _) => report.texts += 1,
                (_, Some(game)) if game.is_chess960() => report.chess960 += 1,
                (_, Some(game)) => {
                    partial.push(!matches!(
                        loaded.annotations_status,
                        DataStatus::Available | DataStatus::Missing
                    ));
                    deleted.push(loaded.summary.deleted);
                    games.push(game);
                }
                (_, None) => report.unreadable += 1,
            }
        }
        let written = writer.append_marked(&games, &deleted)?;
        report.written += written.len() as u64;
        report.deleted += deleted.iter().filter(|&&d| d).count() as u64;
        report.incomplete += written
            .iter()
            .zip(&partial)
            .filter(|((_, omitted), partial)| **partial || !omitted.notes.is_empty())
            .count() as u64;
        progress(&report);
    }
    Ok(report)
}

/// Bytes a 2CBH copy of `games` of `source`'s entries is expected to take,
/// from the sizes of the source's move and annotation files. The figures
/// were measured on the classic Mega Database (docs/conversion.md) and are
/// rounded up.
pub fn estimate(source: &Database, games: u64) -> u64 {
    let entries = source.info().entry_count.max(1);
    let size = |ext: &str| {
        source
            .source_paths()
            .iter()
            .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)))
            .and_then(|p| std::fs::metadata(p).ok())
            .map_or(0, |m| m.len())
    };
    let classic = matches!(source.info().format, crate::DatabaseFormat::Classic);
    let (moves, notes) = if classic {
        (size("cbg"), size("cba"))
    } else {
        (size("2cbg"), size("2cba"))
    };
    let share = |bytes: u64| (bytes as u128 * games as u128 / entries as u128) as u64;
    let (move_factor, note_factor) = if classic { MOVE_FACTOR } else { (1.0, 1.0) };
    let records = games * super::record::RECORD as u64;
    // Block framing of 2CBG and 2CBA: header, padding and slack per game;
    // a 2CBH source's sizes include it already.
    let framing = if classic { games * FRAMING } else { 0 };
    // 2LID keeps one group of slots per name id, so its size follows the
    // longest list: at most the source's own names, and about
    // `NAME_GROUPS_PER_GAME` new names per game.
    let groups = if classic {
        // Classic record strides (entities.rs); annotators share the
        // 2LID player list.
        let players = size("cbp") / 67 + size("cbc") / 62;
        players.max(size("cbt") / 99).max(size("cbs") / 68)
    } else {
        size("2lid").saturating_sub(super::write::LID_HEADER) / super::write::GROUP
    };
    let new_names = (games as f64 * NAME_GROUPS_PER_GAME).ceil() as u64;
    let names = new_names.min(groups) * super::write::GROUP;
    (share(moves) as f64 * move_factor) as u64
        + (share(notes) as f64 * note_factor) as u64
        + records
        + framing
        + names
}

/// 2CBG and 2CBA bytes per byte of classic CBG and CBA (measured, rounded
/// up; see docs/conversion.md).
const MOVE_FACTOR: (f64, f64) = (2.0, 2.0);
/// Name groups added per converted game (measured, rounded up).
const NAME_GROUPS_PER_GAME: f64 = 0.3;
/// Framing bytes of a game's two blocks, rounded up.
const FRAMING: u64 = 34 + 104 + 34 + 200;
