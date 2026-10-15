//! ChessBase's variation tools, as traced in CBase18.exe 18.1 (see
//! docs/game-window.md): Exchange Moves (`EXCHANGEMOVES`), Reorder
//! Variations (`REORDERVARIATIONS`) and Clean up variations
//! (`CLEANUPVARIATIONS`).
use crate::{CoreError, Game, NodeId};
use shakmaty::{EnPassantMode, Position, zobrist::Zobrist64};
use std::collections::HashMap;

impl Game {
    /// Size of the subtree from `node`: the node and every move after it,
    /// variations included (ChessBase's measure, `0x140b4adb0`).
    fn subtree_size(&self, node: NodeId) -> usize {
        let mut n = 0;
        let mut stack = vec![node];
        while let Some(id) = stack.pop() {
            n += 1;
            stack.extend(&self.nodes[id as usize].children);
        }
        n
    }

    /// Exchange Moves ("Replace moves of main line with variation",
    /// `0x140b5bc60`): on the first move of a variation, the variation
    /// becomes the line and the line's continuation takes the variation's
    /// place. Elsewhere it fails, as ChessBase's "Can't replace moves."
    /// (1244). Returns the new id of `node`.
    pub fn exchange_moves(&mut self, node: NodeId) -> Result<NodeId, CoreError> {
        let parent = self
            .nodes
            .get(node as usize)
            .ok_or(CoreError::InvalidNode)?
            .parent
            .ok_or_else(|| CoreError::InvalidPgn("Can't replace moves.".into()))?;
        let kids = &mut self.nodes[parent as usize].children;
        let k = kids
            .iter()
            .position(|&c| c == node)
            .ok_or(CoreError::InvalidNode)?;
        if k == 0 {
            return Err(CoreError::InvalidPgn(
                "Can't replace moves: select the first move of a variation".into(),
            ));
        }
        kids.swap(0, k);
        let map = self.renumber();
        Ok(map[node as usize])
    }

    /// Reorder Variations ("Rearrange deeply nested variations to minimize
    /// depth", `0x140b62270`), from `from` along its line: every variation
    /// is reordered inside, and, with `main_line` (ChessBase's question
    /// "Including the mainline?"), a variation larger than the line's
    /// continuation becomes the line. Returns the new id of `from`.
    pub fn reorder_variations(
        &mut self,
        from: NodeId,
        main_line: bool,
    ) -> Result<NodeId, CoreError> {
        if self.nodes.get(from as usize).is_none() {
            return Err(CoreError::InvalidNode);
        }
        self.reorder_line(from, main_line, 0);
        let map = self.renumber();
        Ok(map[from as usize])
    }

    fn reorder_line(&mut self, from: NodeId, main_line: bool, depth: usize) {
        // Deeply nested games are bounded by the game limits; this guards
        // the recursion all the same.
        if depth > 1_000 {
            return;
        }
        let mut at = from;
        loop {
            let kids = self.nodes[at as usize].children.clone();
            let Some(&main) = kids.first() else {
                return;
            };
            for &v in &kids[1..] {
                self.reorder_line(v, true, depth + 1);
            }
            if main_line && kids.len() > 1 {
                // The first largest variation; ties keep the line.
                let mut best = (0, 0usize);
                for (i, &v) in kids.iter().enumerate().skip(1) {
                    let s = self.subtree_size(v);
                    if s > best.1 {
                        best = (i, s);
                    }
                }
                if best.1 > self.subtree_size(main) {
                    self.nodes[at as usize].children.swap(0, best.0);
                }
            }
            at = self.nodes[at as usize].children[0];
        }
    }

    /// Clean up variations ("Clean up different move orders in a deeply
    /// annotated game", `0x1408e6d70`): the game is rebuilt from its lines,
    /// main line first. As ChessBase's merge (`0x140b64c80`) does, each line
    /// is attached after the deepest of its positions that the rebuilt
    /// game already holds, by any move order; its moves before that
    /// position are dropped. Moves kept carry their annotations; comments
    /// of a move that meets an existing one are added to it. Returns the
    /// number of moves removed.
    pub fn clean_up_variations(&mut self) -> Result<usize, CoreError> {
        let key = |g: &Game, n: NodeId| -> Result<u64, CoreError> {
            Ok(g.chess_position(n)?
                .zobrist_hash::<Zobrist64>(EnPassantMode::Legal)
                .0)
        };
        // Every root-to-leaf line, main line first (pre-order).
        let mut lines: Vec<Vec<NodeId>> = vec![];
        let mut stack: Vec<Vec<NodeId>> = vec![vec![0]];
        while let Some(path) = stack.pop() {
            let last = *path.last().unwrap();
            let kids = &self.nodes[last as usize].children;
            if kids.is_empty() {
                lines.push(path);
                continue;
            }
            for &c in kids.iter().rev() {
                let mut p = path.clone();
                p.push(c);
                stack.push(p);
            }
        }
        let before = self.nodes.len();
        let mut next = self.clone();
        next.nodes.truncate(1);
        next.nodes[0].children.clear();
        let mut known: HashMap<u64, NodeId> = HashMap::new();
        known.insert(key(self, 0)?, 0);
        // Positions are computed once per old node.
        let mut old_keys: HashMap<NodeId, u64> = HashMap::new();
        let limits = crate::GameLimits::default();
        for line in &lines {
            for &n in line {
                if let std::collections::hash_map::Entry::Vacant(e) = old_keys.entry(n) {
                    e.insert(key(self, n)?);
                }
            }
            // The deepest position of the line the rebuilt game holds.
            let (k, mut at) = line
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, n)| known.get(&old_keys[n]).map(|&a| (i, a)))
                .unwrap_or((0, 0));
            if k > 0 {
                merge_comments(&mut next, at, self, line[k]);
            }
            for &old in &line[k + 1..] {
                let uci = self.nodes[old as usize]
                    .uci
                    .clone()
                    .ok_or(CoreError::InvalidNode)?;
                let count = next.nodes.len();
                at = next.append_uci(at, &uci, &limits)?;
                if next.nodes.len() > count {
                    let src = &self.nodes[old as usize];
                    let dst = &mut next.nodes[at as usize];
                    dst.comments = src.comments.clone();
                    dst.pre_comments = src.pre_comments.clone();
                    dst.nags = src.nags.clone();
                    dst.squares = src.squares.clone();
                    dst.arrows = src.arrows.clone();
                } else {
                    merge_comments(&mut next, at, self, old);
                }
                known.entry(old_keys[&old]).or_insert(at);
            }
        }
        next.renumber();
        let removed = before.saturating_sub(next.nodes.len());
        *self = next;
        Ok(removed)
    }
}

/// Adds the comments and, when it has none, the symbols of `from`'s node
/// `old` to `game`'s node `at`.
fn merge_comments(game: &mut Game, at: NodeId, from: &Game, old: NodeId) {
    let src = &from.nodes[old as usize];
    let dst = &mut game.nodes[at as usize];
    for c in &src.comments {
        if !dst.comments.contains(c) {
            dst.comments.push(c.clone());
        }
    }
    if dst.nags.is_empty() {
        dst.nags = src.nags.clone();
    }
}

#[cfg(test)]
mod tests {
    use crate::{Game, GameLimits};

    fn pgn(text: &str) -> Game {
        crate::parse_pgn(text.as_bytes(), &GameLimits::default())
            .unwrap()
            .remove(0)
    }

    fn movetext(g: &Game) -> String {
        let text = crate::write_pgn(g).unwrap();
        text.split("\n\n").nth(1).unwrap().trim().to_string()
    }

    #[test]
    fn exchange_moves_swaps_the_variation_with_the_line() {
        let mut g = pgn("1. e4 e5 (1... c5 2. Nf3) (1... e6) 2. Nf3 *");
        // Node of 1...c5: the second child of 1.e4.
        let c5 = g.nodes[1].children[1];
        let at = g.exchange_moves(c5).unwrap();
        assert_eq!(g.nodes[at as usize].uci.as_deref(), Some("c7c5"));
        assert_eq!(movetext(&g), "1. e4 c5 (1... e5 2. Nf3) (1... e6) 2. Nf3 *");
        // Not on a main-line move, nor inside a variation past its first move.
        assert!(g.exchange_moves(1).is_err());
        let inner = g.nodes[g.nodes[1].children[1] as usize].children[0];
        assert!(g.exchange_moves(inner).is_err());
    }

    #[test]
    fn reorder_makes_the_largest_line_the_main_line() {
        let mut g = pgn("1. e4 e5 (1... c5 2. Nf3 d6 (2... Nc6 3. d4) 3. d4) 2. Nf3 *");
        // Without the main line, only the variation's inside is reordered.
        let mut inside = g.clone();
        inside.reorder_variations(0, false).unwrap();
        assert_eq!(
            movetext(&inside),
            "1. e4 e5 (1... c5 2. Nf3 d6 (2... Nc6 3. d4) 3. d4) 2. Nf3 *"
        );
        g.reorder_variations(0, true).unwrap();
        // 1...c5 with 6 moves beats 1...e5 with 2.
        assert_eq!(
            movetext(&g),
            "1. e4 c5 (1... e5 2. Nf3) 2. Nf3 d6 (2... Nc6 3. d4) 3. d4 *"
        );
    }

    #[test]
    fn clean_up_merges_transposed_move_orders() {
        // The variation 1.c4 e6 2.d4 Nf6 3.Nc3 Bb4 reaches the main line's
        // position; only its continuation 4.Qc2 is kept, after 3...Bb4.
        let mut g = pgn(
            "1. d4 {main} (1. c4 e6 2. d4 Nf6 3. Nc3 Bb4 4. Qc2 {Classical}) 1... Nf6 2. c4 e6 3. Nc3 Bb4 *",
        );
        let removed = g.clean_up_variations().unwrap();
        assert_eq!(removed, 6);
        assert_eq!(
            movetext(&g),
            "1. d4 {main} 1... Nf6 2. c4 e6 3. Nc3 Bb4 4. Qc2 {Classical} *"
        );
        // A variation that never transposes stays as it is.
        let mut h = pgn("1. e4 e5 (1... c5 2. Nf3) 2. Nf3 *");
        assert_eq!(h.clean_up_variations().unwrap(), 0);
        assert_eq!(movetext(&h), "1. e4 e5 (1... c5 2. Nf3) 2. Nf3 *");
    }
}
