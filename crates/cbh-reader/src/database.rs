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
pub struct ClassicDatabase {
    header: crate::source::SourceFile,
    moves: Option<crate::source::SourceFile>,
    annotations: Option<crate::source::SourceFile>,
    entities: crate::entities::EntityTables,
    info: DatabaseInfo,
    limits: ReadLimits,
    source_paths: Vec<std::path::PathBuf>,
}
impl ClassicDatabase {
    pub fn open(path: &Path, limits: ReadLimits) -> Result<Self, CbhError> {
        use crate::{
            companions::CompanionSet, entities::EntityTables, header::ClassicHeader,
            source::SourceFile,
        };
        let files = CompanionSet::discover(path)?;
        let header = SourceFile::open(files.get("cbh").ok_or(CbhError::InvalidHeader)?)?;
        let parsed = ClassicHeader::parse(&header.read_exact_range(0, 46)?, header.len())?;
        let mut diagnostics = parsed.diagnostics;
        let (entities, notes) = EntityTables::open_partial(&files, limits.entity_cache_bytes);
        diagnostics.extend(notes);
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
        let moves = open("cbg");
        let annotations = open("cba");
        let companions = [
            "cbh", "cbg", "cbp", "cbt", "cba", "cbc", "cbe", "cbs", "cbl",
        ]
        .into_iter()
        .map(|extension| CompanionInfo {
            extension: extension.into(),
            status: if files.get(extension).is_none() {
                DataStatus::Missing
            } else if ["cba", "cbc", "cbe", "cbs", "cbl"].contains(&extension) {
                DataStatus::NotDecoded
            } else {
                DataStatus::Available
            },
        })
        .collect();
        Ok(Self {
            source_paths: files.paths.values().cloned().collect(),
            header,
            moves,
            annotations,
            entities,
            info: DatabaseInfo {
                entry_count: parsed.entry_count,
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
        crate::header::parse_record(
            &self
                .header
                .read_exact_range(ordinal.checked_mul(46).ok_or(CbhError::Bounds)?, 46)?,
            ordinal,
        )
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
        if r.eco_raw.is_some_and(|v| v != 0) {
            diagnostics.push(Diagnostic::new(
                "unverifiedEco",
                "Packed ECO code is not yet interpreted",
            ));
        }
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
            eco: None,
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
                "CBG companion is unavailable",
            ));
            return Ok(loaded);
        };
        let offset = offset.ok_or(CbhError::Bounds)?;
        let decode = || -> Result<chess_core::Game, CbhError> {
            let head = file.read_exact_range(offset, 4)?;
            let length = crate::header::be24(&head[1..]) as usize;
            if length > self.limits.max_game_bytes {
                return Err(CbhError::LimitExceeded);
            }
            crate::moves::decode_classic(&file.read_exact_range(offset, length)?, &self.limits)
        };
        match decode() {
            Ok(mut game) => {
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
                diagnostic.file = Some("cbg".into());
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
