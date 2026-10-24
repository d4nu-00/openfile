//! The builder: walks the source games, verifies each game's chain of
//! mates step by step, fills the quotas and makes the puzzle games.
use crate::{
    commands::AppError,
    mate_training::verify::{MateVerifier, StepCheck},
};
use cbh_reader::{
    ending::{Tally, mating_pieces},
    search::final_position,
};
use chess_core::{
    Game, GameLimits,
    training::{Answer, Block, Text, Training, TrainingKind},
};
use shakmaty::{
    Chess, EnPassantMode, Position,
    fen::{Epd, Fen},
    uci::UciMove,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// The deepest mate built.
pub const MAX_DEPTH: u8 = 5;
/// Most puzzles of one combination and depth.
pub const MAX_PER: u32 = 1_000;
/// The Annotator tag of the puzzle games.
pub const ANNOTATOR: &str = "OpenFile mate training";
/// The tags a puzzle keeps from its source game (Elo is both ratings).
const KEPT_TAGS: [&str; 9] = [
    "White", "Black", "Event", "Site", "Date", "Round", "Result", "WhiteElo", "BlackElo",
];

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildOptions {
    pub min_depth: u8,
    pub max_depth: u8,
    /// Puzzles per (combination, depth), and chains per combination.
    pub per: u32,
    /// Only these combinations; every combination found when `None`.
    #[serde(default)]
    pub combinations: Option<Vec<String>>,
    /// Bishops and knights count as one kind of piece.
    #[serde(default)]
    pub pool_minors: bool,
}

impl BuildOptions {
    pub fn check(&self) -> Result<(), AppError> {
        let bad = |m: &str| Err(AppError::new("invalidParameters", m));
        if !(1..=MAX_DEPTH).contains(&self.min_depth)
            || !(1..=MAX_DEPTH).contains(&self.max_depth)
            || self.min_depth > self.max_depth
        {
            return bad("The depths must be from 1 to 5, the first not above the last");
        }
        if !(1..=MAX_PER).contains(&self.per) {
            return bad("Puzzles per combination must be from 1 to 1000");
        }
        for c in self.combinations.iter().flatten() {
            if normalise_combination(c, self.pool_minors).is_none() {
                return bad(&format!("\"{c}\" is not a combination such as Q+N"));
            }
        }
        Ok(())
    }
}

/// The pieces of `t` as a combination: Q, R, B, N, P, K joined by `+`; with
/// `pool_minors` bishops and knights are written `m`. A king taking part is
/// part of it.
pub fn combination(t: &Tally, pool_minors: bool) -> String {
    let mut parts = vec![];
    for (present, name) in [
        (t.queens > 0, "Q"),
        (t.rooks > 0, "R"),
        (pool_minors && t.bishops + t.knights > 0, "m"),
        (!pool_minors && t.bishops > 0, "B"),
        (!pool_minors && t.knights > 0, "N"),
        (t.pawns > 0, "P"),
        (t.kings > 0, "K"),
    ] {
        if present {
            parts.push(name);
        }
    }
    if parts.is_empty() {
        "-".into()
    } else {
        parts.join("+")
    }
}

/// A combination typed by the user in the form [`combination`] writes it:
/// order and spaces do not matter, nor case ("n + q" is "Q+N"). `None` for
/// an unknown piece letter or an empty list. `m` (minor) is a letter only
/// with `pool_minors`, where B and N both mean it.
pub fn normalise_combination(s: &str, pool_minors: bool) -> Option<String> {
    let mut have = BTreeSet::new();
    for part in s.split('+') {
        let part = part.trim().to_ascii_uppercase();
        let order = match (part.as_str(), pool_minors) {
            ("Q", _) => 0,
            ("R", _) => 1,
            ("M" | "B" | "N", true) => 2,
            ("B", false) => 2,
            ("N", false) => 3,
            ("P", _) => 4,
            ("K", _) => 5,
            _ => return None,
        };
        have.insert(order);
    }
    let names = ["Q", "R", if pool_minors { "m" } else { "B" }, "N", "P", "K"];
    Some(
        have.into_iter()
            .map(|i| names[i])
            .collect::<Vec<_>>()
            .join("+"),
    )
}

/// A puzzle found: its game and where it came from.
#[derive(Clone, Debug)]
pub struct Puzzle {
    pub combination: String,
    /// The source game's entry number.
    pub ordinal: u64,
    pub game: Game,
}

/// A source game's chain: its steps from mate in 1 up.
#[derive(Clone, Debug)]
pub struct Chain {
    pub combination: String,
    pub ordinal: u64,
    pub steps: Vec<Game>,
}

/// What the builder has found so far.
pub struct Collected {
    options: BuildOptions,
    filter: Option<BTreeSet<String>>,
    /// Index 0 is mate in 1.
    depths: Vec<Vec<Puzzle>>,
    chains: Vec<Chain>,
    counts: HashMap<(String, u8), u32>,
    chain_counts: HashMap<String, u32>,
    /// Combinations in order of first appearance.
    order: Vec<String>,
    /// Positions collected, by depth.
    seen: HashSet<(u8, String)>,
    seen_chains: HashSet<String>,
    /// Steps already verified valid: depth, position and the game's move.
    valid: HashSet<(u8, String, String)>,
    /// Games and steps not used, by reason: the `StepCheck` kinds, and
    /// `notMateGame`, `chess960`, `unreadable`, `underpromotion`, `unstorable`
    /// and `duplicate`.
    pub rejections: BTreeMap<String, u64>,
    /// Games looked at.
    pub scanned: u64,
    /// Mating games of a combination that was not asked for.
    pub filtered_out: u64,
}

/// The position without its move counters.
fn key(pos: &Chess) -> String {
    Epd::from_position(pos, EnPassantMode::Legal).to_string()
}

/// The node ids of the main line, root first.
fn main_line(game: &Game) -> Vec<u32> {
    let mut line = vec![0];
    while let Some(&next) = game.nodes[*line.last().unwrap() as usize].children.first() {
        line.push(next);
    }
    line
}

impl Collected {
    pub fn new(options: &BuildOptions) -> Result<Self, AppError> {
        options.check()?;
        let filter = options
            .combinations
            .as_ref()
            .filter(|list| !list.is_empty())
            .map(|list| {
                list.iter()
                    .filter_map(|c| normalise_combination(c, options.pool_minors))
                    .collect()
            });
        Ok(Self {
            options: options.clone(),
            filter,
            depths: vec![vec![]; MAX_DEPTH as usize],
            chains: vec![],
            counts: HashMap::new(),
            chain_counts: HashMap::new(),
            order: vec![],
            seen: HashSet::new(),
            seen_chains: HashSet::new(),
            valid: HashSet::new(),
            rejections: BTreeMap::new(),
            scanned: 0,
            filtered_out: 0,
        })
    }

    fn reject(&mut self, kind: &str) {
        *self.rejections.entry(kind.into()).or_default() += 1;
    }

    /// A source entry that is not a readable game.
    pub fn skip_unreadable(&mut self) {
        self.scanned += 1;
        self.reject("unreadable");
    }

    pub fn count(&self, combination: &str, depth: u8) -> u32 {
        self.counts
            .get(&(combination.to_string(), depth))
            .copied()
            .unwrap_or(0)
    }

    pub fn chain_count(&self, combination: &str) -> u32 {
        self.chain_counts.get(combination).copied().unwrap_or(0)
    }

    fn depth_has_room(&self, combination: &str, depth: u8) -> bool {
        depth >= self.options.min_depth
            && depth <= self.options.max_depth
            && self.count(combination, depth) < self.options.per
    }

    /// Whether anything made from step `from` up to step `reach`, the last
    /// the game can have, still has room: a puzzle of one of those depths, or
    /// a chain (which needs two steps).
    fn wanted(&self, combination: &str, from: u8, reach: u8) -> bool {
        (from..=reach).any(|d| self.depth_has_room(combination, d))
            || (reach >= 2 && self.chain_count(combination) < self.options.per)
    }

    /// Every quota is met; only when a list of combinations was given.
    pub fn full(&self) -> bool {
        let Some(filter) = &self.filter else {
            return false;
        };
        filter.iter().all(|c| {
            (self.options.min_depth..=self.options.max_depth)
                .all(|d| self.count(c, d) >= self.options.per)
                && (self.options.max_depth < 2 || self.chain_count(c) >= self.options.per)
        })
    }

    /// The puzzles of `depth` (1..=5), in the order found.
    pub fn puzzles(&self, depth: u8) -> &[Puzzle] {
        &self.depths[usize::from(depth) - 1]
    }

    pub fn chains(&self) -> &[Chain] {
        &self.chains
    }

    /// Combinations in order of first appearance.
    pub fn combinations(&self) -> Vec<String> {
        self.order.clone()
    }

    /// Looks at the source game `game`, entry number `ordinal`: when it ends
    /// in mate, verifies its steps 1, 2, ... and keeps what the quotas allow.
    pub fn add_game(
        &mut self,
        ordinal: u64,
        game: &Game,
        verifier: &mut dyn MateVerifier,
    ) -> Result<(), AppError> {
        self.scanned += 1;
        if game.is_chess960() {
            self.reject("chess960");
            return Ok(());
        }
        let Ok(end) = final_position(game) else {
            self.reject("unreadable");
            return Ok(());
        };
        let Some(tally) = mating_pieces(&end) else {
            self.reject("notMateGame");
            return Ok(());
        };
        let name = combination(&tally, self.options.pool_minors);
        if self.filter.as_ref().is_some_and(|f| !f.contains(&name)) {
            self.filtered_out += 1;
            return Ok(());
        }
        if !self.order.contains(&name) {
            self.order.push(name.clone());
        }
        let line = main_line(game);
        let plies = line.len() - 1;
        // The last step the game has, within the depths asked.
        let reach = plies.div_ceil(2).min(usize::from(self.options.max_depth)) as u8;
        let mut steps: Vec<Game> = vec![];
        let mut top: Option<(Chess, u8)> = None;
        for step in 1..=reach {
            let before = 2 * usize::from(step) - 1;
            if !self.wanted(&name, step, reach) {
                break;
            }
            let at = plies - before;
            let (Ok(pos), Some(uci)) = (
                game.chess_position(line[at]),
                game.nodes[line[at + 1] as usize].uci.as_deref(),
            ) else {
                self.reject("unreadable");
                break;
            };
            let Some(played) = uci
                .parse::<UciMove>()
                .ok()
                .and_then(|m| m.to_move(&pos).ok())
            else {
                self.reject("unreadable");
                break;
            };
            let id = (step, key(&pos), uci.to_string());
            if !self.valid.contains(&id) {
                let result = verifier.check(&pos, step, &played)?;
                if result != StepCheck::Valid {
                    self.reject(result.kind());
                    break;
                }
                self.valid.insert(id);
            }
            let puzzle = match puzzle_game(game, step, &name, ordinal) {
                Ok(puzzle) => puzzle,
                // A puzzle the databases cannot hold ends the chain.
                Err(e) => {
                    self.reject(&e.code);
                    break;
                }
            };
            if step >= self.options.min_depth && self.depth_has_room(&name, step) {
                if self.seen.insert((step, key(&pos))) {
                    *self.counts.entry((name.clone(), step)).or_default() += 1;
                    self.depths[usize::from(step) - 1].push(Puzzle {
                        combination: name.clone(),
                        ordinal,
                        game: puzzle.clone(),
                    });
                } else {
                    self.reject("duplicate");
                }
            }
            steps.push(puzzle);
            top = Some((pos, step));
        }
        if let Some((pos, length)) = top
            && length >= 2
            && self.chain_count(&name) < self.options.per
        {
            // The same game again gives the same chain.
            let moves: Vec<&str> = line[plies + 1 - 2 * usize::from(length) + 1..]
                .iter()
                .filter_map(|&n| game.nodes[n as usize].uci.as_deref())
                .collect();
            if self
                .seen_chains
                .insert(format!("{} {}", key(&pos), moves.join(" ")))
            {
                *self.chain_counts.entry(name.clone()).or_default() += 1;
                self.chains.push(Chain {
                    combination: name,
                    ordinal,
                    steps,
                });
            } else {
                self.reject("duplicate");
            }
        }
        Ok(())
    }
}

fn text_block(text: &str) -> Block {
    Block {
        texts: vec![Text {
            language: 0,
            index: 0,
            text: text.into(),
        }],
        ..Default::default()
    }
}

/// The puzzle for step `step` of the mate that ends `source`: it starts at the
/// step position, plays the rest of the game's main line and asks for each
/// move of the mating side.
pub fn puzzle_game(
    source: &Game,
    step: u8,
    combination: &str,
    ordinal: u64,
) -> Result<Game, AppError> {
    let bad = |m: &str| AppError::new("invalidParameters", m);
    let line = main_line(source);
    let plies = line.len() - 1;
    if step == 0 || plies < 2 * usize::from(step) - 1 {
        return Err(bad("The game has no such step"));
    }
    let before = 2 * usize::from(step) - 1;
    if !final_position(source)?.is_checkmate() {
        return Err(bad("The game does not end in mate"));
    }
    let at = plies - before;
    let pos = source.chess_position(line[at])?;
    let mut puzzle = Game::from_fen(&Fen::from_position(&pos, EnPassantMode::Legal).to_string())?;
    for tag in KEPT_TAGS {
        if let Some(value) = source.tags.get(tag) {
            puzzle.tags.insert(tag.into(), value.clone());
        }
    }
    puzzle.tags.insert("Annotator".into(), ANNOTATOR.into());
    let limits = GameLimits::default();
    let mut ucis = vec![];
    let mut node = 0;
    for &n in &line[at + 1..] {
        let uci = source.nodes[n as usize]
            .uci
            .as_deref()
            .ok_or_else(|| bad("A move is missing"))?;
        node = puzzle.append_uci(node, uci, &limits)?;
        ucis.push((node, uci));
    }
    let mover = if pos.turn().is_white() {
        "White"
    } else {
        "Black"
    };
    // The mating side moves first, then every other ply.
    for (k, &(node, uci)) in ucis.iter().step_by(2).enumerate() {
        // The squares of an answer cannot say which piece a pawn becomes, and
        // the replay reads them as a queen.
        if uci.len() == 5 && !uci.ends_with('q') {
            return Err(AppError::new(
                "underpromotion",
                "A mate by underpromotion cannot be a question",
            ));
        }
        let left = usize::from(step) - k;
        let question = match (k, left) {
            (0, _) => format!("{mover} to move: mate in {left}"),
            (_, 1) => "Mate in 1".into(),
            _ => format!("Continue: mate in {left}"),
        };
        let training = Training {
            kind: TrainingKind::MoveInput,
            seconds: 0,
            score: 1,
            question: text_block(&question),
            wrong: text_block("Not the mate. Look at the squares around the king."),
            help: [
                if k == 0 {
                    text_block(&format!("Pieces in the mate: {combination}"))
                } else {
                    Block::default()
                },
                Block::default(),
            ],
            answers: vec![Answer {
                points: 1,
                ..Default::default()
            }],
        };
        puzzle.set_training(node, Some(training), &[Some(uci.to_string())])?;
    }
    puzzle.nodes[ucis[0].0 as usize].pre_comments = vec![format!(
        "Mate in {step} · {combination} · from game {ordinal}"
    )];
    // The databases refuse some start positions, such as one with an
    // en-passant capture available.
    cbh_reader::v2::encode::encode_moves(&puzzle)
        .map_err(|e| AppError::new("unstorable", e.to_string()))?;
    Ok(puzzle)
}

/// The games of each depth database (index 0 is mate in 1) and of the chain
/// database, in the order they are written: the depth databases interleave
/// the combinations (the first puzzle of each, then the second of each, in
/// order of first appearance), the chain database keeps each chain together.
pub fn ordered(collected: &Collected) -> (Vec<Vec<Game>>, Vec<Game>) {
    let depths = collected
        .depths
        .iter()
        .map(|puzzles| {
            let mut groups: Vec<(&str, Vec<&Puzzle>)> = vec![];
            for p in puzzles {
                match groups.iter_mut().find(|(c, _)| *c == p.combination) {
                    Some((_, list)) => list.push(p),
                    None => groups.push((&p.combination, vec![p])),
                }
            }
            let rounds = groups.iter().map(|(_, l)| l.len()).max().unwrap_or(0);
            (0..rounds)
                .flat_map(|round| groups.iter().filter_map(move |(_, l)| l.get(round)))
                .map(|p| p.game.clone())
                .collect()
        })
        .collect();
    let chains = collected
        .chains
        .iter()
        .flat_map(|c| c.steps.iter().cloned())
        .collect();
    (depths, chains)
}
