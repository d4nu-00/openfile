//! ECO classification of a whole database (ChessBase's Set ECO Codes,
//! `DBTOOLSSETECOCODE`, "Write ECO codes into games"). Every game's main
//! line is classified with the workbench's open table
//! (`chess_core::eco`); the codes that should change are returned for the
//! writer (`Writer::set_eco_many`). See docs/openings.md.
use crate::{CbhError, Database, EntryKind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

/// Databases of at most this many entries are classified. Only the
/// changes are kept (10 bytes each), so memory stays within ~200 MB.
pub const MAX_ECO_ENTRIES: u64 = 20_000_000;

#[derive(Clone, Copy, Debug, Default)]
pub struct EcoOptions {
    /// Leave games that have a code alone.
    pub only_missing: bool,
}

#[derive(Debug, Default)]
pub struct EcoReport {
    /// Game entries read.
    pub games: u64,
    /// New packed ECO words, by entry: (ordinal, word).
    pub changes: Vec<(u64, u16)>,
    /// Games whose code already has the computed main code; their
    /// ChessBase sub-classification (low seven bits) is kept.
    pub unchanged: u64,
    /// Games with another code, kept because of `only_missing`.
    pub kept: u64,
    /// Set-ups, Chess960 games and games the table does not know.
    pub unclassified: u64,
    /// Games whose moves could not be read.
    pub unreadable: u64,
    /// Games that had a code: how many the table gives the same main code.
    pub had_code: u64,
    pub agreed: u64,
    /// Coded games given another code: how many keep the letter, and the
    /// letter and tens digit (A0x, B9x, …).
    pub same_letter: u64,
    pub same_tens: u64,
    pub cancelled: bool,
}

/// A packed ECO word as text ("B90"), or `None` for no code.
pub fn eco_text(word: u16) -> Option<String> {
    crate::header::eco(word)
}

/// Counters shared by the scan's workers.
#[derive(Default)]
struct Counts {
    games: AtomicU64,
    unchanged: AtomicU64,
    kept: AtomicU64,
    unclassified: AtomicU64,
    unreadable: AtomicU64,
    had_code: AtomicU64,
    agreed: AtomicU64,
    same_letter: AtomicU64,
    same_tens: AtomicU64,
}

impl Database {
    /// Classifies every game entry, in parallel, with progress and Stop.
    /// Deleted games are classified too; text entries are skipped.
    pub fn classify_eco(
        &self,
        options: EcoOptions,
        cancel: &AtomicBool,
        progress: impl FnMut(u64, u64),
    ) -> Result<EcoReport, CbhError> {
        let count = self.info().entry_count;
        if count > MAX_ECO_ENTRIES {
            return Err(CbhError::Unsupported(format!(
                "ECO codes are set in databases of at most {MAX_ECO_ENTRIES} entries"
            )));
        }
        let c = Counts::default();
        let add = |n: &AtomicU64, v: bool| {
            if v {
                n.fetch_add(1, Relaxed);
            }
        };
        let found = crate::scan::parallel(
            self,
            crate::scan::default_workers(),
            256,
            usize::MAX,
            cancel,
            progress,
            |_| Ok(()),
            |db, _, start, n, out| {
                for r in db.raw_chunk(start, n)? {
                    if r.kind != EntryKind::Game {
                        continue;
                    }
                    add(&c.games, true);
                    let Some(game) = db.moves_of(&r)? else {
                        add(&c.unreadable, true);
                        continue;
                    };
                    let Some(found) = chess_core::eco::classify_game(&game) else {
                        add(&c.unclassified, true);
                        continue;
                    };
                    let word = crate::v2::write::pack_eco(&found.opening.eco);
                    let old = r.eco_raw.unwrap_or(0);
                    let had = old >> 7 != 0 && old >> 7 <= 500;
                    add(&c.had_code, had);
                    if had && old >> 7 != word >> 7 {
                        let (a, b) = ((old >> 7) - 1, (word >> 7) - 1);
                        add(&c.same_letter, a / 100 == b / 100);
                        add(&c.same_tens, a / 10 == b / 10);
                    }
                    if old >> 7 == word >> 7 {
                        add(&c.agreed, had);
                        add(&c.unchanged, true);
                    } else if had && options.only_missing {
                        add(&c.kept, true);
                    } else {
                        out.push((r.ordinal, word));
                    }
                }
                Ok(())
            },
        )?;
        if found.cancelled {
            return Ok(EcoReport {
                cancelled: true,
                ..Default::default()
            });
        }
        let get = |n: &AtomicU64| n.load(Relaxed);
        Ok(EcoReport {
            games: get(&c.games),
            changes: found.hits,
            unchanged: get(&c.unchanged),
            kept: get(&c.kept),
            unclassified: get(&c.unclassified),
            unreadable: get(&c.unreadable),
            had_code: get(&c.had_code),
            agreed: get(&c.agreed),
            same_letter: get(&c.same_letter),
            same_tens: get(&c.same_tens),
            cancelled: false,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn packed_codes_match_the_reader() {
        // The writer's packing and the reader's unpacking agree.
        for code in ["A00", "B90", "C42", "E99"] {
            let w = crate::v2::write::pack_eco(code);
            assert_eq!(crate::header::eco(w).as_deref(), Some(code));
        }
    }
}
