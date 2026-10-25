//! The deck model and the lines (cards) of a game.
use chess_core::{
    Game, NodeId,
    model::{parse_position, uci_of},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shakmaty::{Chess, Color, Position, san::SanPlus, uci::UciMove};
use std::collections::BTreeMap;

/// Cards per deck at most; a source with more is truncated.
pub const MAX_CARDS: usize = 20_000;
/// Plies per line at most.
pub const MAX_PLIES: usize = 400;
/// The deck file version this build reads and writes.
pub const DECK_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Colour {
    White,
    Black,
}

impl Colour {
    pub fn name(self) -> &'static str {
        match self {
            Colour::White => "white",
            Colour::Black => "black",
        }
    }
    pub fn of(color: Color) -> Self {
        match color {
            Color::White => Colour::White,
            Color::Black => Colour::Black,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TraineeChoice {
    /// The side to move in the game's start position (right for puzzles).
    #[default]
    SideToMove,
    White,
    Black,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NewOrder {
    #[default]
    Source,
    Random,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeckOptions {
    /// Learning steps in minutes.
    pub learning_steps: Vec<f64>,
    /// Relearning steps in minutes.
    pub relearning_steps: Vec<f64>,
    /// Days.
    pub graduating_interval: f64,
    /// Days.
    pub easy_interval: f64,
    pub new_per_day: u32,
    pub reviews_per_day: u32,
    /// Days.
    pub max_interval: f64,
    pub new_order: NewOrder,
    pub include_variations: bool,
    pub trainee: TraineeChoice,
    /// New cards are first shown with each trainee move as an arrow; off,
    /// a new card is tried like any other.
    pub show_new_first: bool,
}

impl Default for DeckOptions {
    fn default() -> Self {
        Self {
            learning_steps: vec![1.0, 10.0],
            relearning_steps: vec![10.0],
            graduating_interval: 1.0,
            easy_interval: 4.0,
            new_per_day: 20,
            reviews_per_day: 200,
            max_interval: 36_500.0,
            new_order: NewOrder::Source,
            include_variations: true,
            trainee: TraineeChoice::SideToMove,
            show_new_first: false,
        }
    }
}

fn invalid(message: &str) -> crate::commands::AppError {
    crate::commands::AppError::new("invalidParameters", message)
}

impl DeckOptions {
    /// Refuses options the scheduler cannot work with.
    pub fn check(&self) -> Result<(), crate::commands::AppError> {
        let steps_ok = |steps: &[f64]| {
            steps.len() <= 20
                && steps
                    .iter()
                    .all(|s| s.is_finite() && *s > 0.0 && *s <= 43_200.0)
        };
        if self.learning_steps.is_empty() || !steps_ok(&self.learning_steps) {
            return Err(invalid(
                "Learning steps are 1 to 20 times from 1 second to 30 days, in minutes",
            ));
        }
        if !steps_ok(&self.relearning_steps) {
            return Err(invalid(
                "Relearning steps are up to 20 times from 1 second to 30 days, in minutes",
            ));
        }
        let days = |v: f64| v.is_finite() && (1.0..=36_500.0).contains(&v);
        if !days(self.graduating_interval) || !days(self.easy_interval) {
            return Err(invalid("Intervals are from 1 to 36500 days"));
        }
        if self.easy_interval < self.graduating_interval {
            return Err(invalid(
                "The easy interval is not shorter than the graduating interval",
            ));
        }
        if !days(self.max_interval) {
            return Err(invalid("The maximum interval is from 1 to 36500 days"));
        }
        if self.new_per_day > 100_000 || self.reviews_per_day > 100_000 {
            return Err(invalid("Daily limits are at most 100000"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardMove {
    pub uci: String,
    pub san: String,
    /// The PGN comment after the move, shown once it is played.
    pub comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub start_fen: String,
    pub moves: Vec<CardMove>,
    pub trainee: Colour,
    pub white: String,
    pub black: String,
    pub event: String,
    pub date: String,
    /// File name or database name and the entry or game number.
    pub source_label: String,
    /// Position in the source, for the source order of new cards.
    pub order: u32,
    pub suspended: bool,
    /// The last update from the source no longer had this line (it is also
    /// suspended); progress is kept.
    #[serde(default)]
    pub removed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    #[default]
    New,
    Learning,
    Review,
    Relearning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Grade {
    Again,
    Hard,
    Good,
    Easy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CardState {
    pub kind: Kind,
    /// Index into the learning or relearning steps.
    pub step: u32,
    /// Milliseconds since the epoch.
    pub due: i64,
    /// Days.
    pub interval: f64,
    pub ease: f64,
    pub reps: u32,
    pub lapses: u32,
}

impl Default for CardState {
    fn default() -> Self {
        Self {
            kind: Kind::New,
            step: 0,
            due: 0,
            interval: 0.0,
            ease: 2.5,
            reps: 0,
            lapses: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLog {
    pub card_id: String,
    /// When the card was answered, milliseconds since the epoch.
    pub time: i64,
    pub grade: Grade,
    /// The kind of the card before the answer.
    pub kind: Kind,
    pub mistakes: u32,
    pub hints: u32,
    pub millis: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    #[default]
    Pgn,
    List,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deck {
    pub version: u32,
    pub id: String,
    pub name: String,
    /// Milliseconds since the epoch.
    pub created: i64,
    /// The PGN file's path, or the name of the database the list came from.
    pub source: String,
    #[serde(default)]
    pub source_kind: SourceKind,
    pub options: DeckOptions,
    pub cards: Vec<Card>,
    /// Cards that have been answered; the others are new.
    pub states: BTreeMap<String, CardState>,
    pub log: Vec<ReviewLog>,
}

/// The id of a line: SHA-256 (the first 16 bytes, hex) of the start FEN's
/// first four fields, the moves in UCI and the trainee colour.
pub fn card_id<'a>(
    start_fen: &str,
    moves: impl IntoIterator<Item = &'a str>,
    trainee: Colour,
) -> String {
    let fen: Vec<&str> = start_fen.split_whitespace().take(4).collect();
    let moves: Vec<&str> = moves.into_iter().collect();
    let text = format!("{} {} {}", fen.join(" "), moves.join(" "), trainee.name());
    let digest = Sha256::digest(text.as_bytes());
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// The comments of a move as one text.
fn comment_of(game: &Game, node: NodeId) -> Option<String> {
    let parts: Vec<&str> = game.nodes[node as usize]
        .comments
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

struct Walk<'a> {
    game: &'a Game,
    include_variations: bool,
    start_turn: Color,
    trainee: Colour,
    max_lines: usize,
    path: Vec<CardMove>,
    out: Vec<Card>,
}

impl Walk<'_> {
    fn emit(&mut self) {
        // The line ends with the trainee's last move.
        let mover = |ply: usize| {
            if ply.is_multiple_of(2) {
                self.start_turn
            } else {
                !self.start_turn
            }
        };
        let Some(last) = (0..self.path.len())
            .rev()
            .find(|&ply| Colour::of(mover(ply)) == self.trainee)
        else {
            return;
        };
        let moves: Vec<CardMove> = self.path[..=last].to_vec();
        let id = card_id(
            &self.game.start_fen,
            moves.iter().map(|m| m.uci.as_str()),
            self.trainee,
        );
        // Paths that differ only after the trainee's last move are one line.
        if self.out.iter().any(|c| c.id == id) {
            return;
        }
        let tag = |name: &str| self.game.tags.get(name).cloned().unwrap_or_default();
        self.out.push(Card {
            id,
            start_fen: self.game.start_fen.clone(),
            moves,
            trainee: self.trainee,
            white: tag("White"),
            black: tag("Black"),
            event: tag("Event"),
            date: tag("Date"),
            source_label: String::new(),
            order: 0,
            suspended: false,
            removed: false,
        });
    }

    fn walk(&mut self, node: NodeId, pos: &Chess) {
        if self.out.len() >= self.max_lines {
            return;
        }
        let children = &self.game.nodes[node as usize].children;
        let take = if self.include_variations {
            children.len()
        } else {
            children.len().min(1)
        };
        let mut extended = false;
        if self.path.len() < MAX_PLIES {
            for &child in &children[..take] {
                let Some(uci) = self.game.nodes[child as usize].uci.as_deref() else {
                    continue;
                };
                // A null move ends the line; so does a move that cannot be played.
                let Some(m) = uci
                    .parse::<UciMove>()
                    .ok()
                    .and_then(|u| u.to_move(pos).ok())
                else {
                    continue;
                };
                let uci = uci_of(pos, m);
                let san = SanPlus::from_move(pos.clone(), m).to_string();
                let mut next = pos.clone();
                next.play_unchecked(m);
                extended = true;
                self.path.push(CardMove {
                    uci,
                    san,
                    comment: comment_of(self.game, child),
                });
                self.walk(child, &next);
                self.path.pop();
            }
        }
        if !extended {
            self.emit();
        }
    }
}

/// At most `max_lines` lines, for a source that may have far more.
pub fn lines_limited(
    game: &Game,
    include_variations: bool,
    trainee: TraineeChoice,
    max_lines: usize,
) -> Vec<Card> {
    let Ok(start) = parse_position(&game.start_fen) else {
        return vec![];
    };
    let trainee = match trainee {
        TraineeChoice::SideToMove => Colour::of(start.turn()),
        TraineeChoice::White => Colour::White,
        TraineeChoice::Black => Colour::Black,
    };
    let mut walk = Walk {
        game,
        include_variations,
        start_turn: start.turn(),
        trainee,
        max_lines,
        path: vec![],
        out: vec![],
    };
    walk.walk(0, &start);
    walk.out
}

/// The lines of a game: every root-to-leaf path with variations, else the
/// main line. Trailing opponent moves are dropped; a path without a trainee
/// move is not a card. The caller sets the source label and the order.
pub fn lines_of(game: &Game, include_variations: bool, trainee: TraineeChoice) -> Vec<Card> {
    lines_limited(game, include_variations, trainee, usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_core::{GameLimits, parse_pgn};

    fn game(pgn: &str) -> Game {
        parse_pgn(pgn.as_bytes(), &GameLimits::default())
            .unwrap()
            .remove(0)
    }

    fn ucis(card: &Card) -> Vec<&str> {
        card.moves.iter().map(|m| m.uci.as_str()).collect()
    }

    const PUZZLE: &str = "[White \"Study\"]\n[Black \"Composer\"]\n[Event \"Back rank\"]\n[Date \"2020.01.02\"]\n[FEN \"r5k1/5ppp/8/8/8/8/5PPP/6K1 b - - 0 1\"]\n[SetUp \"1\"]\n\n1... Ra1# 0-1\n";
    const REPERTOIRE: &str = "1. e4 e5 (1... c5 2. Nf3 d6) 2. Nf3 Nc6 (2... d6 3. d4) 3. Bb5 *\n";

    #[test]
    fn a_puzzle_with_black_to_move_is_one_card_for_black() {
        let cards = lines_of(&game(PUZZLE), true, TraineeChoice::SideToMove);
        assert_eq!(cards.len(), 1);
        let card = &cards[0];
        assert_eq!(card.trainee, Colour::Black);
        assert_eq!(ucis(card), ["a8a1"]);
        assert_eq!(card.moves[0].san, "Ra1#");
        assert_eq!(card.start_fen, "r5k1/5ppp/8/8/8/8/5PPP/6K1 b - - 0 1");
        assert_eq!(
            (
                card.white.as_str(),
                card.black.as_str(),
                card.event.as_str(),
                card.date.as_str()
            ),
            ("Study", "Composer", "Back rank", "2020.01.02")
        );
    }

    #[test]
    fn a_repertoire_with_two_variations_is_three_cards() {
        let cards = lines_of(&game(REPERTOIRE), true, TraineeChoice::White);
        let mut got: Vec<Vec<&str>> = cards.iter().map(ucis).collect();
        got.sort();
        assert_eq!(
            got,
            vec![
                vec!["e2e4", "c7c5", "g1f3"],
                vec!["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"],
                vec!["e2e4", "e7e5", "g1f3", "d7d6", "d2d4"],
            ]
        );
        assert!(cards.iter().all(|c| c.trainee == Colour::White));
    }

    #[test]
    fn without_variations_only_the_main_line_is_a_card() {
        let cards = lines_of(&game(REPERTOIRE), false, TraineeChoice::White);
        assert_eq!(cards.len(), 1);
        assert_eq!(ucis(&cards[0]), ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"]);
    }

    #[test]
    fn trailing_opponent_moves_are_dropped() {
        let cards = lines_of(&game("1. e4 e5 2. Nf3 Nc6 *"), true, TraineeChoice::White);
        assert_eq!(ucis(&cards[0]), ["e2e4", "e7e5", "g1f3"]);
    }

    #[test]
    fn a_line_may_start_with_the_opponents_move() {
        let cards = lines_of(&game("1. e4 e5 2. Nf3 *"), true, TraineeChoice::Black);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].trainee, Colour::Black);
        assert_eq!(ucis(&cards[0]), ["e2e4", "e7e5"]);
    }

    #[test]
    fn a_path_without_a_trainee_move_is_not_a_card() {
        // Black has no move on either path.
        let cards = lines_of(&game("1. e4 (1. d4) *"), true, TraineeChoice::Black);
        assert!(cards.is_empty());
        // The variation is only the opponent's.
        let cards = lines_of(&game("1. e4 e5 (1... c5) *"), true, TraineeChoice::White);
        assert_eq!(cards.len(), 1);
        assert_eq!(ucis(&cards[0]), ["e2e4"]);
    }

    #[test]
    fn equal_lines_have_equal_stable_ids() {
        let a = lines_of(&game(REPERTOIRE), true, TraineeChoice::White);
        let b = lines_of(&game(REPERTOIRE), true, TraineeChoice::White);
        assert_eq!(
            a.iter().map(|c| &c.id).collect::<Vec<_>>(),
            b.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
        let ids: std::collections::BTreeSet<_> = a.iter().map(|c| c.id.clone()).collect();
        assert_eq!(ids.len(), 3);
        // The trainee colour is part of the id.
        let black = lines_of(&game("1. e4 e5 *"), true, TraineeChoice::Black);
        let white = lines_of(&game("1. e4 e5 *"), true, TraineeChoice::White);
        assert_ne!(black[0].id, white[0].id);
    }

    #[test]
    fn the_id_is_the_hash_of_fen_moves_and_colour() {
        let cards = lines_of(&game("1. e4 *"), true, TraineeChoice::White);
        let text = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - e2e4 white".to_string();
        let digest = Sha256::digest(text.as_bytes());
        let expected: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(cards[0].id, expected);
        assert_eq!(expected.len(), 32);
    }

    #[test]
    fn comments_are_kept_per_move() {
        let cards = lines_of(
            &game("1. e4 {King's pawn} e5 2. Nf3 {develops} *"),
            true,
            TraineeChoice::White,
        );
        let comments: Vec<_> = cards[0]
            .moves
            .iter()
            .map(|m| m.comment.as_deref())
            .collect();
        assert_eq!(comments, [Some("King's pawn"), None, Some("develops")]);
    }

    #[test]
    fn a_line_is_cut_at_the_ply_limit_and_the_lines_at_the_limit_given() {
        let mut pgn = String::new();
        for _ in 0..250 {
            pgn.push_str("Nf3 Nf6 Ng1 Ng8 ");
        }
        // Repeated positions are legal; the number of plies is 1000.
        let long = game(&format!("1. {} *", pgn.trim()));
        let cards = lines_of(&long, true, TraineeChoice::White);
        assert_eq!(cards[0].moves.len(), MAX_PLIES - 1);
        let two = lines_limited(&game(REPERTOIRE), true, TraineeChoice::White, 2);
        assert_eq!(two.len(), 2);
    }
}
