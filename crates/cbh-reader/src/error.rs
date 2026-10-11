#[derive(Debug, thiserror::Error)]
pub enum CbhError {
    #[error("Invalid CBH header")]
    InvalidHeader,
    #[error("Truncated database file")]
    Truncated,
    #[error("Database source changed after opening")]
    SourceChanged,
    #[error("Requested range is outside the database")]
    Bounds,
    #[error("Unsupported database format: {0}")]
    Unsupported(String),
    #[error("Unsupported setup position: {0}")]
    UnsupportedSetup(String),
    #[error("Read limit exceeded")]
    LimitExceeded,
    #[error("Ambiguous companion filename")]
    AmbiguousCompanion,
    #[error("Invalid game data: {0}")]
    Decode(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Chess(#[from] chess_core::CoreError),
}
impl CbhError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidHeader => "invalidHeader",
            Self::Truncated => "truncated",
            Self::SourceChanged => "sourceChanged",
            Self::Bounds => "bounds",
            Self::Unsupported(_) => "unsupportedEncoding",
            Self::UnsupportedSetup(_) => "unsupportedSetup",
            Self::LimitExceeded => "limitExceeded",
            Self::AmbiguousCompanion => "ambiguousCompanion",
            Self::Decode(_) => "corrupt",
            Self::Io(_) => "io",
            Self::Chess(_) => "invalidGame",
        }
    }
}
