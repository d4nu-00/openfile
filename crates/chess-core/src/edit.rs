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

    /// Deletes the variation containing `node` (ChessBase "Delete
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
    /// (ChessBase "Promote Variation"). Returns `node`.
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

    /// Removes every coloured square and arrow from the game (ChessBase
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
