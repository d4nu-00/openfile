//! Convert to 2CBH (the reference app's "Convert to free 2CBH format"): writes the
//! games of an open database, or of a selection of its list, into a new
//! 2CBH database. The source is only read. See docs/conversion.md.
use crate::{clipboard::RowRange, commands::AppError, registry::Registry};
use cbh_reader::v2::{
    convert::{self, ConvertReport},
    write::{WRITTEN, Writer},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Space left free on the target's disk, beyond the estimate.
pub const RESERVE: u64 = 256 * 1024 * 1024;
/// A conversion stops when the disk's free space falls below this.
const STOP_BELOW: u64 = 128 * 1024 * 1024;
/// Selected games converted at most; a whole database has no limit.
pub const MAX_SELECTED: usize = 2_000_000;
/// Every family extension, including the reference app's indexes.
const FAMILY: [&str; 6] = ["2cbh", "2cbg", "2cba", "2lid", "2lcd", "2lgd"];

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertProgress {
    pub done: String,
    pub total: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResponse {
    #[serde(flatten)]
    pub report: ConvertReport,
    /// The new database's `.2cbh` path, when it was written.
    pub path: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertEstimate {
    pub games: String,
    pub bytes: String,
    /// Free bytes on the target's disk, when they can be read.
    pub free: Option<String>,
    pub fits: bool,
}

/// Free bytes for this user on the disk holding `dir`.
#[cfg(unix)]
pub fn free_space(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is NUL-terminated and `stat` is a valid out pointer.
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    Some(stat.f_bavail as u64 * stat.f_frsize as u64)
}

/// Free bytes for this user on the disk holding `dir`.
#[cfg(windows)]
pub fn free_space(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            directory: *const u16,
            available: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` is NUL-terminated; the out pointers are valid.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free) };
    (ok != 0).then_some(available)
}

/// The family's files at `base` (a `.2cbh` path).
fn family(base: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    FAMILY.iter().map(|ext| base.with_extension(ext))
}

fn remove_family(base: &Path) {
    for path in family(base) {
        let _ = std::fs::remove_file(path);
    }
}

/// The target as a `.2cbh` path; refused when any file of its family
/// exists, or when it names a source file.
fn check_target(path: PathBuf, sources: &[PathBuf]) -> Result<PathBuf, AppError> {
    if path.file_stem().is_none_or(|s| s.is_empty()) {
        return Err(AppError::new(
            "invalidPath",
            "Choose a name for the database",
        ));
    }
    let path = path.with_extension("2cbh");
    for file in family(&path) {
        if file.exists() {
            return Err(AppError::new(
                "exists",
                format!("A database file already exists: {}", file.display()),
            ));
        }
        for source in sources {
            if source == &file {
                return Err(AppError::new(
                    "sameDatabase",
                    "The new database must not replace the database converted",
                ));
            }
        }
    }
    Ok(path)
}

/// The temporary family the conversion writes before it is renamed.
fn partial(target: &Path) -> PathBuf {
    let stem = target.file_stem().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!("{stem}.converting.2cbh"))
}

fn gb(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / 1e9)
}

impl Registry {
    /// The games a conversion covers: the rows `ranges` of the current
    /// list, or with none, every entry in entry order (`None`).
    fn conversion_games(
        &self,
        database_id: &str,
        ranges: &Option<Vec<RowRange>>,
    ) -> Result<(Option<Vec<u64>>, u64), AppError> {
        match ranges {
            None => {
                let db = self.database(database_id)?;
                let count = db
                    .lock()
                    .map_err(|_| AppError::new("state", "Database unavailable"))?
                    .info()
                    .entry_count;
                Ok((None, count))
            }
            Some(ranges) => {
                let (ordinals, truncated) =
                    self.ordinals_of_rows(database_id, ranges, MAX_SELECTED)?;
                if truncated {
                    return Err(AppError::new(
                        "limitExceeded",
                        format!(
                            "Convert at most {MAX_SELECTED} selected games, or the whole database"
                        ),
                    ));
                }
                let n = ordinals.len() as u64;
                Ok((Some(ordinals), n))
            }
        }
    }

    /// The space a conversion needs, against the free space at `target`.
    pub fn convert_estimate(
        &self,
        database_id: String,
        ranges: Option<Vec<RowRange>>,
        target: PathBuf,
    ) -> Result<ConvertEstimate, AppError> {
        let (_, games) = self.conversion_games(&database_id, &ranges)?;
        let bytes = {
            let db = self.database(&database_id)?;
            let db = db
                .lock()
                .map_err(|_| AppError::new("state", "Database unavailable"))?;
            convert::estimate(&db, games)
        };
        let free = target.parent().and_then(free_space);
        Ok(ConvertEstimate {
            games: games.to_string(),
            bytes: bytes.to_string(),
            free: free.map(|f| f.to_string()),
            fits: free.is_some_and(|f| f >= bytes.saturating_add(RESERVE)),
        })
    }

    /// Converts the games into a new 2CBH database at `target`. Games are
    /// written to a temporary family beside it, which is renamed when all
    /// are written; a cancelled or failed conversion leaves nothing.
    pub fn convert_database(
        &self,
        database_id: String,
        ranges: Option<Vec<RowRange>>,
        target: PathBuf,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<ConvertResponse, AppError> {
        let source_path = self
            .lock()?
            .paths
            .get(&database_id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        let (selected, total) = self.conversion_games(&database_id, &ranges)?;
        let mut source =
            cbh_reader::Database::open(&source_path, cbh_reader::ReadLimits::default())?;
        let target = check_target(target, source.source_paths())?;
        let dir = target
            .parent()
            .ok_or_else(|| AppError::new("invalidPath", "Choose a folder for the database"))?
            .to_path_buf();
        let needed = convert::estimate(&source, total);
        match free_space(&dir) {
            None => {
                return Err(AppError::new(
                    "diskSpace",
                    "The free space on the target disk cannot be read",
                ));
            }
            Some(free) if free < needed.saturating_add(RESERVE) => {
                return Err(AppError::new(
                    "diskSpace",
                    format!(
                        "Converting {total} games needs about {} and {} must stay free; only {} are free on that disk",
                        gb(needed),
                        gb(RESERVE),
                        gb(free)
                    ),
                ));
            }
            Some(_) => {}
        }
        let temp = partial(&target);
        remove_family(&temp);
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .lock()?
            .convert_cancels
            .insert(database_id.clone(), cancel.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        let disk_full = AtomicBool::new(false);
        let result = (|| -> Result<ConvertReport, AppError> {
            let mut writer = Writer::create(&temp)?;
            let ordinals: Box<dyn Iterator<Item = u64>> = match selected {
                Some(o) => Box::new(o.into_iter()),
                None => Box::new(1..=total),
            };
            progress(0, total);
            let report = convert::convert(&mut source, ordinals, &mut writer, &cancel, |r| {
                progress(r.done, total);
                if free_space(&dir).is_none_or(|f| f < STOP_BELOW) {
                    disk_full.store(true, Ordering::Relaxed);
                    cancel.store(true, Ordering::Relaxed);
                }
            })?;
            drop(writer);
            Ok(report)
        })();
        {
            let mut state = self.lock()?;
            if state
                .convert_cancels
                .get(&database_id)
                .is_some_and(|c| Arc::ptr_eq(c, &cancel))
            {
                state.convert_cancels.remove(&database_id);
            }
        }
        let report = match result {
            Ok(report) if disk_full.load(Ordering::Relaxed) => {
                remove_family(&temp);
                return Err(AppError::new(
                    "diskSpace",
                    format!(
                        "The disk is nearly full; the conversion stopped after {} games and wrote nothing",
                        report.done
                    ),
                ));
            }
            Ok(report) if report.cancelled => {
                remove_family(&temp);
                return Ok(ConvertResponse { report, path: None });
            }
            Ok(report) => report,
            Err(e) => {
                remove_family(&temp);
                return Err(e);
            }
        };
        // The target may have appeared meanwhile; never replace it.
        if let Err(e) = check_target(target.clone(), &[]) {
            remove_family(&temp);
            return Err(e);
        }
        for ext in WRITTEN {
            if let Err(e) = std::fs::rename(temp.with_extension(ext), target.with_extension(ext)) {
                remove_family(&temp);
                for ext in WRITTEN {
                    let _ = std::fs::remove_file(target.with_extension(ext));
                }
                return Err(AppError::new("io", e.to_string()));
            }
        }
        Ok(ConvertResponse {
            report,
            path: Some(target.to_string_lossy().into_owned()),
        })
    }

    pub fn cancel_convert(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.convert_cancels.get(&database_id) {
            c.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}
