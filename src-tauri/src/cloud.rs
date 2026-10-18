//! Databases in iCloud Drive whose files macOS has removed to save space.
//! Opening such a database would wait, without progress, until macOS has
//! downloaded every file it reads; when iCloud cannot download, that is
//! for ever. The frontend asks first, and downloads with progress.
use crate::commands::AppError;
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    /// File names (not paths) still in iCloud only.
    pub files: Vec<String>,
    /// Their size, as a decimal string.
    pub bytes: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProgress {
    pub files_done: usize,
    pub files: usize,
    pub bytes_done: String,
    pub bytes: String,
}

/// Running downloads by request id, for Stop.
#[derive(Default)]
pub struct CloudDownloads(Mutex<HashMap<String, Arc<AtomicBool>>>);

fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn status(path: &Path) -> Result<CloudStatus, AppError> {
    let files = cbh_reader::companions::not_downloaded(path)?;
    Ok(CloudStatus {
        bytes: files.iter().map(|f| f.bytes).sum::<u64>().to_string(),
        files: files.iter().map(|f| name(&f.path)).collect(),
    })
}

/// Asks macOS to download one file. `brctl download` returns at once and
/// the download continues in the background; without it, reading the
/// first byte on a thread of its own starts the download too.
fn request_download(path: &Path) {
    let asked = std::process::Command::new("/usr/bin/brctl")
        .arg("download")
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !asked {
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            use std::io::Read;
            if let Ok(mut f) = std::fs::File::open(path) {
                let _ = f.read(&mut [0u8; 1]);
            }
        });
    }
}

impl CloudDownloads {
    /// Downloads the database's missing files and returns once all are on
    /// this computer, reporting each finished file. Stop ends the wait;
    /// macOS may still finish the downloads.
    pub fn download(
        &self,
        path: PathBuf,
        request_id: String,
        mut progress: impl FnMut(CloudProgress),
    ) -> Result<(), AppError> {
        let cancel = Arc::new(AtomicBool::new(false));
        self.0
            .lock()
            .map_err(|_| AppError::new("internal", "Download state unavailable"))?
            .insert(request_id.clone(), cancel.clone());
        let result = (|| {
            let start = cbh_reader::companions::not_downloaded(&path)?;
            let total: u64 = start.iter().map(|f| f.bytes).sum();
            for f in &start {
                request_download(&f.path);
            }
            loop {
                let left = cbh_reader::companions::not_downloaded(&path)?;
                let left_bytes: u64 = left.iter().map(|f| f.bytes).sum();
                progress(CloudProgress {
                    files_done: start.len() - left.len().min(start.len()),
                    files: start.len(),
                    bytes_done: total.saturating_sub(left_bytes).to_string(),
                    bytes: total.to_string(),
                });
                if left.is_empty() {
                    return Ok(());
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err(AppError::new(
                        "cancelled",
                        "Stopped waiting for iCloud; the database was not opened",
                    ));
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        })();
        if let Ok(mut map) = self.0.lock() {
            map.remove(&request_id);
        }
        result
    }
    pub fn cancel(&self, request_id: &str) {
        if let Some(flag) = self.0.lock().ok().and_then(|m| m.get(request_id).cloned()) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

#[tauri::command]
pub async fn cloud_status(path: String) -> Result<CloudStatus, AppError> {
    crate::commands::blocking(move || status(Path::new(&path))).await
}

#[tauri::command]
pub async fn download_cloud_files(
    state: tauri::State<'_, Arc<CloudDownloads>>,
    path: String,
    request_id: String,
    events: tauri::ipc::Channel<CloudProgress>,
) -> Result<(), AppError> {
    let s = state.inner().clone();
    crate::commands::blocking(move || {
        s.download(path.into(), request_id, |p| {
            let _ = events.send(p);
        })
    })
    .await
}

#[tauri::command]
pub fn cancel_cloud_download(state: tauri::State<'_, Arc<CloudDownloads>>, request_id: String) {
    state.cancel(&request_id);
}
