//! Games per year of a whole database, for the opening report's fashion
//! index (see docs/openings.md). Only the entry records are read.
use crate::{CbhError, Database, EntryKind};
use std::{
    collections::BTreeMap,
    sync::{Mutex, atomic::AtomicBool},
};

/// The year of a date as the reader writes it ("1934.05.14", "1946.??.??").
pub fn year_of(date: &str) -> Option<u16> {
    let y: u16 = date.split('.').next()?.parse().ok()?;
    (y > 0).then_some(y)
}

#[derive(Debug, Default)]
pub struct YearCounts {
    /// Game entries by year; games without a year are not counted.
    pub years: BTreeMap<u16, u64>,
    pub cancelled: bool,
}

impl Database {
    /// Counts the game entries of every year, in parallel, with progress
    /// and Stop. Deleted games count; text entries do not.
    pub fn year_counts(
        &self,
        cancel: &AtomicBool,
        progress: impl FnMut(u64, u64),
    ) -> Result<YearCounts, CbhError> {
        let all = Mutex::new(BTreeMap::<u16, u64>::new());
        let found = crate::scan::parallel(
            self,
            crate::scan::default_workers(),
            4_096,
            usize::MAX,
            cancel,
            progress,
            |_| Ok(()),
            |db, _, start, n, _out: &mut Vec<(u64, ())>| {
                let mut local = BTreeMap::<u16, u64>::new();
                for r in db.raw_chunk(start, n)? {
                    if r.kind != EntryKind::Game {
                        continue;
                    }
                    if let Some(y) = r.date.as_deref().and_then(year_of) {
                        *local.entry(y).or_default() += 1;
                    }
                }
                let mut all = all
                    .lock()
                    .map_err(|_| CbhError::Decode("Year counts unavailable".into()))?;
                for (y, n) in local {
                    *all.entry(y).or_default() += n;
                }
                Ok(())
            },
        )?;
        Ok(YearCounts {
            years: if found.cancelled {
                BTreeMap::new()
            } else {
                all.into_inner().unwrap_or_default()
            },
            cancelled: found.cancelled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::year_of;

    #[test]
    fn years_come_from_reader_dates() {
        assert_eq!(year_of("1934.05.14"), Some(1934));
        assert_eq!(year_of("1946.??.??"), Some(1946));
        assert_eq!(year_of("????.??.??"), None);
        assert_eq!(year_of(""), None);
    }
}
