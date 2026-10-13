pub mod annotations;
pub mod companions;
mod database;
pub mod entities;
pub mod error;
pub mod header;
pub mod limits;
pub mod moves;
pub mod position;
pub mod search;
pub mod source;
pub mod v2;
pub use database::*;
pub use error::CbhError;
pub use limits::ReadLimits;
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    Game,
    Text,
    Unknown(u8),
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DataStatus {
    Available,
    Missing,
    Unsupported,
    Corrupt,
    NotDecoded,
    /// Decoded, but some content is not represented (see diagnostics).
    Partial,
}
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub file: Option<String>,
    pub offset: Option<u64>,
}
impl Diagnostic {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            file: None,
            offset: None,
        }
    }
}
