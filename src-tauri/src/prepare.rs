//! Prepare against White or Black (`PREPARE_WHITE`, `PREPARE_BLACK`; see
//! docs/preparation.md): the opening tree of one player's games with one
//! colour. The Book pane can show it in place of the database's tree, and
//! its games can be listed.
use crate::{
    commands::AppError,
    registry::Registry,
    searches::{Hits, SearchResponse},
    trees::{Line, TreeResponse, TreeStatus, fill_moves, replay, san},
};
use cbh_reader::{
    player_stats::{PlayerGame, PlayerGames, Score, eco_code},
    prepare::{self, PlayerTree},
    tree,
};
use shakmaty::{Chess, EnPassantMode, Position, fen::Fen, uci::UciMove};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Latest games and openings listed at most.
const MAX_LATEST: usize = 10;
const MAX_OPENINGS: usize = 20;

/// A preparation kept for the Book pane and the game list.
pub struct Prepared {
    pub player_id: u32,
    pub white: bool,
    tree: PlayerTree,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareProgress {
    /// "games" while the player's games are found, then "lines".
    pub stage: &'static str,
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepertoireRow {
    /// UCI moves from the start position.
    pub moves: Vec<String>,
    /// "1.e4 c5 2.Nf3".
    pub text: String,
    pub eco: Option<String>,
    pub name: Option<String>,
    pub games: u64,
    /// The player's score in percent, over games with a result.
    pub score: Option<f64>,
    pub last_year: Option<u16>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpeningRow {
    pub eco: Option<String>,
    pub score: Score,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestGame {
    pub ordinal: String,
    pub year: Option<u16>,
    pub opponent: Option<String>,
    pub opponent_elo: Option<u16>,
    pub result: Option<String>,
    pub eco: Option<String>,
    pub tournament: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preparation {
    pub player_id: u32,
    pub name: Option<String>,
    pub white: bool,
    /// The player's games with that colour, deleted ones left out.
    pub games: u32,
    pub deleted: u32,
    /// Games whose main lines were read (the latest [`prepare::MAX_GAMES`]).
    pub read: u32,
    pub unreadable: u32,
    pub truncated: bool,
    pub score: Score,
    /// The average rating of the player's last ten games with the colour, as
    /// The reference app computes it when preparing.
    pub recent_rating: Option<u32>,
    pub first_year: u16,
    pub last_year: u16,
    pub repertoire: Vec<RepertoireRow>,
    /// Most played first.
    pub openings: Vec<OpeningRow>,
    /// Latest entry first.
    pub latest: Vec<LatestGame>,
    /// Entry number of the last game, which the reference app loads.
    pub last_game: Option<String>,
}

/// "1.e4 c5 2.Nf3" for UCI moves from the start position.
fn move_text(moves: &[String]) -> String {
    let mut pos = Chess::default();
    let mut out = String::new();
    for (i, uci) in moves.iter().enumerate() {
        if i % 2 == 0 {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&format!("{}.", i / 2 + 1));
        } else {
            out.push(' ');
        }
        out.push_str(&san(&pos, uci));
        let Some(m) = UciMove::from_ascii(uci.as_bytes())
            .ok()
            .and_then(|u| u.to_move(&pos).ok())
        else {
            break;
        };
        pos.play_unchecked(m);
    }
    out
}

/// The lines the player keeps playing, as dialog rows.
pub(crate) fn repertoire_rows(tree: &PlayerTree, white: bool) -> Vec<RepertoireRow> {
    tree.repertoire(white)
        .into_iter()
        .map(|l| {
            let moves: Vec<String> = l
                .moves
                .iter()
                .filter_map(|c| tree::unpack_uci(*c))
                .collect();
            let found = chess_core::eco::classify_uci(moves.iter().map(String::as_str));
            let s = &l.stats;
            let decided = s.wins + s.draws + s.losses;
            // The stats are the mover's; the player's wins are the mover's
            // losses when the opponent is to move.
            let player_wins = if l.player_to_move { s.wins } else { s.losses };
            RepertoireRow {
                text: move_text(&moves),
                eco: found.as_ref().map(|c| c.opening.eco.clone()),
                name: found.map(|c| c.opening.name),
                games: s.games,
                score: (decided > 0)
                    .then(|| (player_wins as f64 + s.draws as f64 / 2.0) * 100.0 / decided as f64),
                last_year: (s.year_max > 0).then_some(s.year_max),
                moves,
            }
        })
        .collect()
}

impl Registry {
    /// Prepares against player `player_id` with White (`white`) or Black:
    /// finds their games, reads the main lines of those with the colour, and
    /// keeps the tree for the database. None when cancelled.
    pub fn prepare_against(
        &self,
        database_id: String,
        player_id: u32,
        white: bool,
        mut progress: impl FnMut(&'static str, u64, u64),
    ) -> Result<Option<Preparation>, AppError> {
        let mut reader = self.own_handle(&database_id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .prepare_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
        let result = (|| -> Result<Option<(PlayerGames, PlayerTree)>, AppError> {
            let Some(mut found) =
                reader.player_games(player_id, workers, &cancel, |d, t| progress("games", d, t))?
            else {
                return Ok(None);
            };
            found.games.retain(|g| g.white == white);
            let ordinals: Vec<u64> = found.games.iter().map(|g| g.ordinal).collect();
            let Some(tree) =
                reader.player_tree(&ordinals, prepare::PLIES, workers, &cancel, |d, t| {
                    progress("lines", d, t)
                })?
            else {
                return Ok(None);
            };
            Ok(Some((found, tree)))
        })();
        {
            let mut state = self.lock()?;
            if state
                .prepare_cancels
                .get(&database_id)
                .is_some_and(|c| Arc::ptr_eq(c, &cancel))
            {
                state.prepare_cancels.remove(&database_id);
            }
        }
        let Some((found, tree)) = result? else {
            return Ok(None);
        };
        let games = &found.games;
        // Scored with the player statistics, which split by colour.
        let stats = reader.player_statistics(player_id, &found)?;
        let score = if white { stats.white } else { stats.black };
        let ratings: Vec<u16> = games.iter().map(|g| g.own_elo).collect();
        let repertoire = repertoire_rows(&tree, white);
        let mut by_eco: HashMap<u16, Vec<&PlayerGame>> = HashMap::new();
        for g in games {
            by_eco.entry(g.eco).or_default().push(g);
        }
        let mut openings: Vec<(u16, usize)> = by_eco.iter().map(|(e, v)| (*e, v.len())).collect();
        openings.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        openings.truncate(MAX_OPENINGS);
        let openings = openings
            .into_iter()
            .filter_map(|(eco, _)| {
                stats
                    .openings
                    .iter()
                    .find(|o| o.eco == eco_code(eco))
                    .map(|o| OpeningRow {
                        eco: o.eco.clone(),
                        score: if white { o.white } else { o.black },
                    })
            })
            .collect();
        let mut latest = vec![];
        for g in games.iter().rev().take(MAX_LATEST) {
            let opponent = match g.opponent {
                Some(id) => reader.player_name(id)?,
                None => None,
            };
            let tournament = match g.tournament {
                Some(id) => reader.tournament_name(id)?.0,
                None => None,
            };
            latest.push(LatestGame {
                ordinal: g.ordinal.to_string(),
                year: (g.year > 0).then_some(g.year),
                opponent,
                opponent_elo: (g.opponent_elo > 0).then_some(g.opponent_elo),
                result: cbh_reader::header::pgn_result(g.result).map(str::to_string),
                eco: eco_code(g.eco),
                tournament,
            });
        }
        let preparation = Preparation {
            player_id,
            name: stats.name.clone(),
            white,
            games: games.len() as u32,
            deleted: found.deleted,
            read: tree.games() as u32,
            unreadable: tree.unreadable as u32,
            truncated: found.truncated || games.len() > prepare::MAX_GAMES,
            score,
            recent_rating: prepare::recent_rating(&ratings),
            first_year: games
                .iter()
                .map(|g| g.year)
                .filter(|y| *y > 0)
                .min()
                .unwrap_or(0),
            last_year: games.iter().map(|g| g.year).max().unwrap_or(0),
            repertoire,
            openings,
            latest,
            last_game: games.last().map(|g| g.ordinal.to_string()),
        };
        let mut state = self.lock()?;
        if !state.databases.contains_key(&database_id) {
            return Err(AppError::new(
                "invalidHandle",
                "Database is closed or unknown",
            ));
        }
        state.preparations.insert(
            database_id,
            Arc::new(Prepared {
                player_id,
                white,
                tree,
            }),
        );
        Ok(Some(preparation))
    }

    pub fn cancel_preparation(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.prepare_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Forgets the database's preparation; the Book pane shows its tree again.
    pub fn close_preparation(&self, database_id: String) -> Result<(), AppError> {
        self.lock()?.preparations.remove(&database_id);
        Ok(())
    }

    fn prepared(&self, database_id: &str) -> Result<Arc<Prepared>, AppError> {
        self.lock()?
            .preparations
            .get(database_id)
            .cloned()
            .ok_or_else(|| AppError::new("noPreparation", "The preparation is no longer current"))
    }

    /// The moves the player's games play in the line's last position, in the
    /// shape of the opening tree's answer.
    pub fn prepared_tree(
        &self,
        database_id: String,
        line: Line,
        request_id: String,
    ) -> Result<TreeResponse, AppError> {
        let (pos, hashes, _) = replay(&line)?;
        let mut response = TreeResponse {
            request_id,
            status: TreeStatus::NoTree,
            games: 0,
            ended: 0,
            moves: vec![],
        };
        let Ok(prepared) = self.prepared(&database_id) else {
            return Ok(response);
        };
        let hash = *hashes.last().unwrap_or(&0);
        let moves = prepared.tree.moves_at(hash);
        response.status = if moves.is_empty() {
            TreeStatus::NotInTree
        } else {
            TreeStatus::Tree
        };
        fill_moves(&mut response, &pos, &moves);
        Ok(response)
    }

    /// Lists the prepared games as the database's search result: all of
    /// them, or those whose main line reaches the line's last position.
    pub fn preparation_games(
        &self,
        database_id: String,
        player_id: u32,
        white: bool,
        line: Option<Line>,
        request_id: String,
    ) -> Result<SearchResponse, AppError> {
        let prepared = self.prepared(&database_id)?;
        if prepared.player_id != player_id || prepared.white != white {
            return Err(AppError::new(
                "noPreparation",
                "The preparation is no longer current",
            ));
        }
        let (ordinals, fen) = match &line {
            Some(line) if !line.moves.is_empty() => {
                let (pos, hashes, _) = replay(line)?;
                let hash = *hashes.last().unwrap_or(&0);
                (
                    prepared.tree.games_at(hash),
                    Some(Fen::from_position(&pos, EnPassantMode::Legal).to_string()),
                )
            }
            _ => (prepared.tree.all_games(), None),
        };
        let response = SearchResponse {
            request_id,
            hits: ordinals.len().to_string(),
            scanned: prepared.tree.games().to_string(),
            truncated: false,
            cancelled: false,
            indexed: false,
        };
        self.lock()?.searches.insert(
            database_id,
            Arc::new(Hits {
                ordinals,
                nodes: None,
                fen,
            }),
        );
        Ok(response)
    }
}
