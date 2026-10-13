//! Position search with an optional per-database index (the workbench's
//! "search booster"; see docs/search.md).
//!
//! The index holds one fixed-size Bloom signature per entry. It records the
//! game's *structure keys*: a hash of the pawn placement, the material of
//! both sides and the side to move. A position can only occur in a game whose
//! signature holds its key. Candidates are then replayed to confirm the exact
//! board and side to move. Replay of a line stops once either side has fewer
//! pawns or pieces than the target, since neither can grow back (promotion
//! only turns a pawn into a piece).
use crate::{CbhError, Database};
use chess_core::{Game, NodeId};
use shakmaty::{Board, Chess, Color, Position, Role};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

/// Bytes of Bloom signature per entry.
pub const SIGNATURE: usize = 128;
const BITS: u64 = (SIGNATURE * 8) as u64;
const MAGIC: &[u8; 8] = b"CWPIDX01";
const HEADER: u64 = 8 + 8 + 4 * 8;

fn mix(mut x: u64) -> u64 {
    // splitmix64 finaliser.
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Pawn placement, material and side to move, hashed.
pub fn structure_key(board: &Board, turn: Color) -> u64 {
    let white = (board.pawns() & board.white()).0;
    let black = (board.pawns() & board.black()).0;
    let mut h = mix(white) ^ mix(black.rotate_left(17) ^ 0x5bd1_e995_c6a4_a793);
    for (i, role) in [Role::Knight, Role::Bishop, Role::Rook, Role::Queen]
        .into_iter()
        .enumerate()
    {
        for (j, color) in [Color::White, Color::Black].into_iter().enumerate() {
            let count = (board.by_role(role) & board.by_color(color)).count() as u64;
            h = mix(h ^ (count << (8 * (2 * i + j))));
        }
    }
    mix(h ^ u64::from(turn.is_white()))
}

fn bloom_bits(key: u64) -> [u64; 3] {
    let k2 = mix(key ^ 0x9e37_79b9_7f4a_7c15);
    [key % BITS, (key >> 21) % BITS, k2 % BITS]
}

fn bloom_insert(sig: &mut [u8; SIGNATURE], key: u64) {
    for bit in bloom_bits(key) {
        sig[(bit / 8) as usize] |= 1 << (bit % 8);
    }
}

fn bloom_has(sig: &[u8], key: u64) -> bool {
    bloom_bits(key)
        .iter()
        .all(|&bit| sig[(bit / 8) as usize] & (1 << (bit % 8)) != 0)
}

/// Calls `visit` with every node of the game and its position, depth first.
/// `visit` returns false to skip the node's continuations.
fn walk(game: &Game, mut visit: impl FnMut(NodeId, &Chess) -> bool) -> Result<(), CbhError> {
    let mut stack: Vec<(NodeId, Chess)> = vec![(0, game.chess_position(0)?)];
    while let Some((id, pos)) = stack.pop() {
        if !visit(id, &pos) {
            continue;
        }
        for &child in game.nodes[id as usize].children.iter().rev() {
            let uci = game.nodes[child as usize].uci.as_deref().unwrap_or("");
            if let Ok(next) = chess_core::model::apply_uci(pos.clone(), uci) {
                stack.push((child, next));
            }
        }
    }
    Ok(())
}

/// The signature of one game: every structure key it passes through.
pub fn signature(game: &Game) -> Result<[u8; SIGNATURE], CbhError> {
    let mut sig = [0u8; SIGNATURE];
    walk(game, |_, pos| {
        bloom_insert(&mut sig, structure_key(pos.board(), pos.turn()));
        true
    })?;
    Ok(sig)
}

/// A position to find: board and side to move.
#[derive(Clone, Debug)]
pub struct Target {
    board: Board,
    turn: Color,
    key: u64,
    pawns: [u32; 2],
    pieces: [u32; 2],
}

fn counts(board: &Board) -> ([u32; 2], [u32; 2]) {
    let side = |c: Color| {
        let all = board.by_color(c);
        let pawns = (all & board.pawns()).count() as u32;
        (pawns, all.count() as u32 - pawns)
    };
    let (wp, wn) = side(Color::White);
    let (bp, bn) = side(Color::Black);
    ([wp, bp], [wn, bn])
}

impl Target {
    pub fn new(pos: &Chess) -> Self {
        let (pawns, pieces) = counts(pos.board());
        Self {
            board: pos.board().clone(),
            turn: pos.turn(),
            key: structure_key(pos.board(), pos.turn()),
            pawns,
            pieces,
        }
    }
    pub fn from_fen(fen: &str) -> Result<Self, CbhError> {
        Ok(Self::new(&chess_core::model::parse_position(fen)?))
    }
    /// The first node of `game` (depth first) whose position is the target.
    pub fn find_in(&self, game: &Game) -> Result<Option<NodeId>, CbhError> {
        let mut found = None;
        walk(game, |id, pos| {
            if found.is_some() {
                return false;
            }
            let board = pos.board();
            if pos.turn() == self.turn && board == &self.board {
                found = Some(id);
                return false;
            }
            // Pawns and pieces never come back; stop when below the target.
            let (pawns, pieces) = counts(board);
            (0..2).all(|i| {
                pawns[i] >= self.pawns[i] && pieces[i] + pawns[i] >= self.pieces[i] + self.pawns[i]
            })
        })?;
        Ok(found)
    }
}

/// An index file opened for searching.
pub struct PositionIndex {
    file: std::fs::File,
    entries: u64,
}

impl PositionIndex {
    /// Opens `path` if it was built from `db` as it is now.
    pub fn open(path: &Path, db: &Database) -> Result<Option<Self>, CbhError> {
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut head = [0u8; HEADER as usize];
        if file.read_exact(&mut head).is_err() || &head[..8] != MAGIC {
            return Ok(None);
        }
        let entries = u64::from_le_bytes(head[8..16].try_into().unwrap());
        let stamp: Vec<u64> = head[16..]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| u64::from_le_bytes(*c))
            .collect();
        if entries != db.info().entry_count
            || stamp != db.fingerprint()
            || file.metadata()?.len() != HEADER + entries * SIGNATURE as u64
        {
            return Ok(None);
        }
        Ok(Some(Self { file, entries }))
    }

    /// Builds the index for `db` at `path`, replacing any older one. Writes
    /// to a temporary file first, so a cancelled build leaves nothing behind.
    pub fn build(
        db: &Database,
        path: &Path,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<bool, CbhError> {
        let count = db.info().entry_count;
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let mut temp = tempfile::NamedTempFile::new_in(dir)?;
        let out = temp.as_file_mut();
        out.write_all(MAGIC)?;
        out.write_all(&count.to_le_bytes())?;
        for v in db.fingerprint() {
            out.write_all(&v.to_le_bytes())?;
        }
        let mut writer = std::io::BufWriter::new(out);
        const CHUNK: u64 = 1024;
        let mut start = 1;
        while start <= count {
            if cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let n = CHUNK.min(count + 1 - start);
            for raw in db.raw_chunk(start, n)? {
                let sig = match db.moves_of(&raw)? {
                    Some(game) => signature(&game)?,
                    None => [0; SIGNATURE],
                };
                writer.write_all(&sig)?;
            }
            start += n;
            progress(start - 1, count);
        }
        writer.flush()?;
        drop(writer);
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| CbhError::Io(e.error))?;
        Ok(true)
    }

    /// Entries whose signature allows the target, from `start` on, at most
    /// `limit` of them; also returns where the next call should start.
    fn candidates(
        &mut self,
        target: &Target,
        start: u64,
        limit: usize,
    ) -> Result<(Vec<u64>, u64), CbhError> {
        const CHUNK: u64 = 8192;
        let mut out = vec![];
        let mut at = start;
        while at <= self.entries && out.len() < limit {
            let n = CHUNK.min(self.entries + 1 - at);
            let mut buf = vec![0u8; n as usize * SIGNATURE];
            self.file
                .seek(SeekFrom::Start(HEADER + (at - 1) * SIGNATURE as u64))?;
            self.file.read_exact(&mut buf)?;
            for (i, sig) in buf.as_chunks::<SIGNATURE>().0.iter().enumerate() {
                if bloom_has(sig, target.key) {
                    out.push(at + i as u64);
                }
            }
            at += n;
        }
        Ok((out, at))
    }
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionResult {
    /// Matching entries and the first node showing the position.
    pub hits: Vec<(u64, NodeId)>,
    /// Games replayed to confirm.
    pub replayed: u64,
    /// Entries examined.
    pub scanned: u64,
    pub truncated: bool,
    pub cancelled: bool,
    pub indexed: bool,
}

impl Database {
    /// The first node of entry `ordinal` (depth first) showing `target`;
    /// `None` for text entries and games that never reach it.
    pub fn find_node(&self, ordinal: u64, target: &Target) -> Result<Option<NodeId>, CbhError> {
        let raw = self.raw_chunk(ordinal, 1)?.remove(0);
        match self.moves_of(&raw)? {
            Some(game) => target.find_in(&game),
            None => Ok(None),
        }
    }

    /// Finds games that reach `target`, using `index` when given and
    /// replaying every game otherwise.
    pub fn find_position(
        &mut self,
        target: &Target,
        mut index: Option<&mut PositionIndex>,
        max_hits: usize,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<PositionResult, CbhError> {
        self.verify()?;
        let count = self.info().entry_count;
        let mut result = PositionResult {
            indexed: index.is_some(),
            ..Default::default()
        };
        let mut start = 1;
        while start <= count {
            if cancel.load(Ordering::Relaxed) {
                result.cancelled = true;
                break;
            }
            let (batch, next) = match index.as_deref_mut() {
                Some(index) => index.candidates(target, start, 1024)?,
                None => {
                    let n = 1024.min(count + 1 - start);
                    ((start..start + n).collect(), start + n)
                }
            };
            for ordinal in batch {
                let raw = self.raw_chunk(ordinal, 1)?.remove(0);
                let Some(game) = self.moves_of(&raw)? else {
                    continue;
                };
                result.replayed += 1;
                if let Some(node) = target.find_in(&game)? {
                    if result.hits.len() == max_hits {
                        result.truncated = true;
                        result.scanned = ordinal;
                        return Ok(result);
                    }
                    result.hits.push((ordinal, node));
                }
            }
            result.scanned = next - 1;
            progress(result.scanned, count);
            start = next;
        }
        Ok(result)
    }
}
