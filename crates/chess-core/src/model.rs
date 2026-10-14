use crate::{CoreError, PositionView};
use serde::{Deserialize, Serialize};
use shakmaty::{
    CastlingMode, Chess, EnPassantMode, Position, fen::Fen, san::SanPlus, uci::UciMove,
};
use std::collections::BTreeMap;
pub type NodeId = u32;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub tags: BTreeMap<String, String>,
    pub start_fen: String,
    pub nodes: Vec<Node>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub uci: Option<String>,
    pub children: Vec<NodeId>,
    pub comments: Vec<String>,
    pub nags: Vec<u16>,
    /// Text shown before the move (ChessBase "text before move").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_comments: Vec<String>,
    /// Coloured squares shown on the position after the move.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub squares: Vec<SquareMark>,
    /// Coloured arrows shown on the position after the move.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arrows: Vec<ArrowMark>,
}
/// Mark colours use the PGN `[%csl]`/`[%cal]` letters: G green, Y yellow,
/// R red, B blue, and the further ChessBase letters C and O.
pub const MARK_COLORS: &str = "GYRBCO";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquareMark {
    pub color: char,
    pub square: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrowMark {
    pub color: char,
    pub from: String,
    pub to: String,
}
#[derive(Clone, Debug)]
pub struct GameLimits {
    pub max_nodes: usize,
    pub max_variation_depth: usize,
    pub max_input_bytes: usize,
}
impl Default for GameLimits {
    fn default() -> Self {
        Self {
            max_nodes: 100_000,
            max_variation_depth: 128,
            max_input_bytes: 20 * 1024 * 1024,
        }
    }
}
impl Game {
    pub fn initial() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }
    pub fn from_fen(fen: &str) -> Result<Self, CoreError> {
        let pos = parse_position(fen)?;
        let mut tags: BTreeMap<String, String> = [
            ("Event", "?"),
            ("Site", "?"),
            ("Date", "????.??.??"),
            ("Round", "?"),
            ("White", "?"),
            ("Black", "?"),
            ("Result", "*"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();
        let start_fen = Fen::from_position(&pos, EnPassantMode::Always).to_string();
        if start_fen != "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1" {
            tags.insert("SetUp".into(), "1".into());
            tags.insert("FEN".into(), start_fen.clone());
        }
        if pos.castles().mode() == CastlingMode::Chess960 {
            tags.insert("Variant".into(), CHESS960.into());
        }
        Ok(Self {
            tags,
            start_fen,
            nodes: vec![Node {
                id: 0,
                parent: None,
                uci: None,
                children: vec![],
                comments: vec![],
                nags: vec![],
                pre_comments: vec![],
                squares: vec![],
                arrows: vec![],
            }],
        })
    }
    /// A Chess960 game: its `Variant` tag says so, or its start position
    /// needs Chess960 castling.
    pub fn is_chess960(&self) -> bool {
        self.tags
            .get("Variant")
            .is_some_and(|v| is_chess960_variant(v))
            || parse_position(&self.start_fen)
                .is_ok_and(|p| p.castles().mode() == CastlingMode::Chess960)
    }
    pub fn path_to(&self, id: NodeId) -> Result<Vec<NodeId>, CoreError> {
        let mut path = Vec::new();
        let mut next = id;
        loop {
            let node = self
                .nodes
                .get(next as usize)
                .ok_or(CoreError::InvalidNode)?;
            if node.id != next || path.len() >= self.nodes.len() {
                return Err(CoreError::InvalidNode);
            }
            if next == 0 {
                if node.parent.is_some() || node.uci.is_some() {
                    return Err(CoreError::InvalidNode);
                }
                break;
            }
            path.push(next);
            next = node.parent.ok_or(CoreError::InvalidNode)?;
        }
        path.reverse();
        Ok(path)
    }
    pub fn chess_position(&self, id: NodeId) -> Result<Chess, CoreError> {
        let mut pos = parse_position(&self.start_fen)?;
        for n in self.path_to(id)? {
            pos = apply_uci(
                pos,
                self.nodes[n as usize]
                    .uci
                    .as_deref()
                    .ok_or(CoreError::InvalidNode)?,
            )?;
        }
        Ok(pos)
    }
    pub fn append_uci(
        &mut self,
        parent: NodeId,
        uci: &str,
        limits: &GameLimits,
    ) -> Result<NodeId, CoreError> {
        let pos = self.chess_position(parent)?;
        self.append_at(parent, uci, limits, pos, true)
    }
    pub(crate) fn append_at(
        &mut self,
        parent: NodeId,
        uci: &str,
        limits: &GameLimits,
        pos: Chess,
        reuse_existing: bool,
    ) -> Result<NodeId, CoreError> {
        let canonical = if uci == "0000" {
            apply_uci(pos.clone(), uci)?;
            uci.to_string()
        } else {
            let m: UciMove = uci
                .parse()
                .map_err(|_| CoreError::IllegalMove(uci.into()))?;
            let m = m
                .to_move(&pos)
                .map_err(|_| CoreError::IllegalMove(uci.into()))?;
            uci_of(&pos, m)
        };
        if reuse_existing {
            for &child in &self.nodes[parent as usize].children {
                let node = self
                    .nodes
                    .get(child as usize)
                    .ok_or(CoreError::InvalidNode)?;
                if node.uci.as_deref() == Some(&canonical) {
                    return Ok(child);
                }
            }
        }
        if self.nodes.len() >= limits.max_nodes {
            return Err(CoreError::LimitExceeded);
        }
        let id = NodeId::try_from(self.nodes.len()).map_err(|_| CoreError::LimitExceeded)?;
        self.nodes.push(Node {
            id,
            parent: Some(parent),
            uci: Some(canonical),
            children: vec![],
            comments: vec![],
            nags: vec![],
            pre_comments: vec![],
            squares: vec![],
            arrows: vec![],
        });
        self.nodes[parent as usize].children.push(id);
        Ok(id)
    }
    pub fn position_at(&self, id: NodeId) -> Result<PositionView, CoreError> {
        let pos = self.chess_position(id)?;
        Ok(PositionView {
            fen: Fen::from_position(&pos, EnPassantMode::Always).to_string(),
            turn: if pos.turn().is_white() {
                "white"
            } else {
                "black"
            }
            .into(),
            legal_uci: pos.legal_moves().iter().map(|&m| uci_of(&pos, m)).collect(),
        })
    }
    pub fn san_at(&self, id: NodeId) -> Result<Option<String>, CoreError> {
        let node = self.nodes.get(id as usize).ok_or(CoreError::InvalidNode)?;
        let Some(parent) = node.parent else {
            return if id == 0 {
                Ok(None)
            } else {
                Err(CoreError::InvalidNode)
            };
        };
        let pos = self.chess_position(parent)?;
        let uci = node.uci.as_deref().ok_or(CoreError::InvalidNode)?;
        if uci == "0000" {
            apply_uci(pos, uci)?;
            return Ok(Some("--".into()));
        }
        let m: UciMove = uci
            .parse()
            .map_err(|_| CoreError::IllegalMove(uci.into()))?;
        let m = m
            .to_move(&pos)
            .map_err(|_| CoreError::IllegalMove(uci.into()))?;
        Ok(Some(SanPlus::from_move(pos, m).to_string()))
    }
}

/// Reads a FEN. Positions whose castling rights standard chess cannot hold
/// (a king or castling rook off its standard square) are read as Chess960,
/// with X-FEN or Shredder-FEN castling letters.
pub fn parse_position(fen: &str) -> Result<Chess, CoreError> {
    let fen: Fen = fen
        .parse()
        .map_err(|e: shakmaty::fen::ParseFenError| CoreError::InvalidFen(e.to_string()))?;
    match fen.clone().into_position(CastlingMode::Standard) {
        Ok(pos) => Ok(pos),
        Err(standard) => fen
            .into_position(CastlingMode::Chess960)
            .map_err(|_| CoreError::InvalidFen(standard.to_string())),
    }
}
/// A move in UCI as this position plays it: castling is king to rook in
/// Chess960 positions and king two squares otherwise.
pub fn uci_of(pos: &Chess, m: shakmaty::Move) -> String {
    UciMove::from_move(m, pos.castles().mode()).to_string()
}
/// The `Variant` tag value of Chess960 games.
pub const CHESS960: &str = "Chess960";
/// Whether a `Variant` tag names Chess960 (also "Fischerandom" and
/// "chess 960", as PGN sources write it).
pub fn is_chess960_variant(variant: &str) -> bool {
    let v: String = variant
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    v == "chess960" || v == "fischerandom" || v == "fischerrandom" || v == "960"
}
pub fn apply_uci(mut pos: Chess, uci: &str) -> Result<Chess, CoreError> {
    if uci == "0000" {
        if pos.is_check() {
            return Err(CoreError::IllegalMove(uci.into()));
        }
        return pos
            .swap_turn()
            .map_err(|_| CoreError::IllegalMove(uci.into()));
    }
    let m: UciMove = uci
        .parse()
        .map_err(|_| CoreError::IllegalMove(uci.into()))?;
    let m = m
        .to_move(&pos)
        .map_err(|_| CoreError::IllegalMove(uci.into()))?;
    pos.play_unchecked(m);
    Ok(pos)
}
