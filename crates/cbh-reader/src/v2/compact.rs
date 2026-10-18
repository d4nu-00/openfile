//! Remove Deleted Games (ChessBase's `DBTOOLSPHYSICALDELETIONS`): rewrites a
//! 2CBH family without the entries marked deleted. The kept entries' 2CBG
//! and 2CBA blocks are copied byte for byte, so nothing is decoded or lost;
//! 2LID is copied unchanged. The new files are written beside the family and
//! swapped in when complete (docs/maintenance.md).
use super::{
    block::{FILE_HEADER, FRAME_HEADER, FRAME_OVERHEAD, MARKER},
    record::RECORD,
    write::WRITTEN,
};
use crate::CbhError;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

/// Records read at once.
const CHUNK: u64 = 4096;
/// ChessBase's indexes over 2LID; they are removed, and ChessBase's Check
/// Integrity rebuilds them ("Rebuild control structures", `0x1401576a0`).
pub const INDEXES: [&str; 2] = ["2lcd", "2lgd"];

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactReport {
    pub kept: u64,
    pub removed: u64,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub cancelled: bool,
    /// The new number of each old entry (index 0 unused); 0 for removed
    /// entries. Empty when cancelled.
    #[serde(skip)]
    pub renumbered: Vec<u64>,
}

fn with(base: &Path, tag: &str, ext: &str) -> PathBuf {
    let stem = base.file_stem().unwrap_or_default().to_string_lossy();
    base.with_file_name(format!("{stem}.{tag}.{ext}"))
}

/// The temporary family a compaction writes.
pub fn temporary(base: &Path) -> PathBuf {
    with(base, "compacting", "2cbh")
}

fn backup(base: &Path, ext: &str) -> PathBuf {
    with(base, "compact-backup", ext)
}

fn stamp(path: &Path) -> Result<(u64, Option<SystemTime>), CbhError> {
    let m = std::fs::metadata(path)?;
    Ok((m.len(), m.modified().ok()))
}

/// The bytes the family's four files take now: an upper bound for the
/// compacted copy.
pub fn family_bytes(base: &Path) -> u64 {
    WRITTEN
        .iter()
        .filter_map(|ext| std::fs::metadata(base.with_extension(ext)).ok())
        .map(|m| m.len())
        .sum()
}

fn read_at(file: &mut File, offset: u64, length: usize) -> Result<Vec<u8>, CbhError> {
    let mut bytes = vec![0; length];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// One stream file being copied: its blocks are appended in order, each
/// copied once even when several records refer to it.
struct Copy {
    from: File,
    /// The used length declared in the file's header.
    from_len: u64,
    to: File,
    at: u64,
    copied: HashMap<u64, u64>,
    ext: &'static str,
}

impl Copy {
    fn open(base: &Path, temp: &Path, ext: &'static str) -> Result<Self, CbhError> {
        let mut from = File::open(base.with_extension(ext))?;
        let from_len = from.metadata()?.len();
        let head = read_at(&mut from, 0, FILE_HEADER as usize)?;
        let used = u64::from_le_bytes(head[..8].try_into().unwrap());
        if used < FILE_HEADER || used > from_len {
            return Err(CbhError::Decode(format!(".{ext} declares a wrong length")));
        }
        let mut to = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp.with_extension(ext))?;
        to.write_all(&head)?;
        Ok(Self {
            from,
            from_len: used,
            to,
            at: FILE_HEADER,
            copied: HashMap::new(),
            ext,
        })
    }
    /// Copies the block at `offset` and returns its new offset.
    fn block(&mut self, offset: u64, ordinal: u64) -> Result<u64, CbhError> {
        if let Some(&at) = self.copied.get(&offset) {
            return Ok(at);
        }
        let damaged = || {
            CbhError::Decode(format!(
                "Entry {ordinal}: the .{} block at {offset} is damaged; run Check Integrity",
                self.ext
            ))
        };
        if offset < FILE_HEADER || offset + FRAME_HEADER as u64 > self.from_len {
            return Err(damaged());
        }
        let head = read_at(&mut self.from, offset, FRAME_HEADER)?;
        let le32 = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap()) as u64;
        if u64::from_le_bytes(head[..8].try_into().unwrap()) != MARKER {
            return Err(damaged());
        }
        let total = FRAME_OVERHEAD + le32(8) + le32(12);
        if offset + total > self.from_len || total > u32::MAX as u64 {
            return Err(damaged());
        }
        let bytes = read_at(&mut self.from, offset, total as usize)?;
        if u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap()) != total {
            return Err(damaged());
        }
        let at = self.at;
        self.to.write_all(&bytes)?;
        self.at += total;
        self.copied.insert(offset, at);
        Ok(at)
    }
    fn finish(mut self) -> Result<(), CbhError> {
        self.to.seek(SeekFrom::Start(0))?;
        self.to.write_all(&self.at.to_le_bytes())?;
        self.to.sync_all()?;
        Ok(())
    }
}

fn remove_family(base: &Path) {
    for ext in WRITTEN {
        let _ = std::fs::remove_file(base.with_extension(ext));
    }
}

/// Rewrites the family at `path` (a `.2cbh` path) without its deleted
/// entries. No other program or writer may change it meanwhile; a change
/// noticed before the swap stops the compaction. A cancelled or failed
/// compaction leaves the family as it was.
pub fn compact(
    path: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<CompactReport, CbhError> {
    let before: Vec<_> = WRITTEN
        .iter()
        .map(|ext| stamp(&path.with_extension(ext)))
        .collect::<Result<_, _>>()?;
    for ext in WRITTEN.iter().chain(&INDEXES) {
        if backup(path, ext).exists() {
            return Err(CbhError::Unsupported(format!(
                "An earlier compaction left {}; check it before compacting again",
                backup(path, ext).display()
            )));
        }
    }
    let temp = temporary(path);
    remove_family(&temp);
    let result = (|| -> Result<CompactReport, CbhError> {
        let mut head_file = File::open(path)?;
        let head = read_at(&mut head_file, 0, RECORD)?;
        let length = head_file.metadata()?.len();
        let parsed = super::record::NewHeader::parse(&head, length)?;
        if head[8] != 0x26 || head[13] != 5 {
            return Err(CbhError::Unsupported(
                "2CBH header version OpenFile does not know".into(),
            ));
        }
        let count = parsed.entry_count;
        let mut records = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        records.write_all(&head)?;
        let mut games = Copy::open(path, &temp, "2cbg")?;
        let mut notes = Copy::open(path, &temp, "2cba")?;
        let mut report = CompactReport {
            bytes_before: family_bytes(path),
            renumbered: vec![0; count as usize + 1],
            ..Default::default()
        };
        let mut next = 1u64;
        let mut start = 1u64;
        while start <= count {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                report.renumbered.clear();
                return Ok(report);
            }
            let n = CHUNK.min(count + 1 - start);
            let bytes = read_at(&mut head_file, start * RECORD as u64, n as usize * RECORD)?;
            let mut out = Vec::with_capacity(bytes.len());
            for (i, record) in bytes.as_chunks::<RECORD>().0.iter().enumerate() {
                let ordinal = start + i as u64;
                if record[0] & 0x80 != 0 {
                    report.removed += 1;
                    continue;
                }
                let mut r = record.to_vec();
                // Games refer to a 2CBG and a 2CBA block; text entries only
                // to a 2CBG block, and their bytes 16.. mean something else.
                let fields: &[usize] = match u16::from_le_bytes([r[2], r[3]]) {
                    super::record::TYPE_GAME => &[8, 16],
                    super::record::TYPE_TEXT => &[8],
                    kind => {
                        return Err(CbhError::Unsupported(format!(
                            "Entry {ordinal} has a kind OpenFile does not know ({kind:#06x})"
                        )));
                    }
                };
                for &at in fields {
                    let copy = if at == 8 { &mut games } else { &mut notes };
                    let offset = u64::from_le_bytes(r[at..at + 8].try_into().unwrap());
                    if offset != 0 {
                        let moved = copy.block(offset, ordinal)?;
                        r[at..at + 8].copy_from_slice(&moved.to_le_bytes());
                    }
                }
                out.extend(r);
                report.renumbered[ordinal as usize] = next;
                next += 1;
                report.kept += 1;
            }
            records.write_all(&out)?;
            start += n;
            progress(start - 1, count);
        }
        games.finish()?;
        notes.finish()?;
        records.seek(SeekFrom::Start(16))?;
        records.write_all(&next.to_le_bytes())?;
        records.sync_all()?;
        std::fs::copy(path.with_extension("2lid"), temp.with_extension("2lid"))?;
        // Windows flushes only a handle opened for writing.
        OpenOptions::new()
            .write(true)
            .open(temp.with_extension("2lid"))?
            .sync_all()?;
        report.bytes_after = family_bytes(&temp);
        Ok(report)
    })();
    let report = match result {
        Ok(r) if !r.cancelled => r,
        other => {
            remove_family(&temp);
            return other;
        }
    };
    let now: Vec<_> = WRITTEN
        .iter()
        .map(|ext| stamp(&path.with_extension(ext)))
        .collect::<Result<_, _>>()?;
    if now != before {
        remove_family(&temp);
        return Err(CbhError::SourceChanged);
    }
    if let Err(e) = swap(path, &temp) {
        remove_family(&temp);
        return Err(e);
    }
    Ok(report)
}

/// Moves the family's files to backups, the new files into place, then
/// deletes the backups. A failure restores the backups.
fn swap(path: &Path, temp: &Path) -> Result<(), CbhError> {
    let mut moved: Vec<&str> = vec![];
    let restore = |moved: &[&str]| {
        for ext in moved {
            let _ = std::fs::rename(backup(path, ext), path.with_extension(ext));
        }
    };
    for ext in WRITTEN.iter().chain(&INDEXES) {
        if !path.with_extension(ext).exists() {
            continue;
        }
        if let Err(e) = std::fs::rename(path.with_extension(ext), backup(path, ext)) {
            restore(&moved);
            return Err(e.into());
        }
        moved.push(ext);
    }
    for (i, ext) in WRITTEN.iter().enumerate() {
        if let Err(e) = std::fs::rename(temp.with_extension(ext), path.with_extension(ext)) {
            // Put back what was already moved in, then the originals.
            for done in &WRITTEN[..i] {
                let _ = std::fs::rename(path.with_extension(done), temp.with_extension(done));
            }
            restore(&moved);
            return Err(e.into());
        }
    }
    for ext in moved {
        let _ = std::fs::remove_file(backup(path, ext));
    }
    Ok(())
}
