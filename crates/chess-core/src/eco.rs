//! Opening classification with ECO codes and opening names.
//!
//! The table is the lichess-org/chess-openings data (CC0-1.0), five TSV
//! files of ECO code, name and the moves in SAN, kept unchanged in
//! `data/eco/` (see docs/openings.md for its provenance). Each line is
//! replayed once, at first use, and its final position indexed. A game is
//! classified by the last position of its main line that the table holds,
//! so transpositions into a named line count, as lichess classifies.
use shakmaty::{CastlingMode, Chess, EnPassantMode, Position, san::San, zobrist::Zobrist64};
use std::{collections::HashMap, sync::OnceLock};

const FILES: [&str; 5] = [
    include_str!("../data/eco/a.tsv"),
    include_str!("../data/eco/b.tsv"),
    include_str!("../data/eco/c.tsv"),
    include_str!("../data/eco/d.tsv"),
    include_str!("../data/eco/e.tsv"),
];

/// Main-line plies looked at; the longest line of the table has 36.
pub const MAX_PLIES: usize = 60;

/// One named opening line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opening {
    /// "A00" to "E99".
    pub eco: String,
    pub name: String,
    /// Plies of the table's line.
    pub plies: u16,
}

/// A game's classification: the opening, and after how many plies of the
/// game its position was reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classification {
    pub opening: Opening,
    pub ply: u16,
}

struct Table {
    by_position: HashMap<u64, usize>,
    openings: Vec<Opening>,
}

fn key(position: &Chess) -> u64 {
    position.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = Table {
            by_position: HashMap::new(),
            openings: vec![],
        };
        for file in FILES {
            for line in file.lines().skip(1) {
                let mut fields = line.split('\t');
                let (Some(eco), Some(name), Some(pgn)) =
                    (fields.next(), fields.next(), fields.next())
                else {
                    continue;
                };
                let mut position = Chess::default();
                let mut plies = 0u16;
                let mut legal = true;
                for token in pgn.split_whitespace() {
                    if token.ends_with('.') {
                        continue;
                    }
                    match token
                        .parse::<San>()
                        .ok()
                        .and_then(|san| san.to_move(&position).ok())
                    {
                        Some(m) => {
                            position.play_unchecked(m);
                            plies += 1;
                        }
                        None => {
                            legal = false;
                            break;
                        }
                    }
                }
                if !legal || plies == 0 {
                    continue;
                }
                // The data has one line per position; should two reach the
                // same one, the first is kept.
                let index = table.openings.len();
                if let std::collections::hash_map::Entry::Vacant(e) =
                    table.by_position.entry(key(&position))
                {
                    e.insert(index);
                    table.openings.push(Opening {
                        eco: eco.to_string(),
                        name: name.to_string(),
                        plies,
                    });
                }
            }
        }
        table
    })
}

/// Lines in the table.
pub fn len() -> usize {
    table().openings.len()
}

/// Classifies a main line given as positions after each ply, the start
/// position first. Only games from the standard start position are
/// classified; the last of the first [`MAX_PLIES`] plies' positions that the
/// table holds wins.
pub fn classify(positions: &[Chess]) -> Option<Classification> {
    let start = positions.first()?;
    if start.castles().mode() != CastlingMode::Standard || key(start) != key(&Chess::default()) {
        return None;
    }
    let table = table();
    positions
        .iter()
        .enumerate()
        .take(MAX_PLIES + 1)
        .skip(1)
        .rev()
        .find_map(|(ply, p)| {
            table.by_position.get(&key(p)).map(|&i| Classification {
                opening: table.openings[i].clone(),
                ply: ply as u16,
            })
        })
}

/// Classifies a game's main line; `None` for set-ups and games the table
/// does not know. The line is read up to [`MAX_PLIES`] plies or its first
/// null move.
pub fn classify_game(game: &crate::Game) -> Option<Classification> {
    let mut positions = vec![crate::model::parse_position(&game.start_fen).ok()?];
    let mut at = 0usize;
    while positions.len() <= MAX_PLIES {
        let Some(&next) = game.nodes.get(at)?.children.first() else {
            break;
        };
        let Some(uci) = game.nodes.get(next as usize)?.uci.as_deref() else {
            break;
        };
        if uci == "0000" {
            break;
        }
        let Ok(p) = crate::model::apply_uci(positions.last()?.clone(), uci) else {
            break;
        };
        positions.push(p);
        at = next as usize;
    }
    classify(&positions)
}

/// Classifies moves in UCI from the standard start position.
pub fn classify_uci<'a>(moves: impl IntoIterator<Item = &'a str>) -> Option<Classification> {
    let mut positions = vec![Chess::default()];
    for uci in moves.into_iter().take(MAX_PLIES) {
        let Ok(m) = uci
            .parse::<shakmaty::uci::UciMove>()
            .map_err(|_| ())
            .and_then(|u| u.to_move(positions.last().unwrap()).map_err(|_| ()))
        else {
            break;
        };
        let mut next = positions.last().unwrap().clone();
        next.play_unchecked(m);
        positions.push(next);
    }
    classify(&positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_of_the_table_is_indexed() {
        // 3,865 lines in the pinned data; all replay legally.
        assert!(len() > 3_800, "{}", len());
    }

    #[test]
    fn classifies_by_the_last_known_position() {
        let c = classify_uci([
            "e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4", "g8f6", "b1c3", "a7a6",
        ])
        .unwrap();
        assert_eq!(c.opening.eco, "B90");
        assert!(
            c.opening.name.starts_with("Sicilian Defense: Najdorf"),
            "{}",
            c.opening.name
        );
        assert_eq!(c.ply, 10);
        // Moves past the table keep the last match.
        let c = classify_uci(["e2e4", "e7e5", "g1f3", "g8f6", "a2a3", "a7a6"]).unwrap();
        assert_eq!(c.opening.eco, "C42");
        assert_eq!(c.ply, 4);
        // A transposition: 1.d4 Nf6 2.c4 e6 3.Nc3 Bb4 is the Nimzo-Indian
        // whichever way it is reached.
        let a = classify_uci(["d2d4", "g8f6", "c2c4", "e7e6", "b1c3", "f8b4"]).unwrap();
        let b = classify_uci(["c2c4", "e7e6", "d2d4", "g8f6", "b1c3", "f8b4"]).unwrap();
        assert_eq!(a.opening.eco, "E20");
        assert_eq!(a.opening, b.opening);
        assert!(classify_uci([]).is_none());
    }

    #[test]
    fn games_are_classified_by_their_main_line() {
        let g = crate::pgn::parse_pgn(
            b"1. e4 c5 (1... e5 2. Nf3) 2. Nf3 d6 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6 6. Be3 *\n",
            &crate::GameLimits::default(),
        )
        .unwrap()
        .remove(0);
        let c = classify_game(&g).unwrap();
        assert_eq!(c.opening.eco, "B90");
        // 6.Be3 is a named Najdorf line too.
        assert_eq!(c.ply, 11);
        // A set-up is not classified.
        let g = crate::Game::from_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap();
        assert!(classify_game(&g).is_none());
    }
}
