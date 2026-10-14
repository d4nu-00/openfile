use crate::CbhError;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Instant, SystemTime},
};

/// Reads check the file by its path at most this often; every operation
/// starts with a full check (`Database::verify`).
const RECHECK_MS: u64 = 200;

/// Milliseconds since the first call, for cheap shared timestamps.
fn now_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

/// Reads exactly `bytes.len()` bytes at `offset` without a shared cursor.
fn read_at(file: &File, bytes: &mut [u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_exact_at(file, bytes, offset)
    }
    #[cfg(windows)]
    {
        let mut done = 0;
        while done < bytes.len() {
            let n = std::os::windows::fs::FileExt::seek_read(
                file,
                &mut bytes[done..],
                offset + done as u64,
            )?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            done += n;
        }
        Ok(())
    }
}

pub struct SourceFile {
    path: PathBuf,
    file: File,
    identity: same_file::Handle,
    length: u64,
    modified: Option<SystemTime>,
    bytes: AtomicU64,
    /// When the path was last checked (`now_ms`), 0 for never.
    checked: AtomicU64,
}
impl SourceFile {
    pub fn open(path: &Path) -> Result<Self, CbhError> {
        let file = File::open(path)?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(CbhError::InvalidHeader);
        }
        let identity = same_file::Handle::from_file(file.try_clone()?)?;
        Ok(Self {
            path: path.to_owned(),
            file,
            identity,
            length: meta.len(),
            modified: meta.modified().ok(),
            bytes: AtomicU64::new(0),
            checked: AtomicU64::new(0),
        })
    }
    pub fn verify_unchanged(&self) -> Result<(), CbhError> {
        let current = std::fs::metadata(&self.path).map_err(|_| CbhError::SourceChanged)?;
        let identity =
            same_file::Handle::from_path(&self.path).map_err(|_| CbhError::SourceChanged)?;
        if identity != self.identity
            || current.len() != self.length
            || current.modified().ok() != self.modified
        {
            return Err(CbhError::SourceChanged);
        }
        self.checked.store(now_ms(), Ordering::Relaxed);
        Ok(())
    }
    /// The full check, unless one passed in the last `RECHECK_MS`. Checking
    /// on every read cost more than the reads themselves once several
    /// threads read the same file (see docs/position-search.md).
    fn verify_recent(&self) -> Result<(), CbhError> {
        let last = self.checked.load(Ordering::Relaxed);
        if last == 0 || now_ms().saturating_sub(last) >= RECHECK_MS {
            self.verify_unchanged()?;
        }
        Ok(())
    }
    pub fn read_exact_range(&self, offset: u64, length: usize) -> Result<Vec<u8>, CbhError> {
        self.verify_recent()?;
        if length > 4 * 1024 * 1024 {
            return Err(CbhError::LimitExceeded);
        }
        let end = offset.checked_add(length as u64).ok_or(CbhError::Bounds)?;
        if end > self.length {
            return Err(CbhError::Truncated);
        }
        let mut bytes = vec![0; length];
        read_at(&self.file, &mut bytes, offset)?;
        self.bytes.fetch_add(length as u64, Ordering::Relaxed);
        self.verify_recent()?;
        Ok(bytes)
    }
    /// Length and modification time (nanoseconds since the epoch, 0 if
    /// unknown), to tell whether files derived from this one are current.
    pub fn stamp(&self) -> (u64, u64) {
        let modified = self
            .modified
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as u64);
        (self.length, modified)
    }
    pub fn bytes_read(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }
    pub fn len(&self) -> u64 {
        self.length
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}
