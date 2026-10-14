//! Changes the workbench saved into a database after its tree was built.
//! The journal beside the tree keeps each changed entry's main line as the
//! tree counted it and as it is now, so answers stay current without a
//! rebuild.
use super::*;
use std::{collections::BTreeMap, io::Write, path::PathBuf};

const JOURNAL_MAGIC: &[u8; 8] = b"CWTJRN01";
/// Changed entries kept at most; beyond this the tree is rebuilt instead.
pub const MAX_CHANGES: usize = 10_000;
const MAX_LINE: usize = 1024;

/// A database's entry count and file stamps.
pub(super) type Stamp = (u64, [u64; 4]);

pub(super) fn stamp(db: &Database) -> Stamp {
    (db.info().entry_count, db.fingerprint())
}

/// A game's main line as the tree counts it: header statistics and, per
/// position, its hash, whether Black is to move and the move played.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameLine {
    pub stats: GameStats,
    pub positions: Vec<(u64, bool, u16)>,
}

impl GameLine {
    /// The side to move and the move played where the line reaches `hash`.
    pub(super) fn at(&self, hash: u64) -> Option<(bool, u16)> {
        self.positions
            .iter()
            .find(|p| p.0 == hash)
            .map(|&(_, black, code)| (black, code))
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Journal {
    /// The database as the tree file saw it.
    pub(super) base: Stamp,
    /// The database after the last recorded change.
    pub(super) current: Stamp,
    /// Per changed entry: its line in the tree file and its line now.
    pub(super) changes: BTreeMap<u64, (Option<GameLine>, Option<GameLine>)>,
}

pub(super) fn journal_path(tree: &Path) -> PathBuf {
    tree.with_extension("cwtj")
}

fn put_stamp(out: &mut Vec<u8>, (count, fingerprint): Stamp) {
    out.extend_from_slice(&count.to_le_bytes());
    for v in fingerprint {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn put_line(out: &mut Vec<u8>, line: &GameLine) {
    out.push(line.stats.result);
    for v in [line.stats.white_elo, line.stats.black_elo, line.stats.year] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&(line.positions.len() as u16).to_le_bytes());
    for &(hash, black, code) in &line.positions {
        out.extend_from_slice(&hash.to_le_bytes());
        out.push(u8::from(black));
        out.extend_from_slice(&code.to_le_bytes());
    }
}

/// A bounded reader over the journal bytes.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], CbhError> {
        let end = self.at.checked_add(n).ok_or(CbhError::Truncated)?;
        let slice = self.bytes.get(self.at..end).ok_or(CbhError::Truncated)?;
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, CbhError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, CbhError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, CbhError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn stamp(&mut self) -> Result<Stamp, CbhError> {
        let count = self.u64()?;
        let mut fingerprint = [0u64; 4];
        for v in &mut fingerprint {
            *v = self.u64()?;
        }
        Ok((count, fingerprint))
    }
    fn line(&mut self) -> Result<GameLine, CbhError> {
        let stats = GameStats {
            result: self.u8()?,
            white_elo: self.u16()?,
            black_elo: self.u16()?,
            year: self.u16()?,
        };
        let n = self.u16()? as usize;
        if n > MAX_LINE {
            return Err(CbhError::LimitExceeded);
        }
        let mut positions = Vec::with_capacity(n);
        for _ in 0..n {
            positions.push((self.u64()?, self.u8()? != 0, self.u16()?));
        }
        Ok(GameLine { stats, positions })
    }
}

impl Journal {
    pub(super) fn load(path: &Path) -> Result<Option<Self>, CbhError> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if bytes.len() < 8 || &bytes[..8] != JOURNAL_MAGIC {
            return Ok(None);
        }
        let mut c = Cursor {
            bytes: &bytes,
            at: 8,
        };
        let parsed = (|| -> Result<Self, CbhError> {
            let base = c.stamp()?;
            let current = c.stamp()?;
            let count = c.u64()? as usize;
            if count > MAX_CHANGES {
                return Err(CbhError::LimitExceeded);
            }
            let mut changes = BTreeMap::new();
            for _ in 0..count {
                let ordinal = c.u64()?;
                let flags = c.u8()?;
                let old = if flags & 1 != 0 {
                    Some(c.line()?)
                } else {
                    None
                };
                let new = if flags & 2 != 0 {
                    Some(c.line()?)
                } else {
                    None
                };
                changes.insert(ordinal, (old, new));
            }
            Ok(Self {
                base,
                current,
                changes,
            })
        })();
        // A damaged journal only means the tree is stale.
        Ok(parsed.ok())
    }

    fn save(&self, path: &Path) -> Result<(), CbhError> {
        let mut out = JOURNAL_MAGIC.to_vec();
        put_stamp(&mut out, self.base);
        put_stamp(&mut out, self.current);
        out.extend_from_slice(&(self.changes.len() as u64).to_le_bytes());
        for (ordinal, (old, new)) in &self.changes {
            out.extend_from_slice(&ordinal.to_le_bytes());
            out.push(u8::from(old.is_some()) | u8::from(new.is_some()) << 1);
            for line in [old, new].into_iter().flatten() {
                put_line(&mut out, line);
            }
        }
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(dir)?;
        temp.write_all(&out)?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| CbhError::Io(e.error))?;
        Ok(())
    }

    /// Changed entries, which the tree file's own data no longer describes.
    pub(super) fn changed(&self, ordinal: u64) -> bool {
        self.changes.contains_key(&ordinal)
    }

    /// Takes the changed entries' old lines out of `moves` (the tree file's
    /// statistics for the position `hash`) and adds their current lines, and
    /// their games to `games`.
    pub(super) fn apply(
        &self,
        hash: u64,
        moves: &mut Vec<MoveStats>,
        games: &mut Vec<(u64, u16)>,
        counted: impl Fn(u64) -> bool,
    ) {
        for (&ordinal, (old, new)) in &self.changes {
            if let Some((black, code)) = old.as_ref().and_then(|l| l.at(hash))
                && counted(ordinal)
                && let Some(s) = moves.iter_mut().find(|s| s.code == code)
            {
                s.remove(&old.as_ref().unwrap().stats, black);
            }
            if let Some(line) = new
                && let Some((black, code)) = line.at(hash)
            {
                add_game(moves, code, &line.stats, black);
                games.push((ordinal, code));
            }
        }
        moves.retain(|s| s.games > 0);
        moves.sort_by(|a, b| b.games.cmp(&a.games).then(a.code.cmp(&b.code)));
        games.sort_unstable();
    }
}

/// The tree file's database stamp and plies.
fn tree_stamp(path: &Path) -> Result<Option<(Stamp, usize)>, CbhError> {
    let mut head = [0u8; HEADER as usize];
    match std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    if &head[..8] != MAGIC {
        return Ok(None);
    }
    let u64_at = |i: usize| u64::from_le_bytes(head[i..i + 8].try_into().unwrap());
    let fingerprint = [u64_at(24), u64_at(32), u64_at(40), u64_at(48)];
    let plies = u32::from_le_bytes(head[12..16].try_into().unwrap()) as usize;
    Ok(Some(((u64_at(16), fingerprint), plies)))
}

/// A change the workbench is about to make to a database with a tree.
#[derive(Debug)]
pub struct PendingChange {
    path: PathBuf,
    before: Stamp,
    plies: usize,
    /// The entry being replaced and its main line before the change.
    replaced: Option<(u64, Option<GameLine>)>,
}

impl OpeningTree {
    /// Call before the workbench writes to `db`, with `replaced` the entry a
    /// replace overwrites. `None` when the tree at `path` is not current.
    pub fn before_change(
        path: &Path,
        db: &Database,
        replaced: Option<u64>,
    ) -> Result<Option<PendingChange>, CbhError> {
        let Some(tree) = Self::open(path, db)? else {
            return Ok(None);
        };
        let replaced = match replaced {
            Some(ordinal) => Some((ordinal, db.main_line(ordinal, tree.plies)?)),
            None => None,
        };
        Ok(Some(PendingChange {
            path: path.to_path_buf(),
            before: stamp(db),
            plies: tree.plies,
            replaced,
        }))
    }
}

impl PendingChange {
    /// Call after the write, with `db` re-opened. `changed` is the entry
    /// saved or replaced; `None` when only flags such as "deleted" changed.
    /// Returns false when the tree could not be kept current (it is then
    /// stale and needs a rebuild).
    pub fn commit(self, db: &Database, changed: Option<u64>) -> Result<bool, CbhError> {
        let journal_file = journal_path(&self.path);
        let Some((base, _)) = tree_stamp(&self.path)? else {
            return Ok(false);
        };
        let mut journal = match Journal::load(&journal_file)? {
            Some(j) if j.base == base => j,
            _ => Journal {
                base,
                current: base,
                changes: BTreeMap::new(),
            },
        };
        if journal.current != self.before
            || journal.changes.len() >= MAX_CHANGES && changed.is_some()
        {
            match std::fs::remove_file(&journal_file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => return Ok(false),
            }
        }
        if let Some(ordinal) = changed {
            let new = db.main_line(ordinal, self.plies)?;
            let old = self.replaced.filter(|r| r.0 == ordinal).and_then(|r| r.1);
            journal.changes.entry(ordinal).or_insert((old, None)).1 = new;
        }
        journal.current = stamp(db);
        journal.save(&journal_file)?;
        Ok(true)
    }

    /// Call after games were appended (a paste), with `db` re-opened: every
    /// new entry is recorded. Returns false when the tree could not be kept
    /// current, e.g. when the journal would exceed its size.
    pub fn commit_appended(self, db: &Database) -> Result<bool, CbhError> {
        let journal_file = journal_path(&self.path);
        let Some((base, _)) = tree_stamp(&self.path)? else {
            return Ok(false);
        };
        let mut journal = match Journal::load(&journal_file)? {
            Some(j) if j.base == base => j,
            _ => Journal {
                base,
                current: base,
                changes: BTreeMap::new(),
            },
        };
        let (from, to) = (self.before.0 + 1, db.info().entry_count);
        let added = to.saturating_sub(from - 1) as usize;
        if journal.current != self.before || journal.changes.len() + added > MAX_CHANGES {
            match std::fs::remove_file(&journal_file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => return Ok(false),
            }
        }
        for ordinal in from..=to {
            let new = db.main_line(ordinal, self.plies)?;
            journal.changes.entry(ordinal).or_insert((None, None)).1 = new;
        }
        journal.current = stamp(db);
        journal.save(&journal_file)?;
        Ok(true)
    }
}
