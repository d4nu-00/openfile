//! Recovery jobs exist independently of an opened database, so damaged
//! headers can be recovered from the start screen.
use crate::{
    commands::AppError,
    convert::{RESERVE, free_space},
};
use cbh_reader::v2::recover::{self, RecoveryReport};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

struct Job {
    id: String,
    cancel: Arc<AtomicBool>,
    started: bool,
}
#[derive(Default)]
pub struct RecoveryJobs {
    active: Mutex<Option<Job>>,
    next: AtomicU64,
}
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryProgress {
    pub job_id: String,
    pub stage: String,
    pub done: u64,
    pub total: u64,
}
impl RecoveryJobs {
    /// Reserve before dispatch, so Stop cannot be lost before worker startup.
    pub fn begin(&self) -> Result<String, AppError> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| AppError::new("state", "Recovery unavailable"))?;
        if active.is_some() {
            return Err(AppError::new("busy", "A recovery is already running"));
        }
        let id = format!("recovery-{}", self.next.fetch_add(1, Ordering::Relaxed));
        *active = Some(Job {
            id: id.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            started: false,
        });
        Ok(id)
    }
    pub fn cancel(&self, id: &str) -> Result<(), AppError> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| AppError::new("state", "Recovery unavailable"))?;
        if let Some(job) = active.as_ref().filter(|j| j.id == id) {
            job.cancel.store(true, Ordering::Relaxed);
        }
        // A cancelled reservation need not leave a worker behind.
        if active.as_ref().is_some_and(|j| j.id == id && !j.started) {
            *active = None;
        }
        Ok(())
    }
    pub fn run(
        &self,
        id: &str,
        source: &Path,
        destination: &Path,
        mut progress: impl FnMut(RecoveryProgress),
    ) -> Result<RecoveryReport, AppError> {
        let cancel = {
            let mut active = self
                .active
                .lock()
                .map_err(|_| AppError::new("state", "Recovery unavailable"))?;
            let job = active
                .as_mut()
                .filter(|j| j.id == id && !j.started)
                .ok_or_else(|| AppError::new("staleJob", "Recovery was stopped or superseded"))?;
            job.started = true;
            job.cancel.clone()
        };
        struct Finish<'a> {
            jobs: &'a RecoveryJobs,
            id: &'a str,
        }
        impl Drop for Finish<'_> {
            fn drop(&mut self) {
                if let Ok(mut active) = self.jobs.active.lock()
                    && active.as_ref().is_some_and(|j| j.id == self.id)
                {
                    *active = None;
                }
            }
        }
        let _finish = Finish { jobs: self, id };
        let parent = destination
            .parent()
            .ok_or_else(|| AppError::new("invalidPath", "Choose a new recovery folder"))?;
        let free = free_space(parent).ok_or_else(|| {
            AppError::new(
                "diskSpace",
                "The destination disk’s free space cannot be read",
            )
        })?;
        if free < RESERVE {
            return Err(AppError::new(
                "diskSpace",
                "Recovery requires at least 256 MB free on the destination disk",
            ));
        }
        let mut disk_full = false;
        let mut last = Instant::now();
        let mut stage = String::new();
        let result = recover::recover(source, destination, &cancel, |s, done, total| {
            if free_space(parent).is_none_or(|bytes| bytes < 128 * 1024 * 1024) {
                disk_full = true;
                cancel.store(true, Ordering::Relaxed);
            }
            if s != stage || done == total || last.elapsed().as_millis() >= 100 {
                progress(RecoveryProgress {
                    job_id: id.into(),
                    stage: s.into(),
                    done,
                    total,
                });
                last = Instant::now();
                stage = s.into();
            }
        });
        if disk_full {
            return Err(AppError::new(
                "diskSpace",
                "Recovery stopped because the destination disk is nearly full. No result was published.",
            ));
        }
        Ok(result?)
    }
}

#[tauri::command]
pub fn begin_recovery(state: tauri::State<'_, Arc<RecoveryJobs>>) -> Result<String, AppError> {
    state.begin()
}
#[tauri::command]
pub fn cancel_recovery(
    state: tauri::State<'_, Arc<RecoveryJobs>>,
    job_id: String,
) -> Result<(), AppError> {
    state.cancel(&job_id)
}
#[tauri::command]
pub async fn recover_database(
    state: tauri::State<'_, Arc<RecoveryJobs>>,
    job_id: String,
    source: PathBuf,
    destination: PathBuf,
    events: tauri::ipc::Channel<RecoveryProgress>,
) -> Result<RecoveryReport, AppError> {
    let jobs = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        jobs.run(&job_id, &source, &destination, |p| {
            let _ = events.send(p);
        })
    })
    .await
    .map_err(|e| AppError::new("state", e.to_string()))?
}
