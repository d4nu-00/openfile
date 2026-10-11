use crate::{CoreError, Game, GameLimits, NodeId, model::apply_uci};
use shakmaty::Chess;
/// Transient import builder. Position cache is released when the game is finished.
pub struct GameBuilder {
    game: Game,
    positions: Vec<Chess>,
    limits: GameLimits,
}
impl GameBuilder {
    pub fn new(game: Game, limits: GameLimits) -> Result<Self, CoreError> {
        if limits.max_nodes == 0 {
            return Err(CoreError::LimitExceeded);
        }
        if game.nodes.len() != 1 || !game.nodes[0].children.is_empty() {
            return Err(CoreError::InvalidNode);
        }
        let pos = game.chess_position(0)?;
        Ok(Self {
            game,
            positions: vec![pos],
            limits,
        })
    }
    pub fn append_uci(&mut self, parent: NodeId, uci: &str) -> Result<NodeId, CoreError> {
        let pos = self.position(parent)?.clone();
        let id = self
            .game
            .append_at(parent, uci, &self.limits, pos.clone())?;
        if id as usize == self.positions.len() {
            let after = apply_uci(
                pos,
                self.game.nodes[id as usize]
                    .uci
                    .as_deref()
                    .ok_or(CoreError::InvalidNode)?,
            )?;
            self.positions.push(after);
        }
        Ok(id)
    }
    pub fn position(&self, id: NodeId) -> Result<&Chess, CoreError> {
        self.positions
            .get(id as usize)
            .ok_or(CoreError::InvalidNode)
    }
    pub fn game(&self) -> &Game {
        &self.game
    }
    pub(crate) fn game_mut(&mut self) -> &mut Game {
        &mut self.game
    }
    pub fn finish(self) -> Game {
        self.game
    }
}
