use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionView {
    pub fen: String,
    pub turn: String,
    pub legal_uci: Vec<String>,
}
