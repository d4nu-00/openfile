//! A bounded latest-date sample for the recovered playing-style report.
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
    /// Blitz=1, Rapid=2, Long=4, Simul=8. All four also admit unknown metadata.
    pub game_types: u8,
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
            game_types: 4,
        }
    }
}
impl StyleCriteria {
    pub fn validate(&self) -> Result<(), CbhError> {
        if self.player_ids.is_empty()
            || self.player_ids.len() > 16
            || !(1..=2000).contains(&self.sample_limit)
            || !(1..=15).contains(&self.game_types)
            || self
                .min_opponent_elo
                .is_some_and(|e| !(1..=4000).contains(&e))
            || self.year_from.is_some_and(|y| y == 0 || y > 4095)
            || self.year_to.is_some_and(|y| y == 0 || y > 4095)
            || self.year_from.zip(self.year_to).is_some_and(|(a, b)| a > b)
        {
            return Err(CbhError::InvalidQuery("Choose 1–16 player IDs, 1–2000 games, game types (1–15), an ordered year range (1–4095) and opponent rating (1–4000)".into()));
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
    pub stored_moves: u8,
    pub category: Option<u8>,
}
#[derive(Default, Debug)]
pub struct StyleSelection {
    pub games: Vec<SelectedGame>,
    pub eligible: u64,
    pub deleted: u64,
    pub filtered: u64,
    /// Native header predicate excludes stored full-move counts below five.
    pub too_short: u64,
    /// Among nondeleted identity matches, before optional filters.
    pub unknown_date: u64,
    pub unknown_opponent_elo: u64,
    pub unknown_result: u64,
    pub unknown_category: u64,
}
impl Database {
    /// Scans every header once, retaining only the latest K eligible entries.
    /// Unknown dates follow dated entries; equal dates use descending ordinal.
    /// If two selected identities oppose each other, White takes precedence;
    /// with a colour filter, that colour's identity is used instead.
    pub fn select_style_games(
        &mut self,
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
                if i > 0 && i % 256 == 0 {
                    progress(at + i as u64 - 1, count);
                }
                // Metadata lookup can read and decode a whole tournament row.
                // Stop before each record, rather than after the header batch.
                if cancel.load(Ordering::Relaxed) {
                    return Ok(None);
                }
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
                let category = match f.tournament {
                    Some(id) => match self.tournament_metadata(id) {
                        Ok(t) => game_category(t.flags, t.event.text.as_deref()),
                        Err(e @ CbhError::SourceChanged) => return Err(e),
                        Err(_) => None,
                    },
                    None => None,
                };
                out.unknown_category += u64::from(category.is_none());
                if f.moves < 5 {
                    out.too_short += 1;
                    continue;
                }
                if p.white.is_some_and(|want| (want && !w) || (!want && !b))
                    || p.year_from.is_some_and(|from| year == 0 || year < from)
                    || p.year_to.is_some_and(|to| year == 0 || year > to)
                    || p.min_opponent_elo.is_some_and(|min| elo == 0 || elo < min)
                    || category.map_or(p.game_types != 15, |bit| p.game_types & bit == 0)
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
                    f.moves,
                    category,
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
                |Reverse((date, ordinal, white, opponent, result, stored_moves, category))| {
                    SelectedGame {
                        date,
                        ordinal,
                        white,
                        opponent,
                        result,
                        stored_moves,
                        category,
                    }
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

/// Recovered category predicates, in native precedence order. A name alone
/// cannot rule out overriding flags, so incomplete metadata remains unknown.
/// Admitting unknown metadata for the explicit all-types mask is an OpenFile
/// policy, rather than a claim about native malformed-record handling.
fn game_category(flags: Option<u16>, event: Option<&str>) -> Option<u8> {
    let flags = flags?;
    let name = event.unwrap_or_default().to_ascii_lowercase();
    Some(if flags & 31 == 7 || name.contains(" sim") {
        8
    } else if flags & 0xe0 == 0x20 || name.contains("blitz") || name.contains("bullet") {
        1
    } else if flags & 0xe0 == 0x40 || name.contains("rapid") {
        2
    } else {
        4
    })
}
