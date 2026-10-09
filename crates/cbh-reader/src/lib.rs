pub mod companions;
pub mod entities;
pub mod error;
pub mod header;
pub mod limits;
pub mod source;
pub use error::CbhError;
pub use limits::ReadLimits;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Game,
    Text,
    Unknown(u8),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataStatus {
    Available,
    Missing,
    Unsupported,
    Corrupt,
    NotDecoded,
}
#[derive(Clone, Debug)]
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
