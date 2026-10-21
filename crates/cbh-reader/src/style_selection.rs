//! A bounded latest-date sample for independently defined playing-style measures.
//! Filters run before limiting. Memory is O(sample_limit), irrespective of the
//! number of entries belonging to the explicitly selected player identities.
use crate::{CbhError, Database, sort::fields};
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StyleCriteria {
    pub player_ids: Vec<u32>,
    pub white: Option<bool>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub min_opponent_elo: Option<u16>,
    pub sample_limit: usize,
}
impl Default for StyleCriteria {
    fn default() -> Self {
        Self {
            player_ids: vec![],
            white: None,
            year_from: None,
            year_to: None,
            min_opponent_elo: None,
            sample_limit: 500,
        }
    }
}
impl StyleCriteria {
    pub fn validate(&self) -> Result<(), CbhError> {
        if self.player_ids.is_empty()
            || self.player_ids.len() > 16
            || !(1..=2000).contains(&self.sample_limit)
            || self
                .min_opponent_elo
                .is_some_and(|e| !(1..=4000).contains(&e))
            || self.year_from.is_some_and(|y| y == 0 || y > 4095)
            || self.year_to.is_some_and(|y| y == 0 || y > 4095)
            || self.year_from.zip(self.year_to).is_some_and(|(a, b)| a > b)
        {
            return Err(CbhError::InvalidQuery("Choose 1–16 player IDs, 1–2000 games an ordered year range (1–4095) and opponent rating (1–4000)".into()));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct SelectedGame {
    pub ordinal: u64,
    pub white: bool,
    pub opponent: Option<u32>,
    pub date: u32,
    pub result: u8,
}
#[derive(Default, Debug)]
pub struct StyleSelection {
    pub games: Vec<SelectedGame>,
    pub eligible: u64,
    pub deleted: u64,
    pub filtered: u64,
    /// Among nondeleted identity matches, before optional filters.
    pub unknown_date: u64,
    pub unknown_opponent_elo: u64,
    pub unknown_result: u64,
}
impl Database {
    /// Scans every header once, retaining only the latest K eligible entries.
    /// Unknown dates follow dated entries; equal dates use descending ordinal.
    /// If two selected identities oppose each other, White takes precedence;
    /// with a colour filter, that colour's identity is used instead.
    pub fn select_style_games(
        &self,
        p: &StyleCriteria,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Option<StyleSelection>, CbhError> {
        p.validate()?;
        self.verify()?;
        let mut out = StyleSelection::default();
        let mut heap = BinaryHeap::new();
        let count = self.info().entry_count;
        let format = self.format();
        progress(0, count);
        let mut at = 1;
        while at <= count {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let n = 8192.min(count + 1 - at);
            for (i, record) in self
                .record_bytes(at, n)?
                .chunks_exact(format.record_size())
                .enumerate()
            {
                let f = fields(format, record);
                if !f.game {
                    continue;
                }
                let w = f.white.is_some_and(|id| p.player_ids.contains(&id));
                let b = f.black.is_some_and(|id| p.player_ids.contains(&id));
                if !w && !b {
                    continue;
                }
                if f.deleted {
                    out.deleted += 1;
                    continue;
                }
                // Pick the requested colour before determining its opponent.
                let white = match p.white {
                    Some(false) if b => false,
                    _ => w,
                };
                let elo = if white { f.black_elo } else { f.white_elo };
                let year = if crate::header::date(f.date).is_some() {
                    ((f.date >> 9) & 4095) as u16
                } else {
                    0
                };
                out.unknown_date += u64::from(year == 0);
                out.unknown_opponent_elo += u64::from(elo == 0);
                out.unknown_result += u64::from(!matches!(f.result,0..=2|4..=6));
                if p.white.is_some_and(|want| (want && !w) || (!want && !b))
                    || p.year_from.is_some_and(|from| year == 0 || year < from)
                    || p.year_to.is_some_and(|to| year == 0 || year > to)
                    || p.min_opponent_elo.is_some_and(|min| elo == 0 || elo < min)
                {
                    out.filtered += 1;
                    continue;
                }
                out.eligible += 1;
                let date = if year == 0 { 0 } else { f.date };
                heap.push(Reverse((
                    date,
                    at + i as u64,
                    white,
                    if white { f.black } else { f.white },
                    f.result,
                )));
                if heap.len() > p.sample_limit {
                    heap.pop();
                }
            }
            at += n;
            progress(at - 1, count);
        }
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        self.verify()?;
        out.games = heap
            .into_iter()
            .map(
                |Reverse((date, ordinal, white, opponent, result))| SelectedGame {
                    date,
                    ordinal,
                    white,
                    opponent,
                    result,
                },
            )
            .collect();
        out.games
            .sort_unstable_by_key(|g| Reverse((g.date, g.ordinal)));
        Ok(Some(out))
    }
    /// Fully verify source identities, lengths and modification times at the
    /// report publication/evidence boundary, including names and annotations.
    pub fn verify_sources(&self) -> Result<(), CbhError> {
        self.verify()
    }
}
