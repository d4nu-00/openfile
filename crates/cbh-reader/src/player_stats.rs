//! Player statistics (the reference app's Player statistic, `PLAYERSTATISTIC`, and
//! Statistics, `STATISTICS`). See docs/player-statistics.md.
//!
//! A player's games are found by one parallel scan of the entry index; no
//! game is decoded. They are then scored by colour, opening (ECO), opponent,
//! tournament and year.
use crate::{CbhError, Database, crosstable::performance, sort::fields};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

/// Games of one player read at most.
pub const MAX_GAMES: usize = 100_000;
/// Rows of the opponent and tournament lists at most.
pub const MAX_ROWS: usize = 5_000;

/// One game of the player, as the entry index stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerGame {
    pub ordinal: u64,
    /// The player had White.
    pub white: bool,
    pub opponent: Option<u32>,
    pub tournament: Option<u32>,
    pub year: u16,
    /// Stored result code.
    pub result: u8,
    /// Ratings; 0 when unrated.
    pub own_elo: u16,
    pub opponent_elo: u16,
    /// ECO index 1..=500 (A00..E99), 0 for none.
    pub eco: u16,
    /// Moves as the record stores them, the annotator and the medal word.
    pub moves: u8,
    pub annotator: Option<u32>,
    pub medals: u16,
}

/// What a game is worth to the player: half points, and whether it was a
/// forfeit. None for games without a result (lines, "0-0", unknown codes).
fn half_points(game: &PlayerGame) -> Option<(u32, bool)> {
    let (white_half, forfeit) = match game.result {
        2 => (2, false),
        1 => (1, false),
        0 => (0, false),
        6 => (2, true),
        5 => (1, true),
        4 => (0, true),
        _ => return None,
    };
    Some((
        if game.white {
            white_half
        } else {
            2 - white_half
        },
        forfeit,
    ))
}

/// Counts while scoring.
#[derive(Clone, Copy, Default)]
struct Tally {
    games: u32,
    wins: u32,
    draws: u32,
    losses: u32,
    half: u32,
    /// Games against rated opponents, forfeits left out.
    rated: u32,
    rated_half: u32,
    rated_elo: u64,
}

impl Tally {
    fn add(&mut self, game: &PlayerGame) {
        let Some((half, forfeit)) = half_points(game) else {
            return;
        };
        self.games += 1;
        self.half += half;
        match half {
            2 => self.wins += 1,
            1 => self.draws += 1,
            _ => self.losses += 1,
        }
        if !forfeit && game.opponent_elo > 0 {
            self.rated += 1;
            self.rated_half += half;
            self.rated_elo += u64::from(game.opponent_elo);
        }
    }
    fn score(&self) -> Score {
        Score {
            games: self.games,
            wins: self.wins,
            draws: self.draws,
            losses: self.losses,
            points: f64::from(self.half) / 2.0,
            percent: (self.games > 0)
                .then(|| (f64::from(self.half) * 50.0 / f64::from(self.games)).round() as u32),
            average_elo: (self.rated > 0)
                .then(|| (self.rated_elo as f64 / f64::from(self.rated)).round() as u32),
            performance: performance(self.rated_elo, self.rated, self.rated_half),
        }
    }
}

/// The player's score in a group of games.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    /// Games with a result.
    pub games: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub points: f64,
    /// Points per game in percent, rounded.
    pub percent: Option<u32>,
    /// Average rating of the rated opponents (forfeits left out).
    pub average_elo: Option<u32>,
    /// That average plus FIDE's difference for the score against them.
    pub performance: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpeningRow {
    /// "A00".."E99"; None for games without an ECO code.
    pub eco: Option<String>,
    pub white: Score,
    pub black: Score,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpponentRow {
    pub id: u32,
    pub name: Option<String>,
    /// The opponent's rating in the latest rated game between them.
    pub elo: Option<u16>,
    pub score: Score,
    pub last_year: u16,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentRow {
    pub id: u32,
    pub name: Option<String>,
    pub site: Option<String>,
    /// Year of the player's games there; 0 when undated.
    pub year: u16,
    pub score: Score,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearRow {
    /// 0 for undated games.
    pub year: u16,
    pub score: Score,
    /// The player's highest rating in the year's games.
    pub elo: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatistics {
    pub player_id: u32,
    pub name: Option<String>,
    pub total: Score,
    pub white: Score,
    pub black: Score,
    /// Games found, deleted ones excluded.
    pub found: u32,
    /// Deleted games, not counted.
    pub deleted: u32,
    /// Games without a result (lines, "0-0"), not scored.
    pub unscored: u32,
    pub first_year: u16,
    pub last_year: u16,
    /// By ECO code, in code order; games without one last.
    pub openings: Vec<OpeningRow>,
    /// Most games first.
    pub opponents: Vec<OpponentRow>,
    /// Latest first.
    pub tournaments: Vec<TournamentRow>,
    /// In year order; undated games first.
    pub years: Vec<YearRow>,
    /// More than [`MAX_GAMES`] games: the statistics cover the first ones.
    pub truncated: bool,
    /// Opponents and tournaments left out beyond [`MAX_ROWS`] rows.
    pub more_opponents: u32,
    pub more_tournaments: u32,
}

/// What a statistics row stands for, to list its games.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Group {
    All,
    /// Games with this colour.
    Color {
        white: bool,
    },
    /// Games with this ECO index (0: none), optionally with one colour.
    Opening {
        eco: u16,
        white: Option<bool>,
    },
    Opponent {
        id: u32,
    },
    Tournament {
        id: u32,
    },
    Year {
        year: u16,
    },
}

impl Group {
    pub fn contains(&self, g: &PlayerGame) -> bool {
        match *self {
            Self::All => true,
            Self::Color { white } => g.white == white,
            Self::Opening { eco, white } => g.eco == eco && white.is_none_or(|w| g.white == w),
            Self::Opponent { id } => g.opponent == Some(id),
            Self::Tournament { id } => g.tournament == Some(id),
            Self::Year { year } => g.year == year,
        }
    }
}

/// "A00".."E99" for an ECO index 1..=500.
pub fn eco_code(index: u16) -> Option<String> {
    (1..=500).contains(&index).then(|| {
        let i = index - 1;
        format!("{}{:02}", char::from(b'A' + (i / 100) as u8), i % 100)
    })
}

/// The player's games: entry numbers in order, deleted games counted apart.
#[derive(Clone, Debug, Default)]
pub struct PlayerGames {
    pub games: Vec<PlayerGame>,
    pub deleted: u32,
    pub truncated: bool,
}

fn game_of(
    format: crate::DatabaseFormat,
    ordinal: u64,
    record: &[u8],
    id: u32,
) -> Option<(PlayerGame, bool)> {
    let f = fields(format, record);
    if !f.game {
        return None;
    }
    let white = if f.white == Some(id) {
        true
    } else if f.black == Some(id) {
        false
    } else {
        return None;
    };
    let (opponent, own_elo, opponent_elo) = if white {
        (f.black, f.white_elo, f.black_elo)
    } else {
        (f.white, f.black_elo, f.white_elo)
    };
    Some((
        PlayerGame {
            ordinal,
            white,
            opponent,
            tournament: f.tournament,
            year: ((f.date >> 9) & 4095) as u16,
            result: f.result,
            own_elo,
            opponent_elo,
            // Codes outside A00..E99 count as none.
            eco: Some(f.eco >> 7).filter(|e| *e <= 500).unwrap_or(0),
            moves: f.moves,
            annotator: f.annotator,
            medals: f.medals,
        },
        f.deleted,
    ))
}

impl Database {
    /// The games of player `id`, found by up to `workers` threads, each
    /// with its own read handle. Returns None when cancelled.
    pub fn player_games(
        &mut self,
        id: u32,
        workers: usize,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Option<PlayerGames>, CbhError> {
        self.verify()?;
        let count = self.info().entry_count;
        let mut out = PlayerGames::default();
        let format = self.format();
        let workers = workers.clamp(1, 64) as u64;
        let handles = (1..workers)
            .map(|_| self.reopen())
            .collect::<Result<Vec<_>, _>>()?;
        let per = count.div_ceil(workers).max(1);
        let done = AtomicU64::new(0);
        let me: &Database = self;
        let parts = std::thread::scope(|scope| -> Result<Option<Vec<PlayerGames>>, CbhError> {
            let mut running = vec![];
            for i in 0..workers {
                let first = 1 + i * per;
                if first > count {
                    break;
                }
                let last = (first + per - 1).min(count);
                let handle = if i == 0 { me } else { &handles[i as usize - 1] };
                let done = &done;
                running.push(
                    scope.spawn(move || -> Result<Option<PlayerGames>, CbhError> {
                        const CHUNK: u64 = 8192;
                        let size = format.record_size();
                        let mut part = PlayerGames::default();
                        let mut at = first;
                        while at <= last {
                            if cancel.load(Ordering::Relaxed) {
                                return Ok(None);
                            }
                            let n = CHUNK.min(last + 1 - at);
                            for (i, record) in
                                handle.record_bytes(at, n)?.chunks_exact(size).enumerate()
                            {
                                let Some((game, deleted)) =
                                    game_of(format, at + i as u64, record, id)
                                else {
                                    continue;
                                };
                                if deleted {
                                    part.deleted += 1;
                                } else if part.games.len() < MAX_GAMES {
                                    part.games.push(game);
                                } else {
                                    part.truncated = true;
                                }
                            }
                            at += n;
                            done.fetch_add(n, Ordering::Relaxed);
                        }
                        Ok(Some(part))
                    }),
                );
            }
            loop {
                let finished = running.iter().all(|p| p.is_finished());
                progress(done.load(Ordering::Relaxed), count);
                if finished {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let mut parts = vec![];
            for p in running {
                match p.join().map_err(|_| {
                    CbhError::Io(std::io::Error::other("statistics worker stopped"))
                })?? {
                    Some(part) => parts.push(part),
                    None => return Ok(None),
                }
            }
            Ok(Some(parts))
        })?;
        let Some(parts) = parts else {
            return Ok(None);
        };
        // Parts cover consecutive ranges, so the games stay in entry order.
        for part in parts {
            out.deleted += part.deleted;
            out.truncated |= part.truncated;
            for g in part.games {
                if out.games.len() == MAX_GAMES {
                    out.truncated = true;
                    break;
                }
                out.games.push(g);
            }
        }
        Ok(Some(out))
    }

    /// Scores the games of player `id`.
    pub fn player_statistics(
        &mut self,
        id: u32,
        games: &PlayerGames,
    ) -> Result<PlayerStatistics, CbhError> {
        let (mut total, mut white, mut black) =
            (Tally::default(), Tally::default(), Tally::default());
        let mut openings: HashMap<u16, (Tally, Tally)> = HashMap::new();
        // Opponent: tally, latest rating and its ordinal, last year.
        let mut opponents: HashMap<u32, (Tally, u16, u64, u16)> = HashMap::new();
        // Tournament: tally, year.
        let mut tournaments: HashMap<u32, (Tally, u16)> = HashMap::new();
        let mut years: HashMap<u16, (Tally, u16)> = HashMap::new();
        let mut unscored = 0;
        let (mut first_year, mut last_year) = (0u16, 0u16);
        for g in &games.games {
            if half_points(g).is_none() {
                unscored += 1;
            }
            total.add(g);
            if g.white { &mut white } else { &mut black }.add(g);
            let o = openings.entry(g.eco).or_default();
            if g.white { &mut o.0 } else { &mut o.1 }.add(g);
            if let Some(id) = g.opponent {
                let o = opponents.entry(id).or_default();
                o.0.add(g);
                if g.opponent_elo > 0 && g.ordinal >= o.2 {
                    (o.1, o.2) = (g.opponent_elo, g.ordinal);
                }
                o.3 = o.3.max(g.year);
            }
            if let Some(id) = g.tournament {
                let t = tournaments.entry(id).or_default();
                t.0.add(g);
                if g.year > 0 && (t.1 == 0 || g.year < t.1) {
                    t.1 = g.year;
                }
            }
            let y = years.entry(g.year).or_default();
            y.0.add(g);
            y.1 = y.1.max(g.own_elo);
            if g.year > 0 {
                first_year = if first_year == 0 {
                    g.year
                } else {
                    first_year.min(g.year)
                };
                last_year = last_year.max(g.year);
            }
        }

        let openings: Vec<OpeningRow> = {
            let mut v: Vec<_> = openings.into_iter().collect();
            // Games without an ECO code last.
            v.sort_unstable_by_key(|(eco, _)| if *eco == 0 { u16::MAX } else { *eco });
            v.into_iter()
                .map(|(eco, (w, b))| OpeningRow {
                    eco: eco_code(eco),
                    white: w.score(),
                    black: b.score(),
                })
                .collect()
        };

        let mut ranked: Vec<_> = opponents.into_iter().collect();
        ranked.sort_unstable_by(|a, b| b.1.0.games.cmp(&a.1.0.games).then(a.0.cmp(&b.0)));
        let more_opponents = ranked.len().saturating_sub(MAX_ROWS) as u32;
        ranked.truncate(MAX_ROWS);
        let mut opponent_rows = Vec::with_capacity(ranked.len());
        for (oid, (t, elo, _, last)) in ranked {
            opponent_rows.push(OpponentRow {
                id: oid,
                name: self.player_name(oid)?,
                elo: (elo > 0).then_some(elo),
                score: t.score(),
                last_year: last,
            });
        }

        let mut events: Vec<_> = tournaments.into_iter().collect();
        events.sort_unstable_by(|a, b| b.1.1.cmp(&a.1.1).then(b.0.cmp(&a.0)));
        let more_tournaments = events.len().saturating_sub(MAX_ROWS) as u32;
        events.truncate(MAX_ROWS);
        let mut tournament_rows = Vec::with_capacity(events.len());
        for (tid, (t, year)) in events {
            let (name, site) = self.tournament_name(tid)?;
            tournament_rows.push(TournamentRow {
                id: tid,
                name,
                site,
                year,
                score: t.score(),
            });
        }

        let mut year_rows: Vec<_> = years
            .into_iter()
            .map(|(year, (t, elo))| YearRow {
                year,
                score: t.score(),
                elo: (elo > 0).then_some(elo),
            })
            .collect();
        year_rows.sort_unstable_by_key(|r| r.year);

        Ok(PlayerStatistics {
            player_id: id,
            name: self.player_name(id)?,
            total: total.score(),
            white: white.score(),
            black: black.score(),
            found: games.games.len() as u32,
            deleted: games.deleted,
            unscored,
            first_year,
            last_year,
            openings,
            opponents: opponent_rows,
            tournaments: tournament_rows,
            years: year_rows,
            truncated: games.truncated,
            more_opponents,
            more_tournaments,
        })
    }
}
