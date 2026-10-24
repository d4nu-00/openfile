//! Game-data search over a whole database (the reference app's search mask, game data
//! tab). See docs/search.md. Name fields are resolved to id sets first; the
//! entry index is then scanned in chunks without decoding any game.
pub use crate::ending::{CountRange, EndingQuery, PieceCounts, Tally, mating_pieces};
use crate::{CbhError, Database, DatabaseFormat, EntryKind, header::RawEntry};
use std::{collections::HashSet, sync::atomic::AtomicBool};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerQuery {
    /// Prefix of the last name.
    pub last: String,
    /// Prefix of the first name.
    pub first: String,
}

impl PlayerQuery {
    fn is_empty(&self) -> bool {
        self.last.trim().is_empty() && self.first.trim().is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EloMode {
    /// Neither rating needs to be in range (the range is ignored).
    #[default]
    None,
    One,
    Both,
    Average,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameFilter {
    pub white: PlayerQuery,
    pub black: PlayerQuery,
    pub ignore_colors: bool,
    pub wins_only: bool,
    pub tournament: String,
    pub annotator: String,
    pub elo_min: Option<u16>,
    pub elo_max: Option<u16>,
    pub elo_mode: EloMode,
    /// At most this rating difference between the players; both must be
    /// rated.
    pub elo_difference: Option<u16>,
    pub year_min: Option<u16>,
    pub year_max: Option<u16>,
    /// Inclusive ECO range, e.g. "B20".."B99".
    pub eco_from: String,
    pub eco_to: String,
    pub moves_min: Option<u16>,
    pub moves_max: Option<u16>,
    /// Allowed PGN results ("1-0", "0-1", "1/2-1/2", "0-0"); empty means any.
    pub results: Vec<String>,
    pub exclude_deleted: bool,
    /// Quick filter: every word must match a player, the tournament or its
    /// site, the annotator, the source, the ECO code, the year or the result.
    pub text: String,
    /// Games of this player id, with either colour (the players pane).
    pub player_id: Option<u32>,
    /// Games of this tournament id (the tournaments pane).
    pub tournament_id: Option<u32>,
    /// The main line ends in mate, stalemate or check (not mate); when any
    /// is set, the final position must be one of those set.
    pub mate: bool,
    pub stalemate: bool,
    pub check: bool,
    /// The main line's final position has equal pieces (see docs/search.md).
    pub final_material: FinalMaterial,
    /// The Ending tab: piece counts in the final position and the pieces
    /// involved in a mate (see docs/search.md).
    pub ending: EndingQuery,
    /// Case-insensitive text in any text annotation, in any language.
    pub full_text: String,
    /// Games whose annotations do not hold `full_text`.
    pub full_text_not: bool,
    /// The search mask's Medals tab (dialog 312): games with any of these
    /// medal bits.
    pub medals: u16,
    /// The search mask's Annotations tab (dialog 343).
    pub annotations: AnnotationQuery,
    pub material: Option<crate::material::MaterialQuery>,
}

/// Final-position material criterion of the game data tab (the workbench's own
/// design; see docs/search.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FinalMaterial {
    /// Both sides have the same queens, rooks and minor pieces.
    pub equal: bool,
    /// Compare bishops and knights separately instead of as minor pieces.
    pub separate_minors: bool,
    /// Pawn counts must be equal too.
    pub count_pawns: bool,
}

impl FinalMaterial {
    /// Whether White and Black hold the same pieces on `board`.
    pub fn matches(&self, board: &shakmaty::Board) -> bool {
        use shakmaty::{Color, Role};
        let count =
            |color: Color, role: Role| (board.by_color(color) & board.by_role(role)).count();
        let same = |role: Role| count(Color::White, role) == count(Color::Black, role);
        let minors = |color: Color| count(color, Role::Bishop) + count(color, Role::Knight);
        same(Role::Queen)
            && same(Role::Rook)
            && if self.separate_minors {
                same(Role::Bishop) && same(Role::Knight)
            } else {
                minors(Color::White) == minors(Color::Black)
            }
            && (!self.count_pawns || same(Role::Pawn))
    }
}

/// The search mask's Annotations tab (dialog 343, `CAnnoMaskDialog`; see
/// docs/search.md). Every criterion set must hold.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnnotationQuery {
    /// Text1 and Text2: each must be in a text annotation.
    pub text1: String,
    pub text2: String,
    pub whole_word: bool,
    /// Symbols as PGN NAG numbers; each must be in the game.
    pub symbols: Vec<u8>,
    pub any_text: bool,
    pub any_symbols: bool,
    /// Coloured squares or arrows.
    pub colours: bool,
    pub training: bool,
    /// Audio, picture or video.
    pub multimedia: bool,
    pub pawn_structure: bool,
    pub piece_path: bool,
    pub critical_opening: bool,
    pub critical_middlegame: bool,
    pub critical_endgame: bool,
    /// The game has variations.
    pub variations: bool,
}

impl AnnotationQuery {
    /// Criteria that read the annotation record.
    fn reads_annotations(&self) -> bool {
        !self.text1.trim().is_empty()
            || !self.text2.trim().is_empty()
            || !self.symbols.is_empty()
            || self.any_text
            || self.any_symbols
            || self.colours
            || self.training
            || self.multimedia
            || self.pawn_structure
            || self.piece_path
            || self.critical_opening
            || self.critical_middlegame
            || self.critical_endgame
    }
    fn is_empty(&self) -> bool {
        !self.reads_annotations() && !self.variations
    }
    /// Whether annotation entries satisfy the criteria.
    pub fn matches(&self, entries: &[crate::annotations::Entry]) -> bool {
        use crate::annotations::Kind;
        let texts: Vec<String> = entries
            .iter()
            .filter_map(|e| match &e.kind {
                Kind::Text { text, .. } => Some(fold(text)),
                _ => None,
            })
            .collect();
        let has_text = |needle: &str| {
            let needle = fold(needle);
            needle.is_empty()
                || texts.iter().any(|t| {
                    if self.whole_word {
                        whole_word(t, &needle)
                    } else {
                        t.contains(&needle)
                    }
                })
        };
        if !has_text(&self.text1) || !has_text(&self.text2) {
            return false;
        }
        let nags: Vec<u8> = entries
            .iter()
            .filter_map(|e| match &e.kind {
                Kind::Symbols(n) => Some(n.iter().copied().filter(|n| *n != 0)),
                _ => None,
            })
            .flatten()
            .collect();
        if !self.symbols.iter().all(|s| nags.contains(s)) {
            return false;
        }
        let any = |f: &dyn Fn(&Kind) -> bool| entries.iter().any(|e| f(&e.kind));
        let other = |types: &[u16]| any(&|k| matches!(k, Kind::Other(t) if types.contains(t)));
        let critical = |n: u8| any(&|k| matches!(k, Kind::Critical(c) if *c == n));
        (!self.any_text || !texts.is_empty())
            && (!self.any_symbols || !nags.is_empty())
            && (!self.colours
                || any(&|k| matches!(k, Kind::Squares(_) | Kind::Arrows(_)))
                || other(&[0x84, 0x85]))
            && (!self.training || any(&|k| matches!(k, Kind::Training(_))) || other(&[0x09]))
            && (!self.multimedia || other(&[0x10, 0x11, 0x12]))
            && (!self.pawn_structure || any(&|k| matches!(k, Kind::PawnStructure(_))))
            && (!self.piece_path || any(&|k| matches!(k, Kind::PiecePath(_))))
            && (!self.critical_opening || critical(1))
            && (!self.critical_middlegame || critical(2))
            && (!self.critical_endgame || critical(3))
    }
}

/// `needle` in `text` with no letter or digit on either side.
fn whole_word(text: &str, needle: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    text.match_indices(needle).any(|(at, _)| {
        !word(text[..at].chars().next_back()) && !word(text[at + needle.len()..].chars().next())
    })
}

/// How a game's main line ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Mate,
    Stalemate,
    /// Check that is not mate.
    Check,
    Other,
}

/// The position at the end of the main line of `game`.
pub fn final_position(game: &chess_core::Game) -> Result<shakmaty::Chess, CbhError> {
    let mut id = 0;
    while let Some(&next) = game.nodes[id as usize].children.first() {
        id = next;
    }
    Ok(game.chess_position(id)?)
}

/// How the main line of `game` ends.
pub fn ending(game: &chess_core::Game) -> Result<Ending, CbhError> {
    use shakmaty::Position;
    let pos = final_position(game)?;
    Ok(if pos.is_checkmate() {
        Ending::Mate
    } else if pos.is_stalemate() {
        Ending::Stalemate
    } else if pos.is_check() {
        Ending::Check
    } else {
        Ending::Other
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub ordinals: Vec<u64>,
    pub nodes: Option<Vec<u32>>,
    /// Entries examined; equals the entry count unless stopped early.
    pub scanned: u64,
    /// Stopped at the hit limit.
    pub truncated: bool,
    pub cancelled: bool,
}

fn fold(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Lower-case prefix test that ignores empty queries.
fn prefix(value: &str, query: &str) -> bool {
    query.is_empty() || value.to_lowercase().starts_with(query)
}

/// Packed ECO index (1..=500) of "A00".."E99".
fn eco_index(code: &str) -> Option<u16> {
    let b = code.trim().as_bytes();
    if b.len() != 3 || !(b'A'..=b'E').contains(&b[0].to_ascii_uppercase()) {
        return None;
    }
    let n: u16 = std::str::from_utf8(&b[1..]).ok()?.parse().ok()?;
    (n < 100).then(|| (b[0].to_ascii_uppercase() - b'A') as u16 * 100 + n + 1)
}

/// The player sides a name matched: white, black.
struct Sides {
    white: Option<HashSet<u32>>,
    black: Option<HashSet<u32>>,
}

struct Compiled {
    sides: Sides,
    tournaments: Option<HashSet<u32>>,
    annotators: Option<HashSet<u32>>,
    eco: Option<(u16, u16)>,
    words: Vec<QuickTerm>,
}

struct QuickTerm {
    kind: QuickKind,
    negated: bool,
    needle: String,
}

enum QuickKind {
    Any(Word),
    White(HashSet<u32>),
    Black(HashSet<u32>),
    Event(HashSet<u32>),
    Site(HashSet<u32>),
    Result(Vec<u8>),
    Eco(u16, u16),
    Year(u16, u16),
    AverageElo {
        threshold: u16,
        greater: bool,
        inclusive: bool,
    },
}

impl QuickTerm {
    fn positive_result(&self) -> bool {
        !self.negated && matches!(&self.kind, QuickKind::Result(_))
    }

    fn matches(&self, r: &RawEntry) -> bool {
        let has = |set: &HashSet<u32>, id: Option<u32>| id.is_some_and(|id| set.contains(&id));
        let found = match &self.kind {
            QuickKind::Any(word) => word.matches(r),
            QuickKind::White(ids) => has(ids, r.white_id),
            QuickKind::Black(ids) => has(ids, r.black_id),
            QuickKind::Event(ids) | QuickKind::Site(ids) => has(ids, r.tournament_id),
            QuickKind::Result(codes) => r.result_code.is_some_and(|code| codes.contains(&code)),
            QuickKind::Eco(from, to) => {
                let code = r.eco_raw.map_or(0, |e| e >> 7);
                code >= *from && code <= *to
            }
            QuickKind::Year(from, to) => r.year.is_some_and(|y| y >= *from && y <= *to),
            QuickKind::AverageElo {
                threshold,
                greater,
                inclusive,
            } => {
                let (Some(w), Some(b)) = (r.white_elo, r.black_elo) else {
                    return false;
                };
                if w == 0 || b == 0 {
                    return false;
                }
                let sum = u32::from(w) + u32::from(b);
                let bound = u32::from(*threshold) * 2;
                if *greater {
                    sum > bound || (*inclusive && sum == bound)
                } else {
                    sum < bound || (*inclusive && sum == bound)
                }
            }
        };
        found != self.negated
    }
}

/// What one quick-filter word matches.
#[derive(Default)]
struct Word {
    players: HashSet<u32>,
    tournaments: HashSet<u32>,
    annotators: HashSet<u32>,
    sources: HashSet<u32>,
    eco: Option<(u16, u16)>,
    year: Option<u16>,
}

impl Word {
    fn matches(&self, r: &RawEntry) -> bool {
        let has = |set: &HashSet<u32>, id: Option<u32>| id.is_some_and(|id| set.contains(&id));
        has(&self.players, r.white_id)
            || has(&self.players, r.black_id)
            || has(&self.tournaments, r.tournament_id)
            || has(&self.annotators, r.annotator_id)
            || has(&self.sources, r.source_id)
            || self.eco.is_some_and(|(from, to)| {
                let code = r.eco_raw.map_or(0, |e| e >> 7);
                code >= from && code <= to
            })
            || self.year.is_some_and(|y| r.year == Some(y))
    }
}

/// The ECO codes a word names: "B" (B00–B99), "B9" (B90–B99) or "B90".
fn eco_prefix(word: &str) -> Option<(u16, u16)> {
    let b = word.as_bytes();
    if b.is_empty() || b.len() > 3 || !(b'a'..=b'e').contains(&b[0]) {
        return None;
    }
    let digits = std::str::from_utf8(&b[1..]).ok()?;
    if !digits.bytes().all(|d| d.is_ascii_digit()) {
        return None;
    }
    let letter = (b[0] - b'a') as u16 * 100;
    let (lo, hi) = match digits.len() {
        0 => (0, 99),
        1 => {
            let d: u16 = digits.parse().ok()?;
            (d * 10, d * 10 + 9)
        }
        _ => {
            let d: u16 = digits.parse().ok()?;
            (d, d)
        }
    };
    Some((letter + lo + 1, letter + hi + 1))
}

/// Keep quoted values together, including `Event:"World Championship"`.
fn quick_tokens(text: &str) -> Result<Vec<String>, CbhError> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    let mut started = false;
    for ch in text.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut token));
                    started = false;
                }
            }
            c => {
                token.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err(CbhError::InvalidQuery("Unclosed quote".into()));
    }
    if started {
        tokens.push(token);
    }
    if tokens.len() > 16 {
        return Err(CbhError::InvalidQuery("Use at most 16 search terms".into()));
    }
    Ok(tokens)
}

fn quick_term(token: &str) -> Result<QuickTerm, CbhError> {
    let (negated, value) = match token.strip_prefix('!') {
        Some(value) => (true, value),
        None => (false, token),
    };
    if value.is_empty() {
        return Err(CbhError::InvalidQuery("Empty search term".into()));
    }
    let value_lower = value.to_lowercase();
    let mut needle = value_lower.clone();
    let invalid = || CbhError::InvalidQuery(token.to_string());
    let kind = if let Some(codes) = result_codes(&value_lower) {
        QuickKind::Result(codes)
    } else if value.starts_with('>') || value.starts_with('<') {
        let rest = &value[1..];
        let greater = value.starts_with('>');
        let (inclusive, number) = match rest.strip_prefix('=') {
            Some(n) => (true, n),
            None => (false, rest),
        };
        let threshold = number.parse::<u16>().map_err(|_| invalid())?;
        QuickKind::AverageElo {
            threshold,
            greater,
            inclusive,
        }
    } else if let Some((field, name)) = value.split_once(':') {
        if name.is_empty() {
            return Err(invalid());
        }
        needle = name.to_lowercase();
        match field.to_ascii_lowercase().as_str() {
            "white" => QuickKind::White(HashSet::new()),
            "black" => QuickKind::Black(HashSet::new()),
            "event" => QuickKind::Event(HashSet::new()),
            "site" => QuickKind::Site(HashSet::new()),
            _ => return Err(invalid()),
        }
    } else if let Some(name) = value.strip_prefix('-') {
        if name.is_empty() {
            return Err(invalid());
        }
        needle = name.to_lowercase();
        QuickKind::Black(HashSet::new())
    } else if let Some(name) = value.strip_suffix('-') {
        if name.is_empty() {
            return Err(invalid());
        }
        needle = name.to_lowercase();
        QuickKind::White(HashSet::new())
    } else if let Some((from, to)) = value.split_once('-') {
        if let (Some(a), Some(b)) = (eco_index(from), eco_index(to)) {
            if a > b {
                return Err(invalid());
            }
            QuickKind::Eco(a, b)
        } else if from.len() == 4
            && to.len() == 4
            && from.bytes().all(|b| b.is_ascii_digit())
            && to.bytes().all(|b| b.is_ascii_digit())
        {
            let a = from.parse::<u16>().map_err(|_| invalid())?;
            let b = to.parse::<u16>().map_err(|_| invalid())?;
            if a > b {
                return Err(invalid());
            }
            QuickKind::Year(a, b)
        } else if eco_prefix(&from.to_lowercase()).is_some()
            || (from.len() == 4 && from.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(invalid());
        } else {
            QuickKind::Any(Word::from_text(&value_lower))
        }
    } else {
        QuickKind::Any(Word::from_text(&value_lower))
    };
    Ok(QuickTerm {
        kind,
        negated,
        needle,
    })
}

impl Word {
    fn from_text(needle: &str) -> Self {
        Self {
            eco: eco_prefix(needle),
            year: (needle.len() == 4)
                .then(|| needle.parse().ok())
                .flatten()
                .filter(|y| *y > 0),
            ..Self::default()
        }
    }
}

fn result_codes(needle: &str) -> Option<Vec<u8>> {
    match needle {
        "1-0" => Some(vec![2, 6]),
        "0-1" => Some(vec![0, 4]),
        "1/2-1/2" | "½-½" | "1/2" | "½" | "=" => Some(vec![1, 5]),
        _ => None,
    }
}

impl GameFilter {
    fn matches(&self, c: &Compiled, r: &RawEntry) -> bool {
        if r.kind != EntryKind::Game || (self.exclude_deleted && r.deleted) {
            return false;
        }
        let has = |set: &Option<HashSet<u32>>, id: Option<u32>| {
            set.as_ref()
                .is_none_or(|s| id.is_some_and(|id| s.contains(&id)))
        };
        let code = r.result_code.unwrap_or(3);
        let pgn = crate::header::pgn_result(code);
        let (white_won, black_won) = (pgn == Some("1-0"), pgn == Some("0-1"));
        let straight = has(&c.sides.white, r.white_id) && has(&c.sides.black, r.black_id);
        let swapped = self.ignore_colors
            && has(&c.sides.white, r.black_id)
            && has(&c.sides.black, r.white_id);
        if !(straight || swapped) {
            return false;
        }
        if self.wins_only {
            // The first named player won: White, or Black when swapped.
            let won = (straight && white_won) || (swapped && black_won);
            if !won {
                return false;
            }
        }
        if !has(&c.tournaments, r.tournament_id) || !has(&c.annotators, r.annotator_id) {
            return false;
        }
        if self
            .player_id
            .is_some_and(|p| r.white_id != Some(p) && r.black_id != Some(p))
            || self
                .tournament_id
                .is_some_and(|t| r.tournament_id != Some(t))
        {
            return false;
        }
        if self.elo_mode != EloMode::None && (self.elo_min.is_some() || self.elo_max.is_some()) {
            let range = |e: Option<u16>| {
                e.is_some_and(|e| {
                    self.elo_min.is_none_or(|m| e >= m) && self.elo_max.is_none_or(|m| e <= m)
                })
            };
            let ok = match self.elo_mode {
                EloMode::One => range(r.white_elo) || range(r.black_elo),
                EloMode::Both => range(r.white_elo) && range(r.black_elo),
                EloMode::Average => match (r.white_elo, r.black_elo) {
                    (Some(w), Some(b)) => range(Some((w + b) / 2)),
                    _ => false,
                },
                EloMode::None => true,
            };
            if !ok {
                return false;
            }
        }
        if let Some(max) = self.elo_difference {
            match (r.white_elo, r.black_elo) {
                (Some(w), Some(b)) if w > 0 && b > 0 && w.abs_diff(b) <= max => {}
                _ => return false,
            }
        }
        if self.year_min.is_some() || self.year_max.is_some() {
            let Some(y) = r.year else { return false };
            if self.year_min.is_some_and(|m| y < m) || self.year_max.is_some_and(|m| y > m) {
                return false;
            }
        }
        if let Some((from, to)) = c.eco {
            let code = r.eco_raw.map_or(0, |e| e >> 7);
            if code < from || code > to {
                return false;
            }
        }
        let moves = r.move_count as u16;
        if self.moves_min.is_some_and(|m| moves < m) || self.moves_max.is_some_and(|m| moves > m) {
            return false;
        }
        if self.medals != 0 && r.medals & self.medals == 0 {
            return false;
        }
        if !self.results.is_empty() {
            let shown = if code == 7 { Some("0-0") } else { pgn };
            if !shown.is_some_and(|s| self.results.iter().any(|r| r == s)) {
                return false;
            }
        }
        let results = c.words.iter().filter(|term| term.positive_result());
        let has_result = c.words.iter().any(|term| term.positive_result());
        (!has_result || results.into_iter().any(|term| term.matches(r)))
            && c.words
                .iter()
                .filter(|term| !term.positive_result())
                .all(|term| term.matches(r))
    }
}

impl GameFilter {
    fn wants_ending(&self) -> bool {
        self.mate || self.stalemate || self.check
    }
    /// The filter needs more than the entry index.
    fn reads_games(&self) -> bool {
        self.wants_ending()
            || self.final_material.equal
            || self.ending.final_active()
            || self.ending.mate_by_enabled
            || !self.full_text.trim().is_empty()
            || !self.annotations.is_empty()
    }
    /// The parts of the filter that read the game: its final position and
    /// its text annotations. Games that cannot be read do not match.
    fn matches_game(&self, db: &Database, r: &RawEntry) -> Result<bool, CbhError> {
        let needle = fold(&self.full_text);
        if !needle.is_empty() {
            let Some(texts) = db.annotation_texts(r)? else {
                return Ok(false);
            };
            let found = texts.iter().any(|t| t.to_lowercase().contains(&needle));
            if found == self.full_text_not {
                return Ok(false);
            }
        }
        if self.annotations.reads_annotations() {
            let Some(entries) = db.annotation_entries(r)? else {
                return Ok(false);
            };
            if !self.annotations.matches(&entries) {
                return Ok(false);
            }
        }
        // The moves are decoded once for all criteria that need them.
        let game = if self.annotations.variations
            || self.wants_ending()
            || self.final_material.equal
            || self.ending.final_active()
            || self.ending.mate_by_enabled
        {
            match db.moves_of(r)? {
                Some(game) => Some(game),
                None => return Ok(false),
            }
        } else {
            None
        };
        if let Some(game) = &game {
            if self.annotations.variations && !game.nodes.iter().any(|n| n.children.len() > 1) {
                return Ok(false);
            }
            if self.wants_ending() {
                let ok = match ending(game)? {
                    Ending::Mate => self.mate,
                    Ending::Stalemate => self.stalemate,
                    Ending::Check => self.check,
                    Ending::Other => false,
                };
                if !ok {
                    return Ok(false);
                }
            }
            if self.final_material.equal {
                use shakmaty::Position;
                if !self.final_material.matches(final_position(game)?.board()) {
                    return Ok(false);
                }
            }
            if self.ending.final_active() {
                use shakmaty::Position;
                if !self.ending.matches_final(final_position(game)?.board()) {
                    return Ok(false);
                }
            }
            if self.ending.mate_by_enabled {
                match mating_pieces(&final_position(game)?) {
                    Some(tally) if self.ending.mate_by.matches(&tally) => {}
                    _ => return Ok(false),
                }
            }
        }
        Ok(true)
    }
}

impl Database {
    fn players(&self, q: &PlayerQuery) -> Result<Option<HashSet<u32>>, CbhError> {
        if q.is_empty() {
            return Ok(None);
        }
        let (last, first) = (fold(&q.last), fold(&q.first));
        let mut set = HashSet::new();
        self.names_each_player(&mut |id, l, f| {
            if prefix(l, &last) && prefix(f, &first) {
                set.insert(id);
            }
            true
        })?;
        Ok(Some(set))
    }

    /// Compiles quick-filter terms before the entry-index scan.
    fn words(&self, text: &str) -> Result<Vec<QuickTerm>, CbhError> {
        let mut words: Vec<QuickTerm> = quick_tokens(text)?
            .iter()
            .map(|token| quick_term(token))
            .collect::<Result<_, _>>()?;
        if words.is_empty() {
            return Ok(words);
        }
        let contains = |hay: &str, needle: &str| hay.to_lowercase().contains(needle);
        self.names_each_player(&mut |id, last, first| {
            for term in &mut words {
                if contains(last, &term.needle)
                    || contains(first, &term.needle)
                    || (term.needle.contains(',')
                        && contains(&format!("{last}, {first}"), &term.needle))
                {
                    match &mut term.kind {
                        QuickKind::Any(w) => {
                            w.players.insert(id);
                        }
                        QuickKind::White(ids) | QuickKind::Black(ids) => {
                            ids.insert(id);
                        }
                        _ => {}
                    }
                }
            }
            true
        })?;
        self.names_each_tournament(&mut |id, event, site| {
            for term in &mut words {
                match &mut term.kind {
                    QuickKind::Any(w)
                        if contains(event, &term.needle) || contains(site, &term.needle) =>
                    {
                        w.tournaments.insert(id);
                    }
                    QuickKind::Event(ids) if contains(event, &term.needle) => {
                        ids.insert(id);
                    }
                    QuickKind::Site(ids) if contains(site, &term.needle) => {
                        ids.insert(id);
                    }
                    _ => {}
                }
            }
            true
        })?;
        self.names_each_annotator(&mut |id, name| {
            for term in &mut words {
                if let QuickKind::Any(w) = &mut term.kind
                    && contains(name, &term.needle)
                {
                    w.annotators.insert(id);
                }
            }
            true
        })?;
        self.names_each_source(&mut |id, title| {
            for term in &mut words {
                if let QuickKind::Any(w) = &mut term.kind
                    && contains(title, &term.needle)
                {
                    w.sources.insert(id);
                }
            }
            true
        })?;
        Ok(words)
    }

    /// Searches every entry for games matching `filter`. `progress` receives
    /// the number of entries scanned so far; `cancel` stops the scan.
    pub fn search(
        &mut self,
        filter: &GameFilter,
        max_hits: usize,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64),
    ) -> Result<SearchResult, CbhError> {
        self.search_selected(filter, None, max_hits, cancel, |done, _| progress(done))
    }

    /// Searches only the sorted unique entries in `selection`, or every entry with None.
    pub fn search_selected(
        &mut self,
        filter: &GameFilter,
        selection: Option<&[u64]>,
        max_hits: usize,
        cancel: &AtomicBool,
        progress: impl FnMut(u64, u64),
    ) -> Result<SearchResult, CbhError> {
        self.verify()?;
        if let Some(material) = &filter.material {
            material.validate()?;
        }
        let tournaments = if filter.tournament.trim().is_empty() {
            None
        } else {
            let needle = fold(&filter.tournament);
            let mut set = HashSet::new();
            self.names_each_tournament(&mut |id, event, site| {
                if event.to_lowercase().contains(&needle) || site.to_lowercase().contains(&needle) {
                    set.insert(id);
                }
                true
            })?;
            Some(set)
        };
        let annotators = if filter.annotator.trim().is_empty() {
            None
        } else {
            let needle = fold(&filter.annotator);
            let mut set = HashSet::new();
            self.names_each_annotator(&mut |id, name| {
                if name.to_lowercase().contains(&needle) {
                    set.insert(id);
                }
                true
            })?;
            Some(set)
        };
        let eco = match (eco_index(&filter.eco_from), eco_index(&filter.eco_to)) {
            (None, None) => None,
            (a, b) => Some((a.unwrap_or(1), b.unwrap_or(500))),
        };
        let compiled = Compiled {
            sides: Sides {
                white: self.players(&filter.white)?,
                black: self.players(&filter.black)?,
            },
            tournaments,
            annotators,
            eco,
            words: self.words(&filter.text)?,
        };
        // Header fields are compared first; games are read only for what
        // passes them.
        let step = if filter.reads_games() || filter.material.is_some() {
            64
        } else {
            8192
        };
        let found = crate::scan::parallel_selected(
            self,
            selection,
            crate::scan::default_workers(),
            step,
            max_hits,
            cancel,
            progress,
            |_| Ok(()),
            |db, _, start, n, out| {
                for raw in db.raw_chunk(start, n)? {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    if !filter.matches(&compiled, &raw)
                        || (filter.reads_games() && !filter.matches_game(db, &raw)?)
                    {
                        continue;
                    }
                    let node = if let Some(material) = &filter.material {
                        let Some(game) = db.moves_of(&raw)? else {
                            continue;
                        };
                        let Some(node) = material.find_in(&game, cancel)? else {
                            continue;
                        };
                        node
                    } else {
                        0
                    };
                    out.push((raw.ordinal, node));
                }
                Ok(())
            },
        )?;
        let (ordinals, nodes): (Vec<_>, Vec<_>) = found.hits.into_iter().unzip();
        Ok(SearchResult {
            ordinals,
            nodes: filter.material.as_ref().map(|_| nodes),
            scanned: found.scanned,
            truncated: found.truncated,
            cancelled: found.cancelled,
        })
    }
}

impl DatabaseFormat {
    pub(crate) fn record_size(self) -> usize {
        match self {
            Self::Classic => 46,
            Self::New => crate::v2::record::RECORD,
        }
    }
}

#[cfg(test)]
mod material_tests {
    use super::FinalMaterial;
    use shakmaty::Position;

    fn equal(fen: &str, m: FinalMaterial) -> bool {
        let pos = chess_core::model::parse_position(fen).unwrap();
        m.matches(pos.board())
    }
    const ON: FinalMaterial = FinalMaterial {
        equal: true,
        separate_minors: false,
        count_pawns: false,
    };

    #[test]
    fn bare_kings_are_equal() {
        assert!(equal("8/8/4k3/8/8/4K3/8/8 w - - 0 1", ON));
    }

    #[test]
    fn bishop_against_knight_is_equal_unless_minors_are_separate() {
        let fen = "8/8/4k3/3b4/8/4K3/3N4/8 w - - 0 1";
        assert!(equal(fen, ON));
        let separate = FinalMaterial {
            separate_minors: true,
            ..ON
        };
        assert!(!equal(fen, separate));
        // The same piece on both sides stays equal.
        assert!(equal("8/8/4k3/3n4/8/4K3/3N4/8 w - - 0 1", separate));
    }

    #[test]
    fn two_bishops_against_two_knights() {
        let fen = "8/8/4k3/2nn4/8/4K3/2BB4/8 w - - 0 1";
        assert!(equal(fen, ON));
        let separate = FinalMaterial {
            separate_minors: true,
            ..ON
        };
        assert!(!equal(fen, separate));
        // Bishop and knight against two knights: two minors a side when
        // pooled, but not when separate.
        let mixed = "8/8/4k3/2nn4/8/4K3/2BN4/8 w - - 0 1";
        assert!(equal(mixed, ON));
        assert!(!equal(mixed, separate));
    }

    #[test]
    fn queen_against_two_rooks_is_unequal() {
        assert!(!equal("8/8/4k3/3rr3/8/4K3/3Q4/8 w - - 0 1", ON));
    }

    #[test]
    fn rook_against_bishop_is_unequal() {
        assert!(!equal("8/8/4k3/3b4/8/4K3/3R4/8 w - - 0 1", ON));
    }

    #[test]
    fn pawns_count_only_when_asked() {
        let fen = "8/8/4k3/8/8/4K3/3P4/8 w - - 0 1";
        assert!(equal(fen, ON));
        let pawns = FinalMaterial {
            count_pawns: true,
            ..ON
        };
        assert!(!equal(fen, pawns));
        assert!(equal("8/3p4/4k3/8/8/4K3/3P4/8 w - - 0 1", pawns));
    }
}
