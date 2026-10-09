#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Invalid position: {0}")]
    InvalidFen(String),
    #[error("Invalid move: {0}")]
    IllegalMove(String),
    #[error("Invalid game tree or node")]
    InvalidNode,
    #[error("Game limit exceeded")]
    LimitExceeded,
    #[error("PGN comment cannot contain braces")]
    UnrepresentableComment,
    #[error("Invalid PGN: {0}")]
    InvalidPgn(String),
}
