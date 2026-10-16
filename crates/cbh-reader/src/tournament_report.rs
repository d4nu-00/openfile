//! Tournament report (ChessBase's `TournamentReport`): the crosstable's games
//! round by round, with each round's date, the standings after it and the
//! tournament's results. See docs/tournament-report.md.
use crate::{CbhError, Database, crosstable::Crosstable, sort::SortOrder};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

/// Players listed in the standings after each round at most.
pub const MAX_STANDINGS: usize = 10;

/// A game of a round, with the players as rows of the crosstable.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportGame {
    pub ordinal: u64,
    pub white: usize,
    pub black: usize,
    /// "1-0", "1/2-1/2", "0-1", or a forfeit as ChessBase writes it.
    pub result: String,
    pub moves: u8,
}

/// A player's points after a round.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Standing {
    pub row: usize,
    pub points: f64,
    pub games: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRound {
    /// 0 for games without a round, listed last.
    pub round: u16,
    /// The earliest date of the round's games, as stored ("2020.01.15").
    pub date: Option<String>,
    pub games: Vec<ReportGame>,
    /// The leaders after the round, at most [`MAX_STANDINGS`].
    pub standings: Vec<Standing>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultStats {
    pub games: u32,
    pub white_wins: u32,
    pub draws: u32,
    pub black_wins: u32,
    pub forfeits: u32,
    /// White's score in percent, forfeits left out.
    pub white_score: Option<f64>,
    pub average_moves: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentReport {
    pub table: Crosstable,
    pub rounds: Vec<ReportRound>,
    pub stats: ResultStats,
}

/// White's half points for a scored result code, and whether it is a forfeit.
fn white_half(code: u8) -> Option<(u32, bool)> {
    Some(match code {
        2 => (2, false),
        1 => (1, false),
        0 => (0, false),
        6 => (2, true),
        5 => (1, true),
        4 => (0, true),
        _ => return None,
    })
}

impl Database {
    /// The report of a tournament, from its crosstable (see
    /// [`Database::crosstable_with`]). None when cancelled.
    pub fn tournament_report(
        &mut self,
        tournament_id: u32,
        order: Option<&mut SortOrder>,
        cancel: &AtomicBool,
    ) -> Result<Option<TournamentReport>, CbhError> {
        let Some(table) = self.crosstable_with(tournament_id, None, order, cancel)? else {
            return Ok(None);
        };
        // Each scored game once, from White's row.
        let mut by_round: BTreeMap<u16, Vec<(u64, usize, usize)>> = BTreeMap::new();
        for (row, p) in table.players.iter().enumerate() {
            for g in p.cells.iter().flatten().filter(|g| g.white) {
                by_round
                    .entry(g.round)
                    .or_default()
                    .push((g.ordinal, row, g.opponent));
            }
        }
        let mut stats = ResultStats::default();
        let mut moves_sum = 0u64;
        let mut white_half_sum = 0u32;
        let mut half = vec![0u32; table.players.len()];
        let mut played = vec![0u32; table.players.len()];
        let mut rounds = vec![];
        // Numbered rounds first, then games without a round.
        let order: Vec<u16> = by_round
            .keys()
            .copied()
            .filter(|r| *r > 0)
            .chain(by_round.contains_key(&0).then_some(0))
            .collect();
        for round in order {
            let mut list = by_round.remove(&round).unwrap_or_default();
            list.sort_unstable();
            let mut games = vec![];
            let mut date: Option<String> = None;
            for (ordinal, white, black) in list {
                let r = self.raw(ordinal)?;
                let Some((wh, forfeit)) = r.result_code.and_then(white_half) else {
                    continue;
                };
                half[white] += wh;
                half[black] += 2 - wh;
                played[white] += 1;
                played[black] += 1;
                stats.games += 1;
                if forfeit {
                    stats.forfeits += 1;
                } else {
                    match wh {
                        2 => stats.white_wins += 1,
                        1 => stats.draws += 1,
                        _ => stats.black_wins += 1,
                    }
                    white_half_sum += wh;
                    moves_sum += u64::from(r.move_count);
                }
                if let Some(d) = r.date.as_deref().filter(|d| !d.starts_with('?'))
                    && date.as_deref().is_none_or(|old| d < old)
                {
                    date = Some(d.to_string());
                }
                games.push(ReportGame {
                    ordinal,
                    white,
                    black,
                    result: r.result.clone().unwrap_or_else(|| "*".into()),
                    moves: r.move_count,
                });
            }
            let mut standings: Vec<Standing> = (0..table.players.len())
                .filter(|&row| played[row] > 0)
                .map(|row| Standing {
                    row,
                    points: f64::from(half[row]) / 2.0,
                    games: played[row],
                })
                .collect();
            standings.sort_by(|a, b| b.points.total_cmp(&a.points).then(a.row.cmp(&b.row)));
            standings.truncate(MAX_STANDINGS);
            rounds.push(ReportRound {
                round,
                date,
                games,
                standings,
            });
        }
        let normal = stats.white_wins + stats.draws + stats.black_wins;
        if normal > 0 {
            stats.white_score = Some(f64::from(white_half_sum) * 50.0 / f64::from(normal));
            stats.average_moves = Some((moves_sum / u64::from(normal)) as u32);
        }
        Ok(Some(TournamentReport {
            table,
            rounds,
            stats,
        }))
    }
}
