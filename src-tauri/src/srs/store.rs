//! Decks in the app data folder (`srs/decks/<id>.json`), one file each,
//! replaced atomically. A file that does not parse, or that a newer version
//! wrote, is listed as unreadable and never written.
use crate::{
    clipboard::RowRange,
    commands::AppError,
    registry::Registry,
    srs::{
        deck::{
            Card, CardState, Colour, DECK_VERSION, Deck, DeckOptions, Grade, Kind, ReviewLog,
            SourceKind,
        },
        schedule::{self, DeckStats, Queue},
        source::{Collector, Skipped, collect_pgn, collect_rows},
    },
};
use chess_core::model::{apply_uci, parse_position, uci_of};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shakmaty::{EnPassantMode, Position, fen::Fen};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/// The log keeps this many answers at most.
const MAX_LOG: usize = 100_000;
const MAX_NAME: usize = 200;

fn err(message: impl std::fmt::Display) -> AppError {
    AppError::new("srs", message.to_string())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckSummary {
    pub id: String,
    pub name: String,
    pub source: String,
    pub source_kind: SourceKind,
    pub cards: u32,
    pub stats: DeckStats,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnreadableDeck {
    /// The file name in the decks folder.
    pub file: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckListing {
    pub decks: Vec<DeckSummary>,
    pub unreadable: Vec<UnreadableDeck>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResponse {
    pub deck: DeckSummary,
    pub skipped: Skipped,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResponse {
    pub deck: DeckSummary,
    pub skipped: Skipped,
    /// Lines the deck did not have.
    pub added: u32,
    /// Lines the deck had and the source still has; their progress is kept.
    pub kept: u32,
    /// Lines the source no longer has; they are suspended, not deleted.
    pub suspended: u32,
    /// Lines that had gone and are back.
    pub restored: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRow {
    pub id: String,
    pub source_label: String,
    pub white: String,
    pub black: String,
    pub event: String,
    pub date: String,
    pub plies: u32,
    pub suspended: bool,
    pub removed: bool,
    pub state: CardState,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckDetail {
    pub id: String,
    pub name: String,
    pub source: String,
    pub source_kind: SourceKind,
    pub created: i64,
    pub options: DeckOptions,
    pub cards: Vec<CardRow>,
}

/// One move of a card, with what the board needs to play it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlyInfo {
    /// The position before the move.
    pub fen: String,
    /// The legal moves where the trainee moves; empty for the opponent.
    pub legal_uci: Vec<String>,
    /// Moves other cards of the deck play from this position, where the
    /// trainee moves.
    pub other_moves: Vec<String>,
}

/// What a grade would do to the card.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub grade: Grade,
    /// When the card would be due, from now.
    pub due_in_ms: i64,
    pub interval_days: f64,
    pub kind: Kind,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyCard {
    pub card: Card,
    /// The card is new and the deck shows new cards first: it is a lesson.
    pub learn: bool,
    /// The line ends in mate by the trainee: their number of moves.
    pub mate_in: Option<u32>,
    pub state: CardState,
    /// One entry for each move of the card.
    pub plies: Vec<PlyInfo>,
    /// The position after the last move.
    pub end_fen: String,
    pub previews: Vec<Preview>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueResponse {
    pub queue: Queue,
    pub next: Option<StudyCard>,
    pub stats: DeckStats,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerResponse {
    pub state: CardState,
    #[serde(flatten)]
    pub session: QueueResponse,
}

/// The moves each position of the deck's cards has, by position without
/// the move counters.
type LineIndex = HashMap<String, Vec<String>>;

#[derive(Default)]
pub struct SrsStore {
    dir: Mutex<Option<PathBuf>>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    lines: Mutex<HashMap<String, (usize, Arc<LineIndex>)>>,
    counter: AtomicU64,
}

/// Why a deck file could not be used.
enum Failure {
    Missing,
    Newer,
    Unreadable(String),
}

impl Failure {
    fn message(&self) -> String {
        match self {
            Failure::Missing => "No such deck".into(),
            Failure::Newer => "This deck was made by a newer version of the app".into(),
            Failure::Unreadable(why) => format!("The deck file cannot be read: {why}"),
        }
    }
}

fn parse_deck(bytes: &[u8]) -> Result<Deck, Failure> {
    #[derive(Deserialize)]
    struct Peek {
        version: Option<u64>,
    }
    match serde_json::from_slice::<Deck>(bytes) {
        Ok(deck) if deck.version > DECK_VERSION => Err(Failure::Newer),
        Ok(deck) => Ok(deck),
        Err(e) => match serde_json::from_slice::<Peek>(bytes) {
            Ok(Peek { version: Some(v) }) if v > u64::from(DECK_VERSION) => Err(Failure::Newer),
            _ => Err(Failure::Unreadable(e.to_string())),
        },
    }
}

fn check_id(id: &str) -> Result<(), AppError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(err("Invalid deck"));
    }
    Ok(())
}

fn clean_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME || name.contains('\0') {
        return Err(AppError::new(
            "invalidParameters",
            "Choose a name of up to 200 characters",
        ));
    }
    Ok(name.to_string())
}

fn position_key(pos: &shakmaty::Chess) -> String {
    Fen::from_position(pos, EnPassantMode::Legal)
        .to_string()
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The moves that every card of the deck plays from each of its positions.
fn build_index(deck: &Deck) -> LineIndex {
    let mut sets: HashMap<String, BTreeSet<String>> = HashMap::new();
    for card in deck.cards.iter().filter(|c| !c.removed) {
        let Ok(mut pos) = parse_position(&card.start_fen) else {
            continue;
        };
        for m in &card.moves {
            sets.entry(position_key(&pos))
                .or_default()
                .insert(m.uci.clone());
            match apply_uci(pos, &m.uci) {
                Ok(next) => pos = next,
                Err(_) => break,
            }
        }
    }
    sets.into_iter()
        .map(|(key, set)| (key, set.into_iter().collect()))
        .collect()
}

fn summary(deck: &Deck, now: i64, utc_offset: i32) -> DeckSummary {
    DeckSummary {
        id: deck.id.clone(),
        name: deck.name.clone(),
        source: deck.source.clone(),
        source_kind: deck.source_kind,
        cards: deck.cards.len() as u32,
        stats: schedule::stats(deck, now, utc_offset),
    }
}

impl SrsStore {
    /// `dir` is the srs folder of the app data; decks go in its `decks`.
    pub fn set_dir(&self, dir: PathBuf) {
        *self.dir.lock().expect("srs directory") = Some(dir);
    }

    fn decks_dir(&self) -> Result<PathBuf, AppError> {
        Ok(self
            .dir
            .lock()
            .map_err(err)?
            .as_ref()
            .ok_or_else(|| err("No app data folder"))?
            .join("decks"))
    }

    fn path_of(&self, id: &str) -> Result<PathBuf, AppError> {
        check_id(id)?;
        Ok(self.decks_dir()?.join(format!("{id}.json")))
    }

    fn lock_for(&self, id: &str) -> Result<Arc<Mutex<()>>, AppError> {
        Ok(self
            .locks
            .lock()
            .map_err(err)?
            .entry(id.to_string())
            .or_default()
            .clone())
    }

    fn read(&self, id: &str) -> Result<Deck, AppError> {
        let failure = match std::fs::read(self.path_of(id)?) {
            Ok(bytes) => match parse_deck(&bytes) {
                Ok(deck) if deck.id == id => return Ok(deck),
                Ok(_) => Failure::Unreadable("the deck id does not match its file".into()),
                Err(failure) => failure,
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Failure::Missing,
            Err(e) => Failure::Unreadable(e.to_string()),
        };
        Err(err(failure.message()))
    }

    /// Replaces the deck's file; the old file stays until the new one is
    /// complete.
    fn write(&self, deck: &Deck) -> Result<(), AppError> {
        let path = self.path_of(&deck.id)?;
        let dir = path.parent().ok_or_else(|| err("No app data folder"))?;
        std::fs::create_dir_all(dir).map_err(err)?;
        let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(err)?;
        {
            let mut out = std::io::BufWriter::new(temp.as_file_mut());
            serde_json::to_writer(&mut out, deck).map_err(err)?;
            out.flush().map_err(err)?;
        }
        temp.as_file().sync_all().map_err(err)?;
        temp.persist(path).map_err(|e| err(e.error))?;
        Ok(())
    }

    /// Reads a deck under its lock.
    fn view<R>(
        &self,
        id: &str,
        f: impl FnOnce(&Deck) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        let lock = self.lock_for(id)?;
        let _guard = lock.lock().map_err(err)?;
        f(&self.read(id)?)
    }

    /// Reads, changes and writes a deck under its lock. A deck that cannot
    /// be read is never written.
    fn modify<R>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Deck) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        let lock = self.lock_for(id)?;
        let _guard = lock.lock().map_err(err)?;
        let mut deck = self.read(id)?;
        let out = f(&mut deck)?;
        self.write(&deck)?;
        Ok(out)
    }

    fn forget_lines(&self, id: &str) {
        if let Ok(mut lines) = self.lines.lock() {
            lines.remove(id);
        }
    }

    fn new_id(&self, name: &str) -> Result<String, AppError> {
        for _ in 0..100 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let n = self.counter.fetch_add(1, Ordering::Relaxed);
            let digest =
                Sha256::digest(format!("{nanos}:{n}:{}:{name}", std::process::id()).as_bytes());
            let id: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
            if !self.path_of(&id)?.exists() {
                return Ok(id);
            }
        }
        Err(err("Cannot make a deck id"))
    }

    /// The decks and the files that are not decks this version can use.
    pub fn decks(&self, now: i64, utc_offset: i32) -> Result<DeckListing, AppError> {
        let mut out = DeckListing {
            decks: vec![],
            unreadable: vec![],
        };
        let entries = match std::fs::read_dir(self.decks_dir()?) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(err(e)),
        };
        let mut decks: Vec<Deck> = vec![];
        for entry in entries {
            let path = entry.map_err(err)?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") || !path.is_file() {
                continue;
            }
            let file = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let stem = path
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let failure = match std::fs::read(&path) {
                Ok(bytes) => match parse_deck(&bytes) {
                    Ok(deck) if deck.id == stem => {
                        decks.push(deck);
                        continue;
                    }
                    Ok(_) => Failure::Unreadable("the deck id does not match its file".into()),
                    Err(failure) => failure,
                },
                Err(e) => Failure::Unreadable(e.to_string()),
            };
            out.unreadable.push(UnreadableDeck {
                file,
                reason: failure.message(),
            });
        }
        decks.sort_by(|a, b| (a.created, &a.name, &a.id).cmp(&(b.created, &b.name, &b.id)));
        out.decks = decks.iter().map(|d| summary(d, now, utc_offset)).collect();
        out.unreadable.sort_by(|a, b| a.file.cmp(&b.file));
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn create(
        &self,
        name: String,
        source: String,
        source_kind: SourceKind,
        options: DeckOptions,
        collected: (Vec<Card>, Skipped),
        now: i64,
        utc_offset: i32,
    ) -> Result<CreateResponse, AppError> {
        let (cards, skipped) = collected;
        if cards.is_empty() {
            return Err(AppError::new(
                "emptyDeck",
                "The source has no lines to study with these options",
            ));
        }
        let id = self.new_id(&name)?;
        let deck = Deck {
            version: DECK_VERSION,
            id: id.clone(),
            name,
            created: now,
            source,
            source_kind,
            options,
            cards,
            states: BTreeMap::new(),
            log: vec![],
        };
        let lock = self.lock_for(&id)?;
        let _guard = lock.lock().map_err(err)?;
        self.write(&deck)?;
        Ok(CreateResponse {
            deck: summary(&deck, now, utc_offset),
            skipped,
        })
    }

    pub fn create_from_pgn(
        &self,
        path: PathBuf,
        name: String,
        options: DeckOptions,
        now: i64,
        utc_offset: i32,
    ) -> Result<CreateResponse, AppError> {
        let name = clean_name(&name)?;
        options.check()?;
        let mut collector = Collector::new(&options);
        collect_pgn(&path, &mut collector)?;
        self.create(
            name,
            path.to_string_lossy().into_owned(),
            SourceKind::Pgn,
            options,
            collector.finish(),
            now,
            utc_offset,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_from_rows(
        &self,
        registry: &Registry,
        database_id: String,
        ranges: Vec<RowRange>,
        name: String,
        options: DeckOptions,
        now: i64,
        utc_offset: i32,
    ) -> Result<CreateResponse, AppError> {
        let name = clean_name(&name)?;
        options.check()?;
        let mut collector = Collector::new(&options);
        let source = collect_rows(registry, &database_id, &ranges, &mut collector)?;
        self.create(
            name,
            source,
            SourceKind::List,
            options,
            collector.finish(),
            now,
            utc_offset,
        )
    }

    /// Makes the deck's cards those of the source: progress is kept for the
    /// lines that remain, new lines are added and lines that are gone are
    /// suspended.
    fn update(
        &self,
        id: &str,
        source: String,
        source_kind: SourceKind,
        collected: (Vec<Card>, Skipped),
        now: i64,
        utc_offset: i32,
    ) -> Result<UpdateResponse, AppError> {
        let (fresh, skipped) = collected;
        if fresh.is_empty() {
            return Err(AppError::new(
                "emptyDeck",
                "The source has no lines to study with these options",
            ));
        }
        let response = self.modify(id, |deck| {
            let mut position: HashMap<String, usize> = deck
                .cards
                .iter()
                .enumerate()
                .map(|(i, c)| (c.id.clone(), i))
                .collect();
            let (mut added, mut kept, mut restored) = (0, 0, 0);
            let total = fresh.len() as u32;
            let mut present = BTreeSet::new();
            for mut card in fresh {
                present.insert(card.id.clone());
                match position.get(&card.id) {
                    Some(&i) => {
                        let old = &deck.cards[i];
                        if old.removed {
                            restored += 1;
                        } else {
                            card.suspended = old.suspended;
                        }
                        kept += 1;
                        deck.cards[i] = card;
                    }
                    None => {
                        position.insert(card.id.clone(), deck.cards.len());
                        deck.cards.push(card);
                        added += 1;
                    }
                }
            }
            let mut suspended = 0;
            for card in deck.cards.iter_mut().filter(|c| !present.contains(&c.id)) {
                if !card.removed {
                    card.removed = true;
                    card.suspended = true;
                    suspended += 1;
                }
                // Gone lines follow the source's lines.
                card.order = card.order.max(total);
            }
            deck.source = source;
            deck.source_kind = source_kind;
            Ok(UpdateResponse {
                deck: summary(deck, now, utc_offset),
                skipped,
                added,
                kept,
                suspended,
                restored,
            })
        })?;
        self.forget_lines(id);
        Ok(response)
    }

    pub fn update_from_pgn(
        &self,
        deck_id: &str,
        path: PathBuf,
        now: i64,
        utc_offset: i32,
    ) -> Result<UpdateResponse, AppError> {
        let options = self.view(deck_id, |d| Ok(d.options.clone()))?;
        let mut collector = Collector::new(&options);
        collect_pgn(&path, &mut collector)?;
        self.update(
            deck_id,
            path.to_string_lossy().into_owned(),
            SourceKind::Pgn,
            collector.finish(),
            now,
            utc_offset,
        )
    }

    pub fn update_from_rows(
        &self,
        registry: &Registry,
        deck_id: &str,
        database_id: String,
        ranges: Vec<RowRange>,
        now: i64,
        utc_offset: i32,
    ) -> Result<UpdateResponse, AppError> {
        let options = self.view(deck_id, |d| Ok(d.options.clone()))?;
        let mut collector = Collector::new(&options);
        let source = collect_rows(registry, &database_id, &ranges, &mut collector)?;
        self.update(
            deck_id,
            source,
            SourceKind::List,
            collector.finish(),
            now,
            utc_offset,
        )
    }

    pub fn deck(&self, deck_id: &str) -> Result<DeckDetail, AppError> {
        self.view(deck_id, |deck| {
            Ok(DeckDetail {
                id: deck.id.clone(),
                name: deck.name.clone(),
                source: deck.source.clone(),
                source_kind: deck.source_kind,
                created: deck.created,
                options: deck.options.clone(),
                cards: deck
                    .cards
                    .iter()
                    .map(|c| CardRow {
                        id: c.id.clone(),
                        source_label: c.source_label.clone(),
                        white: c.white.clone(),
                        black: c.black.clone(),
                        event: c.event.clone(),
                        date: c.date.clone(),
                        plies: c.moves.len() as u32,
                        suspended: c.suspended,
                        removed: c.removed,
                        state: deck.states.get(&c.id).cloned().unwrap_or_default(),
                    })
                    .collect(),
            })
        })
    }

    pub fn set_options(&self, deck_id: &str, options: DeckOptions) -> Result<(), AppError> {
        options.check()?;
        self.modify(deck_id, |deck| {
            deck.options = options;
            Ok(())
        })
    }

    pub fn rename(&self, deck_id: &str, name: String) -> Result<(), AppError> {
        let name = clean_name(&name)?;
        self.modify(deck_id, |deck| {
            deck.name = name;
            Ok(())
        })
    }

    /// Deletes the deck's file, also one that cannot be read.
    pub fn delete(&self, deck_id: &str) -> Result<(), AppError> {
        let path = self.path_of(deck_id)?;
        let lock = self.lock_for(deck_id)?;
        let _guard = lock.lock().map_err(err)?;
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(err("No such deck"));
            }
            Err(e) => return Err(err(e)),
        }
        self.forget_lines(deck_id);
        Ok(())
    }

    pub fn suspend(&self, deck_id: &str, card_id: &str, suspended: bool) -> Result<(), AppError> {
        self.modify(deck_id, |deck| {
            let card = deck
                .cards
                .iter_mut()
                .find(|c| c.id == card_id)
                .ok_or_else(|| err("No such card"))?;
            card.suspended = suspended;
            if !suspended {
                card.removed = false;
            }
            Ok(())
        })
    }

    fn line_index(&self, deck: &Deck) -> Arc<LineIndex> {
        if let Ok(lines) = self.lines.lock()
            && let Some((len, index)) = lines.get(&deck.id)
            && *len == deck.cards.len()
        {
            return index.clone();
        }
        let index = Arc::new(build_index(deck));
        if let Ok(mut lines) = self.lines.lock() {
            lines.insert(deck.id.clone(), (deck.cards.len(), index.clone()));
        }
        index
    }

    fn response(&self, deck: &Deck, now: i64, utc_offset: i32) -> QueueResponse {
        let queue = schedule::queue(deck, now, utc_offset);
        let next = queue
            .cards
            .first()
            .and_then(|id| study_card(deck, id, &self.line_index(deck), now));
        QueueResponse {
            queue,
            next,
            stats: schedule::stats(deck, now, utc_offset),
        }
    }

    pub fn queue(
        &self,
        deck_id: &str,
        now: i64,
        utc_offset: i32,
    ) -> Result<QueueResponse, AppError> {
        self.view(deck_id, |deck| Ok(self.response(deck, now, utc_offset)))
    }

    /// Records the answer to a card. Mistakes always make it Again; a hint
    /// makes it Hard at most.
    #[allow(clippy::too_many_arguments)]
    pub fn answer(
        &self,
        deck_id: &str,
        card_id: &str,
        grade: Grade,
        mistakes: u32,
        hints: u32,
        millis: u64,
        now: i64,
        utc_offset: i32,
    ) -> Result<AnswerResponse, AppError> {
        self.modify(deck_id, |deck| {
            if !deck.cards.iter().any(|c| c.id == card_id) {
                return Err(err("No such card"));
            }
            let grade = if mistakes > 0 {
                Grade::Again
            } else if hints > 0 && matches!(grade, Grade::Good | Grade::Easy) {
                Grade::Hard
            } else {
                grade
            };
            let before = deck.states.get(card_id).cloned().unwrap_or_default();
            let state = schedule::answer(&before, grade, now, &deck.options, card_id);
            deck.states.insert(card_id.to_string(), state.clone());
            deck.log.push(ReviewLog {
                card_id: card_id.to_string(),
                time: now,
                grade,
                kind: before.kind,
                mistakes,
                hints,
                millis,
            });
            if deck.log.len() > MAX_LOG {
                let extra = deck.log.len() - MAX_LOG;
                deck.log.drain(..extra);
            }
            Ok(AnswerResponse {
                state,
                session: self.response(deck, now, utc_offset),
            })
        })
    }
}

/// A card with what the study view needs to play it.
fn study_card(deck: &Deck, id: &str, index: &LineIndex, now: i64) -> Option<StudyCard> {
    let card = deck.cards.iter().find(|c| c.id == id)?;
    let state = deck.states.get(id).cloned().unwrap_or_default();
    let mut pos = parse_position(&card.start_fen).ok()?;
    let mut plies = vec![];
    let mut trainee_moves = 0;
    for m in &card.moves {
        let trainee = Colour::of(pos.turn()) == card.trainee;
        trainee_moves += u32::from(trainee);
        let (legal_uci, other_moves) = if trainee {
            (
                pos.legal_moves().iter().map(|&m| uci_of(&pos, m)).collect(),
                index
                    .get(&position_key(&pos))
                    .map(|moves| moves.iter().filter(|u| **u != m.uci).cloned().collect())
                    .unwrap_or_default(),
            )
        } else {
            (vec![], vec![])
        };
        plies.push(PlyInfo {
            fen: Fen::from_position(&pos, EnPassantMode::Always).to_string(),
            legal_uci,
            other_moves,
        });
        pos = apply_uci(pos, &m.uci).ok()?;
    }
    let previews = [Grade::Again, Grade::Hard, Grade::Good, Grade::Easy]
        .into_iter()
        .map(|grade| {
            let after = schedule::answer(&state, grade, now, &deck.options, id);
            Preview {
                grade,
                due_in_ms: after.due - now,
                interval_days: after.interval,
                kind: after.kind,
            }
        })
        .collect();
    Some(StudyCard {
        card: card.clone(),
        learn: state.kind == Kind::New && deck.options.show_new_first,
        mate_in: (pos.is_checkmate() && Colour::of(pos.turn()) != card.trainee)
            .then_some(trainee_moves),
        state,
        plies,
        end_fen: Fen::from_position(&pos, EnPassantMode::Always).to_string(),
        previews,
    })
}
