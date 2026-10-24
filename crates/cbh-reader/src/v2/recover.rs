//! Bounded salvage into a separate verified 2CBH copy. This is an OpenFile
//! operation, not a port of ChessBase's in-place Check Integrity repair.
use super::{
    block,
    record::{self, RECORD},
    recover_names::Names,
    recover_publish::{publish, sync_dir},
    write::{WRITTEN, Writer},
};
use crate::{
    CbhError, Database, EntryKind, ReadLimits, companions::CompanionSet, source::SourceFile,
};
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const MAX_ENTRIES: u64 = 2_000_000;
const MAX_PROBLEMS: usize = 500;
const MAX_FRAME: u64 = 8 * 1024 * 1024;
const MAX_OUTPUT: u64 = 32 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryProblem {
    pub ordinal: u64,
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReport {
    pub checked: u64,
    pub total: u64,
    pub recovered: u64,
    pub corrupt: u64,
    pub unsupported: u64,
    pub warnings: u64,
    pub cancelled: bool,
    pub bytes: u64,
    pub problems: Vec<RecoveryProblem>,
    pub problems_truncated: bool,
    pub path: Option<PathBuf>,
    pub report_path: Option<PathBuf>,
}
impl RecoveryReport {
    fn problem(&mut self, ordinal: u64, code: &str, message: impl Into<String>) {
        if self.problems.len() < MAX_PROBLEMS {
            self.problems.push(RecoveryProblem {
                ordinal,
                code: code.into(),
                message: message.into(),
            });
        } else {
            self.problems_truncated = true;
        }
    }
}
fn log(
    out: &mut impl Write,
    ordinal: u64,
    status: &str,
    new: Option<u64>,
    reason: &str,
) -> Result<(), CbhError> {
    serde_json::to_writer(
        &mut *out,
        &serde_json::json!({"entry":ordinal,"status":status,"recoveredEntry":new,"reason":reason}),
    )
    .map_err(|e| CbhError::Decode(e.to_string()))?;
    out.write_all(b"\n")?;
    Ok(())
}
struct Stream {
    source: Option<SourceFile>,
    out: File,
    at: u64,
    kind: u16,
}
impl Stream {
    fn open(source: Option<&Path>, output: &Path, kind: u16) -> Result<Self, CbhError> {
        Ok(Self {
            source: source.map(SourceFile::open).transpose()?,
            out: OpenOptions::new().write(true).open(output)?,
            at: 12,
            kind,
        })
    }
    fn read(&self, offset: u64, limit: usize) -> Result<(block::Block, u64), CbhError> {
        let f = self
            .source
            .as_ref()
            .ok_or_else(|| CbhError::Decode("Required stream companion is missing".into()))?;
        let head = f.read_exact_range(offset, block::FRAME_HEADER)?;
        let total = block::FRAME_OVERHEAD
            + u32::from_le_bytes(head[8..12].try_into().unwrap()) as u64
            + u32::from_le_bytes(head[12..16].try_into().unwrap()) as u64;
        if total > MAX_FRAME {
            return Err(CbhError::LimitExceeded);
        }
        let b = block::read_block(f, offset, limit)?;
        if b.kind != self.kind {
            return Err(CbhError::Decode(format!(
                "Wrong stream block type {:#x}",
                b.kind
            )));
        }
        Ok((b, total))
    }
    fn copy(&mut self, offset: u64, total: u64) -> Result<u64, CbhError> {
        if self.at.saturating_add(total) > MAX_OUTPUT {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "Recovery stream exceeds the 32 GiB output limit",
            )
            .into());
        }
        let f = self.source.as_ref().ok_or(CbhError::Truncated)?;
        let start = self.at;
        self.out.seek(SeekFrom::Start(start))?;
        let mut done = 0;
        while done < total {
            let n = (total - done).min(64 * 1024) as usize;
            self.out.write_all(&f.read_exact_range(offset + done, n)?)?;
            done += n as u64;
        }
        self.at += total;
        Ok(start)
    }
    fn finish(mut self) -> Result<(), CbhError> {
        self.out.seek(SeekFrom::Start(0))?;
        self.out.write_all(&self.at.to_le_bytes())?;
        self.out.sync_all()?;
        Ok(())
    }
}
fn classify(e: &CbhError) -> &'static str {
    if matches!(
        e,
        CbhError::Unsupported(_) | CbhError::UnsupportedSetup(_) | CbhError::LimitExceeded
    ) {
        "unsupported"
    } else {
        "corrupt"
    }
}

/// `destination` is a new folder. The four files and full JSON-lines report
/// appear together through one exclusive directory rename. Crash leftovers
/// are isolated hidden temporary directories and never a published database.
pub fn recover(
    source: &Path,
    destination: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&str, u64, u64),
) -> Result<RecoveryReport, CbhError> {
    let source = std::fs::canonicalize(source)?;
    let files = CompanionSet::discover(&source)?;
    if files.format != crate::DatabaseFormat::New {
        return Err(CbhError::Unsupported(
            "Recovery currently supports 2CBH only; classic CBH is never modified".into(),
        ));
    }
    let parent = std::fs::canonicalize(destination.parent().ok_or(CbhError::Bounds)?)?;
    let name = destination
        .file_name()
        .filter(|n| !n.is_empty())
        .ok_or(CbhError::Bounds)?;
    let destination = parent.join(name);
    if destination.symlink_metadata().is_ok()
        || destination == source
        || source.starts_with(&destination)
    {
        return Err(CbhError::Unsupported(
            "Choose a new, absent recovery folder separate from the source".into(),
        ));
    }
    let index = SourceFile::open(files.get("2cbh").ok_or(CbhError::InvalidHeader)?)?;
    let head = index.read_exact_range(0, RECORD)?;
    if head[8] != 0x26
        || head[13] != 5
        || u16::from_le_bytes(head[10..12].try_into().unwrap()) != 192
    {
        return Err(CbhError::Unsupported(
            "The 2CBH version/192-byte record boundary cannot be established".into(),
        ));
    }
    let physical = index.len() / 192 - 1;
    let count = u64::from_le_bytes(head[16..24].try_into().unwrap());
    // A damaged lower count must not silently hide readable records. The
    // established fixed stride, rather than the possibly damaged count,
    // determines candidate boundaries. This may include orphan records.
    let total = physical;
    if total > MAX_ENTRIES {
        return Err(CbhError::Unsupported(
            "Recovery is limited to 2,000,000 entries per source".into(),
        ));
    }
    let mut report = RecoveryReport {
        total,
        ..Default::default()
    };
    if count != physical + 1 {
        report.warnings += 1;
        report.problem(
            0,
            "headerCount",
            format!("Invalid declared count {count}; examining {total} complete 192-byte entries"),
        );
    }
    if !index.len().is_multiple_of(192) {
        report.corrupt += 1;
        report.problem(
            physical + 1,
            "truncatedIndex",
            "Incomplete final index record omitted",
        );
    }
    if cancel.load(Ordering::Relaxed) {
        report.cancelled = true;
        return Ok(report);
    }
    let tracked: Vec<_> = WRITTEN
        .iter()
        .map(|e| files.get(e).map(SourceFile::open).transpose())
        .collect::<Result<_, _>>()?;
    let temp = tempfile::Builder::new()
        .prefix(".openfile-recovery-")
        .tempdir_in(&parent)?;
    let output = temp.path().join("Recovered.2cbh");
    drop(Writer::create(&output)?);
    let mut records = OpenOptions::new().write(true).open(&output)?;
    records.seek(SeekFrom::Start(192))?;
    let mut moves = Stream::open(
        files.get("2cbg"),
        &output.with_extension("2cbg"),
        block::TYPE_GAME,
    )?;
    let mut notes = Stream::open(
        files.get("2cba"),
        &output.with_extension("2cba"),
        block::TYPE_ANNOTATIONS,
    )?;
    let mut names = Names::new(files.get("2lid"), &output.with_extension("2lid"))?;
    let mut out = BufWriter::new(File::create(temp.path().join("recovery.jsonl"))?);
    log(&mut out, 0, "source", None, &source.display().to_string())?;
    if !index.len().is_multiple_of(192) {
        log(
            &mut out,
            physical + 1,
            "corrupt",
            None,
            "Incomplete final index record",
        )?;
    }
    for stream in [(&moves, "2cbg"), (&notes, "2cba")] {
        if let Some(s) = &stream.0.source {
            let h = s.read_exact_range(0, 12);
            let valid = h.as_ref().is_ok_and(|h| {
                u64::from_le_bytes(h[..8].try_into().unwrap()) >= 12
                    && u64::from_le_bytes(h[..8].try_into().unwrap()) <= s.len()
                    && h[8..10] == [12, 0]
            });
            if !valid {
                report.warnings += 1;
                report.problem(0,"streamHeader",format!(".{} header is damaged; only individually verified referenced frames are retained",stream.1));
            }
        }
    }
    let limits = ReadLimits::default();
    progress("recovering", 0, total);
    for ordinal in 1..=total {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            return Ok(report);
        }
        let mut bytes = index.read_exact_range(ordinal * 192, 192)?;
        let recover_entry = (|| -> Result<String, CbhError> {
            let raw = record::parse_record(&bytes, ordinal)?;
            if raw.kind != EntryKind::Game {
                return Err(CbhError::Unsupported(
                    "Text or unknown entry kind; this recovery currently retains chess games only"
                        .into(),
                ));
            }
            if !raw.diagnostics.is_empty() {
                return Err(CbhError::Decode(
                    raw.diagnostics
                        .iter()
                        .map(|d| d.message.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                ));
            }
            let offset = raw
                .moves_offset
                .ok_or_else(|| CbhError::Decode("Missing move block reference".into()))?;
            let (b, move_len) = moves.read(offset, limits.max_game_bytes)?;
            let decoded = super::moves::decode_moves(&b.payload, &limits)?;
            if !decoded.diagnostics.is_empty() {
                return Err(CbhError::Unsupported(
                    "Move flags cannot be fully verified".into(),
                ));
            }
            let mut warning = if count > 0 && ordinal >= count {
                "Outside the declared count; recovered at an established 192-byte boundary. "
                    .to_string()
            } else {
                String::new()
            };
            let annotation = if let Some(at) = raw.annotation_offset {
                let (b, len) = notes.read(at, limits.max_game_bytes)?;
                let parsed = crate::annotations::parse_new(&b.payload)?;
                let (_, diagnostics) = crate::annotations::apply(
                    &mut decoded.game.clone(),
                    parsed,
                    crate::annotations::Order::Line,
                );
                if !diagnostics.is_empty() {
                    warning.push_str("Original annotation bytes retained, including content OpenFile cannot display");
                }
                Some((at, len))
            } else {
                None
            };
            names.remap(&mut bytes)?;
            let at = moves.copy(offset, move_len)?;
            bytes[8..16].copy_from_slice(&at.to_le_bytes());
            if let Some((offset, len)) = annotation {
                let at = notes.copy(offset, len)?;
                bytes[16..24].copy_from_slice(&at.to_le_bytes());
            }
            records.write_all(&bytes)?;
            Ok(warning)
        })();
        match recover_entry {
            Ok(warning) => {
                report.recovered += 1;
                if !warning.is_empty() {
                    report.warnings += 1;
                    report.problem(ordinal, "retainedContent", &warning);
                }
                log(
                    &mut out,
                    ordinal,
                    "recovered",
                    Some(report.recovered),
                    &warning,
                )?;
            }
            Err(CbhError::SourceChanged) => return Err(CbhError::SourceChanged),
            Err(e @ CbhError::Io(_)) => return Err(e),
            Err(e) => {
                let status = classify(&e);
                if status == "unsupported" {
                    report.unsupported += 1
                } else {
                    report.corrupt += 1
                }
                report.problem(ordinal, status, e.to_string());
                log(&mut out, ordinal, status, None, &e.to_string())?;
            }
        }
        report.checked = ordinal;
        if ordinal.is_multiple_of(100) || ordinal == total {
            progress("recovering", ordinal, total);
        }
    }
    moves.finish()?;
    notes.finish()?;
    names.finish()?;
    records.seek(SeekFrom::Start(16))?;
    records.write_all(&(report.recovered + 1).to_le_bytes())?;
    records.sync_all()?;
    drop(records);
    out.flush()?;
    out.get_ref().sync_all()?;
    drop(out);
    if cancel.load(Ordering::Relaxed) {
        report.cancelled = true;
        return Ok(report);
    }
    progress("verifying", 0, report.recovered);
    let db = Database::open(&output, limits)?;
    let integrity =
        db.check_integrity(cancel, |done| progress("verifying", done, report.recovered))?;
    if integrity.cancelled || cancel.load(Ordering::Relaxed) {
        report.cancelled = true;
        return Ok(report);
    }
    if !integrity.problems.is_empty() {
        return Err(CbhError::Decode(format!(
            "Recovered copy did not pass verification: {}",
            integrity.problems[0].message
        )));
    }
    drop(db);
    for file in tracked.into_iter().flatten() {
        file.verify_unchanged()?;
    }
    index.verify_unchanged()?;
    let current = CompanionSet::discover(&source)?;
    for ext in WRITTEN {
        if current.get(ext) != files.get(ext) {
            return Err(CbhError::SourceChanged);
        }
    }
    report.bytes = super::compact::family_bytes(&output);
    report.path = Some(destination.join("Recovered.2cbh"));
    report.report_path = Some(destination.join("recovery.jsonl"));
    let mut summary = File::create(temp.path().join("recovery-summary.json"))?;
    serde_json::to_writer_pretty(&mut summary, &report)
        .map_err(|e| CbhError::Decode(e.to_string()))?;
    summary.sync_all()?;
    drop(summary);
    sync_dir(temp.path())?;
    if cancel.load(Ordering::Relaxed) {
        report.cancelled = true;
        report.path = None;
        report.report_path = None;
        return Ok(report);
    }
    publish(temp.path(), &destination)?;
    // The rename is the commit point: a complete verified result now exists.
    // A parent-directory sync failure cannot make it an incomplete family.
    if let Err(e) = sync_dir(&parent) {
        report.warnings += 1;
        report.problem(
            0,
            "directorySync",
            format!("Verified copy published; parent directory sync failed: {e}"),
        );
    }
    Ok(report)
}
