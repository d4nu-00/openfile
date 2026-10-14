use crate::{
    ArrowMark, CoreError, Game, GameBuilder, GameLimits, MARK_COLORS, Node, NodeId, SquareMark,
    model::{apply_uci, parse_position},
};
use pgn_reader::{Nag, Outcome, RawComment, RawTag, Reader, SanPlus, Skip, Visitor};
use shakmaty::{Chess, Position, uci::UciMove};
use std::{collections::BTreeMap, io::Cursor, ops::ControlFlow};

type Flow<T = ()> = ControlFlow<Result<Game, CoreError>, T>;
struct Importer<'a> {
    limits: &'a GameLimits,
}
struct Movetext {
    game: GameBuilder,
    current: NodeId,
    stack: Vec<NodeId>,
    partial_comment: String,
    declared_result: Option<String>,
    /// True after `(` until the variation's first move: comments there are
    /// text before that move.
    variation_start: bool,
    pending_pre: Vec<String>,
}
impl Visitor for Importer<'_> {
    type Tags = BTreeMap<String, String>;
    type Movetext = Movetext;
    type Output = Result<Game, CoreError>;
    fn begin_tags(&mut self) -> Flow<Self::Tags> {
        ControlFlow::Continue(BTreeMap::new())
    }
    fn tag(&mut self, tags: &mut Self::Tags, name: &[u8], value: RawTag<'_>) -> Flow {
        let (Ok(name), Ok(value)) = (std::str::from_utf8(name), value.decode_utf8()) else {
            return ControlFlow::Break(Err(CoreError::InvalidPgn("Tags must be UTF-8".into())));
        };
        tags.insert(name.into(), value.into_owned());
        ControlFlow::Continue(())
    }
    fn begin_movetext(&mut self, tags: Self::Tags) -> Flow<Self::Movetext> {
        let mut game = match tags.get("FEN") {
            Some(fen) => match Game::from_fen(fen) {
                Ok(g) => g,
                Err(e) => return ControlFlow::Break(Err(e)),
            },
            None => Game::initial(),
        };
        let declared_result = tags.get("Result").cloned();
        game.tags.extend(tags);
        if game.tags.contains_key("FEN") {
            game.tags.insert("FEN".into(), game.start_fen.clone());
            game.tags.insert("SetUp".into(), "1".into());
        }
        let game = match GameBuilder::new(game, self.limits.clone()) {
            Ok(game) => game,
            Err(e) => return ControlFlow::Break(Err(e)),
        };
        ControlFlow::Continue(Movetext {
            game,
            current: 0,
            stack: vec![],
            partial_comment: String::new(),
            declared_result,
            variation_start: false,
            pending_pre: vec![],
        })
    }
    fn san(&mut self, s: &mut Movetext, san: SanPlus) -> Flow {
        let result = (|| {
            let pos = s.game.position(s.current)?.clone();
            let uci = if san.san == shakmaty::san::San::Null {
                "0000".into()
            } else {
                let m = san
                    .san
                    .to_move(&pos)
                    .map_err(|_| CoreError::IllegalMove(san.to_string()))?;
                crate::model::uci_of(&pos, m)
            };
            s.current = s.game.append_uci(s.current, &uci)?;
            s.variation_start = false;
            s.game.game_mut().nodes[s.current as usize].pre_comments =
                std::mem::take(&mut s.pending_pre);
            Ok(())
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(e) => ControlFlow::Break(Err(e)),
        }
    }
    fn nag(&mut self, s: &mut Movetext, nag: Nag) -> Flow {
        s.game.game_mut().nodes[s.current as usize]
            .nags
            .push(nag.0.into());
        ControlFlow::Continue(())
    }
    fn partial_comment(&mut self, s: &mut Movetext, comment: RawComment<'_>) -> Flow {
        match std::str::from_utf8(comment.as_bytes()) {
            Ok(text) => {
                s.partial_comment.push_str(text);
                ControlFlow::Continue(())
            }
            Err(e) => ControlFlow::Break(Err(CoreError::InvalidPgn(e.to_string()))),
        }
    }
    fn comment(&mut self, s: &mut Movetext, comment: RawComment<'_>) -> Flow {
        match self.partial_comment(s, comment) {
            ControlFlow::Break(e) => ControlFlow::Break(e),
            ControlFlow::Continue(()) => {
                let raw = std::mem::take(&mut s.partial_comment);
                let (text, squares, arrows) = split_marks(&raw);
                let only_marks = (!squares.is_empty() || !arrows.is_empty()) && text.is_empty();
                let node = &mut s.game.game_mut().nodes[s.current as usize];
                node.squares.extend(squares);
                node.arrows.extend(arrows);
                if !only_marks {
                    if s.variation_start {
                        s.pending_pre.push(text);
                    } else {
                        node.comments.push(text);
                    }
                }
                ControlFlow::Continue(())
            }
        }
    }
    fn begin_variation(&mut self, s: &mut Movetext) -> Flow<Skip> {
        if s.stack.len() >= self.limits.max_variation_depth {
            return ControlFlow::Break(Err(CoreError::LimitExceeded));
        }
        let Some(parent) = s.game.game().nodes[s.current as usize].parent else {
            return ControlFlow::Break(Err(CoreError::InvalidPgn(
                "Variation without a preceding move".into(),
            )));
        };
        s.stack.push(s.current);
        s.current = parent;
        s.variation_start = true;
        ControlFlow::Continue(Skip(false))
    }
    fn end_variation(&mut self, s: &mut Movetext) -> Flow {
        match s.stack.pop() {
            Some(node) => {
                s.current = node;
                s.variation_start = false;
                // Text in an empty variation has no move to precede.
                let pending = std::mem::take(&mut s.pending_pre);
                s.game.game_mut().nodes[node as usize]
                    .comments
                    .extend(pending);
                ControlFlow::Continue(())
            }
            None => {
                ControlFlow::Break(Err(CoreError::InvalidPgn("Unmatched variation end".into())))
            }
        }
    }
    fn outcome(&mut self, s: &mut Movetext, outcome: Outcome) -> Flow {
        let result = outcome.to_string();
        if s.stack.is_empty() {
            if s.declared_result.as_ref().is_some_and(|tag| tag != &result) {
                return ControlFlow::Break(Err(CoreError::InvalidPgn(
                    "Result tag and movetext disagree".into(),
                )));
            }
            s.game.game_mut().tags.insert("Result".into(), result);
        }
        ControlFlow::Continue(())
    }
    fn end_game(&mut self, s: Movetext) -> Result<Game, CoreError> {
        if !s.stack.is_empty() {
            return Err(CoreError::InvalidPgn("Unclosed variation".into()));
        }
        Ok(s.game.finish())
    }
}

pub fn parse_pgn(input: &[u8], limits: &GameLimits) -> Result<Vec<Game>, CoreError> {
    if input.len() > limits.max_input_bytes || input.len() > 20 * 1024 * 1024 {
        return Err(CoreError::LimitExceeded);
    }
    let mut reader = Reader::new(Cursor::new(input));
    let mut visitor = Importer { limits };
    let mut games = Vec::new();
    let mut nodes = 0usize;
    while let Some(game) = reader
        .read_game(&mut visitor)
        .map_err(|e| CoreError::InvalidPgn(e.to_string()))?
    {
        let game = game?;
        nodes = nodes
            .checked_add(game.nodes.len())
            .ok_or(CoreError::LimitExceeded)?;
        if games.len() >= 2000 || nodes > 100_000 {
            return Err(CoreError::LimitExceeded);
        }
        games.push(game);
    }
    if games.is_empty() {
        return Err(CoreError::InvalidPgn("No games found".into()));
    }
    Ok(games)
}

/// Removes `[%csl ...]` and `[%cal ...]` commands from a comment and returns
/// the remaining text with the parsed marks. Malformed commands stay as text.
fn split_marks(raw: &str) -> (String, Vec<SquareMark>, Vec<ArrowMark>) {
    let (mut squares, mut arrows) = (vec![], vec![]);
    let mut text = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find("[%c") {
        let Some(len) = rest[start..].find(']') else {
            break;
        };
        let command = &rest[start + 1..start + len];
        let parsed = command
            .strip_prefix("%csl ")
            .map(|v| parse_squares(v).map(|m| squares.extend(m)))
            .or_else(|| {
                command
                    .strip_prefix("%cal ")
                    .map(|v| parse_arrows(v).map(|m| arrows.extend(m)))
            });
        text.push_str(&rest[..start]);
        if !matches!(parsed, Some(Some(()))) {
            text.push_str(&rest[start..start + len + 1]);
        }
        rest = &rest[start + len + 1..];
    }
    text.push_str(rest);
    (text.trim().to_string(), squares, arrows)
}
fn mark_square(s: &str) -> Option<String> {
    let b = s.as_bytes();
    (b.len() == 2 && (b'a'..=b'h').contains(&b[0]) && (b'1'..=b'8').contains(&b[1]))
        .then(|| s.to_string())
}
fn mark_color(s: &str) -> Option<(char, &str)> {
    let c = s.chars().next()?;
    MARK_COLORS.contains(c).then(|| (c, &s[1..]))
}
fn parse_squares(value: &str) -> Option<Vec<SquareMark>> {
    value
        .split(',')
        .map(|item| {
            let (color, square) = mark_color(item.trim())?;
            Some(SquareMark {
                color,
                square: mark_square(square)?,
            })
        })
        .collect()
}
fn parse_arrows(value: &str) -> Option<Vec<ArrowMark>> {
    value
        .split(',')
        .map(|item| {
            let (color, squares) = mark_color(item.trim())?;
            if squares.len() != 4 {
                return None;
            }
            Some(ArrowMark {
                color,
                from: mark_square(&squares[..2])?,
                to: mark_square(&squares[2..])?,
            })
        })
        .collect()
}
fn marks(out: &mut String, node: &Node) {
    let mut parts = vec![];
    if !node.squares.is_empty() {
        let items: Vec<_> = node
            .squares
            .iter()
            .map(|m| format!("{}{}", m.color, m.square))
            .collect();
        parts.push(format!("[%csl {}]", items.join(",")));
    }
    if !node.arrows.is_empty() {
        let items: Vec<_> = node
            .arrows
            .iter()
            .map(|m| format!("{}{}{}", m.color, m.from, m.to))
            .collect();
        parts.push(format!("[%cal {}]", items.join(",")));
    }
    if !parts.is_empty() {
        out.push_str(&format!(" {{{}}}", parts.join(" ")));
    }
}
fn comments(out: &mut String, values: &[String]) -> Result<(), CoreError> {
    for value in values {
        if value.contains(['{', '}']) {
            return Err(CoreError::UnrepresentableComment);
        }
        out.push_str(&format!(" {{{value}}}"));
    }
    Ok(())
}
struct Exporter<'a> {
    game: &'a Game,
    seen: Vec<bool>,
    out: String,
}
impl Exporter<'_> {
    fn line(
        &mut self,
        first: NodeId,
        mut pos: Chess,
        depth: usize,
        include_siblings: bool,
    ) -> Result<(), CoreError> {
        if depth > 128 {
            return Err(CoreError::LimitExceeded);
        }
        let mut current = first;
        let mut siblings = include_siblings;
        let mut force_number = true;
        loop {
            let node = self
                .game
                .nodes
                .get(current as usize)
                .ok_or(CoreError::InvalidNode)?;
            if node.id != current || self.seen[current as usize] {
                return Err(CoreError::InvalidNode);
            }
            self.seen[current as usize] = true;
            let parent = node.parent.ok_or(CoreError::InvalidNode)?;
            let parent_node = self
                .game
                .nodes
                .get(parent as usize)
                .ok_or(CoreError::InvalidNode)?;
            if !parent_node.children.contains(&current) {
                return Err(CoreError::InvalidNode);
            }
            let before = pos.clone();
            let uci = node.uci.as_deref().ok_or(CoreError::InvalidNode)?;
            let san = if uci == "0000" {
                "--".into()
            } else {
                let m: UciMove = uci
                    .parse()
                    .map_err(|_| CoreError::IllegalMove(uci.into()))?;
                let m = m
                    .to_move(&pos)
                    .map_err(|_| CoreError::IllegalMove(uci.into()))?;
                shakmaty::san::SanPlus::from_move(pos.clone(), m).to_string()
            };
            comments(&mut self.out, &node.pre_comments)?;
            if !node.pre_comments.is_empty() {
                force_number = true;
            }
            if !self.out.ends_with(['(', '\n']) {
                self.out.push(' ');
            }
            if pos.turn().is_white() {
                self.out.push_str(&format!("{}. {san}", pos.fullmoves()));
            } else if force_number {
                self.out.push_str(&format!("{}... {san}", pos.fullmoves()));
            } else {
                self.out.push_str(&san);
            }
            for nag in &node.nags {
                self.out.push_str(&format!(" ${nag}"));
            }
            marks(&mut self.out, node);
            comments(&mut self.out, &node.comments)?;
            let next = node.children.first().copied();
            let alternatives = if siblings {
                parent_node
                    .children
                    .iter()
                    .copied()
                    .filter(|&n| n != current)
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            force_number = !alternatives.is_empty()
                || !node.comments.is_empty()
                || !node.nags.is_empty()
                || !node.squares.is_empty()
                || !node.arrows.is_empty();
            for alt in alternatives {
                self.out.push_str(" (");
                self.line(alt, before.clone(), depth + 1, false)?;
                self.out.push(')');
            }
            pos = apply_uci(pos, uci)?;
            match next {
                Some(n) => {
                    current = n;
                    siblings = true;
                }
                None => break,
            }
        }
        Ok(())
    }
}
pub fn write_pgn(game: &Game) -> Result<String, CoreError> {
    if game.nodes.is_empty() || game.nodes.len() > 100_000 {
        return Err(CoreError::InvalidNode);
    }
    let mut tags = game.tags.clone();
    let initial = Game::initial();
    if game.start_fen != initial.start_fen {
        tags.insert("SetUp".into(), "1".into());
        tags.insert("FEN".into(), game.start_fen.clone());
    }
    let roster = ["Event", "Site", "Date", "Round", "White", "Black", "Result"];
    let mut out = String::new();
    for key in roster.iter().copied().chain(
        tags.keys()
            .map(String::as_str)
            .filter(|k| !roster.contains(k)),
    ) {
        let value = tags
            .get(key)
            .ok_or_else(|| CoreError::InvalidPgn(format!("Missing tag {key}")))?;
        if key.is_empty()
            || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || value.contains(['\r', '\n'])
        {
            return Err(CoreError::InvalidPgn("Invalid tag".into()));
        }
        out.push_str(&format!(
            "[{key} \"{}\"]\n",
            value.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    out.push('\n');
    let mut exporter = Exporter {
        game,
        seen: vec![false; game.nodes.len()],
        out,
    };
    exporter.seen[0] = true;
    for nag in &game.nodes[0].nags {
        exporter.out.push_str(&format!(" ${nag}"));
    }
    marks(&mut exporter.out, &game.nodes[0]);
    comments(&mut exporter.out, &game.nodes[0].comments)?;
    if let Some(&first) = game.nodes[0].children.first() {
        exporter.line(first, parse_position(&game.start_fen)?, 0, true)?;
    }
    if exporter.seen.iter().any(|seen| !seen) {
        return Err(CoreError::InvalidNode);
    }
    let result = tags
        .get("Result")
        .ok_or_else(|| CoreError::InvalidPgn("Missing result".into()))?;
    if !["1-0", "0-1", "1/2-1/2", "*"].contains(&result.as_str()) {
        return Err(CoreError::InvalidPgn("Unknown result".into()));
    }
    exporter.out.push_str(&format!(" {result}\n\n"));
    Ok(exporter.out)
}
impl Game {
    pub fn set_comments(&mut self, node: NodeId, comments: Vec<String>) -> Result<(), CoreError> {
        if comments.iter().any(|s| s.contains(['{', '}'])) {
            return Err(CoreError::UnrepresentableComment);
        }
        self.nodes
            .get_mut(node as usize)
            .ok_or(CoreError::InvalidNode)?
            .comments = comments;
        Ok(())
    }
    pub fn set_nags(&mut self, node: NodeId, nags: Vec<u16>) -> Result<(), CoreError> {
        if nags.iter().any(|&n| n > 255) {
            return Err(CoreError::InvalidPgn(
                "NAG is outside supported PGN range".into(),
            ));
        }
        self.nodes
            .get_mut(node as usize)
            .ok_or(CoreError::InvalidNode)?
            .nags = nags;
        Ok(())
    }
}
