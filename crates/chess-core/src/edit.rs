//! Editing a game tree: deleting and promoting variations, cutting moves
//! before or after a node, text before moves and the game data (tags).
use crate::{ArrowMark, CoreError, Game, MARK_COLORS, NodeId, SquareMark};
use shakmaty::{EnPassantMode, fen::Fen};
use std::collections::BTreeMap;

/// Tags derived from the start position; they cannot be edited directly.
const DERIVED_TAGS: [&str; 2] = ["SetUp", "FEN"];
const MAX_TAGS: usize = 64;
const MAX_TAG_VALUE: usize = 255;
/// Coloured squares and arrows one position may carry.
const MAX_SQUARE_MARKS: usize = 64;
const MAX_ARROW_MARKS: usize = 128;
/// The longest line pasted at once.
const MAX_LINE_MOVES: usize = 256;

fn valid_square(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 2 && (b'a'..=b'h').contains(&b[0]) && (b'1'..=b'8').contains(&b[1])
}

impl Game {
    fn node_parent(&self, node: NodeId) -> Result<Option<NodeId>, CoreError> {
        Ok(self
            .nodes
            .get(node as usize)
            .ok_or(CoreError::InvalidNode)?
            .parent)
    }

    /// The first move of the variation that contains `node`: the nearest of
    /// `node` and its ancestors that is not its parent's first child. `None`
    /// on the main line.
    pub fn variation_start(&self, node: NodeId) -> Result<Option<NodeId>, CoreError> {
        for id in self.path_to(node)?.into_iter().rev() {
            let parent = self.node_parent(id)?.ok_or(CoreError::InvalidNode)?;
            if self.nodes[parent as usize].children.first() != Some(&id) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// Keeps only the nodes for which `keep` holds, renumbering them in
    /// order. Returns each old node's new id.
    fn retain(&mut self, keep: &[bool]) -> Vec<Option<NodeId>> {
        let mut map = vec![None; self.nodes.len()];
        let mut next = 0;
        for (old, &k) in keep.iter().enumerate() {
            if k {
                map[old] = Some(next);
                next += 1;
            }
        }
        let nodes = std::mem::take(&mut self.nodes);
        self.nodes = nodes
            .into_iter()
            .enumerate()
            .filter(|(old, _)| keep[*old])
            .map(|(old, mut n)| {
                n.id = map[old].unwrap();
                n.parent = n.parent.and_then(|p| map[p as usize]);
                n.children = n.children.iter().filter_map(|&c| map[c as usize]).collect();
                n
            })
            .collect();
        map
    }

    /// Marks `node` and everything after it.
    fn subtree(&self, node: NodeId) -> Vec<bool> {
        let mut inside = vec![false; self.nodes.len()];
        let mut stack = vec![node];
        while let Some(id) = stack.pop() {
            inside[id as usize] = true;
            stack.extend(&self.nodes[id as usize].children);
        }
        inside
    }

    /// Deletes the variation containing `node` (the reference app "Delete
    /// Variation"). Returns the move before the variation, now selected.
    pub fn delete_variation(&mut self, node: NodeId) -> Result<NodeId, CoreError> {
        let start = self
            .variation_start(node)?
            .ok_or_else(|| CoreError::InvalidPgn("The move is not in a variation".into()))?;
        let parent = self.node_parent(start)?.ok_or(CoreError::InvalidNode)?;
        let inside = self.subtree(start);
        let map = self.retain(&inside.iter().map(|i| !i).collect::<Vec<_>>());
        map[parent as usize].ok_or(CoreError::InvalidNode)
    }

    /// Makes the variation containing `node` the line it branches from
    /// (the reference app "Promote Variation"). Returns `node`.
    pub fn promote_variation(&mut self, node: NodeId) -> Result<NodeId, CoreError> {
        let start = self
            .variation_start(node)?
            .ok_or_else(|| CoreError::InvalidPgn("The move is not in a variation".into()))?;
        let parent = self.node_parent(start)?.ok_or(CoreError::InvalidNode)?;
        let children = &mut self.nodes[parent as usize].children;
        children.retain(|&c| c != start);
        children.insert(0, start);
        Ok(node)
    }

    /// Deletes every move after `node`, variations included.
    pub fn delete_remaining(&mut self, node: NodeId) -> Result<NodeId, CoreError> {
        let children = self
            .nodes
            .get(node as usize)
            .ok_or(CoreError::InvalidNode)?
            .children
            .clone();
        let mut keep = vec![true; self.nodes.len()];
        for child in children {
            for (k, inside) in keep.iter_mut().zip(self.subtree(child)) {
                *k &= !inside;
            }
        }
        let map = self.retain(&keep);
        map[node as usize].ok_or(CoreError::InvalidNode)
    }

    /// Deletes the moves before `node`: the game then starts from its
    /// position, with the moves after it. Returns the new start (node 0).
    pub fn delete_previous(&mut self, node: NodeId) -> Result<NodeId, CoreError> {
        if node == 0 {
            return Ok(0);
        }
        let pos = self.chess_position(node)?;
        let fen = Fen::from_position(&pos, EnPassantMode::Always).to_string();
        let keep = self.subtree(node);
        self.retain(&keep);
        // A node is created after its parent, so `node` is now the first.
        let root = &mut self.nodes[0];
        root.parent = None;
        root.uci = None;
        root.nags.clear();
        root.pre_comments.clear();
        self.tags.insert("SetUp".into(), "1".into());
        self.tags.insert("FEN".into(), fen.clone());
        self.start_fen = fen;
        Ok(0)
    }

    pub fn set_pre_comments(
        &mut self,
        node: NodeId,
        comments: Vec<String>,
    ) -> Result<(), CoreError> {
        if comments.iter().any(|s| s.contains(['{', '}'])) {
            return Err(CoreError::UnrepresentableComment);
        }
        if node == 0 {
            return Err(CoreError::InvalidNode);
        }
        self.nodes
            .get_mut(node as usize)
            .ok_or(CoreError::InvalidNode)?
            .pre_comments = comments;
        Ok(())
    }

    /// Replaces the coloured squares and arrows shown on the position after
    /// `node` (the start position for node 0). A square keeps its last mark
    /// and an arrow its last colour.
    pub fn set_marks(
        &mut self,
        node: NodeId,
        squares: Vec<SquareMark>,
        arrows: Vec<ArrowMark>,
    ) -> Result<(), CoreError> {
        let color = |c: char| MARK_COLORS.contains(c);
        if squares.len() > MAX_SQUARE_MARKS || arrows.len() > MAX_ARROW_MARKS {
            return Err(CoreError::LimitExceeded);
        }
        if squares
            .iter()
            .any(|m| !color(m.color) || !valid_square(&m.square))
            || arrows.iter().any(|a| {
                !color(a.color) || !valid_square(&a.from) || !valid_square(&a.to) || a.from == a.to
            })
        {
            return Err(CoreError::InvalidPgn("Invalid square or arrow mark".into()));
        }
        let mut kept_squares: Vec<SquareMark> = vec![];
        for m in squares {
            kept_squares.retain(|k| k.square != m.square);
            kept_squares.push(m);
        }
        let mut kept_arrows: Vec<ArrowMark> = vec![];
        for a in arrows {
            kept_arrows.retain(|k| (&k.from, &k.to) != (&a.from, &a.to));
            kept_arrows.push(a);
        }
        let n = self
            .nodes
            .get_mut(node as usize)
            .ok_or(CoreError::InvalidNode)?;
        n.squares = kept_squares;
        n.arrows = kept_arrows;
        Ok(())
    }

    /// Plays `moves` from `parent`, following moves already there and
    /// adding the rest (a variation where the position has a continuation).
    /// Nothing changes unless every move is legal. Returns the last node.
    pub fn append_line(
        &mut self,
        parent: NodeId,
        moves: &[String],
        limits: &crate::GameLimits,
    ) -> Result<NodeId, CoreError> {
        if moves.is_empty() || moves.len() > MAX_LINE_MOVES {
            return Err(CoreError::LimitExceeded);
        }
        let mut next = self.clone();
        let mut at = parent;
        for uci in moves {
            at = next.append_uci(at, uci, limits)?;
        }
        *self = next;
        Ok(at)
    }

    /// Renumbers the nodes in pre-order (each node before its children,
    /// the main continuation first), so every child has a higher id than
    /// its parent. Returns each old node's new id.
    pub(crate) fn renumber(&mut self) -> Vec<NodeId> {
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut stack = vec![0 as NodeId];
        while let Some(id) = stack.pop() {
            order.push(id);
            stack.extend(self.nodes[id as usize].children.iter().rev());
        }
        let mut map = vec![0 as NodeId; self.nodes.len()];
        for (new, &old) in order.iter().enumerate() {
            map[old as usize] = new as NodeId;
        }
        let mut old = std::mem::take(&mut self.nodes);
        self.nodes = order
            .iter()
            .map(|&o| {
                let mut n = std::mem::replace(
                    &mut old[o as usize],
                    crate::Node {
                        id: 0,
                        parent: None,
                        uci: None,
                        children: vec![],
                        comments: vec![],
                        nags: vec![],
                        pre_comments: vec![],
                        squares: vec![],
                        arrows: vec![],
                        critical: None,
                    },
                );
                n.id = map[o as usize];
                n.parent = n.parent.map(|p| map[p as usize]);
                n.children = n.children.iter().map(|&c| map[c as usize]).collect();
                n
            })
            .collect();
        map
    }

    /// Checks every move of the tree against its position.
    fn check_moves(&self) -> Result<(), CoreError> {
        let mut stack = vec![(0 as NodeId, crate::model::parse_position(&self.start_fen)?)];
        while let Some((id, pos)) = stack.pop() {
            for &child in &self.nodes[id as usize].children {
                let uci = self.nodes[child as usize]
                    .uci
                    .as_deref()
                    .ok_or(CoreError::InvalidNode)?;
                stack.push((child, crate::model::apply_uci(pos.clone(), uci)?));
            }
        }
        Ok(())
    }

    /// The reference app's Insert Into Game (`INSERTVARIATIONINTOGAME`, "Insert moves
    /// of variation into main line. Even number needed"): the variation that
    /// contains `node` becomes the line, and the moves it replaced, with
    /// their variations, continue after its last move. The variation must
    /// have an even number of moves, so the same side moves next, and every
    /// later move must still be legal. Returns the variation's last move.
    pub fn insert_variation(&mut self, node: NodeId) -> Result<NodeId, CoreError> {
        let start = self
            .variation_start(node)?
            .ok_or_else(|| CoreError::InvalidPgn("The move is not in a variation".into()))?;
        let parent = self.node_parent(start)?.ok_or(CoreError::InvalidNode)?;
        let replaced = self.nodes[parent as usize].children[0];
        let mut last = start;
        let mut plies = 1;
        while let Some(&next) = self.nodes[last as usize].children.first() {
            last = next;
            plies += 1;
        }
        if plies % 2 != 0 {
            return Err(CoreError::InvalidPgn(
                "Insert Into Game needs a variation with an even number of moves".into(),
            ));
        }
        let mut next = self.clone();
        let children = &mut next.nodes[parent as usize].children;
        children.retain(|&c| c != start && c != replaced);
        children.insert(0, start);
        next.nodes[last as usize].children.push(replaced);
        next.nodes[replaced as usize].parent = Some(last);
        next.check_moves().map_err(|_| {
            CoreError::InvalidPgn("The moves after the variation are not legal after it".into())
        })?;
        let map = next.renumber();
        *self = next;
        Ok(map[last as usize])
    }

    /// The reference app's Delete Commentary commands. Returns the new id of
    /// `keep` (the selected move), or the nearest of its ancestors that
    /// remains.
    pub fn delete_commentary(
        &mut self,
        kind: Commentary,
        keep: NodeId,
    ) -> Result<NodeId, CoreError> {
        let path = self.path_to(keep)?;
        match kind {
            // "Delete all variations and commentary" (Ctrl+Shift+Y).
            Commentary::All => {
                let mut main = vec![false; self.nodes.len()];
                let mut at = 0usize;
                main[0] = true;
                while let Some(&c) = self.nodes[at].children.first() {
                    main[c as usize] = true;
                    at = c as usize;
                }
                for n in &mut self.nodes {
                    n.comments.clear();
                    n.pre_comments.clear();
                    n.nags.clear();
                    n.squares.clear();
                    n.arrows.clear();
                }
                let map = self.retain(&main);
                Ok(path
                    .iter()
                    .rev()
                    .find_map(|&id| map[id as usize])
                    .unwrap_or(0))
            }
            // "Delete all text commentary".
            Commentary::Text => {
                for n in &mut self.nodes {
                    n.comments.clear();
                    n.pre_comments.clear();
                }
                Ok(keep)
            }
            // "Delete all evaluation and time commentary": the scores the
            // workbench writes as text ("+0.35/22") and PGN's [%eval],
            // [%clk] and [%emt] commands.
            Commentary::EvaluationAndTime => {
                for n in &mut self.nodes {
                    n.comments = n
                        .comments
                        .iter()
                        .map(|c| strip_engine_commands(c))
                        .filter(|c| !c.is_empty() && !is_score(c))
                        .collect();
                }
                Ok(keep)
            }
        }
    }

    /// Removes every coloured square and arrow from the game (the reference app
    /// "Delete Colour Commentary").
    pub fn clear_marks(&mut self) {
        for n in &mut self.nodes {
            n.squares.clear();
            n.arrows.clear();
        }
    }

    /// Replaces the game data. Tag names are PGN symbols; values are one
    /// line. SetUp and FEN follow the start position and are kept as they
    /// are; empty values remove a tag, except the seven required ones.
    pub fn set_tags(&mut self, tags: BTreeMap<String, String>) -> Result<(), CoreError> {
        if tags.len() > MAX_TAGS {
            return Err(CoreError::LimitExceeded);
        }
        let required = ["Event", "Site", "Date", "Round", "White", "Black", "Result"];
        let mut next: BTreeMap<String, String> = self
            .tags
            .iter()
            .filter(|(k, _)| DERIVED_TAGS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (key, value) in tags {
            if DERIVED_TAGS.contains(&key.as_str()) {
                continue;
            }
            let value = value.trim().to_string();
            if key.is_empty()
                || key.len() > 64
                || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || value.contains(['\r', '\n'])
                || value.chars().count() > MAX_TAG_VALUE
            {
                return Err(CoreError::InvalidPgn(format!("Invalid tag {key}")));
            }
            if !value.is_empty() {
                next.insert(key, value);
            }
        }
        for key in required {
            let default = match key {
                "Date" => "????.??.??",
                "Result" => "*",
                _ => "?",
            };
            next.entry(key.into()).or_insert_with(|| default.into());
        }
        if !["1-0", "0-1", "1/2-1/2", "*"].contains(&next["Result"].as_str()) {
            return Err(CoreError::InvalidPgn("Unknown result".into()));
        }
        for key in ["WhiteElo", "BlackElo"] {
            if next
                .get(key)
                .is_some_and(|v| v.parse::<u16>().is_err() && v != "-")
            {
                return Err(CoreError::InvalidPgn(format!("{key} is not a rating")));
            }
        }
        let date = &next["Date"];
        let parts: Vec<&str> = date.split('.').collect();
        let valid = parts.len() == 3
            && [4, 2, 2].iter().zip(&parts).all(|(&n, p)| {
                p.len() == n
                    && (p.bytes().all(|b| b.is_ascii_digit()) || p.bytes().all(|b| b == b'?'))
            });
        if !valid {
            return Err(CoreError::InvalidPgn(
                "Date must be YYYY.MM.DD, with ? for unknown parts".into(),
            ));
        }
        self.tags = next;
        Ok(())
    }
}

/// Kinds of commentary the reference app deletes at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Commentary {
    /// Variations and every annotation (`ANNODELETEALL`).
    All,
    /// Text before and after moves (`ANNODELETETEXT`).
    Text,
    /// Evaluation and time annotations (`ANNODELETEFRITZ`).
    EvaluationAndTime,
}

/// A score as the workbench writes it after a pasted line or an analysis:
/// "+0.35", "-1.20/18", "+M3/30".
fn is_score(text: &str) -> bool {
    let t = text.trim();
    let Some(rest) = t.strip_prefix(['+', '-']) else {
        return false;
    };
    let (value, depth) = rest.split_once('/').unwrap_or((rest, ""));
    let number = |v: &str| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit());
    let value_ok = match value.strip_prefix('M') {
        Some(m) => number(m),
        None => value
            .split_once('.')
            .is_some_and(|(a, b)| number(a) && b.len() == 2 && number(b)),
    };
    value_ok && (depth.is_empty() || number(depth))
}

/// The comment without PGN's [%eval …], [%clk …] and [%emt …] commands.
fn strip_engine_commands(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut removed = false;
    while let Some(at) = rest.find("[%") {
        let tail = &rest[at + 2..];
        let is_engine = ["eval ", "clk ", "emt "]
            .iter()
            .any(|k| tail.starts_with(k));
        match (is_engine, tail.find(']')) {
            (true, Some(end)) => {
                out.push_str(&rest[..at]);
                rest = &tail[end + 1..];
                removed = true;
            }
            _ => {
                out.push_str(&rest[..at + 2]);
                rest = tail;
            }
        }
    }
    if !removed {
        return text.to_string();
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
