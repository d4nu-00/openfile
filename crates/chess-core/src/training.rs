//! Training annotations: a question about the position after a move, with
//! hints and graded answers, as the reference app stores them (annotation
//! type 0x09). See docs/training.md.
use crate::{ArrowMark, SquareMark};
use serde::{Deserialize, Serialize};

/// Language slots of a text block, in the reference app's tab order.
pub const LANGUAGES: [&str; 7] = ["en", "de", "fr", "es", "it", "nl", "pt"];
/// The slot of a block's arrows and of its coloured squares.
pub const ARROW_SLOT: u8 = 7;
pub const SQUARE_SLOT: u8 = 8;
/// Answers of one question at most (the stored count is one byte).
pub const MAX_ANSWERS: usize = 32;
/// Characters of one text at most.
pub const MAX_TEXT: usize = 8_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrainingKind {
    /// The answer is a move on the board.
    MoveInput,
    /// The answer is one of several texts.
    MultipleChoice,
}

impl TrainingKind {
    /// The stored kind number.
    pub fn code(self) -> u32 {
        match self {
            TrainingKind::MoveInput => 1,
            TrainingKind::MultipleChoice => 2,
        }
    }
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(TrainingKind::MoveInput),
            2 => Some(TrainingKind::MultipleChoice),
            _ => None,
        }
    }
}

/// One text in one language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    /// 0..=6, see [`LANGUAGES`].
    pub language: u8,
    /// The string index; 0 for the text itself (1 was never seen).
    #[serde(default)]
    pub index: u8,
    pub text: String,
}

/// A stored string the workbench does not represent, kept as it was.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawString {
    pub slot: u8,
    pub index: u8,
    pub bytes: Vec<u8>,
}

/// A text block: the texts in each language, and the marks shown with it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    #[serde(default)]
    pub texts: Vec<Text>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub squares: Vec<SquareMark>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arrows: Vec<ArrowMark>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other: Vec<RawString>,
}

impl Block {
    pub fn is_empty(&self) -> bool {
        self.texts.iter().all(|t| t.text.is_empty())
            && self.squares.is_empty()
            && self.arrows.is_empty()
            && self.other.is_empty()
    }
    /// The text in `language`, else the first in slot order.
    pub fn text(&self, language: u8) -> Option<&str> {
        let main = |l: u8| {
            self.texts
                .iter()
                .find(|t| t.language == l && t.index == 0 && !t.text.is_empty())
        };
        main(language)
            .or_else(|| (0..LANGUAGES.len() as u8).find_map(main))
            .map(|t| t.text.as_str())
    }
}

/// One answer: a move (move input) or a choice (multiple choice).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// The move's squares as stored, and the stored byte after them
    /// (its meaning was not traced); 0 for a choice.
    #[serde(default)]
    pub from: u8,
    #[serde(default)]
    pub to: u8,
    #[serde(default)]
    pub extra: u8,
    pub points: u8,
    /// The choice's text (multiple choice).
    #[serde(default)]
    pub choice: Block,
    /// The reply when this answer is given.
    #[serde(default)]
    pub feedback: Block,
}

/// A training question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Training {
    pub kind: TrainingKind,
    /// The time allowed.
    pub seconds: i32,
    /// The question's points.
    pub score: i16,
    pub question: Block,
    /// The reply to a wrong answer.
    #[serde(default)]
    pub wrong: Block,
    #[serde(default)]
    pub help: [Block; 2],
    #[serde(default)]
    pub answers: Vec<Answer>,
}

/// The square name of an answer's stored square: 0-based, file-major (a1 =
/// 0, a2 = 1, …, h8 = 63). Unlike the 1-based marks, the test database's 3,687
/// move answers are all legal this way, in the position before the move the
/// question is attached to.
pub fn square_name(code: u8) -> Option<String> {
    (code < 64).then(|| format!("{}{}", (b'a' + code / 8) as char, code % 8 + 1))
}

/// The stored code of an answer's square.
pub fn square_code(name: &str) -> Option<u8> {
    let b = name.as_bytes();
    (b.len() == 2 && (b'a'..=b'h').contains(&b[0]) && (b'1'..=b'8').contains(&b[1]))
        .then(|| (b[0] - b'a') * 8 + (b[1] - b'1'))
}

impl Training {
    /// Checks the bounds of a question entered in the workbench.
    pub fn check(&self) -> Result<(), crate::CoreError> {
        let long = |b: &Block| b.texts.iter().any(|t| t.text.chars().count() > MAX_TEXT);
        let blocks = [&self.question, &self.wrong, &self.help[0], &self.help[1]];
        if self.answers.len() > MAX_ANSWERS
            || blocks.into_iter().any(long)
            || self
                .answers
                .iter()
                .any(|a| long(&a.choice) || long(&a.feedback))
            || self
                .answers
                .iter()
                .flat_map(|a| [&a.choice, &a.feedback])
                .chain(blocks)
                .flat_map(|b| &b.texts)
                .any(|t| usize::from(t.language) >= LANGUAGES.len() || t.index > 1)
            || !(0..=86_400).contains(&self.seconds)
            || !(0..=1_000).contains(&self.score)
        {
            return Err(crate::CoreError::LimitExceeded);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_squares_are_zero_based_and_file_major() {
        assert_eq!(square_name(0).as_deref(), Some("a1"));
        assert_eq!(square_name(8).as_deref(), Some("b1"));
        assert_eq!(square_name(63).as_deref(), Some("h8"));
        assert_eq!(square_name(64), None);
        assert_eq!(square_code("e4"), Some(35));
        assert_eq!(square_code("i1"), None);
    }

    #[test]
    fn a_block_falls_back_to_the_first_language() {
        let b = Block {
            texts: vec![Text {
                language: 1,
                index: 0,
                text: "Wie?".into(),
            }],
            ..Default::default()
        };
        assert_eq!(b.text(0), Some("Wie?"));
        assert!(!b.is_empty());
        assert!(Block::default().is_empty());
    }

    #[test]
    fn set_training_resolves_answer_moves_before_the_move() {
        let mut g = crate::Game::initial();
        let limits = crate::GameLimits::default();
        let e4 = g.append_uci(0, "e2e4", &limits).unwrap();
        let text = |t: &str| Block {
            texts: vec![Text {
                language: 0,
                index: 0,
                text: t.into(),
            }],
            ..Default::default()
        };
        let t = Training {
            kind: TrainingKind::MoveInput,
            seconds: 60,
            score: 10,
            question: text("Your move?"),
            wrong: Block::default(),
            help: Default::default(),
            answers: vec![
                Answer {
                    points: 10,
                    ..Default::default()
                },
                Answer {
                    points: 5,
                    ..Default::default()
                },
            ],
        };
        // The question is about the start position: 1.e4 and 1.Nf3.
        g.set_training(
            e4,
            Some(t.clone()),
            &[Some("e4".into()), Some("g1f3".into())],
        )
        .unwrap();
        let stored = g.nodes[e4 as usize].training.as_ref().unwrap();
        let name = |a: &Answer| (square_name(a.from).unwrap(), square_name(a.to).unwrap());
        assert_eq!(name(&stored.answers[0]), ("e2".into(), "e4".into()));
        assert_eq!(name(&stored.answers[1]), ("g1".into(), "f3".into()));
        // An illegal answer changes nothing; the start has no move to ask about.
        assert!(
            g.set_training(e4, Some(t.clone()), &[Some("e5".into())])
                .is_err()
        );
        assert!(g.set_training(0, Some(t.clone()), &[]).is_err());
        // No question text removes it.
        let mut empty = t;
        empty.question = Block::default();
        g.set_training(e4, Some(empty), &[]).unwrap();
        assert!(g.nodes[e4 as usize].training.is_none());
        assert_eq!(g.clear_training(), 0);
    }
}
