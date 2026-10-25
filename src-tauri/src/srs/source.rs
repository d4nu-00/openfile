//! Reading the games a deck is made from: a PGN file or the rows of a
//! database list. Sources are only read.
use crate::{
    clipboard::RowRange,
    commands::AppError,
    registry::Registry,
    srs::deck::{Card, DeckOptions, MAX_CARDS, lines_limited},
};
use chess_core::{Game, GameLimits, parse_pgn};
use serde::Serialize;
use std::{collections::HashSet, path::Path};

/// A PGN file is read up to this size.
pub const MAX_PGN_BYTES: u64 = 256 * 1024 * 1024;
/// Games of a list used at most.
pub const MAX_ROWS: usize = 1_000_000;

/// What a source had that did not make it into the deck.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    /// Games that could not be read or parsed.
    pub games: u32,
    /// Lines over the limit of cards per deck.
    pub cards_over_limit: u32,
    /// Games after the limit was reached, which were not read.
    pub games_not_read: u32,
}

/// Gathers the cards of the games of a source, in order.
pub struct Collector {
    include_variations: bool,
    trainee: crate::srs::deck::TraineeChoice,
    limit: usize,
    cards: Vec<Card>,
    seen: HashSet<String>,
    skipped: Skipped,
}

impl Collector {
    pub fn new(options: &DeckOptions) -> Self {
        Self::with_limit(options, MAX_CARDS)
    }

    pub fn with_limit(options: &DeckOptions, limit: usize) -> Self {
        Self {
            include_variations: options.include_variations,
            trainee: options.trainee,
            limit,
            cards: vec![],
            seen: HashSet::new(),
            skipped: Skipped::default(),
        }
    }

    /// No more cards fit.
    pub fn full(&self) -> bool {
        self.cards.len() >= self.limit
    }

    /// The lines of a game. A line that is already in the deck (equal id)
    /// is kept once, at its first place.
    pub fn add_game(&mut self, label: &str, game: &Game) {
        for mut card in lines_limited(game, self.include_variations, self.trainee, MAX_CARDS) {
            if !self.seen.insert(card.id.clone()) {
                continue;
            }
            if self.full() {
                self.skipped.cards_over_limit += 1;
                continue;
            }
            card.source_label = label.to_string();
            card.order = self.cards.len() as u32;
            self.cards.push(card);
        }
    }

    pub fn add_unreadable(&mut self) {
        self.skipped.games += 1;
    }

    pub fn add_unread(&mut self, games: usize) {
        self.skipped.games_not_read += games.min(u32::MAX as usize) as u32;
    }

    pub fn finish(self) -> (Vec<Card>, Skipped) {
        (self.cards, self.skipped)
    }
}

/// The games of a PGN text as separate pieces: a tag line that follows
/// movetext starts the next game. Braces keep a comment's lines together.
pub fn split_games(text: &str) -> Vec<&str> {
    let mut pieces = vec![];
    let (mut start, mut at) = (0, 0);
    let (mut in_comment, mut has_movetext) = (false, false);
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if !in_comment && trimmed.starts_with('[') {
            if has_movetext {
                pieces.push(&text[start..at]);
                start = at;
                has_movetext = false;
            }
        } else if !trimmed.is_empty() {
            has_movetext = true;
            for c in trimmed.chars() {
                match (in_comment, c) {
                    (false, '{') => in_comment = true,
                    (true, '}') => in_comment = false,
                    _ => {}
                }
            }
        }
        at += line.len();
    }
    if !text[start..].trim().is_empty() {
        pieces.push(&text[start..]);
    }
    pieces
}

/// Collects the games of a PGN file. A game that does not parse is counted
/// and skipped; the others are used.
pub fn collect_pgn(path: &Path, collector: &mut Collector) -> Result<(), AppError> {
    let meta = std::fs::metadata(path).map_err(|e| AppError::new("io", e.to_string()))?;
    if meta.len() > MAX_PGN_BYTES {
        return Err(AppError::new("limitExceeded", "The PGN file is too large"));
    }
    let bytes = std::fs::read(path).map_err(|e| AppError::new("io", e.to_string()))?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let limits = GameLimits::default();
    let pieces = split_games(text);
    let mut number = 0usize;
    for (i, piece) in pieces.iter().enumerate() {
        if collector.full() {
            collector.add_unread(pieces.len() - i);
            break;
        }
        match parse_pgn(piece.as_bytes(), &limits) {
            Ok(games) => {
                for game in games {
                    number += 1;
                    collector.add_game(&format!("{name} \u{b7} game {number}"), &game);
                }
            }
            Err(_) => {
                number += 1;
                collector.add_unreadable();
            }
        }
    }
    Ok(())
}

/// Collects the games in `ranges` of the current list of `database_id`,
/// reading them with a separate handle so the list stays usable. Returns
/// the database's name.
pub fn collect_rows(
    registry: &Registry,
    database_id: &str,
    ranges: &[RowRange],
    collector: &mut Collector,
) -> Result<String, AppError> {
    let (ordinals, truncated) = registry.ordinals_of_rows(database_id, ranges, MAX_ROWS)?;
    if truncated {
        return Err(AppError::new(
            "limitExceeded",
            format!("Use at most {MAX_ROWS} games"),
        ));
    }
    let source_path = registry
        .lock()?
        .paths
        .get(database_id)
        .cloned()
        .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
    let name = source_path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut source = cbh_reader::Database::open(&source_path, cbh_reader::ReadLimits::default())?;
    for (i, &ordinal) in ordinals.iter().enumerate() {
        if collector.full() {
            collector.add_unread(ordinals.len() - i);
            break;
        }
        match source.load(ordinal) {
            Ok(loaded) => match loaded.game {
                Some(game) => collector.add_game(&format!("{name} \u{b7} entry {ordinal}"), &game),
                None => collector.add_unreadable(),
            },
            Err(cbh_reader::CbhError::SourceChanged) => {
                return Err(cbh_reader::CbhError::SourceChanged.into());
            }
            Err(_) => collector.add_unreadable(),
        }
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_are_split_at_the_tags_that_follow_movetext() {
        let text =
            "[A \"1\"]\n[B \"2\"]\n\n1. e4 {a\n[comment line]\n} e5 *\n\n[A \"3\"]\n\n1. d4 *\n";
        let pieces = split_games(text);
        assert_eq!(pieces.len(), 2);
        assert!(pieces[0].contains("[comment line]") && !pieces[0].contains("d4"));
        assert!(pieces[1].starts_with("[A \"3\"]"));
        assert_eq!(split_games("1. e4 e5 *").len(), 1);
        assert!(split_games("  \n").is_empty());
    }

    #[test]
    fn lines_over_the_limit_are_counted_and_equal_lines_kept_once() {
        let options = DeckOptions::default();
        let games = parse_pgn(
            b"1. e4 (1. d4) (1. c4) (1. e4) *\n\n1. e4 *\n",
            &GameLimits::default(),
        )
        .unwrap();
        let mut c = Collector::with_limit(&options, 2);
        for (i, game) in games.iter().enumerate() {
            c.add_game(&format!("g{i}"), game);
        }
        assert!(c.full());
        let (cards, skipped) = c.finish();
        assert_eq!(cards.len(), 2);
        assert_eq!(skipped.cards_over_limit, 1);
        assert_eq!(cards[0].source_label, "g0");
        assert_eq!((cards[0].order, cards[1].order), (0, 1));
    }
}
