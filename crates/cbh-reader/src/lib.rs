pub mod annotations;
pub mod companions;
pub mod crosstable;
mod database;
pub mod dossier;
pub mod eco;
pub mod entities;
pub mod error;
pub mod header;
pub mod history;
pub mod limits;
pub mod maintain;
pub mod moves;
pub mod name_list;
pub mod pattern;
pub mod player_stats;
pub mod position;
pub mod prepare;
pub mod scan;
pub mod search;
pub mod sort;
pub mod source;
pub mod tree;
pub mod v2;
pub use database::*;
pub use error::CbhError;
pub use limits::ReadLimits;
/// The PGN tag that keeps a ChessBase result PGN cannot express: a forfeit
/// ("+:-", "=:=", "-:+") or "0-0".
pub const RESULT_TAG: &str = "CBResult";
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
