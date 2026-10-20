//! Progress for authored replay questions, separate from databases and other practice.
use crate::commands::AppError;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Write, path::PathBuf, sync::Mutex};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub attempts: u32,
    pub needs_retry: bool,
    pub best_points: u16,
}

#[derive(Default, Deserialize, Serialize)]
struct ProgressFile {
    version: u8,
    items: BTreeMap<String, Progress>,
}

#[derive(Default)]
pub struct ReplayTrainingStore {
    dir: Mutex<Option<PathBuf>>,
    write_lock: Mutex<()>,
}

fn err(message: impl std::fmt::Display) -> AppError {
    AppError::new("replayTraining", message.to_string())
}

impl ReplayTrainingStore {
    pub fn set_dir(&self, dir: PathBuf) {
        *self.dir.lock().expect("replay training directory") = Some(dir);
    }
    fn path(&self) -> Result<PathBuf, AppError> {
        Ok(self
            .dir
            .lock()
            .map_err(err)?
            .as_ref()
            .ok_or_else(|| err("No app data folder"))?
            .join("progress.json"))
    }
    fn read_file(&self) -> Result<ProgressFile, AppError> {
        let path = self.path()?;
        let file: ProgressFile = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(err)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ProgressFile::default(),
            Err(e) => return Err(err(e)),
        };
        if file.version > 1 {
            return Err(err("Newer replay training progress version"));
        }
        Ok(file)
    }
    pub fn load(&self) -> Result<BTreeMap<String, Progress>, AppError> {
        Ok(self.read_file()?.items)
    }
    pub fn record(&self, key: String, progress: Progress) -> Result<(), AppError> {
        if key.len() > 131_072 || key.is_empty() {
            return Err(err("Invalid question key"));
        }
        let _guard = self.write_lock.lock().map_err(err)?;
        let mut file = self.read_file()?;
        file.version = 1;
        file.items.insert(key, progress);
        let path = self.path()?;
        let dir = path.parent().ok_or_else(|| err("No app data folder"))?;
        std::fs::create_dir_all(dir).map_err(err)?;
        let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(err)?;
        serde_json::to_writer_pretty(&mut temp, &file).map_err(err)?;
        temp.flush().map_err(err)?;
        temp.as_file().sync_all().map_err(err)?;
        temp.persist(path).map_err(|e| err(e.error))?;
        Ok(())
    }
}

#[tauri::command]
pub fn replay_training_progress(
    state: tauri::State<'_, ReplayTrainingStore>,
) -> Result<BTreeMap<String, Progress>, AppError> {
    state.load()
}

#[tauri::command]
pub fn replay_training_record(
    state: tauri::State<'_, ReplayTrainingStore>,
    key: String,
    progress: Progress,
) -> Result<(), AppError> {
    state.record(key, progress)
}
