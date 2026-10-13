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
    /// As ChessBase shows it: `1-0`, `+:-` (forfeit), a line's evaluation…
    pub result: Option<String>,
    /// The stored code, 0..=7 (see `header::pgn_result`).
    pub result_code: Option<u8>,
    pub round: Option<String>,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    pub eco: Option<String>,
    pub white_id: Option<u32>,
    pub black_id: Option<u32>,
    pub tournament_id: Option<u32>,
    pub annotator_id: Option<u32>,
    pub source_id: Option<u32>,
    pub annotator: Option<String>,
    /// Source title, publisher and publication date.
    pub source: Option<String>,
    pub source_publisher: Option<String>,
    pub source_date: Option<String>,
    pub white_team: Option<String>,
    pub black_team: Option<String>,
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
    Classic(Box<crate::entities::EntityTables>),
    New(Box<crate::v2::names::NameIndex>),
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
    fn annotator(&mut self, id: u32) -> Result<crate::entities::EntityText, CbhError> {
        match self {
            Self::Classic(t) => t.annotator(id),
            Self::New(t) => t.annotator(id),
        }
    }
    fn team(&mut self, id: u32) -> Result<crate::entities::EntityText, CbhError> {
        match self {
            Self::Classic(t) => t.team(id),
            Self::New(t) => t.team(id),
        }
    }
    fn source(&mut self, id: u32) -> Result<crate::entities::SourceText, CbhError> {
        match self {
            Self::Classic(t) => t.source(id),
            Self::New(t) => t.source(id),
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
    /// Classic CBJ: per-entry records whose first eight bytes are the BE32
    /// white and black team ids.
    teams: Option<TeamIndex>,
    entities: Names,
    info: DatabaseInfo,
    limits: ReadLimits,
    source_paths: Vec<std::path::PathBuf>,
    /// The path this database was opened from.
    path: std::path::PathBuf,
}
struct TeamIndex {
    file: crate::source::SourceFile,
    stride: u64,
    count: u64,
}
impl TeamIndex {
    /// CBJ header: LE32 version, LE32 record size, LE32 entry count; records
    /// start at byte 32, one per entry from entry 1.
    fn open(file: crate::source::SourceFile) -> Result<Self, CbhError> {
        let head = file.read_exact_range(0, 32)?;
        let le32 = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap()) as u64;
        let (stride, count) = (le32(4), le32(8));
        if !(8..=4096).contains(&stride) {
            return Err(CbhError::Unsupported(format!("CBJ record size {stride}")));
        }
        if 32 + stride * count > file.len() {
            return Err(CbhError::Truncated);
        }
        Ok(Self {
            file,
            stride,
            count,
        })
    }
    fn ids(&self, ordinal: u64) -> Result<(Option<u32>, Option<u32>), CbhError> {
        if ordinal == 0 || ordinal > self.count {
            return Ok((None, None));
        }
        let b = self
            .file
            .read_exact_range(32 + (ordinal - 1) * self.stride, 8)?;
        let id = |i: usize| {
            let v = u32::from_be_bytes(b[i..i + 4].try_into().unwrap());
            (v != u32::MAX).then_some(v)
        };
        Ok((id(0), id(4)))
    }
}
/// Former name, kept for callers that only open classic files.
pub type ClassicDatabase = Database;
const CLASSIC_COMPANIONS: &[&str] = &[
    "cbh", "cbg", "cbp", "cbt", "cba", "cbc", "cbe", "cbs", "cbj", "cbl",
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
                Names::Classic(Box::new(tables))
            }
            DatabaseFormat::New => {
                let (index, notes) =
                    crate::v2::names::NameIndex::open_partial(&files, limits.entity_cache_bytes);
                diagnostics.extend(notes);
                Names::New(Box::new(index))
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
        let teams = match format {
            DatabaseFormat::Classic => open("cbj").and_then(|file| match TeamIndex::open(file) {
                Ok(index) => Some(index),
                Err(error) => {
                    diagnostics.push(Diagnostic::new(error.code(), format!("cbj: {error}")));
                    None
                }
            }),
            DatabaseFormat::New => None,
        };
        let (listed, decoded): (&[&str], &[&str]) = match format {
            DatabaseFormat::Classic => (
                CLASSIC_COMPANIONS,
                &[
                    "cbh", "cbg", "cbp", "cbt", "cba", "cbc", "cbs", "cbe", "cbj",
                ],
            ),
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
            path: path.to_path_buf(),
            header,
            moves,
            annotations,
            teams,
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
    /// Stamps of the entry index and the move file; a position index built
    /// from this database is current while they are unchanged.
    pub fn fingerprint(&self) -> [u64; 4] {
        let (a, b) = self.header.stamp();
        let (c, d) = self.moves.as_ref().map_or((0, 0), |f| f.stamp());
        [a, b, c, d]
    }
    /// Another read handle on the same files, e.g. for a worker thread.
    pub fn reopen(&self) -> Result<Self, CbhError> {
        Self::open(&self.path, self.limits.clone())
    }
    pub fn source_paths(&self) -> &[std::path::PathBuf] {
        &self.source_paths
    }
    pub fn info(&self) -> &DatabaseInfo {
        &self.info
    }
    pub(crate) fn verify(&self) -> Result<(), CbhError> {
        self.header.verify_unchanged()?;
        self.entities.verify_unchanged()?;
        if let Some(file) = &self.moves {
            file.verify_unchanged()?;
        }
        if let Some(file) = &self.annotations {
            file.verify_unchanged()?;
        }
        if let Some(index) = &self.teams {
            index.file.verify_unchanged()?;
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
            DatabaseFormat::Classic => {
                let mut raw = crate::header::parse_record(&bytes, ordinal)?;
                if raw.kind == EntryKind::Game
                    && let Some(index) = &self.teams
                {
                    (raw.white_team_id, raw.black_team_id) = index.ids(ordinal)?;
                }
                Ok(raw)
            }
            DatabaseFormat::New => crate::v2::record::parse_record(&bytes, ordinal),
        }
    }
    /// Records `start .. start + n` read in one range (game entries only get
    /// their classic team ids in `raw`, which search does not need).
    pub(crate) fn raw_chunk(
        &self,
        start: u64,
        n: u64,
    ) -> Result<Vec<crate::header::RawEntry>, CbhError> {
        if start == 0 || start + n - 1 > self.info.entry_count {
            return Err(CbhError::Bounds);
        }
        let size = self.format.record_size();
        let bytes = self
            .header
            .read_exact_range(start * size as u64, n as usize * size)?;
        bytes
            .chunks_exact(size)
            .enumerate()
            .map(|(i, b)| match self.format {
                DatabaseFormat::Classic => crate::header::parse_record(b, start + i as u64),
                DatabaseFormat::New => crate::v2::record::parse_record(b, start + i as u64),
            })
            .collect()
    }
    pub(crate) fn names_each_player(
        &self,
        f: &mut dyn FnMut(u32, &str, &str) -> bool,
    ) -> Result<(), CbhError> {
        match &self.entities {
            Names::Classic(t) => t.each_player(f),
            Names::New(t) => t.each_player(f),
        }
    }
    pub(crate) fn names_each_tournament(
        &self,
        f: &mut dyn FnMut(u32, &str, &str) -> bool,
    ) -> Result<(), CbhError> {
        match &self.entities {
            Names::Classic(t) => t.each_tournament(f),
            Names::New(t) => t.each_tournament(f),
        }
    }
    pub(crate) fn names_each_annotator(
        &self,
        f: &mut dyn FnMut(u32, &str) -> bool,
    ) -> Result<(), CbhError> {
        match &self.entities {
            Names::Classic(t) => t.each_annotator(f),
            Names::New(t) => t.each_annotator(f),
        }
    }
    fn summary(&mut self, r: crate::header::RawEntry) -> Result<EntrySummary, CbhError> {
        let mut diagnostics = r.diagnostics;
        let entities = &mut self.entities;
        // Resolves one name; only a changed source is an error.
        let mut resolve = |kind: u8, id: Option<u32>, diagnostics: &mut Vec<Diagnostic>| {
            let Some(id) = id else {
                return Ok(None);
            };
            let found = match kind {
                crate::entities::PLAYER => entities.player(id),
                crate::entities::ANNOTATOR => entities.annotator(id),
                _ => entities.team(id),
            };
            match found {
                Ok(v) => {
                    diagnostics.extend(v.diagnostics);
                    Ok(v.text)
                }
                Err(CbhError::SourceChanged) => Err(CbhError::SourceChanged),
                Err(error) => {
                    let code = match kind {
                        crate::entities::PLAYER => "unresolvedPlayer",
                        crate::entities::ANNOTATOR => "unresolvedAnnotator",
                        _ => "unresolvedTeam",
                    };
                    diagnostics.push(Diagnostic::new(code, error.to_string()));
                    Ok(None)
                }
            }
        };
        use crate::entities::{ANNOTATOR, PLAYER, TEAM};
        let white = resolve(PLAYER, r.white_id, &mut diagnostics)?;
        let black = resolve(PLAYER, r.black_id, &mut diagnostics)?;
        let annotator = resolve(ANNOTATOR, r.annotator_id, &mut diagnostics)?;
        let white_team = resolve(TEAM, r.white_team_id, &mut diagnostics)?;
        let black_team = resolve(TEAM, r.black_team_id, &mut diagnostics)?;
        let (source, source_publisher, source_date) = match r.source_id {
            None => (None, None, None),
            Some(id) => match self.entities.source(id) {
                Ok(v) => {
                    diagnostics.extend(v.title.diagnostics);
                    diagnostics.extend(v.publisher.diagnostics);
                    (v.title.text, v.publisher.text, v.date)
                }
                Err(CbhError::SourceChanged) => return Err(CbhError::SourceChanged),
                Err(error) => {
                    diagnostics.push(Diagnostic::new("unresolvedSource", error.to_string()));
                    (None, None, None)
                }
            },
        };
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
            result_code: r.result_code,
            round: r.round,
            white_elo: r.white_elo,
            black_elo: r.black_elo,
            eco,
            white_id: r.white_id,
            black_id: r.black_id,
            tournament_id: r.tournament_id,
            annotator_id: r.annotator_id,
            source_id: r.source_id,
            annotator,
            source,
            source_publisher,
            source_date,
            white_team,
            black_team,
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
        let annotations_status = DataStatus::Missing;
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
        let decode = || self.decode_moves_at(file, offset);
        match decode() {
            Ok((mut game, notes)) => {
                loaded.diagnostics.extend(notes);
                for (key, value) in [
                    ("White", &loaded.summary.white),
                    ("Black", &loaded.summary.black),
                    ("Event", &loaded.summary.event),
                    ("Site", &loaded.summary.site),
                    ("Date", &loaded.summary.date),
                    ("Round", &loaded.summary.round),
                    ("ECO", &loaded.summary.eco),
                    ("Annotator", &loaded.summary.annotator),
                    ("WhiteTeam", &loaded.summary.white_team),
                    ("BlackTeam", &loaded.summary.black_team),
                    ("SourceTitle", &loaded.summary.source),
                    ("Source", &loaded.summary.source_publisher),
                    ("SourceDate", &loaded.summary.source_date),
                ] {
                    if let Some(value) = value {
                        game.tags.insert(key.into(), value.clone());
                    }
                }
                if let Some(result) = loaded
                    .summary
                    .result_code
                    .and_then(crate::header::pgn_result)
                {
                    game.tags.insert("Result".into(), result.into());
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
                if let Some(at) = annotation_offset {
                    let (status, notes) = self.annotate(&mut game, at, ordinal)?;
                    loaded.annotations_status = status;
                    loaded.diagnostics.extend(notes);
                }
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
    /// Decodes the move tree stored at `offset` of the move file.
    fn decode_moves_at(
        &self,
        file: &crate::source::SourceFile,
        offset: u64,
    ) -> Result<(chess_core::Game, Vec<Diagnostic>), CbhError> {
        match self.format {
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
                let block = crate::v2::block::read_block(file, offset, self.limits.max_game_bytes)?;
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
    }
    /// The move tree of a game entry from [`Database::raw_chunk`], without
    /// names or annotations. `None` for text entries and games whose moves
    /// are missing, unsupported or damaged; only a changed source is an error.
    pub(crate) fn moves_of(
        &self,
        raw: &crate::header::RawEntry,
    ) -> Result<Option<chess_core::Game>, CbhError> {
        let (Some(file), Some(offset), EntryKind::Game) =
            (&self.moves, raw.moves_offset, &raw.kind)
        else {
            return Ok(None);
        };
        match self.decode_moves_at(file, offset) {
            Ok((game, _)) => Ok(Some(game)),
            Err(CbhError::SourceChanged) => Err(CbhError::SourceChanged),
            Err(_) => Ok(None),
        }
    }
    /// Decodes this game's CBA/2CBA annotations into `game`. Only a changed
    /// source is an error; anything else becomes a status and diagnostics.
    fn annotate(
        &self,
        game: &mut chess_core::Game,
        offset: u64,
        ordinal: u64,
    ) -> Result<(DataStatus, Vec<Diagnostic>), CbhError> {
        let Some(file) = &self.annotations else {
            return Ok((
                DataStatus::Missing,
                vec![Diagnostic::new(
                    "missingAnnotations",
                    "Annotation companion (CBA/2CBA) is unavailable",
                )],
            ));
        };
        let limit = self.limits.max_game_bytes;
        let parsed = match self.format {
            DatabaseFormat::Classic => (|| {
                let head = file.read_exact_range(offset, 14)?;
                let total = u32::from_be_bytes(head[10..14].try_into().unwrap()) as usize;
                if total > limit {
                    return Err(CbhError::LimitExceeded);
                }
                crate::annotations::parse_classic(&file.read_exact_range(offset, total)?, ordinal)
            })(),
            DatabaseFormat::New => {
                crate::v2::block::read_block(file, offset, limit).and_then(|b| {
                    if b.kind != crate::v2::block::TYPE_ANNOTATIONS {
                        return Err(CbhError::Decode(format!("2CBA block type {:#06x}", b.kind)));
                    }
                    crate::annotations::parse_new(&b.payload)
                })
            }
        };
        match parsed {
            Ok(parsed) => Ok(crate::annotations::apply(
                game,
                parsed,
                match self.format {
                    DatabaseFormat::Classic => crate::annotations::Order::Stream,
                    DatabaseFormat::New => crate::annotations::Order::Line,
                },
            )),
            Err(CbhError::SourceChanged) => Err(CbhError::SourceChanged),
            Err(error) => {
                let mut diagnostic = Diagnostic::new(error.code(), error.to_string());
                diagnostic.file = Some(
                    match self.format {
                        DatabaseFormat::Classic => "cba",
                        DatabaseFormat::New => "2cba",
                    }
                    .into(),
                );
                diagnostic.offset = Some(offset);
                let status = if matches!(error, CbhError::LimitExceeded) {
                    DataStatus::Unsupported
                } else {
                    DataStatus::Corrupt
                };
                Ok((status, vec![diagnostic]))
            }
        }
    }
    pub fn bytes_read(&self) -> u64 {
        self.header.bytes_read()
            + self.entities.bytes_read()
            + self.moves.as_ref().map_or(0, |f| f.bytes_read())
            + self.annotations.as_ref().map_or(0, |f| f.bytes_read())
            + self.teams.as_ref().map_or(0, |t| t.file.bytes_read())
    }
}
