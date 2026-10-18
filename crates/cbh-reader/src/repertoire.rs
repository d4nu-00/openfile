//! Generate Repertoire: the reference app's White and Black repertoire
//! databases from a player's own games (`GenerateRepertoireDBsCmd`,
//! `0x14070e970`), and the positions a repertoire marks, for the Repertoire
//! Report (`0x14070f570`). See docs/repertoire.md.
//!
//! The player's games of one colour are grouped by the ECO code of the open
//! classification table. Each code played at least [`MIN_GAMES`] times
//! becomes one game: the games' opening moves merged into one tree, with the
//! player's move where the classification ends marked as a critical opening
//! position (annotation 0x18, kind 1: the reference app's "blue" move).
use crate::{
    CbhError, Database,
    prepare::map_games,
    tree::{GameStats, position_hash, unpack_uci},
};
use chess_core::{Game, GameLimits, NodeId, eco};
use shakmaty::{Chess, Position};
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::AtomicBool,
};

/// Plies of each game merged, as the reference app merges (`0x140711f20`).
pub const PLIES: usize = 24;
/// Plies read of each main line: enough for the classification.
pub const READ_PLIES: usize = eco::MAX_PLIES;
/// Games an ECO code needs to become a repertoire game (`0x140712500`).
pub const MIN_GAMES: usize = 2;
/// From this many games a code is split by opening variation
/// (the reference app clusters from 16, `0x140711a20`).
pub const SPLIT_FROM: usize = 16;
/// Games read per colour at most: the latest in entry order.
pub const MAX_GAMES: usize = 20_000;
/// Nodes of one repertoire game at most.
pub const MAX_NODES: usize = 60_000;
/// Repertoire games read for a report at most (the reference app's limit,
/// message "More than 5000 games in repertoire database").
pub const MAX_REPERTOIRE_GAMES: usize = 5_000;
/// Marked positions collected for a report at most.
pub const MAX_POSITIONS: usize = 20_000;

/// The critical position kind of a repertoire move.
pub const OPENING: u8 = 1;

/// A main line in UCI from the standard start position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub ordinal: u64,
    pub moves: Vec<String>,
}

/// One game of a repertoire database.
#[derive(Clone, Debug)]
pub struct RepertoireGame {
    pub eco: String,
    /// The opening's name in the classification table.
    pub name: Option<String>,
    /// Games merged.
    pub games: usize,
    /// Games whose moves did not all fit [`MAX_NODES`].
    pub cut: usize,
    pub game: Game,
}

#[derive(Clone, Debug, Default)]
pub struct Repertoire {
    /// In ECO order; a split code's variations by games, most first.
    pub games: Vec<RepertoireGame>,
    /// Lines the table does not classify (set-ups, fewer than 3 plies,
    /// unknown openings).
    pub unclassified: usize,
    /// Games of codes played fewer than [`MIN_GAMES`] times.
    pub rare: usize,
}

/// Splits a table name "Family: Variation, Line" in two, for the White and
/// Black name fields, as the reference app splits its ECO text
/// (`0x14070f020`); parentheses are removed.
pub fn name_fields(name: &str) -> (String, String) {
    let clean: String = name.chars().filter(|c| !matches!(c, '(' | ')')).collect();
    match clean.split_once(':') {
        Some((family, rest)) => (family.trim().to_string(), rest.trim().to_string()),
        None => (clean.trim().to_string(), String::new()),
    }
}

/// The move to mark: the player's move at or next to the last ply the
/// classification reached (`0x14070fcd0` forces the parity to the player's
/// side). `ply` counts plies, so the move index is `ply - 1`.
fn marked_move(ply: u16, white: bool) -> Option<usize> {
    let last = usize::from(ply).checked_sub(1)?;
    Some(if white { last & !1 } else { last | 1 })
}

/// Merges the lines into one game to [`PLIES`] plies and marks each line's
/// repertoire move. Returns the game and how many lines were cut short.
fn merge(lines: &[(&Line, u16)], white: bool) -> (Game, usize) {
    let mut game = Game::initial();
    let limits = GameLimits {
        max_nodes: MAX_NODES,
        ..GameLimits::default()
    };
    let mut cut = 0;
    for (line, ply) in lines {
        let mark = marked_move(*ply, white);
        let mut at: NodeId = 0;
        for (i, uci) in line.moves.iter().take(PLIES).enumerate() {
            match game.append_uci(at, uci, &limits) {
                Ok(id) => at = id,
                Err(_) => {
                    cut += 1;
                    break;
                }
            }
            if Some(i) == mark {
                game.nodes[at as usize].critical = Some(OPENING);
            }
        }
    }
    (game, cut)
}

/// Builds the repertoire of the player's `lines` with one colour.
pub fn build(lines: &[Line], white: bool) -> Repertoire {
    let mut out = Repertoire::default();
    let mut by_code: BTreeMap<String, Vec<(&Line, eco::Classification)>> = BTreeMap::new();
    for line in lines {
        let found = (line.moves.len() >= 3)
            .then(|| eco::classify_uci(line.moves.iter().map(String::as_str)))
            .flatten();
        match found {
            Some(c) => by_code
                .entry(c.opening.eco.clone())
                .or_default()
                .push((line, c)),
            None => out.unclassified += 1,
        }
    }
    for (code, games) in by_code {
        if games.len() < MIN_GAMES {
            out.rare += games.len();
            continue;
        }
        // Groups of the code: one, or one per table entry reached, with the
        // entries played once joining the code's most played entry.
        let groups: Vec<Vec<&(&Line, eco::Classification)>> = if games.len() < SPLIT_FROM {
            vec![games.iter().collect()]
        } else {
            let mut by_name: BTreeMap<&str, Vec<&(&Line, eco::Classification)>> = BTreeMap::new();
            for g in &games {
                by_name.entry(&g.1.opening.name).or_default().push(g);
            }
            let mut groups: Vec<_> = by_name.into_values().collect();
            groups.sort_by_key(|g| std::cmp::Reverse(g.len()));
            let (mut kept, small): (Vec<_>, Vec<_>) =
                groups.into_iter().partition(|g| g.len() >= MIN_GAMES);
            if kept.is_empty() {
                kept.push(vec![]);
            }
            for g in small {
                kept[0].extend(g);
            }
            for g in &mut kept {
                g.sort_by_key(|(line, _)| line.ordinal);
            }
            kept
        };
        for group in groups {
            let lines: Vec<(&Line, u16)> = group.iter().map(|(l, c)| (*l, c.ply)).collect();
            let (mut game, cut) = merge(&lines, white);
            let name = eco::classify_game(&game).map(|c| c.opening.name);
            let (first, second) = name_fields(name.as_deref().unwrap_or(&code));
            game.tags.insert("White".into(), first);
            game.tags.insert(
                "Black".into(),
                if second.is_empty() {
                    "?".into()
                } else {
                    second
                },
            );
            game.tags.insert("ECO".into(), code.clone());
            game.tags.insert(
                "Event".into(),
                if white {
                    "Repertoire White"
                } else {
                    "Repertoire Black"
                }
                .into(),
            );
            out.games.push(RepertoireGame {
                eco: code.clone(),
                name,
                games: lines.len(),
                cut,
                game,
            });
        }
    }
    out
}

/// A position a repertoire marks: after a move with the critical opening
/// mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkedPosition {
    /// Position hash as the opening tree computes it.
    pub hash: u64,
    /// Index of the repertoire game in the collected list.
    pub topic: usize,
    /// UCI moves from the game's start to the position.
    pub moves: Vec<String>,
}

/// The positions after `game`'s moves marked as critical opening positions,
/// in pre-order (each line before its variations); only games from the
/// standard start position.
pub fn marked_positions(game: &Game, topic: usize, limit: usize) -> Vec<MarkedPosition> {
    let mut out = vec![];
    let Ok(start) = chess_core::model::parse_position(&game.start_fen) else {
        return out;
    };
    if position_hash(start.board(), start.turn())
        != position_hash(Chess::default().board(), Chess::default().turn())
    {
        return out;
    }
    let mut stack: Vec<(NodeId, Chess, Vec<String>)> = vec![(0, start, vec![])];
    while let Some((id, pos, moves)) = stack.pop() {
        if out.len() >= limit {
            break;
        }
        let node = &game.nodes[id as usize];
        if id != 0 && node.critical == Some(OPENING) {
            out.push(MarkedPosition {
                hash: position_hash(pos.board(), pos.turn()),
                topic,
                moves: moves.clone(),
            });
        }
        for &child in node.children.iter().rev() {
            let Some(uci) = game.nodes[child as usize].uci.as_deref() else {
                continue;
            };
            let Ok(next) = chess_core::model::apply_uci(pos.clone(), uci) else {
                continue;
            };
            let mut line = moves.clone();
            line.push(uci.to_string());
            stack.push((child, next, line));
        }
    }
    out
}

impl Database {
    /// The main lines of `ordinals` in UCI from the standard start position,
    /// up to `plies`, read by up to `workers` threads; set-ups and lines that
    /// cannot be read are left out. None when cancelled.
    pub fn uci_lines(
        &self,
        ordinals: &[u64],
        plies: usize,
        workers: usize,
        cancel: &AtomicBool,
        progress: impl FnMut(u64, u64),
    ) -> Result<Option<Vec<Line>>, CbhError> {
        let start = Chess::default();
        let root = position_hash(start.board(), start.turn());
        let lines = map_games(
            self,
            ordinals,
            workers,
            cancel,
            progress,
            |db, ordinal| match db.main_line(ordinal, plies) {
                Ok(Some(line)) if line.positions.first().is_some_and(|p| p.0 == root) => {
                    Ok(Some(Line {
                        ordinal,
                        moves: line
                            .positions
                            .iter()
                            .map_while(|p| unpack_uci(p.2))
                            .collect(),
                    }))
                }
                Ok(_) => Ok(None),
                Err(e @ (CbhError::Io(_) | CbhError::SourceChanged)) => Err(e),
                Err(_) => Ok(None),
            },
        )?;
        Ok(lines.map(|l| l.into_iter().flatten().collect()))
    }

    /// For each of `ordinals`, the lowest rank among `ranks` of the
    /// positions its main line reaches within `plies`, with the game's
    /// statistics; `None` for deleted or unreadable games and games that
    /// reach none. Read by up to `workers` threads; None when cancelled.
    #[allow(clippy::type_complexity)]
    pub fn first_marked(
        &self,
        ordinals: &[u64],
        plies: usize,
        ranks: &HashMap<u64, usize>,
        workers: usize,
        cancel: &AtomicBool,
        progress: impl FnMut(u64, u64),
    ) -> Result<Option<Vec<Option<(usize, GameStats)>>>, CbhError> {
        map_games(self, ordinals, workers, cancel, progress, |db, ordinal| {
            let raw = db.raw_chunk(ordinal, 1)?.remove(0);
            if raw.deleted {
                return Ok(None);
            }
            match db.line_of(&raw, plies) {
                Ok(Some(line)) => Ok(line
                    .positions
                    .iter()
                    .filter_map(|p| ranks.get(&p.0).copied())
                    .min()
                    .map(|rank| (rank, line.stats))),
                Ok(None) => Ok(None),
                Err(e @ (CbhError::Io(_) | CbhError::SourceChanged)) => Err(e),
                Err(_) => Ok(None),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(ordinal: u64, moves: &[&str]) -> Line {
        Line {
            ordinal,
            moves: moves.iter().map(|m| m.to_string()).collect(),
        }
    }

    const NAJDORF: [&str; 10] = [
        "e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4", "g8f6", "b1c3", "a7a6",
    ];

    #[test]
    fn splits_names_like_the_name_fields() {
        assert_eq!(
            name_fields("Sicilian Defense: Najdorf Variation, English Attack"),
            (
                "Sicilian Defense".into(),
                "Najdorf Variation, English Attack".into()
            )
        );
        assert_eq!(
            name_fields("Bird Opening"),
            ("Bird Opening".into(), "".into())
        );
        assert_eq!(name_fields("King's Pawn (Game)").0, "King's Pawn Game");
    }

    #[test]
    fn marks_the_players_move_where_the_classification_ends() {
        // Plies 1..=10 reached: White's last move is index 8, Black's 9.
        assert_eq!(marked_move(10, true), Some(8));
        assert_eq!(marked_move(10, false), Some(9));
        assert_eq!(marked_move(5, true), Some(4));
        assert_eq!(marked_move(5, false), Some(5));
        assert_eq!(marked_move(0, true), None);
    }

    #[test]
    fn codes_played_twice_become_merged_games() {
        // Both are B90 (the English Attack), branching at White's 7th move.
        let mut a = NAJDORF.to_vec();
        a.extend(["c1e3", "e7e5", "d4b3"]);
        let mut b = NAJDORF.to_vec();
        b.extend(["c1e3", "e7e5", "d4f3"]);
        let lines = vec![
            line(1, &a),
            line(2, &b),
            // One game of another code is left out.
            line(3, &["d2d4", "d7d5", "c2c4"]),
            line(4, &["e2e4"]),
        ];
        let rep = build(&lines, false);
        assert_eq!(rep.games.len(), 1);
        assert_eq!(rep.rare, 1);
        assert_eq!(rep.unclassified, 1);
        let g = &rep.games[0];
        assert_eq!(g.eco, "B90");
        assert_eq!(g.games, 2);
        assert_eq!(g.game.tags["ECO"], "B90");
        assert_eq!(g.game.tags["White"], "Sicilian Defense");
        // The table knows 6.Be3 (11 plies), so Black's 6...e5 is marked,
        // once for both games.
        let marked = marked_positions(&g.game, 0, 100);
        assert_eq!(marked.len(), 1);
        assert_eq!(marked[0].moves.len(), 12);
        assert_eq!(marked[0].moves[11], "e7e5");
        assert_eq!(g.game.nodes.len(), 1 + 12 + 2);
    }

    #[test]
    fn a_large_code_is_split_by_variation() {
        let with = |m: &'static str| {
            let mut v = NAJDORF.to_vec();
            v.push(m);
            v
        };
        let mut lines = vec![];
        for i in 0..10 {
            lines.push(line(i, &with("c1e3")));
        }
        for i in 10..14 {
            lines.push(line(i, &with("h2h3")));
        }
        // Fewer than 16 games: one game for the code.
        let rep = build(&lines, true);
        assert_eq!(rep.games.len(), 1);
        assert_eq!(rep.games[0].games, 14);
        for i in 14..18 {
            lines.push(line(i, &with("h2h3")));
        }
        // 18 games: one game per table entry, the most played first.
        let rep = build(&lines, true);
        let names: Vec<(usize, Option<&str>)> = rep
            .games
            .iter()
            .map(|g| (g.games, g.name.as_deref()))
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert_eq!(names[0].0, 10);
        assert_eq!(names[1].0, 8);
        assert!(rep.games.iter().all(|g| g.eco == "B90"));
        // White's sixth move is the marked move in both.
        for g in &rep.games {
            let marked = marked_positions(&g.game, 0, 10);
            assert_eq!(marked.len(), 1);
            assert_eq!(marked[0].moves.len(), 11);
        }
    }
}
