//! Prepare against White or Black (ChessBase's `PREPARE_WHITE`,
//! `PREPARE_BLACK`): the opening tree of one player's games with one colour,
//! held in memory. See docs/preparation.md.
//!
//! The tree keeps each game's main line (position hashes and moves), so it
//! answers any position by one pass over those lines; a few thousand games
//! of 30 plies take well under a millisecond.
use crate::{
    CbhError, Database,
    tree::{GameLine, MoveStats, NO_MOVE, Replay, add_game, total},
};
use shakmaty::{Chess, Position};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Games read into a preparation at most: the latest in entry order.
pub const MAX_GAMES: usize = 20_000;
/// Half-moves of each main line kept.
pub const PLIES: usize = 30;
/// Repertoire lines: the deepest ply, the most lines, and the share of the
/// games a move needs to continue a line (at least [`LINE_MIN_GAMES`]).
pub const LINE_PLIES: usize = 16;
pub const MAX_LINES: usize = 30;
pub const LINE_SHARE_PERCENT: u64 = 3;
pub const LINE_MIN_GAMES: u64 = 2;

/// The main lines of a player's games.
#[derive(Clone, Debug, Default)]
pub struct PlayerTree {
    /// Entry number and main line of each game read, in entry order.
    lines: Vec<(u64, GameLine)>,
    /// Games that are text entries or whose moves cannot be replayed.
    pub unreadable: u64,
}

/// A line the player keeps playing, from the standard start position.
#[derive(Clone, Debug, PartialEq)]
pub struct RepertoireLine {
    /// Packed moves.
    pub moves: Vec<u16>,
    /// Games reaching the line's last position, transpositions included.
    pub stats: MoveStats,
    /// Whether the player is the side to move there (scores are the mover's).
    pub player_to_move: bool,
}

impl PlayerTree {
    pub fn games(&self) -> usize {
        self.lines.len()
    }

    /// The moves played in the position `hash`, most played first; a row with
    /// [`NO_MOVE`] counts the games that end there.
    pub fn moves_at(&self, hash: u64) -> Vec<MoveStats> {
        let mut moves = vec![];
        for (_, line) in &self.lines {
            if let Some((black, code)) = line.at(hash) {
                add_game(&mut moves, code, &line.stats, black);
            }
        }
        moves.sort_by(|a, b| b.games.cmp(&a.games).then(a.code.cmp(&b.code)));
        moves
    }

    /// Every game read, in entry order.
    pub fn all_games(&self) -> Vec<u64> {
        self.lines.iter().map(|(ordinal, _)| *ordinal).collect()
    }

    /// The games whose main line reaches `hash`, in entry order.
    pub fn games_at(&self, hash: u64) -> Vec<u64> {
        self.lines
            .iter()
            .filter(|(_, line)| line.at(hash).is_some())
            .map(|(ordinal, _)| *ordinal)
            .collect()
    }

    /// The lines the player keeps playing from the standard start position:
    /// a move continues a line while at least [`LINE_SHARE_PERCENT`] % of the
    /// games (and [`LINE_MIN_GAMES`], or the one game there is) play it, up
    /// to [`LINE_PLIES`]. The most
    /// played [`MAX_LINES`] lines, most played first.
    pub fn repertoire(&self, player_white: bool) -> Vec<RepertoireLine> {
        let start = Chess::default();
        let root = Replay::new(start.board(), start.turn());
        let all: u64 = self.moves_at(root.hash()).iter().map(|m| m.games).sum();
        // A player with a single game still has that game's line.
        let min = (all * LINE_SHARE_PERCENT)
            .div_ceil(100)
            .max(LINE_MIN_GAMES.min(all.max(1)));
        let mut out = vec![];
        let mut stack = vec![(root, Vec::<u16>::new())];
        while let Some((replay, moves)) = stack.pop() {
            let here = self.moves_at(replay.hash());
            let next: Vec<&MoveStats> = if moves.len() < LINE_PLIES {
                here.iter()
                    .filter(|m| m.code != NO_MOVE && m.games >= min)
                    .collect()
            } else {
                vec![]
            };
            if next.is_empty() {
                if !moves.is_empty() {
                    out.push(RepertoireLine {
                        moves,
                        stats: total(&here),
                        player_to_move: replay.black_to_move() != player_white,
                    });
                }
                continue;
            }
            // Pushed in reverse, so the most played move is expanded first.
            for m in next.into_iter().rev() {
                let mut child = replay.clone();
                if child.play(m.code).is_none() {
                    continue;
                }
                let mut line = moves.clone();
                line.push(m.code);
                stack.push((child, line));
            }
        }
        out.sort_by_key(|l| std::cmp::Reverse(l.stats.games));
        out.truncate(MAX_LINES);
        out
    }
}

impl Database {
    /// Reads the main lines of `ordinals` (at most [`MAX_GAMES`], the last
    /// ones) up to `plies`, with up to `workers` threads, each with its own
    /// read handle. Returns None when cancelled.
    pub fn player_tree(
        &self,
        ordinals: &[u64],
        plies: usize,
        workers: usize,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Option<PlayerTree>, CbhError> {
        let ordinals = &ordinals[ordinals.len().saturating_sub(MAX_GAMES)..];
        let total = ordinals.len() as u64;
        let workers = workers.clamp(1, 64);
        let per = ordinals.len().div_ceil(workers).max(1);
        let parts: Vec<&[u64]> = ordinals.chunks(per).collect();
        let handles = (1..parts.len())
            .map(|_| self.reopen())
            .collect::<Result<Vec<_>, _>>()?;
        let done = AtomicU64::new(0);
        let me: &Database = self;
        let results = std::thread::scope(|scope| -> Result<Option<Vec<PlayerTree>>, CbhError> {
            let mut running = vec![];
            for (i, part) in parts.iter().enumerate() {
                let handle = if i == 0 { me } else { &handles[i - 1] };
                let done = &done;
                running.push(
                    scope.spawn(move || -> Result<Option<PlayerTree>, CbhError> {
                        let mut tree = PlayerTree::default();
                        for &ordinal in *part {
                            if cancel.load(Ordering::Relaxed) {
                                return Ok(None);
                            }
                            match handle.main_line(ordinal, plies) {
                                Ok(Some(line)) => tree.lines.push((ordinal, line)),
                                Err(e @ (CbhError::Io(_) | CbhError::SourceChanged)) => {
                                    return Err(e);
                                }
                                Ok(None) | Err(_) => tree.unreadable += 1,
                            }
                            done.fetch_add(1, Ordering::Relaxed);
                        }
                        Ok(Some(tree))
                    }),
                );
            }
            loop {
                let finished = running.iter().all(|p| p.is_finished());
                progress(done.load(Ordering::Relaxed), total);
                if finished {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let mut out = vec![];
            for p in running {
                match p.join().map_err(|_| {
                    CbhError::Io(std::io::Error::other("preparation worker stopped"))
                })?? {
                    Some(part) => out.push(part),
                    None => return Ok(None),
                }
            }
            Ok(Some(out))
        })?;
        let Some(results) = results else {
            return Ok(None);
        };
        // Parts cover consecutive slices, so the games stay in entry order.
        let mut tree = PlayerTree::default();
        for part in results {
            tree.unreadable += part.unreadable;
            tree.lines.extend(part.lines);
        }
        Ok(Some(tree))
    }
}

/// The average rating over the rated games among the last ten, as ChessBase
/// shows when preparing (`0x1407ab590`); `None` when none is rated.
pub fn recent_rating(ratings: &[u16]) -> Option<u32> {
    let last = &ratings[ratings.len().saturating_sub(10)..];
    let rated: Vec<u32> = last
        .iter()
        .filter(|r| **r > 0)
        .map(|r| u32::from(*r))
        .collect();
    (!rated.is_empty()).then(|| rated.iter().sum::<u32>() / rated.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{GameStats, pack_move, position_hash};
    use shakmaty::{Color, uci::UciMove};

    /// A main line as `main_line` gives it, from the start position.
    fn line(moves: &[&str], result: u8) -> GameLine {
        let mut pos = Chess::default();
        let mut positions = vec![];
        for uci in moves {
            let m = UciMove::from_ascii(uci.as_bytes())
                .unwrap()
                .to_move(&pos)
                .unwrap();
            positions.push((
                position_hash(pos.board(), pos.turn()),
                pos.turn() == Color::Black,
                pack_move(&m).unwrap(),
            ));
            pos.play_unchecked(m);
        }
        positions.push((
            position_hash(pos.board(), pos.turn()),
            pos.turn() == Color::Black,
            NO_MOVE,
        ));
        GameLine {
            stats: GameStats {
                result,
                white_elo: 2700,
                black_elo: 2600,
                year: 2020,
            },
            positions,
        }
    }

    fn sample() -> PlayerTree {
        PlayerTree {
            lines: vec![
                (1, line(&["e2e4", "c7c5", "g1f3"], 1)),
                (2, line(&["e2e4", "c7c5", "g1f3"], 2)),
                (3, line(&["e2e4", "e7e5", "g1f3"], 3)),
                (4, line(&["d2d4", "d7d5"], 1)),
            ],
            unreadable: 0,
        }
    }

    #[test]
    fn answers_positions_from_the_lines() {
        let tree = sample();
        let start = Chess::default();
        let moves = tree.moves_at(position_hash(start.board(), start.turn()));
        assert_eq!(moves.len(), 2);
        assert_eq!(moves[0].games, 3);
        assert_eq!(moves[0].uci().as_deref(), Some("e2e4"));
        // White's score after 1.e4: a win, a draw and a loss.
        assert_eq!(moves[0].score(), Some(50.0));
        assert_eq!(
            tree.games_at(position_hash(start.board(), start.turn())),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn repertoire_follows_the_moves_played_often_enough() {
        let tree = sample();
        let lines = tree.repertoire(true);
        // With four games a move needs two to continue a line, so 1.d4 and
        // 1.e4 e5 are left out and 1.e4 c5 2.Nf3 is the only line.
        assert_eq!(lines.len(), 1);
        let moves: Vec<String> = lines[0]
            .moves
            .iter()
            .map(|c| crate::tree::unpack_uci(*c).unwrap())
            .collect();
        assert_eq!(moves, ["e2e4", "c7c5", "g1f3"]);
        assert_eq!(lines[0].stats.games, 2);
        // Black is to move there, so the scores are the opponent's.
        assert!(!lines[0].player_to_move);
    }

    #[test]
    fn recent_rating_averages_the_rated_of_the_last_ten() {
        assert_eq!(recent_rating(&[]), None);
        assert_eq!(recent_rating(&[0, 0]), None);
        let mut r = vec![1000u16; 5];
        r.extend([2700, 0, 2600, 2800, 0, 0, 0, 0, 0, 0]);
        assert_eq!(recent_rating(&r), Some(2700));
    }
}
