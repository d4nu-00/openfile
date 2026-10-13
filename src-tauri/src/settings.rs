//! The workbench's own settings, kept as JSON in the app's config folder:
//! the most game tabs (ChessBase's "Maximum Board Windows") and the engines
//! the user installed.
use crate::commands::AppError;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Settings files larger than this are not read.
const MAX_BYTES: u64 = 64 * 1024;
/// ChessBase allows 1 to 8 board windows, 8 by default (`MaxGameWins`).
pub const MAX_GAME_TABS: u8 = 8;
pub const MAX_ENGINES: usize = 32;
const MAX_NAME: usize = 128;
const MAX_ENGINE_OPTIONS: usize = 128;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub max_game_tabs: u8,
    pub engines: Vec<InstalledEngine>,
    /// The id of the engine Default Kibitzer starts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_engine: Option<String>,
    /// The folder of Syzygy tablebase files the user chose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tablebase_dir: Option<PathBuf>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            max_game_tabs: MAX_GAME_TABS,
            engines: vec![],
            default_engine: None,
            tablebase_dir: None,
        }
    }
}

/// An engine in Engine Management: a UCI executable and the options the
/// user changed from the engine's defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledEngine {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub options: std::collections::BTreeMap<String, String>,
}

impl AppSettings {
    /// Checks the settings before they are kept.
    pub fn check(&self) -> Result<(), AppError> {
        let invalid = |m: &str| Err(AppError::new("invalidSettings", m));
        if !(1..=MAX_GAME_TABS).contains(&self.max_game_tabs) {
            return invalid("The most game tabs must be from 1 to 8");
        }
        if self.engines.len() > MAX_ENGINES {
            return invalid("Too many engines");
        }
        let mut ids = std::collections::BTreeSet::new();
        for e in &self.engines {
            if e.id.is_empty()
                || e.id.len() > MAX_NAME
                || e.name.trim().is_empty()
                || e.name.len() > MAX_NAME
                || !ids.insert(&e.id)
            {
                return invalid("Each engine needs a name and its own id");
            }
            if !e.path.is_absolute() {
                return invalid("An engine path must be absolute");
            }
            if e.options.len() > MAX_ENGINE_OPTIONS
                || e.options
                    .iter()
                    .any(|(k, v)| k.len() > MAX_NAME || v.len() > 1024)
            {
                return invalid("Too many or too long engine options");
            }
        }
        if self
            .tablebase_dir
            .as_ref()
            .is_some_and(|d| !d.is_absolute())
        {
            return invalid("The tablebase folder must be an absolute path");
        }
        if let Some(id) = &self.default_engine
            && !ids.contains(id)
        {
            return invalid("The default engine is not installed");
        }
        Ok(())
    }
}

/// Where the settings live; set once the app knows its config folder.
#[derive(Default)]
pub struct Settings {
    path: Mutex<Option<PathBuf>>,
}

fn io(e: impl std::fmt::Display) -> AppError {
    AppError::new("io", e.to_string())
}

impl Settings {
    pub fn set_dir(&self, dir: &Path) {
        if let Ok(mut p) = self.path.lock() {
            *p = Some(dir.join("settings.json"));
        }
    }
    fn path(&self) -> Option<PathBuf> {
        self.path.lock().ok()?.clone()
    }
    /// The saved settings; the defaults when there are none, or when the
    /// file is too large or unreadable (it is left as it is).
    pub fn load(&self) -> AppSettings {
        let Some(path) = self.path() else {
            return AppSettings::default();
        };
        let read = || -> Option<AppSettings> {
            if std::fs::metadata(&path).ok()?.len() > MAX_BYTES {
                return None;
            }
            let s: AppSettings = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
            s.check().ok()?;
            Some(s)
        };
        read().unwrap_or_default()
    }
    /// Checks and saves `settings`, replacing the file in one step.
    pub fn save(&self, settings: &AppSettings) -> Result<(), AppError> {
        settings.check()?;
        let path = self
            .path()
            .ok_or_else(|| AppError::new("state", "No settings folder"))?;
        let dir = path.parent().ok_or_else(|| io("No settings folder"))?;
        std::fs::create_dir_all(dir).map_err(io)?;
        let text = serde_json::to_vec_pretty(settings).map_err(io)?;
        let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(io)?;
        temp.write_all(&text).map_err(io)?;
        temp.as_file().sync_all().map_err(io)?;
        temp.persist(&path).map_err(|e| io(e.error))?;
        Ok(())
    }
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, Settings>) -> AppSettings {
    state.load()
}

#[tauri::command]
pub fn set_settings(
    state: tauri::State<'_, Settings>,
    tablebases: tauri::State<'_, std::sync::Arc<crate::tablebases::Tablebases>>,
    settings: AppSettings,
) -> Result<AppSettings, AppError> {
    settings.check()?;
    // A tablebase folder is opened before it is kept.
    tablebases.set_dir(settings.tablebase_dir.as_deref())?;
    state.save(&settings)?;
    Ok(settings)
}
