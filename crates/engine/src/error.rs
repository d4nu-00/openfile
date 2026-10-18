#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Malformed UCI response: {0}")]
    Protocol(String),
    #[error("Invalid engine option: {0}")]
    Option(String),
    #[error("UCI input exceeds the line limit")]
    LineLimit,
    #[error("Invalid engine position: {0}")]
    Position(String),
    #[error("Engine I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Engine timed out: {0}")]
    Timeout(String),
    #[error("Engine process exited")]
    Exited,
    /// The system stopped the program (a quarantined download on macOS).
    #[error("{0}")]
    Blocked(String),
    /// The chosen path is a folder, not a program.
    #[error("{0}")]
    NotAProgram(String),
}
