//! Deep Analysis: the reference app's iterated tree analysis of one position (see
//! docs/analysis.md). An engine of the user's analyses the tree's
//! positions with several lines; lines within 70 centipawns of the best are
//! followed, a little wider and deeper each iteration. The tree goes into
//! the game as one edit (`Edit::AddLines`).
use crate::{
    blunder_check::{MATE, evaluation_symbol, white_text},
    commands::AppError,
    registry::Registry,
};
use engine::{AnalysisRequest, EnginePayload, EngineSession, EngineSpec, Score};
use serde::{Deserialize, Serialize};
use shakmaty::{Chess, Position, san::SanPlus, uci::UciMove};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// The reference app's score window: lines more than this below the best, or below
/// the line before them, are cut (`0x140b94da0` with 70, 70).
pub const WINDOW: i32 = 70;
/// Nodes in the tree at most; it stops growing then.
pub const MAX_NODES: usize = 400;
/// Iterations at most.
pub const MAX_ITERATIONS: u32 = 30;
/// Depth in plies at most.
pub const MAX_DEPTH: usize = 12;
/// Lines asked of the engine for one position at most (the reference app keeps up
/// to 10).
pub const MAX_LINES: usize = 10;
/// The reference app's base time: 4 s at the first iteration.
pub const BASE_MILLIS: u64 = 4_000;
/// Moves of a line written at most.
const MAX_LINE_MOVES: usize = 40;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepParams {
    pub engine: PathBuf,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// The node whose position is analysed.
    pub node: u32,
    /// Seconds; 0 runs until stopped.
    #[serde(default)]
    pub time_limit_secs: u64,
    /// The first iteration's time per position, 4,000 ms as in the reference app;
    /// smaller values scale every time down (for tests).
    #[serde(default)]
    pub base_millis: Option<u64>,
}

impl DeepParams {
    fn check(&self) -> Result<u64, AppError> {
        let base = self.base_millis.unwrap_or(BASE_MILLIS);
        if !(10..=60_000).contains(&base) {
            return Err(AppError::new(
                "invalidParameters",
                "The base time must be from 0.01 to 60 seconds",
            ));
        }
        if self.time_limit_secs > 86_400 {
            return Err(AppError::new(
                "invalidParameters",
                "The time limit must be at most 24 hours",
            ));
        }
        Ok(base)
    }
}

/// One iteration's parameters (`0x14042a790`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Iteration {
    /// The base time per position, in ms.
    pub millis: u64,
    pub depth: usize,
    /// Lines at the root and at the reply; deeper plies take one.
    pub widths: [usize; 2],
}

/// The reference app's schedule for iteration `i`: round(4 × 1.7^i) s per position,
/// a depth of ⌊√(0.6 i) + 2.5⌋ plies and widths ⌊√(0.3 i) + 3.5⌋ and
/// ⌊√(0.3 i) + 1.5⌋. `base` scales the time (4,000 for the reference app's).
pub fn iteration(i: u32, base: u64) -> Iteration {
    let seconds = (4.0 * 1.7f64.powi(i as i32)).round();
    let i = i as f64;
    Iteration {
        millis: (seconds * base as f64 / 4.0).round() as u64,
        depth: ((0.6 * i).sqrt() + 2.5) as usize,
        widths: [
            ((0.3 * i).sqrt() + 3.5) as usize,
            ((0.3 * i).sqrt() + 1.5) as usize,
        ],
    }
}

/// Time for a position at `ply` (`0x1404280e0`): half as much again at the
/// root, 0.7 at the deepest ply, at least 3 s (scaled like the base).
pub fn position_millis(it: &Iteration, ply: usize, base: u64) -> u64 {
    let t = it.millis;
    let millis = if ply == 0 {
        t + (t as f64 * 0.5).round() as u64
    } else if ply == it.depth {
        (t as f64 * 0.7) as u64
    } else {
        t
    };
    millis.max(3 * base / 4)
}

/// Lines to ask for at a node (`0x140427780`): the ply's width, one less for
/// a node in the first half of more than two siblings below the root, and
/// at least one more than the moves there already marked ? or ??
/// (`0x140428820`).
pub fn lines_wanted(
    it: &Iteration,
    ply: usize,
    index: usize,
    siblings: usize,
    refuted: usize,
) -> usize {
    let mut width = match ply {
        0 => it.widths[0],
        1 => it.widths[1],
        _ => 1,
    };
    if ply > 0 && width > 1 && index < siblings / 2 && siblings > 2 {
        width -= 1;
    }
    width.max(refuted + 1).min(MAX_LINES)
}

/// One engine line: the score for the side to move, best first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineLine {
    pub cp: i32,
    pub depth: Option<u32>,
    pub pv: Vec<String>,
}

/// How many of `lines` (best first) the reference app keeps: it cuts where a line
/// scores more than `best` below the first or more than `step` below the
/// one before it.
pub fn kept_lines(lines: &[EngineLine], best: i32, step: i32) -> usize {
    let Some(first) = lines.first() else {
        return 0;
    };
    let mut n = 1;
    while n < lines.len() && first.cp - lines[n].cp <= best && lines[n - 1].cp - lines[n].cp <= step
    {
        n += 1;
    }
    n
}

/// Something that analyses positions: the engine, or a script in tests.
pub trait Analyst {
    /// Up to `count` lines for the position after `history`, best first;
    /// `None` when cancelled.
    fn lines(
        &mut self,
        history: &[String],
        position: &Chess,
        count: usize,
        millis: u64,
    ) -> Result<Option<Vec<EngineLine>>, AppError>;
}

/// A score from the side to move, as centipawns.
fn centipawns(score: &Score) -> i32 {
    match score {
        Score::Cp { value, .. } => *value,
        Score::Mate { moves, .. } if *moves > 0 => MATE - moves,
        Score::Mate { moves, .. } => -MATE - moves,
    }
}

/// The engine as an [`Analyst`]: `MultiPV` is set as needed, and the engine
/// thinks for the time given.
pub struct EngineAnalyst<'a> {
    pub session: &'a mut EngineSession,
    pub start_fen: String,
    pub cancel: &'a AtomicBool,
    multipv: Option<usize>,
    requests: u64,
}

impl<'a> EngineAnalyst<'a> {
    pub fn new(session: &'a mut EngineSession, start_fen: String, cancel: &'a AtomicBool) -> Self {
        Self {
            session,
            start_fen,
            cancel,
            multipv: None,
            requests: 0,
        }
    }
    fn has_multipv(&self) -> bool {
        self.session.options().iter().any(|o| o.name == "MultiPV")
    }
}

impl Analyst for EngineAnalyst<'_> {
    fn lines(
        &mut self,
        history: &[String],
        position: &Chess,
        count: usize,
        millis: u64,
    ) -> Result<Option<Vec<EngineLine>>, AppError> {
        let err = |e: engine::EngineError| AppError::new("engine", e.to_string());
        if position.is_game_over() {
            return Ok(Some(vec![]));
        }
        let count = if self.has_multipv() { count.max(1) } else { 1 };
        if self.has_multipv() && self.multipv != Some(count) {
            self.session
                .set_option("MultiPV", &count.to_string())
                .map_err(err)?;
            self.multipv = Some(count);
        }
        self.requests += 1;
        let id = format!("deep-{}", self.requests);
        self.session
            .analyse(AnalysisRequest {
                request_id: id.clone(),
                start_fen: self.start_fen.clone(),
                history: history.to_vec(),
                root_turn: if position.turn().is_white() {
                    "white"
                } else {
                    "black"
                }
                .into(),
            })
            .map_err(err)?;
        let mut lines: BTreeMap<u32, EngineLine> = BTreeMap::new();
        let mut take = |info: engine::AnalysisInfo| {
            if info.multipv as usize > count {
                return;
            }
            if let (Some(score), Some(pv)) = (info.score, info.pv)
                && !pv.is_empty()
            {
                lines.insert(
                    info.multipv,
                    EngineLine {
                        cp: centipawns(&score),
                        depth: info.depth,
                        pv,
                    },
                );
            }
        };
        let deadline = Instant::now() + Duration::from_millis(millis);
        while Instant::now() < deadline {
            if self.cancel.load(Ordering::Relaxed) {
                self.session.stop().map_err(err)?;
                return Ok(None);
            }
            let wait = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50));
            if let Some(e) = self.session.recv_event(wait).map_err(err)?
                && e.request_id == id
            {
                match e.payload {
                    EnginePayload::Info(info) => take(info),
                    EnginePayload::Error { message } => {
                        return Err(AppError::new("engine", message));
                    }
                    _ => {}
                }
            }
        }
        self.session.stop().map_err(err)?;
        while let Some(e) = self.session.recv_event(Duration::ZERO).map_err(err)? {
            if e.request_id == id
                && let EnginePayload::Info(info) = e.payload
            {
                take(info);
            }
        }
        let mut lines: Vec<EngineLine> = lines.into_values().collect();
        // Lines arrive by MultiPV number; order them by score, as the reference app
        // reads them, keeping the engine's order for equal scores.
        lines.sort_by_key(|l| std::cmp::Reverse(l.cp));
        if lines.is_empty() {
            return Err(AppError::new("engine", "The engine gave no evaluation"));
        }
        Ok(Some(lines))
    }
}

/// A node of the analysis tree; node 0 is the analysed position.
#[derive(Clone, Debug)]
pub struct TreeNode {
    pub uci: String,
    pub children: Vec<usize>,
    /// The score for the side that played the move, from the line that
    /// found it, and that line's depth.
    pub cp: Option<i32>,
    pub depth: Option<u32>,
    /// Moves marked ? or ?? in the game at this node and its children.
    pub refuted: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepProgress {
    pub iteration: u32,
    pub positions: u32,
    pub nodes: u32,
    pub elapsed_ms: u64,
    /// The best line in SAN, with its score from White's side.
    pub best: Vec<String>,
    pub score: Option<String>,
}

/// One line of the result: moves from the analysed node, and for the last
/// move an evaluation symbol and the score and depth as text.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineItem {
    pub moves: Vec<String>,
    #[serde(default)]
    pub evaluation: Option<u16>,
    #[serde(default)]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepReport {
    /// Cancel was pressed: nothing is to be added.
    pub cancelled: bool,
    /// Iterations finished, and the one stopped in, if any.
    pub iterations: u32,
    pub positions: u32,
    /// The tree reached `MAX_NODES`.
    pub limited: bool,
    pub node: u32,
    pub lines: Vec<LineItem>,
    pub revision: String,
    pub best: Vec<String>,
    pub score: Option<String>,
}

/// Why a search ended early.
enum Halt {
    Stop,
    Cancel,
}

/// ? and ?? marks of the game by the moves leading to a node.
pub type Refuted<'a> = Box<dyn Fn(&[String]) -> usize + 'a>;

/// The tree search itself, independent of the engine.
pub struct Search<'a, A: Analyst> {
    pub analyst: &'a mut A,
    pub root: Chess,
    pub nodes: Vec<TreeNode>,
    pub base: u64,
    pub time_limit: Option<Duration>,
    pub stop: &'a AtomicBool,
    pub started: Instant,
    pub positions: u32,
    pub limited: bool,
    /// ? and ?? marks of the game, by the moves leading to a node.
    pub refuted: Refuted<'a>,
}

impl<'a, A: Analyst> Search<'a, A> {
    pub fn new(
        analyst: &'a mut A,
        root: Chess,
        base: u64,
        time_limit: Option<Duration>,
        stop: &'a AtomicBool,
        refuted: Refuted<'a>,
    ) -> Self {
        let root_refuted = refuted(&[]);
        Self {
            analyst,
            root,
            nodes: vec![TreeNode {
                uci: String::new(),
                children: vec![],
                cp: None,
                depth: None,
                refuted: root_refuted,
            }],
            base,
            time_limit,
            stop,
            started: Instant::now(),
            positions: 0,
            limited: false,
            refuted,
        }
    }

    /// Runs iterations until stopped, cancelled, out of time or at
    /// [`MAX_ITERATIONS`]. Returns the iterations run and whether it was
    /// cancelled.
    pub fn run(&mut self, mut progress: impl FnMut(&Self, u32)) -> Result<(u32, bool), AppError> {
        // The last two iterations' durations, in seconds.
        let (mut previous, mut last) = (0.0f64, 0.0f64);
        for i in 0..MAX_ITERATIONS {
            if let Some(limit) = self.time_limit
                && i > 0
            {
                // The reference app stops once 80 % of the time has passed, or when
                // the next iteration, predicted as the last one times its
                // growth over the one before, would take over 125 % of it.
                let limit = limit.as_secs_f64();
                let growth = if previous > 0.0 { last / previous } else { 0.0 };
                if self.started.elapsed().as_secs_f64() > limit * 0.8
                    || last * growth > limit * 1.25
                {
                    return Ok((i, false));
                }
            }
            let it = iteration(i, self.base);
            let began = Instant::now();
            let mut path = vec![];
            match self.visit(0, 0, 0, 1, &it, i, &mut path, &mut progress)? {
                Some(Halt::Stop) => return Ok((i + 1, false)),
                Some(Halt::Cancel) => return Ok((i, true)),
                None => {}
            }
            (previous, last) = (last, began.elapsed().as_secs_f64());
            if self.limited {
                return Ok((i + 1, false));
            }
        }
        Ok((MAX_ITERATIONS, false))
    }

    fn halted(&self) -> Option<Halt> {
        if self.stop.load(Ordering::Relaxed) {
            return Some(Halt::Stop);
        }
        if let Some(limit) = self.time_limit
            && self.started.elapsed().as_secs_f64() > limit.as_secs_f64() * 1.5
        {
            return Some(Halt::Stop);
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(
        &mut self,
        node: usize,
        ply: usize,
        index: usize,
        siblings: usize,
        it: &Iteration,
        number: u32,
        path: &mut Vec<String>,
        progress: &mut impl FnMut(&Self, u32),
    ) -> Result<Option<Halt>, AppError> {
        if let Some(h) = self.halted() {
            return Ok(Some(h));
        }
        let mut position = self.root.clone();
        for uci in path.iter() {
            position = chess_core::model::apply_uci(position, uci)?;
        }
        let legal = position.legal_moves().len();
        if legal == 0 {
            return Ok(None);
        }
        let count = lines_wanted(it, ply, index, siblings, self.nodes[node].refuted).min(legal);
        let millis = position_millis(it, ply, self.base);
        let Some(lines) = self.analyst.lines(path, &position, count, millis)? else {
            return Ok(Some(if self.stop.load(Ordering::Relaxed) {
                Halt::Stop
            } else {
                Halt::Cancel
            }));
        };
        self.positions += 1;
        let keep = kept_lines(&lines, WINDOW, WINDOW).min(MAX_LINES);
        let mut kids = vec![];
        for line in &lines[..keep] {
            let uci = &line.pv[0];
            // Only legal first moves become nodes.
            if chess_core::model::apply_uci(position.clone(), uci).is_err() {
                continue;
            }
            let child = match self.nodes[node]
                .children
                .iter()
                .copied()
                .find(|&c| &self.nodes[c].uci == uci)
            {
                Some(c) => c,
                None => {
                    if self.nodes.len() >= MAX_NODES {
                        self.limited = true;
                        continue;
                    }
                    path.push(uci.clone());
                    let refuted = (self.refuted)(path);
                    path.pop();
                    self.nodes.push(TreeNode {
                        uci: uci.clone(),
                        children: vec![],
                        cp: None,
                        depth: None,
                        refuted,
                    });
                    let c = self.nodes.len() - 1;
                    self.nodes[node].children.push(c);
                    c
                }
            };
            self.nodes[child].cp = Some(line.cp);
            self.nodes[child].depth = line.depth;
            kids.push(child);
        }
        progress(self, number);
        if ply >= it.depth.min(MAX_DEPTH) {
            return Ok(None);
        }
        let n = kids.len();
        for (rank, child) in kids.into_iter().enumerate() {
            path.push(self.nodes[child].uci.clone());
            let halt = self.visit(child, ply + 1, rank, n, it, number, path, progress)?;
            path.pop();
            if halt.is_some() {
                return Ok(halt);
            }
        }
        Ok(None)
    }

    /// The backed-up score of `node` for the side that played its move:
    /// the negated best of its children, or its own line's score at a leaf.
    pub fn value(&self, node: usize) -> Option<i32> {
        let best = self.nodes[node]
            .children
            .iter()
            .filter_map(|&c| self.value(c))
            .max();
        match best {
            Some(b) => Some(-b),
            None => self.nodes[node].cp,
        }
    }

    /// Children of `node`, best first by [`Search::value`].
    fn ordered(&self, node: usize) -> Vec<usize> {
        let mut kids = self.nodes[node].children.clone();
        kids.sort_by_key(|&c| std::cmp::Reverse(self.value(c).unwrap_or(i32::MIN)));
        kids
    }

    /// The best line from the root, in UCI, and its score for the side to
    /// move at the root.
    pub fn best_line(&self) -> (Vec<String>, Option<i32>) {
        let mut line = vec![];
        let mut at = 0;
        while let Some(&c) = self.ordered(at).first() {
            line.push(self.nodes[c].uci.clone());
            at = c;
        }
        let score = self.ordered(0).first().and_then(|&c| self.value(c));
        (line, score)
    }

    /// Every root-to-leaf path, best first, with the leaf's evaluation.
    pub fn line_items(&self) -> Vec<LineItem> {
        let mut out = vec![];
        let mut stack: Vec<(usize, Vec<String>, Chess)> = vec![(0, vec![], self.root.clone())];
        while let Some((node, moves, position)) = stack.pop() {
            let kids = self.ordered(node);
            if kids.is_empty() {
                if moves.is_empty() {
                    continue;
                }
                let n = &self.nodes[node];
                // The leaf's score is for the side that moved into it.
                let white_cp =
                    n.cp.map(|cp| if position.turn().is_white() { -cp } else { cp });
                let mut moves = moves;
                moves.truncate(MAX_LINE_MOVES);
                out.push(LineItem {
                    moves,
                    evaluation: white_cp.and_then(evaluation_symbol),
                    comment: white_cp.map(|cp| {
                        let text = white_text(cp, true);
                        match n.depth {
                            Some(d) => format!("{text}/{d}"),
                            None => text,
                        }
                    }),
                });
                continue;
            }
            // Pushed in reverse so the best child is written first.
            for &c in kids.iter().rev() {
                let uci = &self.nodes[c].uci;
                let Ok(next) = chess_core::model::apply_uci(position.clone(), uci) else {
                    continue;
                };
                let mut m = moves.clone();
                m.push(uci.clone());
                stack.push((c, m, next));
            }
        }
        out
    }
}

/// `moves` from `position` in SAN; stops at the first illegal move.
pub fn san_line(position: &Chess, moves: &[String]) -> Vec<String> {
    let mut out = vec![];
    let mut p = position.clone();
    for uci in moves {
        let Some(m) = uci.parse::<UciMove>().ok().and_then(|u| u.to_move(&p).ok()) else {
            break;
        };
        out.push(SanPlus::from_move(p.clone(), m).to_string());
        p.play_unchecked(m);
    }
    out
}

/// ? and ?? marks of `game` at the node reached from `start` by `path`, and
/// at its children.
pub fn game_refuted(game: &chess_core::Game, start: u32, path: &[String]) -> usize {
    let mut at = start as usize;
    for uci in path {
        match game.nodes[at]
            .children
            .iter()
            .find(|&&c| game.nodes[c as usize].uci.as_deref() == Some(uci.as_str()))
        {
            Some(&c) => at = c as usize,
            None => return 0,
        }
    }
    let marked = |n: usize| game.nodes[n].nags.iter().any(|g| *g == 2 || *g == 4);
    usize::from(at != start as usize && marked(at))
        + game.nodes[at]
            .children
            .iter()
            .filter(|&&c| marked(c as usize))
            .count()
}

impl Registry {
    /// Runs Deep Analysis on document `id` at `params.node`.
    pub fn deep_analysis(
        &self,
        id: &str,
        params: &DeepParams,
        stop: &AtomicBool,
        cancel: &AtomicBool,
        mut progress: impl FnMut(DeepProgress),
    ) -> Result<DeepReport, AppError> {
        let base = params.check()?;
        let (game, revision) = {
            let d = self.document(id)?;
            let d = d
                .lock()
                .map_err(|_| AppError::new("state", "Document unavailable"))?;
            (d.game.clone(), d.revision)
        };
        if game.nodes.get(params.node as usize).is_none() {
            return Err(AppError::new("invalidNode", "Unknown node"));
        }
        let history: Vec<String> = game
            .path_to(params.node)?
            .iter()
            .filter_map(|&n| game.nodes[n as usize].uci.clone())
            .collect();
        let root = game.chess_position(params.node)?;
        let mut session = EngineSession::spawn(
            EngineSpec {
                executable: params.engine.clone(),
                arguments: vec![],
            },
            Duration::from_secs(5),
        )
        .map_err(|e| AppError::new("engine", e.to_string()))?;
        crate::tablebases::configure_engine(&mut session);
        let result = (|| {
            for (name, value) in &params.options {
                crate::engine_commands::standard_option(name, value)?;
                if name == "MultiPV" {
                    continue;
                }
                if session.options().iter().any(|o| &o.name == name) {
                    session
                        .set_option(name, value)
                        .map_err(|e| AppError::new("engine", e.to_string()))?;
                }
            }
            // The engine sees the game from its start; the tree's paths
            // continue from the analysed node.
            let mut analyst = Prefixed {
                inner: EngineAnalyst::new(&mut session, game.start_fen.clone(), cancel),
                prefix: history.clone(),
            };
            let node = params.node;
            let game_ref = &game;
            let mut search = Search::new(
                &mut analyst,
                root.clone(),
                base,
                (params.time_limit_secs > 0).then(|| Duration::from_secs(params.time_limit_secs)),
                stop,
                Box::new(move |path| game_refuted(game_ref, node, path)),
            );
            let mut last = Instant::now() - Duration::from_secs(1);
            let (iterations, cancelled) = search.run(|s, number| {
                // At most a few updates a second.
                if last.elapsed() < Duration::from_millis(200) {
                    return;
                }
                last = Instant::now();
                progress(summary(s, number));
            })?;
            let (best, score) = best_text(&search);
            if cancelled {
                return Ok(DeepReport {
                    cancelled: true,
                    iterations,
                    positions: search.positions,
                    limited: search.limited,
                    node: params.node,
                    lines: vec![],
                    revision: revision.to_string(),
                    best,
                    score,
                });
            }
            Ok(DeepReport {
                cancelled: false,
                iterations,
                positions: search.positions,
                limited: search.limited,
                node: params.node,
                lines: search.line_items(),
                revision: revision.to_string(),
                best,
                score,
            })
        })();
        let _ = session.shutdown();
        result
    }
}

/// An analyst whose histories start at the game's start, not the node.
pub struct Prefixed<A: Analyst> {
    pub inner: A,
    pub prefix: Vec<String>,
}

impl<A: Analyst> Analyst for Prefixed<A> {
    fn lines(
        &mut self,
        history: &[String],
        position: &Chess,
        count: usize,
        millis: u64,
    ) -> Result<Option<Vec<EngineLine>>, AppError> {
        let mut full = self.prefix.clone();
        full.extend_from_slice(history);
        self.inner.lines(&full, position, count, millis)
    }
}

fn best_text<A: Analyst>(s: &Search<'_, A>) -> (Vec<String>, Option<String>) {
    let (line, score) = s.best_line();
    let white = s.root.turn().is_white();
    (
        san_line(&s.root, &line),
        score.map(|cp| white_text(cp, white)),
    )
}

fn summary<A: Analyst>(s: &Search<'_, A>, number: u32) -> DeepProgress {
    let (best, score) = best_text(s);
    DeepProgress {
        iteration: number + 1,
        positions: s.positions,
        nodes: s.nodes.len() as u32 - 1,
        elapsed_ms: s.started.elapsed().as_millis() as u64,
        best,
        score,
    }
}

/// A Deep Analysis's stop and cancel flags.
type Flags = (std::sync::Arc<AtomicBool>, std::sync::Arc<AtomicBool>);

/// Running Deep Analyses, by document id.
#[derive(Default)]
pub struct DeepJobs(std::sync::Mutex<std::collections::HashMap<String, Flags>>);

#[tauri::command]
pub async fn deep_analysis(
    registry: tauri::State<'_, std::sync::Arc<Registry>>,
    jobs: tauri::State<'_, std::sync::Arc<DeepJobs>>,
    document_id: String,
    params: DeepParams,
    events: tauri::ipc::Channel<DeepProgress>,
) -> Result<DeepReport, AppError> {
    let registry = registry.inner().clone();
    let jobs = jobs.inner().clone();
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    if let Ok(mut j) = jobs.0.lock()
        && let Some((_, old)) = j.insert(document_id.clone(), (stop.clone(), cancel.clone()))
    {
        old.store(true, Ordering::Relaxed);
    }
    let id = document_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        registry.deep_analysis(&id, &params, &stop, &cancel, |p| {
            let _ = events.send(p);
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?;
    if let Ok(mut j) = jobs.0.lock() {
        j.remove(&document_id);
    }
    result
}

/// Stops (keeping what was found) or cancels a running Deep Analysis.
#[tauri::command]
pub fn stop_deep_analysis(
    jobs: tauri::State<'_, std::sync::Arc<DeepJobs>>,
    document_id: String,
    discard: bool,
) {
    if let Ok(j) = jobs.0.lock()
        && let Some((stop, cancel)) = j.get(&document_id)
    {
        if discard {
            cancel.store(true, Ordering::Relaxed);
        } else {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schedule_follows_reference() {
        let it = |i| iteration(i, BASE_MILLIS);
        assert_eq!(
            it(0),
            Iteration {
                millis: 4_000,
                depth: 2,
                widths: [3, 1]
            }
        );
        assert_eq!(
            it(1),
            Iteration {
                millis: 7_000,
                depth: 3,
                widths: [4, 2]
            }
        );
        assert_eq!(
            it(4),
            Iteration {
                millis: 33_000,
                depth: 4,
                widths: [4, 2]
            }
        );
        assert_eq!(
            it(14),
            Iteration {
                millis: 6_735_000,
                depth: 5,
                widths: [5, 3]
            }
        );
        // Scaled down for tests.
        assert_eq!(iteration(1, 40).millis, 70);
        let i1 = it(1);
        assert_eq!(position_millis(&i1, 0, BASE_MILLIS), 10_500);
        assert_eq!(position_millis(&i1, 1, BASE_MILLIS), 7_000);
        assert_eq!(position_millis(&i1, 3, BASE_MILLIS), 4_900);
        // At least 3 s.
        let i0 = it(0);
        assert_eq!(position_millis(&i0, 2, BASE_MILLIS), 3_000);
    }

    #[test]
    fn widths_follow_the_sibling_and_refuted_rules() {
        let i1 = iteration(1, BASE_MILLIS);
        assert_eq!(lines_wanted(&i1, 0, 0, 1, 0), 4);
        assert_eq!(lines_wanted(&i1, 1, 3, 4, 0), 2);
        // The first half of more than two siblings gets one line less.
        assert_eq!(lines_wanted(&i1, 1, 0, 4, 0), 1);
        assert_eq!(lines_wanted(&i1, 1, 0, 2, 0), 2);
        assert_eq!(lines_wanted(&i1, 2, 0, 1, 0), 1);
        // One more than the moves marked ? or ??.
        assert_eq!(lines_wanted(&i1, 2, 0, 1, 2), 3);
        assert_eq!(lines_wanted(&i1, 0, 0, 1, 20), MAX_LINES);
    }

    fn line(cp: i32) -> EngineLine {
        EngineLine {
            cp,
            depth: Some(10),
            pv: vec!["e2e4".into()],
        }
    }

    #[test]
    fn lines_are_cut_at_seventy_centipawns() {
        let l = |s: &[i32]| s.iter().map(|&c| line(c)).collect::<Vec<_>>();
        assert_eq!(kept_lines(&l(&[30, 10, -10, -30, -50]), 70, 70), 4);
        assert_eq!(kept_lines(&l(&[50, -30]), 70, 70), 1);
        assert_eq!(kept_lines(&l(&[100, 40, -20]), 70, 70), 2);
        assert_eq!(kept_lines(&[], 70, 70), 0);
    }

    /// Scores each position's legal moves in UCI order: 30, 10, -10, …
    struct Scripted(u32);
    impl Analyst for Scripted {
        fn lines(
            &mut self,
            _: &[String],
            position: &Chess,
            count: usize,
            _: u64,
        ) -> Result<Option<Vec<EngineLine>>, AppError> {
            self.0 += 1;
            let mut moves: Vec<String> = position
                .legal_moves()
                .iter()
                .map(|m| UciMove::from_standard(*m).to_string())
                .collect();
            moves.sort();
            Ok(Some(
                moves
                    .into_iter()
                    .take(count)
                    .enumerate()
                    .map(|(k, uci)| EngineLine {
                        cp: 30 - 20 * k as i32,
                        depth: Some(5),
                        pv: vec![uci],
                    })
                    .collect(),
            ))
        }
    }

    #[test]
    fn the_first_iteration_builds_three_lines_two_moves_deep() {
        let stop = AtomicBool::new(false);
        let mut a = Scripted(0);
        let mut s = Search::new(&mut a, Chess::default(), 10, None, &stop, Box::new(|_| 0));
        let it = iteration(0, 10);
        let mut path = vec![];
        assert!(
            s.visit(0, 0, 0, 1, &it, 0, &mut path, &mut |_, _| {})
                .unwrap()
                .is_none()
        );
        // Root: 3 lines; each reply and the move after it take one line,
        // down to ply 2, whose move is a leaf: 3 + 3 + 3 nodes.
        assert_eq!(s.nodes.len() - 1, 9);
        assert_eq!(s.positions, 1 + 3 + 3);
        let items = s.line_items();
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| i.moves.len() == 3));
        // The leaves are White's moves, scored +0.30 for White: "=" (NAG
        // 11, since ⩲ starts above 0.30), with the depth.
        assert_eq!(items[0].evaluation, Some(11));
        assert_eq!(items[0].comment.as_deref(), Some("+0.30/5"));
    }

    #[test]
    fn iterations_stop_when_asked_and_the_tree_stays_bounded() {
        let stop = AtomicBool::new(false);
        let mut a = Scripted(0);
        let mut s = Search::new(&mut a, Chess::default(), 10, None, &stop, Box::new(|_| 0));
        let (iterations, cancelled) = s
            .run(|s, _| {
                if s.positions >= 60 {
                    s.stop.store(true, Ordering::Relaxed);
                }
            })
            .unwrap();
        assert!(!cancelled);
        assert!(iterations >= 2, "{iterations}");
        assert!(s.nodes.len() <= MAX_NODES);
        let (best, score) = s.best_line();
        assert!(!best.is_empty());
        assert!(score.is_some());
    }
}
