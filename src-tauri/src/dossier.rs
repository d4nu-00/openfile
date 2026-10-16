//! Player dossier (`PLAYERDOSSIER`; see docs/player-dossier.md): the
//! sections of ChessBase's dossier that the user's database can supply.
//! The player's games are kept like the player statistics', so the rows of
//! its lists open their games.
use crate::{
    commands::AppError,
    prepare::{RepertoireRow, repertoire_rows},
    registry::Registry,
    statistics::KeptGames,
};
use cbh_reader::{
    dossier::{self, ColourStats, RatingBand},
    player_stats::{OpponentRow, PlayerGame, PlayerStatistics, TournamentRow, YearRow, eco_code},
    prepare,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Rows of each list at most.
const MAX_ROWS: usize = 30;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DossierProgress {
    /// "games", "lines" (white, then black) or "mates".
    pub stage: &'static str,
    pub done: String,
    pub total: String,
}

/// A game in one of the dossier's lists.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DossierGame {
    pub ordinal: String,
    /// The player had White.
    pub white: bool,
    pub opponent: Option<String>,
    pub year: Option<u16>,
    pub result: Option<String>,
    pub moves: u8,
    pub eco: Option<String>,
    pub tournament: Option<String>,
    pub medals: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dossier {
    pub player_id: u32,
    pub name: Option<String>,
    pub games: u32,
    pub deleted: u32,
    pub truncated: bool,
    pub first_year: u16,
    pub last_year: u16,
    /// Elo profile and games played in years.
    pub years: Vec<YearRow>,
    /// "Statistics": all games, with White and with Black.
    pub all: ColourStats,
    pub white: ColourStats,
    pub black: ColourStats,
    pub rating_bands: Vec<RatingBand>,
    /// Latest first, then the most played opponents.
    pub tournaments: Vec<TournamentRow>,
    pub opponents: Vec<OpponentRow>,
    pub more_tournaments: u32,
    pub more_opponents: u32,
    pub repertoire_white: Vec<RepertoireRow>,
    pub repertoire_black: Vec<RepertoireRow>,
    /// Games with a medal (bits 0–14), games ending in mate, and games the
    /// player annotated; latest first, each with its full count.
    pub medal_games: Vec<DossierGame>,
    pub medal_count: u32,
    pub mates_given: Vec<DossierGame>,
    pub mates_given_count: u32,
    pub mates_received: Vec<DossierGame>,
    pub mates_received_count: u32,
    /// Games read for mates: the latest decisive ones.
    pub mates_read: u32,
    pub annotated: Vec<DossierGame>,
    pub annotated_count: u32,
}

/// A name compared loosely: case and spaces ignored.
fn loose(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

impl Registry {
    /// The dossier of player `player_id`. None when cancelled.
    pub fn player_dossier(
        &self,
        database_id: String,
        player_id: u32,
        mut progress: impl FnMut(&'static str, u64, u64),
    ) -> Result<Option<Dossier>, AppError> {
        let mut reader = self.own_handle(&database_id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .dossier_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
        let result = (|| -> Result<Option<Dossier>, AppError> {
            let Some(found) =
                reader.player_games(player_id, workers, &cancel, |d, t| progress("games", d, t))?
            else {
                return Ok(None);
            };
            let stats: PlayerStatistics = reader.player_statistics(player_id, &found)?;
            let games = &found.games;
            let of = |white: bool| -> Vec<u64> {
                games
                    .iter()
                    .filter(|g| g.white == white)
                    .map(|g| g.ordinal)
                    .collect()
            };
            let mut trees = vec![];
            for white in [true, false] {
                let Some(tree) =
                    reader.player_tree(&of(white), prepare::PLIES, workers, &cancel, |d, t| {
                        progress("lines", d, t)
                    })?
                else {
                    return Ok(None);
                };
                trees.push(repertoire_rows(&tree, white));
            }
            let decisive: Vec<u64> = games
                .iter()
                .filter(|g| g.result == 0 || g.result == 2)
                .map(|g| g.ordinal)
                .collect();
            let Some(mates) =
                reader.mate_games(&decisive, workers, &cancel, |d, t| progress("mates", d, t))?
            else {
                return Ok(None);
            };
            let refs = |white: Option<bool>| -> Vec<&PlayerGame> {
                games
                    .iter()
                    .filter(|g| white.is_none_or(|w| g.white == w))
                    .collect()
            };
            let won = |g: &PlayerGame| (g.result == 2) == g.white;
            let mated: Vec<&PlayerGame> = games
                .iter()
                .filter(|g| mates.binary_search(&g.ordinal).is_ok())
                .collect();
            let given: Vec<&PlayerGame> = mated.iter().copied().filter(|g| won(g)).collect();
            let received: Vec<&PlayerGame> = mated.iter().copied().filter(|g| !won(g)).collect();
            let medalled: Vec<&PlayerGame> =
                games.iter().filter(|g| g.medals & 0x7fff != 0).collect();
            let own = stats.name.as_deref().map(loose);
            let mut annotators = std::collections::HashMap::new();
            let mut annotated = vec![];
            for g in games {
                let Some(id) = g.annotator else { continue };
                let same = match annotators.get(&id) {
                    Some(same) => *same,
                    None => {
                        let name = reader.annotator_name(id)?;
                        let same = name.as_deref().map(loose).is_some_and(|n| Some(n) == own);
                        annotators.insert(id, same);
                        same
                    }
                };
                if same {
                    annotated.push(g);
                }
            }
            let mut rows = |list: &[&PlayerGame]| -> Result<Vec<DossierGame>, AppError> {
                let mut out = vec![];
                for g in list.iter().rev().take(MAX_ROWS) {
                    out.push(DossierGame {
                        ordinal: g.ordinal.to_string(),
                        white: g.white,
                        opponent: match g.opponent {
                            Some(id) => reader.player_name(id)?,
                            None => None,
                        },
                        year: (g.year > 0).then_some(g.year),
                        result: cbh_reader::header::pgn_result(g.result).map(str::to_string),
                        moves: g.moves,
                        eco: eco_code(g.eco),
                        tournament: match g.tournament {
                            Some(id) => reader.tournament_name(id)?.0,
                            None => None,
                        },
                        medals: cbh_reader::header::medal_names(g.medals & 0x7fff)
                            .into_iter()
                            .map(str::to_string)
                            .collect(),
                    });
                }
                Ok(out)
            };
            let medal_games = rows(&medalled)?;
            let mates_given = rows(&given)?;
            let mates_received = rows(&received)?;
            let annotated_rows = rows(&annotated)?;
            let mut repertoire = trees.into_iter();
            let dossier = Dossier {
                player_id,
                name: stats.name.clone(),
                games: stats.found,
                deleted: stats.deleted,
                truncated: stats.truncated,
                first_year: stats.first_year,
                last_year: stats.last_year,
                years: stats.years.clone(),
                all: dossier::colour_stats(&refs(None), None),
                white: dossier::colour_stats(&refs(Some(true)), Some(true)),
                black: dossier::colour_stats(&refs(Some(false)), Some(false)),
                rating_bands: dossier::rating_bands(games),
                tournaments: stats.tournaments.iter().take(MAX_ROWS).cloned().collect(),
                opponents: stats.opponents.iter().take(MAX_ROWS).cloned().collect(),
                more_tournaments: stats.tournaments.len().saturating_sub(MAX_ROWS) as u32
                    + stats.more_tournaments,
                more_opponents: stats.opponents.len().saturating_sub(MAX_ROWS) as u32
                    + stats.more_opponents,
                repertoire_white: repertoire.next().unwrap_or_default(),
                repertoire_black: repertoire.next().unwrap_or_default(),
                medal_count: medalled.len() as u32,
                medal_games,
                mates_given_count: given.len() as u32,
                mates_given,
                mates_received_count: received.len() as u32,
                mates_received,
                mates_read: decisive.len().min(dossier::MAX_MATE_GAMES) as u32,
                annotated_count: annotated.len() as u32,
                annotated: annotated_rows,
            };
            // Kept for listing the games of a row, as the statistics are.
            let mut state = self.lock()?;
            if !state.databases.contains_key(&database_id) {
                return Err(AppError::new(
                    "invalidHandle",
                    "Database is closed or unknown",
                ));
            }
            state.player_games.insert(
                database_id.clone(),
                Arc::new(KeptGames {
                    player_id,
                    games: found,
                }),
            );
            Ok(Some(dossier))
        })();
        let mut state = self.lock()?;
        if state
            .dossier_cancels
            .get(&database_id)
            .is_some_and(|c| Arc::ptr_eq(c, &cancel))
        {
            state.dossier_cancels.remove(&database_id);
        }
        result
    }

    pub fn cancel_dossier(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.dossier_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}
