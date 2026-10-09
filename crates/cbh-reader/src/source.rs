use crate::CbhError;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};

pub struct SourceFile {
    path: PathBuf,
    file: Mutex<File>,
    identity: same_file::Handle,
    length: u64,
    modified: Option<SystemTime>,
    bytes: AtomicU64,
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
            file: Mutex::new(file),
            identity,
            length: meta.len(),
            modified: meta.modified().ok(),
            bytes: AtomicU64::new(0),
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
        Ok(())
    }
    pub fn read_exact_range(&self, offset: u64, length: usize) -> Result<Vec<u8>, CbhError> {
        self.verify_unchanged()?;
        if length > 4 * 1024 * 1024 {
            return Err(CbhError::LimitExceeded);
        }
        let end = offset.checked_add(length as u64).ok_or(CbhError::Bounds)?;
        if end > self.length {
            return Err(CbhError::Truncated);
        }
        let mut file = self
            .file
            .lock()
            .map_err(|_| CbhError::Decode("File lock unavailable".into()))?;
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes)?;
        self.bytes.fetch_add(length as u64, Ordering::Relaxed);
        self.verify_unchanged()?;
        Ok(bytes)
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
