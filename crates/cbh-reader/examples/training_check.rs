//! Checks the training annotations of a database: every move answer is a
//! legal move in the position before the move it is attached to, under
//! 0-based or 1-based (the marks' rule) squares; each game round-trips
//! through the 2CBA encoder; and, given a 2CBH copy, its questions agree.
//!   training_check <db> <file of entry numbers> [<2cbh to compare>]
use cbh_reader::{Database, ReadLimits};
use chess_core::training::TrainingKind;
use shakmaty::{Position, Square};

fn legal(pos: &shakmaty::Chess, from: u8, to: u8, one_based: bool) -> bool {
    let sq = |c: u8| -> Option<Square> {
        let i = if one_based { c.checked_sub(1)? } else { c };
        (i < 64).then(|| {
            Square::from_coords(
                shakmaty::File::new(u32::from(i / 8)),
                shakmaty::Rank::new(u32::from(i % 8)),
            )
        })
    };
    let (Some(f), Some(t)) = (sq(from), sq(to)) else {
        return false;
    };
    pos.legal_moves().iter().any(|m| {
        m.from() == Some(f)
            && (m.to() == t
                || matches!(m, shakmaty::Move::Castle { .. }) && t.file() == shakmaty::File::G
                || t.file() == shakmaty::File::C && matches!(m, shakmaty::Move::Castle { .. }))
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("db")?;
    let list = std::fs::read_to_string(args.next().ok_or("list")?)?;
    let other = args.next();
    let mut db = Database::open(std::path::Path::new(&path), ReadLimits::default())?;
    let mut other = other
        .map(|p| Database::open(std::path::Path::new(&p), ReadLimits::default()))
        .transpose()?;
    let (
        mut questions,
        mut answers,
        mut one,
        mut zero,
        mut games,
        mut incomplete,
        mut same,
        mut compared,
    ) = (0, 0, 0, 0, 0, 0, 0, 0);
    let (mut choice, mut marks, mut equal_played, mut round_trips) = (0, 0, 0, 0);
    for line in list.lines() {
        let ordinal: u64 = line.trim().parse()?;
        let loaded = db.load(ordinal)?;
        let Some(game) = loaded.game else { continue };
        games += 1;
        if loaded
            .diagnostics
            .iter()
            .any(|d| d.message.contains("0x09"))
        {
            incomplete += 1;
        }
        for node in &game.nodes {
            let Some(t) = &node.training else { continue };
            questions += 1;
            if t.kind == TrainingKind::MultipleChoice {
                choice += 1;
            }
            for b in std::iter::once(&t.question)
                .chain(&t.help)
                .chain(t.answers.iter().map(|a| &a.feedback))
            {
                marks += b.squares.len() + b.arrows.len();
            }
            if t.kind != TrainingKind::MoveInput {
                continue;
            }
            let Some(parent) = node.parent else { continue };
            let pos = game.chess_position(parent)?;
            let played = node.uci.clone().unwrap_or_default();
            for a in &t.answers {
                answers += 1;
                if legal(&pos, a.from, a.to, true) {
                    one += 1;
                }
                if legal(&pos, a.from, a.to, false) {
                    zero += 1;
                }
                let name = |c: u8| chess_core::training::square_name(c).unwrap_or_default();
                if played.starts_with(&format!("{}{}", name(a.from), name(a.to))) {
                    equal_played += 1;
                }
            }
        }
        // Round trip through the 2CBA encoder.
        let (payload, omitted) = cbh_reader::v2::encode::encode_annotations(&game);
        let parsed = cbh_reader::annotations::parse_new(&payload)?;
        let mut back = game.clone();
        for n in &mut back.nodes {
            n.training = None;
        }
        cbh_reader::annotations::apply(&mut back, parsed, cbh_reader::annotations::Order::Line);
        let trainings = |g: &chess_core::Game| -> Vec<_> {
            g.nodes.iter().map(|n| n.training.clone()).collect()
        };
        if trainings(&back) == trainings(&game) && omitted.notes.is_empty() {
            round_trips += 1;
        }
        if let Some(o) = other.as_mut()
            && ordinal <= o.info().entry_count
        {
            let theirs = o.load(ordinal)?.game;
            compared += 1;
            let mine: Vec<_> = game
                .nodes
                .iter()
                .filter_map(|n| n.training.clone())
                .collect();
            let back: Vec<_> = theirs
                .iter()
                .flat_map(|g| g.nodes.iter().filter_map(|n| n.training.clone()))
                .collect();
            if mine == back {
                same += 1;
            }
        }
    }
    println!(
        "{games} games, {questions} questions ({choice} multiple choice), {marks} marks; {answers} move answers: legal 0-based {zero}, 1-based {one}; {equal_played} are the move played; {incomplete} games still report 0x09; compared {compared}, equal {same}; {round_trips} round trips through 2CBA"
    );
    Ok(())
}
