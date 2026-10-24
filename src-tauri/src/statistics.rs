//! Player statistics (see docs/player-statistics.md). The player's games
//! are kept after the statistics are shown, so a row's games open in the
//! game list without another scan.
use crate::{
    commands::AppError,
    registry::Registry,
    searches::{Hits, SearchResponse},
};
use cbh_reader::player_stats::{Group, PlayerGames, PlayerStatistics};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsProgress {
    pub done: String,
    pub total: String,
}

/// The games behind the statistics last shown for a database.
pub struct KeptGames {
    pub player_id: u32,
    pub games: PlayerGames,
}

impl Registry {
    /// The statistics of player `player_id`, read through a separate
    /// handle. None when cancelled.
    pub fn player_statistics(
        &self,
        database_id: String,
        player_id: u32,
        progress: impl FnMut(u64, u64),
    ) -> Result<Option<PlayerStatistics>, AppError> {
        let mut reader = self.own_handle(&database_id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .statistics_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
        let found = reader.player_games(player_id, workers, &cancel, progress);
        {
            let mut state = self.lock()?;
            if state
                .statistics_cancels
                .get(&database_id)
                .is_some_and(|c| Arc::ptr_eq(c, &cancel))
            {
                state.statistics_cancels.remove(&database_id);
            }
        }
        let Some(games) = found? else {
            return Ok(None);
        };
        let stats = reader.player_statistics(player_id, &games)?;
        let mut state = self.lock()?;
        if !state.databases.contains_key(&database_id) {
            return Err(AppError::new(
                "invalidHandle",
                "Database is closed or unknown",
            ));
        }
        state
            .player_games
            .insert(database_id, Arc::new(KeptGames { player_id, games }));
        Ok(Some(stats))
    }

    pub fn cancel_player_statistics(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.statistics_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Shows the games of a statistics row as the database's search result.
    pub fn statistics_games(
        &self,
        database_id: String,
        player_id: u32,
        group: Group,
        request_id: String,
    ) -> Result<SearchResponse, AppError> {
        let mut state = self.lock()?;
        let kept = state
            .player_games
            .get(&database_id)
            .filter(|k| k.player_id == player_id)
            .cloned()
            .ok_or_else(|| AppError::new("noStatistics", "The statistics are no longer current"))?;
        let ordinals: Vec<u64> = kept
            .games
            .games
            .iter()
            .filter(|g| group.contains(g))
            .map(|g| g.ordinal)
            .collect();
        let response = SearchResponse {
            request_id,
            hits: ordinals.len().to_string(),
            scanned: ordinals.len().to_string(),
            truncated: kept.games.truncated,
            cancelled: false,
            indexed: false,
        };
        state.searches.insert(
            database_id,
            Arc::new(Hits {
                ordinals,
                nodes: None,
                fen: None,
                truncated: kept.games.truncated,
            }),
        );
        Ok(response)
    }
}
