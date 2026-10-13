use crate::{CbhError, DataStatus, Diagnostic, EntryKind, ReadLimits};
use std::path::Path;
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrySummary {
    pub ordinal: u64,
    pub kind: EntryKind,
    pub deleted: bool,
    pub white: Option<String>,
    pub black: Option<String>,
    pub event: Option<String>,
    pub site: Option<String>,
    pub date: Option<String>,
    pub result: Option<String>,
    pub round: Option<String>,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    pub eco: Option<String>,
    pub white_id: Option<u32>,
    pub black_id: Option<u32>,
    pub tournament_id: Option<u32>,
    pub annotator_id: Option<u32>,
    pub source_id: Option<u32>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub format: DatabaseFormat,
    pub entry_count: u64,
    pub companions: Vec<CompanionInfo>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, serde::Serialize)]
pub struct CompanionInfo {
    pub extension: String,
    pub status: DataStatus,
}
#[derive(Debug, serde::Serialize)]
pub struct EntryPage {
    pub entries: Vec<EntrySummary>,
    pub next: Option<u64>,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedEntry {
    pub summary: EntrySummary,
    pub game: Option<chess_core::Game>,
    pub metadata_status: DataStatus,
    pub moves_status: DataStatus,
    pub annotations_status: DataStatus,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseFormat {
    /// Classic CBH family: 46-byte records, byte-coded CBG moves.
    Classic,
    /// Newer 2CBH family: 192-byte records, framed 2CBG/2CBA blocks.
    New,
}
enum Names {
    Classic(crate::entities::EntityTables),
    New(crate::v2::names::NameIndex),
}
impl Names {
    fn player(&mut self, id: u32) -> Result<crate::entities::EntityText, CbhError> {
        match self {
            Self::Classic(t) => t.player(id),
            Self::New(t) => t.player(id),
        }
    }
    fn tournament(&mut self, id: u32) -> Result<crate::entities::TournamentText, CbhError> {
        match self {
            Self::Classic(t) => t.tournament(id),
            Self::New(t) => t.tournament(id),
        }
    }
    fn verify_unchanged(&self) -> Result<(), CbhError> {
        match self {
            Self::Classic(t) => t.verify_unchanged(),
            Self::New(t) => t.verify_unchanged(),
        }
    }
    fn bytes_read(&self) -> u64 {
        match self {
            Self::Classic(t) => t.bytes_read(),
            Self::New(t) => t.bytes_read(),
        }
    }
}
/// A read-only CBH-family database in either supported format.
pub struct Database {
    format: DatabaseFormat,
    header: crate::source::SourceFile,
    moves: Option<crate::source::SourceFile>,
    annotations: Option<crate::source::SourceFile>,
    entities: Names,
    info: DatabaseInfo,
    limits: ReadLimits,
    source_paths: Vec<std::path::PathBuf>,
}
/// Former name, kept for callers that only open classic files.
pub type ClassicDatabase = Database;
const CLASSIC_COMPANIONS: &[&str] = &[
    "cbh", "cbg", "cbp", "cbt", "cba", "cbc", "cbe", "cbs", "cbl",
];
const NEW_COMPANIONS: &[&str] = &["2cbh", "2cbg", "2cba", "2lid", "2lcd", "2lgd"];
impl Database {
    pub fn open(path: &Path, limits: ReadLimits) -> Result<Self, CbhError> {
        use crate::{companions::CompanionSet, source::SourceFile};
        let files = CompanionSet::discover(path)?;
        let format = files.format;
        let (head, moves_ext, notes_ext, record) = match format {
            DatabaseFormat::Classic => ("cbh", "cbg", "cba", 46),
            DatabaseFormat::New => ("2cbh", "2cbg", "2cba", crate::v2::record::RECORD),
        };
        let header = SourceFile::open(files.get(head).ok_or(CbhError::InvalidHeader)?)?;
        let (entry_count, mut diagnostics) = match format {
            DatabaseFormat::Classic => {
                let parsed = crate::header::ClassicHeader::parse(
                    &header.read_exact_range(0, record)?,
                    header.len(),
                )?;
                (parsed.entry_count, parsed.diagnostics)
            }
            DatabaseFormat::New => {
                let parsed = crate::v2::record::NewHeader::parse(
                    &header.read_exact_range(0, record)?,
                    header.len(),
                )?;
                (parsed.entry_count, parsed.diagnostics)
            }
        };
        let entities = match format {
            DatabaseFormat::Classic => {
                let (tables, notes) =
                    crate::entities::EntityTables::open_partial(&files, limits.entity_cache_bytes);
                diagnostics.extend(notes);
                Names::Classic(tables)
            }
            DatabaseFormat::New => {
                let (index, notes) =
                    crate::v2::names::NameIndex::open_partial(&files, limits.entity_cache_bytes);
                diagnostics.extend(notes);
                Names::New(index)
            }
        };
        let mut open = |extension| {
            files
                .get(extension)
                .and_then(|path| match SourceFile::open(path) {
                    Ok(file) => Some(file),
                    Err(error) => {
                        diagnostics.push(Diagnostic::new(
                            error.code(),
                            format!("{extension}: {error}"),
                        ));
                        None
                    }
                })
        };
        let moves = open(moves_ext);
        let annotations = open(notes_ext);
        let (listed, decoded): (&[&str], &[&str]) = match format {
            DatabaseFormat::Classic => (CLASSIC_COMPANIONS, &["cbh", "cbg", "cbp", "cbt"]),
            DatabaseFormat::New => (NEW_COMPANIONS, crate::v2::names::DECODED_COMPANIONS),
        };
        let companions = listed
            .iter()
            .map(|&extension| CompanionInfo {
                extension: extension.into(),
                status: if files.get(extension).is_none() {
                    DataStatus::Missing
                } else if decoded.contains(&extension) {
                    DataStatus::Available
                } else {
                    DataStatus::NotDecoded
                },
            })
            .collect();
        Ok(Self {
            format,
            source_paths: files.paths.values().cloned().collect(),
            header,
            moves,
            annotations,
            entities,
            info: DatabaseInfo {
                format,
                entry_count,
                companions,
                diagnostics,
            },
            limits,
        })
    }
    pub fn source_paths(&self) -> &[std::path::PathBuf] {
        &self.source_paths
    }
    pub fn info(&self) -> &DatabaseInfo {
        &self.info
    }
    fn verify(&self) -> Result<(), CbhError> {
        self.header.verify_unchanged()?;
        self.entities.verify_unchanged()?;
        if let Some(file) = &self.moves {
            file.verify_unchanged()?;
        }
        if let Some(file) = &self.annotations {
            file.verify_unchanged()?;
        }
        Ok(())
    }
    fn raw(&self, ordinal: u64) -> Result<crate::header::RawEntry, CbhError> {
        if ordinal == 0 || ordinal > self.info.entry_count {
            return Err(CbhError::Bounds);
        }
        let size = match self.format {
            DatabaseFormat::Classic => 46,
            DatabaseFormat::New => crate::v2::record::RECORD,
        };
        let bytes = self.header.read_exact_range(
            ordinal.checked_mul(size as u64).ok_or(CbhError::Bounds)?,
            size,
        )?;
        match self.format {
            DatabaseFormat::Classic => crate::header::parse_record(&bytes, ordinal),
            DatabaseFormat::New => crate::v2::record::parse_record(&bytes, ordinal),
        }
    }
    fn summary(&mut self, r: crate::header::RawEntry) -> Result<EntrySummary, CbhError> {
        let mut diagnostics = r.diagnostics;
        let mut name = |id| -> Result<Option<String>, CbhError> {
            let Some(id) = id else {
                return Ok(None);
            };
            match self.entities.player(id) {
                Ok(v) => {
                    diagnostics.extend(v.diagnostics);
                    Ok(v.text)
                }
                Err(CbhError::SourceChanged) => Err(CbhError::SourceChanged),
                Err(error) => {
                    diagnostics.push(Diagnostic::new("unresolvedPlayer", error.to_string()));
                    Ok(None)
                }
            }
        };
        let white = name(r.white_id)?;
        let black = name(r.black_id)?;
        let (event, site) = if let Some(id) = r.tournament_id {
            match self.entities.tournament(id) {
                Ok(v) => {
                    diagnostics.extend(v.event.diagnostics);
                    diagnostics.extend(v.site.diagnostics);
                    (v.event.text, v.site.text)
                }
                Err(CbhError::SourceChanged) => return Err(CbhError::SourceChanged),
                Err(error) => {
                    diagnostics.push(Diagnostic::new("unresolvedTournament", error.to_string()));
                    (None, None)
                }
            }
        } else {
            (None, None)
        };
        let eco = r.eco_raw.and_then(|raw| {
            let eco = crate::header::eco(raw);
            if raw >> 7 != 0 && eco.is_none() {
                diagnostics.push(Diagnostic::new(
                    "invalidEco",
                    format!("Packed ECO value {raw:#06x} is out of range"),
                ));
            }
            eco
        });
        Ok(EntrySummary {
            ordinal: r.ordinal,
            kind: r.kind,
            deleted: r.deleted,
            white,
            black,
            event,
            site,
            date: r.date,
            result: r.result,
            round: r.round,
            white_elo: r.white_elo,
            black_elo: r.black_elo,
            eco,
            white_id: r.white_id,
            black_id: r.black_id,
            tournament_id: r.tournament_id,
            annotator_id: r.annotator_id,
            source_id: r.source_id,
            diagnostics,
        })
    }
    pub fn page(&mut self, start: u64, limit: usize) -> Result<EntryPage, CbhError> {
        self.verify()?;
        if start == 0 || start > self.info.entry_count || limit == 0 || limit > self.limits.max_page
        {
            return Err(CbhError::Bounds);
        }
        let end = start
            .checked_add(limit as u64)
            .ok_or(CbhError::Bounds)?
            .min(self.info.entry_count + 1);
        let mut entries = Vec::with_capacity((end - start) as usize);
        for ordinal in start..end {
            let raw = self.raw(ordinal)?;
            entries.push(self.summary(raw)?);
        }
        Ok(EntryPage {
            entries,
            next: if end <= self.info.entry_count {
                Some(end)
            } else {
                None
            },
        })
    }
    pub fn load(&mut self, ordinal: u64) -> Result<LoadedEntry, CbhError> {
        self.verify()?;
        let raw = self.raw(ordinal)?;
        let offset = raw.moves_offset;
        let annotation_offset = raw.annotation_offset;
        let summary = self.summary(raw)?;
        let metadata_status = if summary.kind == EntryKind::Game {
            DataStatus::Available
        } else {
            DataStatus::Unsupported
        };
        let annotations_status = if annotation_offset.is_some() || self.annotations.is_some() {
            DataStatus::NotDecoded
        } else {
            DataStatus::Missing
        };
        let mut loaded = LoadedEntry {
            summary,
            game: None,
            metadata_status,
            moves_status: DataStatus::Missing,
            annotations_status,
            diagnostics: vec![],
        };
        if loaded.summary.kind != EntryKind::Game {
            loaded.moves_status = DataStatus::Unsupported;
            loaded.diagnostics.push(Diagnostic::new(
                "unsupportedEntry",
                "Entry is not a chess game",
            ));
            return Ok(loaded);
        }
        let Some(file) = &self.moves else {
            loaded.diagnostics.push(Diagnostic::new(
                "missingMoves",
                "Move companion (CBG/2CBG) is unavailable",
            ));
            return Ok(loaded);
        };
        let offset = offset.ok_or(CbhError::Bounds)?;
        let format = self.format;
        let decode = || -> Result<(chess_core::Game, Vec<Diagnostic>), CbhError> {
            match format {
                DatabaseFormat::Classic => {
                    let head = file.read_exact_range(offset, 4)?;
                    let length = crate::header::be24(&head[1..]) as usize;
                    if length > self.limits.max_game_bytes {
                        return Err(CbhError::LimitExceeded);
                    }
                    let record = file.read_exact_range(offset, length)?;
                    Ok((crate::moves::decode_classic(&record, &self.limits)?, vec![]))
                }
                DatabaseFormat::New => {
                    let block =
                        crate::v2::block::read_block(file, offset, self.limits.max_game_bytes)?;
                    if block.kind != crate::v2::block::TYPE_GAME {
                        return Err(CbhError::Unsupported(format!(
                            "2CBG block type {:#06x}",
                            block.kind
                        )));
                    }
                    let decoded = crate::v2::moves::decode_moves(&block.payload, &self.limits)?;
                    Ok((decoded.game, decoded.diagnostics))
                }
            }
        };
        match decode() {
            Ok((mut game, notes)) => {
                loaded.diagnostics.extend(notes);
                for (key, value) in [
                    ("White", &loaded.summary.white),
                    ("Black", &loaded.summary.black),
                    ("Event", &loaded.summary.event),
                    ("Site", &loaded.summary.site),
                    ("Date", &loaded.summary.date),
                    ("Result", &loaded.summary.result),
                    ("Round", &loaded.summary.round),
                    ("ECO", &loaded.summary.eco),
                ] {
                    if let Some(value) = value {
                        game.tags.insert(key.into(), value.clone());
                    }
                }
                for (key, value) in [
                    ("WhiteElo", loaded.summary.white_elo),
                    ("BlackElo", loaded.summary.black_elo),
                ] {
                    if let Some(value) = value {
                        game.tags.insert(key.into(), value.to_string());
                    }
                }
                loaded.moves_status = DataStatus::Available;
                loaded.game = Some(game);
            }
            Err(CbhError::SourceChanged) => return Err(CbhError::SourceChanged),
            Err(error) => {
                loaded.moves_status = if matches!(
                    error,
                    CbhError::Unsupported(_)
                        | CbhError::UnsupportedSetup(_)
                        | CbhError::LimitExceeded
                ) {
                    DataStatus::Unsupported
                } else {
                    DataStatus::Corrupt
                };
                let mut diagnostic = Diagnostic::new(error.code(), error.to_string());
                diagnostic.file = Some(
                    match self.format {
                        DatabaseFormat::Classic => "cbg",
                        DatabaseFormat::New => "2cbg",
                    }
                    .into(),
                );
                diagnostic.offset = Some(offset);
                loaded.diagnostics.push(diagnostic);
            }
        }
        Ok(loaded)
    }
    pub fn bytes_read(&self) -> u64 {
        self.header.bytes_read()
            + self.entities.bytes_read()
            + self.moves.as_ref().map_or(0, |f| f.bytes_read())
    }
}
