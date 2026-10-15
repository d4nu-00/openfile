//! Player dossier (ChessBase's `PLAYERDOSSIER`): the statistics ChessBase's
//! dossier writes about a player, from the player's games. See
//! docs/player-dossier.md.
//!
//! ChessBase counts only games with a normal result (1-0, ½-½, 0-1) here;
//! the band tables are its own (`.rdata` `0x1411f7880`, `0x1411f78b0`,
//! `0x1411f78d0`).
use crate::{CbhError, Database, player_stats::PlayerGame, prepare::map_games, tree::DP};
use std::sync::atomic::AtomicBool;

/// The score bands (`0x140839cf0`): the player's score less 55 % with White
/// or 45 % with Black, in percentage points.
pub const SCORE_BANDS: [i32; 8] = [-100, -20, -8, -2, 2, 8, 20, 100];
pub const SCORE_WORDS: [&str; 7] = [
    "miserably",
    "badly",
    "below average",
    "averagely",
    "above average",
    "well",
    "excellently",
];
/// Average length in moves (`0x140838f90`).
pub const LENGTH_BANDS: [i32; 8] = [0, 25, 32, 38, 43, 48, 55, 999];
pub const LENGTH_WORDS: [&str; 7] = [
    "very short",
    "short",
    "shorter than average",
    "of average length",
    "longer than average",
    "long",
    "very long",
];
/// The share of draws in percent (`0x140838af0`).
pub const DRAW_BANDS: [i32; 7] = [0, 20, 28, 34, 38, 47, 100];
pub const DRAW_WORDS: [&str; 6] = [
    "very low",
    "low",
    "lower than average",
    "average",
    "higher than average",
    "high",
];
/// A draw shorter than this many moves is a quick draw (`0x140838a30`).
pub const QUICK_DRAW_MOVES: u8 = 20;
/// Games whose last position is read for mates at most: the latest.
pub const MAX_MATE_GAMES: usize = 20_000;

/// The band of `bands` holding `value`, as ChessBase looks it up: `low <=
/// value < high`; None outside the table.
pub fn band(bands: &[i32], value: i32) -> Option<usize> {
    bands.windows(2).position(|w| w[0] <= value && value < w[1])
}

/// What ChessBase tallies for a group of games (`0x140838a30`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Tally {
    white_wins: u32,
    draws: u32,
    black_wins: u32,
    white_win_moves: u32,
    draw_moves: u32,
    black_win_moves: u32,
    quick_draws: u32,
}

/// Statistics of the player's games with one colour, or all of them.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColourStats {
    /// Games with a normal result.
    pub games: u32,
    pub white_wins: u32,
    pub draws: u32,
    pub black_wins: u32,
    /// The player's score in percent, rounded as ChessBase does; with one
    /// colour only.
    pub score: Option<u32>,
    /// Index into [`SCORE_WORDS`].
    pub score_band: Option<usize>,
    /// The rated opponents' average and the performance against them.
    pub opponents_average: Option<u32>,
    pub performance: Option<u32>,
    /// Average moves of White's wins, the draws and Black's wins, with
    /// their indexes into [`LENGTH_WORDS`].
    pub white_win_moves: Option<u32>,
    pub white_win_band: Option<usize>,
    pub draw_moves: Option<u32>,
    pub draw_band: Option<usize>,
    pub black_win_moves: Option<u32>,
    pub black_win_band: Option<usize>,
    /// Draws and quick draws in percent of the games, and the draws' index
    /// into [`DRAW_WORDS`].
    pub draw_percent: Option<u32>,
    pub draw_quote_band: Option<usize>,
    pub quick_draw_percent: Option<u32>,
}

/// Games against opponents rated `from .. from + 50`.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RatingBand {
    pub from: u16,
    pub games: u32,
    pub points: f64,
    pub percent: u32,
}

fn percent(part: u32, whole: u32) -> Option<u32> {
    (whole > 0).then(|| (part * 100 + whole / 2) / whole)
}

/// The colour statistics of `games`: `white` Some for one colour.
pub fn colour_stats(games: &[&PlayerGame], white: Option<bool>) -> ColourStats {
    let mut t = Tally::default();
    let (mut opponents, mut rated) = (0u64, 0u32);
    for g in games {
        let moves = u32::from(g.moves);
        match g.result {
            2 => {
                t.white_wins += 1;
                t.white_win_moves += moves;
            }
            1 => {
                t.draws += 1;
                t.draw_moves += moves;
                if g.moves < QUICK_DRAW_MOVES {
                    t.quick_draws += 1;
                }
            }
            0 => {
                t.black_wins += 1;
                t.black_win_moves += moves;
            }
            _ => continue,
        }
        if g.opponent_elo > 0 {
            opponents += u64::from(g.opponent_elo);
            rated += 1;
        }
    }
    let games = t.white_wins + t.draws + t.black_wins;
    let average = |sum: u32, n: u32| (n > 0).then(|| sum / n);
    let length = |avg: Option<u32>| avg.and_then(|a| band(&LENGTH_BANDS, a as i32));
    // `0x14083a080`: (2 × wins + draws) × 50 / games, rounded.
    let score = white.and_then(|w| {
        let wins = if w { t.white_wins } else { t.black_wins };
        (games > 0).then(|| ((2 * wins + t.draws) * 50 + games / 2) / games)
    });
    let opponents_average =
        (rated > 0).then(|| (opponents as f64 / f64::from(rated)).round() as u32);
    let performance = match (score, opponents_average) {
        (Some(s), Some(a)) => {
            let s = s.min(100) as i64;
            let dp = if s >= 50 {
                i64::from(DP[(s - 50) as usize])
            } else {
                -i64::from(DP[(50 - s) as usize])
            };
            Some((i64::from(a) + dp).max(0) as u32)
        }
        _ => None,
    };
    let white_win_moves = average(t.white_win_moves, t.white_wins);
    let draw_moves = average(t.draw_moves, t.draws);
    let black_win_moves = average(t.black_win_moves, t.black_wins);
    let draw_percent = percent(t.draws, games);
    ColourStats {
        games,
        white_wins: t.white_wins,
        draws: t.draws,
        black_wins: t.black_wins,
        score,
        score_band: score
            .zip(white)
            .and_then(|(s, w)| band(&SCORE_BANDS, s as i32 - if w { 55 } else { 45 })),
        opponents_average: white.and(opponents_average),
        performance: white.and(performance),
        white_win_moves,
        white_win_band: length(white_win_moves),
        draw_moves,
        draw_band: length(draw_moves),
        black_win_moves,
        black_win_band: length(black_win_moves),
        draw_percent,
        draw_quote_band: draw_percent.and_then(|d| band(&DRAW_BANDS, d as i32)),
        quick_draw_percent: percent(t.quick_draws, games),
    }
}

/// The player's score by the opponent's rating, in bands of 50 from 1000 to
/// 2999, as ChessBase tallies it (`0x1408369a0`); bands without games are
/// left out.
pub fn rating_bands(games: &[PlayerGame]) -> Vec<RatingBand> {
    let mut bands = [(0u32, 0u32); 40];
    for g in games {
        let half = match (g.result, g.white) {
            (2, true) | (0, false) => 2,
            (1, _) => 1,
            (0, true) | (2, false) => 0,
            _ => continue,
        };
        if (1000..3000).contains(&g.opponent_elo) {
            let b = &mut bands[usize::from((g.opponent_elo - 1000) / 50)];
            b.0 += 1;
            b.1 += half;
        }
    }
    bands
        .iter()
        .enumerate()
        .filter(|(_, b)| b.0 > 0)
        .map(|(i, &(games, half))| RatingBand {
            from: 1000 + 50 * i as u16,
            games,
            points: f64::from(half) / 2.0,
            percent: (half * 50 + games / 2) / games,
        })
        .collect()
}

impl Database {
    /// Which of `ordinals` (at most [`MAX_MATE_GAMES`], the last ones) end
    /// in checkmate on their main line; unreadable games count as not. None
    /// when cancelled.
    pub fn mate_games(
        &self,
        ordinals: &[u64],
        workers: usize,
        cancel: &AtomicBool,
        progress: impl FnMut(u64, u64),
    ) -> Result<Option<Vec<u64>>, CbhError> {
        let ordinals = &ordinals[ordinals.len().saturating_sub(MAX_MATE_GAMES)..];
        let ends = map_games(self, ordinals, workers, cancel, progress, |db, ordinal| {
            let raw = db.raw_chunk(ordinal, 1)?.remove(0);
            let game = match db.moves_of(&raw) {
                Ok(Some(game)) => game,
                Err(e @ (CbhError::Io(_) | CbhError::SourceChanged)) => return Err(e),
                _ => return Ok(false),
            };
            let mut last = 0usize;
            while let Some(&child) = game.nodes.get(last).and_then(|n| n.children.first()) {
                last = child as usize;
            }
            Ok(last > 0
                && game
                    .chess_position(last as u32)
                    .is_ok_and(|p| shakmaty::Position::is_checkmate(&p)))
        })?;
        Ok(ends.map(|ends| {
            ordinals
                .iter()
                .zip(ends)
                .filter(|(_, mate)| *mate)
                .map(|(o, _)| *o)
                .collect()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(white: bool, result: u8, moves: u8, opponent_elo: u16) -> PlayerGame {
        PlayerGame {
            ordinal: 1,
            white,
            opponent: None,
            tournament: None,
            year: 2020,
            result,
            own_elo: 2600,
            opponent_elo,
            eco: 0,
            moves,
            annotator: None,
            medals: 0,
        }
    }

    #[test]
    fn bands_follow_chessbase_s_tables() {
        assert_eq!(band(&SCORE_BANDS, -55), Some(0));
        assert_eq!(band(&SCORE_BANDS, 0), Some(3));
        assert_eq!(band(&SCORE_BANDS, 8), Some(5));
        assert_eq!(band(&SCORE_BANDS, 100), None);
        assert_eq!(band(&LENGTH_BANDS, 24), Some(0));
        assert_eq!(band(&LENGTH_BANDS, 40), Some(3));
        assert_eq!(band(&DRAW_BANDS, 35), Some(3));
        assert_eq!(band(&DRAW_BANDS, 100), None);
    }

    #[test]
    fn colour_stats_score_lengths_and_draws() {
        let games = [
            game(true, 2, 30, 2500),
            game(true, 2, 40, 2500),
            game(true, 1, 15, 0),
            game(true, 0, 60, 2700),
            // A forfeit is not counted.
            game(true, 6, 0, 2500),
        ];
        let refs: Vec<&PlayerGame> = games.iter().collect();
        let s = colour_stats(&refs, Some(true));
        assert_eq!((s.games, s.white_wins, s.draws, s.black_wins), (4, 2, 1, 1));
        // (2 × 2 + 1) × 50 / 4 = 62.5, rounded to 63; 63 − 55 = 8: "well".
        assert_eq!(s.score, Some(63));
        assert_eq!(s.score_band.map(|b| SCORE_WORDS[b]), Some("well"));
        assert_eq!(s.opponents_average, Some(2567));
        assert_eq!(s.performance, Some(2567 + 95));
        assert_eq!(s.white_win_moves, Some(35));
        assert_eq!(
            s.white_win_band.map(|b| LENGTH_WORDS[b]),
            Some("shorter than average")
        );
        assert_eq!(s.black_win_band.map(|b| LENGTH_WORDS[b]), Some("very long"));
        assert_eq!(s.draw_percent, Some(25));
        assert_eq!(s.draw_quote_band.map(|b| DRAW_WORDS[b]), Some("low"));
        assert_eq!(s.quick_draw_percent, Some(25));
        // All games together have no score or verdict.
        let all = colour_stats(&refs, None);
        assert_eq!(
            (all.score, all.score_band, all.performance),
            (None, None, None)
        );
    }

    #[test]
    fn rating_bands_count_the_player_s_points() {
        let games = [
            game(true, 2, 30, 2510),
            game(false, 2, 30, 2540),
            game(false, 1, 30, 2600),
            game(true, 1, 30, 0),
        ];
        let bands = rating_bands(&games);
        assert_eq!(bands.len(), 2);
        assert_eq!(
            (bands[0].from, bands[0].games, bands[0].points),
            (2500, 2, 1.0)
        );
        assert_eq!(bands[0].percent, 50);
        assert_eq!((bands[1].from, bands[1].points), (2600, 0.5));
    }
}
