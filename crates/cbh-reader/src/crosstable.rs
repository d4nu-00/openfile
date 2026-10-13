//! Tournament crosstables (ChessBase's Cross Table, `DBTOOLSCROSSTABLE`, and
//! Show Crosstable, `SHOWCROSSTABLE`). See docs/game-list.md.
use crate::{CbhError, Database, EntryKind, header::RawEntry, search::GameFilter, tree::DP};
use std::{collections::HashMap, sync::atomic::AtomicBool};

/// Games read for one table at most.
pub const MAX_GAMES: usize = 5_000;
/// Players shown at most.
pub const MAX_PLAYERS: usize = 500;
/// Round robins have at most this many players; larger fields are Swiss.
const MAX_ROUND_ROBIN: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    /// One column per opponent.
    RoundRobin,
    /// One column per round.
    Swiss,
}

/// One game in a cell, from the row player's side.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossGame {
    pub ordinal: u64,
    /// "1", "½" or "0"; forfeits are "+", "=" and "-".
    pub mark: String,
    pub white: bool,
    pub round: u16,
    /// The opponent's row (from 0).
    pub opponent: usize,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossPlayer {
    pub id: u32,
    pub name: Option<String>,
    /// The rating of the player's first rated game in round order.
    pub elo: Option<u16>,
    pub points: f64,
    pub games: u32,
    pub wins: u32,
    pub black_games: u32,
    pub black_wins: u32,
    /// Sonneborn-Berger: opponents' points, weighted by the score against them.
    pub sonneborn_berger: f64,
    /// Buchholz: opponents' points, once per game.
    pub buchholz: f64,
    /// Opponents' average rating plus FIDE's difference for the score,
    /// over rated opponents; forfeits do not count.
    pub performance: Option<u32>,
    /// Round robin: one cell per opponent row; Swiss: one cell per round.
    pub cells: Vec<Vec<CrossGame>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Crosstable {
    pub event: Option<String>,
    pub site: Option<String>,
    pub layout: Layout,
    /// Highest round number; 0 when the games have none.
    pub rounds: u16,
    /// Players in ranking order.
    pub players: Vec<CrossPlayer>,
    /// Games with a result.
    pub games: u32,
    /// Games without a result (lines, "0-0", unknown codes); not counted.
    pub unscored: u32,
    /// The tournament had more than [`MAX_GAMES`] games or
    /// [`MAX_PLAYERS`] players; the table covers the first ones.
    pub truncated: bool,
    pub average_elo: Option<u32>,
    /// FIDE category from the average rating (category 1 is 2251–2275).
    pub category: Option<u32>,
}

/// White's score in half points and the mark for White, or None for a game
/// without a result.
fn score(code: u8) -> Option<(u32, &'static str, &'static str)> {
    Some(match code {
        2 => (2, "1", "0"),
        1 => (1, "½", "½"),
        0 => (0, "0", "1"),
        6 => (2, "+", "-"),
        5 => (1, "=", "="),
        4 => (0, "-", "+"),
        _ => return None,
    })
}

fn round_of(r: &RawEntry) -> (u16, u16) {
    let Some(round) = &r.round else { return (0, 0) };
    let mut parts = round.split('.');
    let main = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let sub = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (main, sub)
}

fn performance(opponents: u64, count: u32, half_points: u32) -> Option<u32> {
    if count == 0 {
        return None;
    }
    let average = opponents as f64 / count as f64;
    let percent = (half_points as f64 * 50.0 / count as f64).round() as i64;
    let dp = if percent >= 50 {
        i64::from(DP[(percent - 50).min(50) as usize])
    } else {
        -i64::from(DP[(50 - percent).min(50) as usize])
    };
    Some((average.round() as i64 + dp).max(0) as u32)
}

impl Database {
    /// The crosstable of a tournament, from its games in this database.
    /// `layout` forces a layout; otherwise fields of at most 24 players that
    /// played at least half of all pairings are round robins.
    pub fn crosstable(
        &mut self,
        tournament_id: u32,
        layout: Option<Layout>,
        cancel: &AtomicBool,
    ) -> Result<Option<Crosstable>, CbhError> {
        let found = self.search(
            &GameFilter {
                tournament_id: Some(tournament_id),
                ..Default::default()
            },
            MAX_GAMES,
            cancel,
            |_| {},
        )?;
        if found.cancelled {
            return Ok(None);
        }
        let mut truncated = found.truncated;
        let mut games = vec![];
        for &o in &found.ordinals {
            let r = self.raw(o)?;
            if r.kind == EntryKind::Game && !r.deleted {
                games.push(r);
            }
        }
        games.sort_by_key(|r| (round_of(r), r.ordinal));
        let (event, site) = self.tournament_name(tournament_id)?;

        // Players in order of appearance, then the scored games between them.
        let mut index: HashMap<u32, usize> = HashMap::new();
        let mut players: Vec<CrossPlayer> = vec![];
        let mut scored = vec![];
        let mut unscored = 0;
        for r in &games {
            let (Some(w), Some(b)) = (r.white_id, r.black_id) else {
                unscored += 1;
                continue;
            };
            let Some(s) = r.result_code.and_then(score) else {
                unscored += 1;
                continue;
            };
            let mut seat = |id: u32, elo: Option<u16>| -> Option<usize> {
                if let Some(&i) = index.get(&id) {
                    if players[i].elo.is_none() {
                        players[i].elo = elo;
                    }
                    return Some(i);
                }
                if players.len() == MAX_PLAYERS {
                    return None;
                }
                index.insert(id, players.len());
                players.push(CrossPlayer {
                    id,
                    name: None,
                    elo,
                    points: 0.0,
                    games: 0,
                    wins: 0,
                    black_games: 0,
                    black_wins: 0,
                    sonneborn_berger: 0.0,
                    buchholz: 0.0,
                    performance: None,
                    cells: vec![],
                });
                Some(players.len() - 1)
            };
            let (Some(wi), Some(bi)) = (seat(w, r.white_elo), seat(b, r.black_elo)) else {
                truncated = true;
                continue;
            };
            scored.push((r, wi, bi, s));
        }
        let mut half = vec![0u32; players.len()];
        for &(_, wi, bi, (white, _, _)) in &scored {
            half[wi] += white;
            half[bi] += 2 - white;
        }
        let pairs: std::collections::HashSet<(usize, usize)> = scored
            .iter()
            .map(|&(_, w, b, _)| (w.min(b), w.max(b)))
            .collect();
        let n = players.len();
        let layout = layout.unwrap_or(
            if n <= MAX_ROUND_ROBIN && pairs.len() * 4 >= n * n.saturating_sub(1) {
                Layout::RoundRobin
            } else {
                Layout::Swiss
            },
        );
        // Statistics and tiebreaks.
        let mut rated = vec![(0u64, 0u32, 0u32); n];
        for &(r, wi, bi, (white, _, _)) in &scored {
            let forfeit = r.result_code.is_some_and(|c| c >= 4);
            for (me, other, mine, black) in [(wi, bi, white, false), (bi, wi, 2 - white, true)] {
                let p = &mut players[me];
                p.games += 1;
                if mine == 2 {
                    p.wins += 1;
                }
                if black {
                    p.black_games += 1;
                    if mine == 2 {
                        p.black_wins += 1;
                    }
                }
                let opponent = half[other] as f64 / 2.0;
                p.buchholz += opponent;
                p.sonneborn_berger += opponent * mine as f64 / 2.0;
                let elo = if black { r.white_elo } else { r.black_elo };
                if let (false, Some(e)) = (forfeit, elo.filter(|e| *e > 0)) {
                    rated[me].0 += u64::from(e);
                    rated[me].1 += 1;
                    rated[me].2 += mine;
                }
            }
        }
        for (i, p) in players.iter_mut().enumerate() {
            p.points = half[i] as f64 / 2.0;
            p.performance = performance(rated[i].0, rated[i].1, rated[i].2);
        }
        for p in &mut players {
            p.name = self.player_name(p.id)?;
        }
        // Ranking: points, then the layout's tiebreak, then wins and name.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            let (pa, pb) = (&players[a], &players[b]);
            let tiebreak = |p: &CrossPlayer| match layout {
                Layout::RoundRobin => (p.sonneborn_berger, p.buchholz),
                Layout::Swiss => (p.buchholz, p.sonneborn_berger),
            };
            pb.points
                .total_cmp(&pa.points)
                .then(tiebreak(pb).0.total_cmp(&tiebreak(pa).0))
                .then(tiebreak(pb).1.total_cmp(&tiebreak(pa).1))
                .then(pb.wins.cmp(&pa.wins))
                .then(pa.name.cmp(&pb.name))
        });
        let mut row = vec![0; n];
        for (place, &i) in order.iter().enumerate() {
            row[i] = place;
        }
        let rounds = games.iter().map(|r| round_of(r).0).max().unwrap_or(0);
        let columns = match layout {
            Layout::RoundRobin => n,
            Layout::Swiss => rounds.max(1) as usize,
        };
        for p in &mut players {
            p.cells = vec![vec![]; columns];
        }
        for &(r, wi, bi, (_, white_mark, black_mark)) in &scored {
            let round = round_of(r).0;
            for (me, other, mark, white) in
                [(wi, bi, white_mark, true), (bi, wi, black_mark, false)]
            {
                let column = match layout {
                    Layout::RoundRobin => row[other],
                    Layout::Swiss => (round.max(1) - 1) as usize,
                };
                if let Some(cell) = players[me].cells.get_mut(column) {
                    cell.push(CrossGame {
                        ordinal: r.ordinal,
                        mark: mark.into(),
                        white,
                        round,
                        opponent: row[other],
                    });
                }
            }
        }
        let ranked: Vec<CrossPlayer> = order.iter().map(|&i| players[i].clone()).collect();
        let ratings: Vec<u32> = ranked
            .iter()
            .filter_map(|p| p.elo.filter(|e| *e > 0).map(u32::from))
            .collect();
        let average_elo = (!ratings.is_empty())
            .then(|| (ratings.iter().sum::<u32>() as f64 / ratings.len() as f64).round() as u32);
        let category = average_elo
            .filter(|a| *a >= 2251)
            .map(|a| (a - 2251) / 25 + 1);
        Ok(Some(Crosstable {
            event,
            site,
            layout,
            rounds,
            players: ranked,
            games: scored.len() as u32,
            unscored,
            truncated,
            average_elo,
            category,
        }))
    }
}
