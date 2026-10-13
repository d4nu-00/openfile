//! Building a tree file: decode every main line once, then sort positions
//! one hash range at a time (see docs/opening-tree.md).
use super::*;
use crate::EntryKind;
use std::{
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    sync::atomic::AtomicU64,
};

/// How a tree is built.
#[derive(Clone, Debug)]
pub struct BuildOptions {
    /// Positions after at most this many half-moves are stored.
    pub plies: usize,
    /// Positions reached by fewer games are not stored; they are answered
    /// by replaying the few games that reach them.
    pub min_games: usize,
    /// Threads decoding games.
    pub workers: usize,
    /// Bytes of position records sorted at a time; larger databases take
    /// more passes over the decoded lines.
    pub memory: usize,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            plies: DEFAULT_PLIES,
            min_games: DEFAULT_MIN_GAMES,
            workers: std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .clamp(1, 8),
            memory: 512 << 20,
        }
    }
}

/// Half-moves covered by default: the first 30 moves of each side.
pub const DEFAULT_PLIES: usize = 60;
/// Games a position needs to be stored, by default.
pub const DEFAULT_MIN_GAMES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildStage {
    /// Decoding games; counts entries.
    Reading,
    /// Sorting positions; counts passes.
    Sorting,
}

/// One position record while building.
#[derive(Clone, Copy)]
struct Record {
    hash: u64,
    ordinal: u32,
    code: u16,
    black: bool,
}

/// A decoded chunk of games: line records and header statistics.
struct Chunk {
    start: u64,
    lines: Vec<u8>,
    stats: Vec<GameStats>,
    positions: u64,
}

fn encode_chunk(db: &Database, start: u64, n: u64, plies: usize) -> Result<Chunk, CbhError> {
    let mut chunk = Chunk {
        start,
        lines: vec![],
        stats: Vec::with_capacity(n as usize),
        positions: 0,
    };
    let standard = Chess::default();
    for raw in db.raw_chunk(start, n)? {
        chunk.stats.push(GameStats::of(&raw));
        if raw.kind != EntryKind::Game {
            continue;
        }
        let Some((first, moves)) = db
            .moves_of(&raw)?
            .and_then(|game| line_moves(&game, plies + 1))
        else {
            continue;
        };
        let ordinal = u32::try_from(raw.ordinal).map_err(|_| CbhError::Bounds)?;
        chunk.lines.extend_from_slice(&ordinal.to_le_bytes());
        if first.board() == standard.board() && first.turn() == Color::White {
            chunk.lines.push(0);
        } else {
            chunk.lines.push(1);
            let replay = Replay::new(first.board(), first.turn());
            chunk.lines.extend_from_slice(&replay.squares);
            chunk.lines.push(u8::from(replay.black));
        }
        chunk.lines.push(moves.len() as u8);
        for m in &moves {
            chunk.lines.extend_from_slice(&m.to_le_bytes());
        }
        chunk.positions += (moves.len() + 1).min(plies + 1) as u64;
    }
    Ok(chunk)
}

/// Reads the next line record: ordinal, start replay and moves.
fn read_line(
    input: &mut impl Read,
    moves: &mut Vec<u16>,
) -> Result<Option<(u32, Replay)>, CbhError> {
    let mut head = [0u8; 5];
    match input.read_exact(&mut head) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let ordinal = u32::from_le_bytes(head[..4].try_into().unwrap());
    let replay = if head[4] == 0 {
        let start = Chess::default();
        Replay::new(start.board(), Color::White)
    } else {
        let mut setup = [0u8; 65];
        input.read_exact(&mut setup)?;
        let mut replay = Replay {
            squares: setup[..64].try_into().unwrap(),
            black: false,
            hash: 0,
        };
        for (square, &piece) in replay.squares.iter().enumerate() {
            if piece != 0 {
                replay.hash ^= KEYS[(piece as usize - 1) * 64 + square];
            }
        }
        if setup[64] != 0 {
            replay.black = true;
            replay.hash ^= BLACK_KEY;
        }
        replay
    };
    let mut count = [0u8; 1];
    input.read_exact(&mut count)?;
    let mut bytes = vec![0u8; count[0] as usize * 2];
    input.read_exact(&mut bytes)?;
    moves.clear();
    moves.extend(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b)),
    );
    Ok(Some((ordinal, replay)))
}

impl OpeningTree {
    /// Builds the tree for `db` at `path`, replacing any older one. Writes to
    /// temporary files beside it first, so a cancelled build leaves nothing
    /// behind. Returns false when cancelled.
    pub fn build(
        db: &Database,
        path: &Path,
        options: &BuildOptions,
        cancel: &AtomicBool,
        mut progress: impl FnMut(BuildStage, u64, u64),
    ) -> Result<bool, CbhError> {
        db.verify()?;
        let count = db.info().entry_count;
        if count > u64::from(u32::MAX) {
            return Err(CbhError::LimitExceeded);
        }
        let plies = options.plies.clamp(1, 250);
        let min_games = options.min_games.max(2);
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;

        // Pass 1: decode every game's main line once, in parallel.
        let mut lines = BufWriter::new(tempfile::tempfile_in(dir)?);
        let mut stats = vec![GameStats::default(); count as usize];
        let mut total_positions = 0u64;
        let mut games = 0u64;
        const CHUNK: u64 = 1024;
        let next = AtomicU64::new(1);
        let stop = AtomicBool::new(false);
        let handles = (0..options.workers.max(1))
            .map(|_| db.reopen())
            .collect::<Result<Vec<_>, _>>()?;
        let decoded = std::thread::scope(|scope| -> Result<bool, CbhError> {
            let (tx, rx) =
                std::sync::mpsc::sync_channel::<Result<Chunk, CbhError>>(2 * handles.len());
            for handle in &handles {
                let (tx, next, stop) = (tx.clone(), &next, &stop);
                scope.spawn(move || {
                    while !stop.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) {
                        let start = next.fetch_add(CHUNK, Ordering::Relaxed);
                        if start > count {
                            break;
                        }
                        let n = CHUNK.min(count + 1 - start);
                        let chunk = encode_chunk(handle, start, n, plies);
                        let failed = chunk.is_err();
                        if tx.send(chunk).is_err() || failed {
                            break;
                        }
                    }
                });
            }
            drop(tx);
            let mut done = 0;
            let result = (|| {
                for chunk in rx.iter() {
                    let chunk = chunk?;
                    lines.write_all(&chunk.lines)?;
                    let at = chunk.start as usize - 1;
                    stats[at..at + chunk.stats.len()].copy_from_slice(&chunk.stats);
                    total_positions += chunk.positions;
                    done += chunk.stats.len() as u64;
                    progress(BuildStage::Reading, done, count);
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(false);
                    }
                }
                Ok(done == count)
            })();
            stop.store(true, Ordering::Relaxed);
            // Unblock workers waiting to send.
            drop(rx);
            result
        })?;
        if !decoded || cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let mut lines = lines
            .into_inner()
            .map_err(|e| CbhError::Io(e.into_error()))?;

        // Pass 2: sort position records, a range of hashes at a time, and
        // write the positions that at least two games reach.
        let passes = (total_positions * std::mem::size_of::<Record>() as u64)
            .div_ceil(options.memory.max(1 << 20) as u64)
            .max(1);
        let mut out = tempfile::NamedTempFile::new_in(dir)?;
        let mut dir_file = BufWriter::new(tempfile::tempfile_in(dir)?);
        let mut summary: Vec<u64> = vec![];
        let mut positions = 0u64;
        {
            let mut writer = BufWriter::with_capacity(1 << 20, out.as_file_mut());
            writer.write_all(&[0u8; HEADER as usize])?;
            let mut offset = HEADER;
            let mut records: Vec<Record> = vec![];
            let mut moves = vec![];
            let mut entry = Vec::new();
            let mut head = Vec::new();
            let mut postings = Vec::new();
            for pass in 0..passes {
                records.clear();
                lines.seek(SeekFrom::Start(0))?;
                let mut input = BufReader::with_capacity(1 << 20, &mut lines);
                while let Some((ordinal, replay)) = read_line(&mut input, &mut moves)? {
                    if pass == 0 {
                        games += 1;
                    }
                    each_position(replay, &moves, plies, |hash, black, code| {
                        if ((u128::from(hash) * u128::from(passes)) >> 64) as u64 == pass {
                            records.push(Record {
                                hash,
                                ordinal,
                                code,
                                black,
                            });
                        }
                        true
                    });
                }
                if cancel.load(Ordering::Relaxed) {
                    return Ok(false);
                }
                records.sort_unstable_by_key(|r| (r.hash, r.code, r.ordinal));
                let mut i = 0;
                while i < records.len() {
                    let hash = records[i].hash;
                    let mut j = i;
                    while j < records.len() && records[j].hash == hash {
                        j += 1;
                    }
                    if j - i >= min_games {
                        write_entry(&records[i..j], &stats, &mut entry, &mut head, &mut postings);
                        if positions.is_multiple_of(STRIDE) {
                            summary.push(hash);
                        }
                        dir_file.write_all(&hash.to_le_bytes())?;
                        dir_file.write_all(&offset.to_le_bytes())?;
                        writer.write_all(&entry)?;
                        offset += entry.len() as u64;
                        positions += 1;
                    }
                    i = j;
                }
                progress(BuildStage::Sorting, pass + 1, passes);
            }
            drop(records);
            let dir_offset = offset;
            let mut dir_file = dir_file
                .into_inner()
                .map_err(|e| CbhError::Io(e.into_error()))?;
            dir_file.seek(SeekFrom::Start(0))?;
            std::io::copy(&mut dir_file, &mut writer)?;
            let summary_offset = dir_offset + positions * DIR_ENTRY;
            for s in &summary {
                writer.write_all(&s.to_le_bytes())?;
            }
            let length = summary_offset + summary.len() as u64 * 8;
            writer.flush()?;
            drop(writer);
            let mut header = Vec::with_capacity(HEADER as usize);
            header.extend_from_slice(MAGIC);
            header.extend_from_slice(&1u32.to_le_bytes());
            header.extend_from_slice(&(plies as u32).to_le_bytes());
            header.extend_from_slice(&count.to_le_bytes());
            for v in db.fingerprint() {
                header.extend_from_slice(&v.to_le_bytes());
            }
            for v in [games, positions, dir_offset, summary_offset, length] {
                header.extend_from_slice(&v.to_le_bytes());
            }
            header.extend_from_slice(&(min_games as u32).to_le_bytes());
            header.resize(HEADER as usize, 0);
            let file = out.as_file_mut();
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&header)?;
        }
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        out.as_file().sync_all()?;
        out.persist(path).map_err(|e| CbhError::Io(e.error))?;
        // A journal belongs to the tree it was written for.
        match std::fs::remove_file(journal::journal_path(path)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(true),
        }
    }
}

/// Encodes one stored position. `records` share a hash and are sorted by
/// move, then entry.
fn write_entry(
    records: &[Record],
    stats: &[GameStats],
    entry: &mut Vec<u8>,
    head: &mut Vec<u8>,
    postings: &mut Vec<u8>,
) {
    let black = records[0].black;
    // (statistics, posting bytes range)
    let mut moves: Vec<(MoveStats, std::ops::Range<usize>)> = vec![];
    postings.clear();
    let mut i = 0;
    while i < records.len() {
        let code = records[i].code;
        let mut s = MoveStats::new(code);
        let begin = postings.len();
        let mut previous = 0u64;
        while i < records.len() && records[i].code == code {
            let ordinal = u64::from(records[i].ordinal);
            s.add(&stats[ordinal as usize - 1], black);
            put_varint(postings, ordinal - previous);
            previous = ordinal;
            i += 1;
        }
        moves.push((s, begin..postings.len()));
    }
    moves.sort_by(|a, b| b.0.games.cmp(&a.0.games).then(a.0.code.cmp(&b.0.code)));
    head.clear();
    put_varint(head, u64::from(black));
    put_varint(head, moves.len() as u64);
    for (s, range) in &moves {
        put_varint(head, u64::from(s.code));
        put_varint(head, s.games);
        if s.games == 1 {
            // A single game is stored by its entry number; its statistics
            // come from its header.
            let mut at = range.start;
            put_varint(head, get_varint(postings, &mut at).unwrap_or(0));
            continue;
        }
        for v in [
            s.wins,
            s.draws,
            s.losses,
            s.elo_sum,
            s.elo_count,
            s.opponent_sum,
            s.opponent_count,
            s.points2,
            u64::from(s.year_min),
            u64::from(s.year_max),
            range.len() as u64,
        ] {
            put_varint(head, v);
        }
    }
    entry.clear();
    put_varint(entry, head.len() as u64);
    entry.extend_from_slice(head);
    for (s, range) in &moves {
        if s.games > 1 {
            entry.extend_from_slice(&postings[range.clone()]);
        }
    }
}
