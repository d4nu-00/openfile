//! The database start screen: My Databases, recent databases and folder
//! shortcuts, kept in the app's settings (see docs/start-screen.md).
use crate::{
    commands::AppError,
    settings::{
        AppSettings, DATABASE_KINDS, DatabaseProperties, DatabaseView, FavoriteDatabase,
        MAX_FAVORITES, MAX_FOLDERS, MAX_RECENT, Settings,
    },
};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Databases listed from one folder at most.
pub const MAX_FOLDER_DATABASES: usize = 500;
/// Directory entries examined in one folder at most.
const MAX_FOLDER_ENTRIES: usize = 20_000;

/// A database symbol.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSymbol {
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub folder: String,
    /// Effective local property type, favorite type, or "unspecific".
    pub kind: String,
    pub favorite: bool,
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<cbh_reader::DatabaseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<u64>,
    /// Why the header could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderShortcut {
    pub path: PathBuf,
    pub name: String,
    pub exists: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartScreen {
    pub favorites: Vec<DatabaseSymbol>,
    pub recent: Vec<DatabaseSymbol>,
    pub folders: Vec<FolderShortcut>,
    pub kinds: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderListing {
    pub symbols: Vec<DatabaseSymbol>,
    /// More databases than [`MAX_FOLDER_DATABASES`], or a very large folder.
    pub truncated: bool,
}

fn name_of(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn symbol(path: &Path, settings: &AppSettings) -> DatabaseSymbol {
    let favorite = settings.favorites.iter().find(|f| f.path == path);
    let properties = settings.database_properties.iter().find(|p| p.path == path);
    let exists = path.is_file();
    let (format, entries, problem) = if exists {
        match cbh_reader::Database::peek(path) {
            Ok(p) => (Some(p.format), Some(p.entry_count), None),
            Err(e) => (None, None, Some(e.to_string())),
        }
    } else {
        (None, None, None)
    };
    DatabaseSymbol {
        path: path.to_path_buf(),
        name: properties.map_or_else(|| name_of(path), |p| p.name.clone()),
        description: properties.map_or_else(String::new, |p| p.description.clone()),
        folder: path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        kind: properties.map_or_else(
            || favorite.map_or_else(|| "unspecific".into(), |f| f.kind.clone()),
            |p| p.kind.clone(),
        ),
        favorite: favorite.is_some(),
        exists,
        format,
        entries,
        problem,
    }
}

fn is_database(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cbh") || e.eq_ignore_ascii_case("2cbh"))
}

fn absolute(path: &Path) -> Result<(), AppError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(AppError::new("invalidPath", "The path must be absolute"))
    }
}

impl Settings {
    /// Notes that `path` was opened or created: first in Recent, and in My
    /// Databases unless it is there already ("Open a database and add symbol
    /// to 'My the reference app's", the reference app message 50592).
    pub fn record_opened(&self, path: &Path) -> Result<(), AppError> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        absolute(&path)?;
        self.update(|s| {
            s.recent_databases.retain(|p| p != &path);
            s.recent_databases.insert(0, path.clone());
            s.recent_databases.truncate(MAX_RECENT);
            if !s.favorites.iter().any(|f| f.path == path) && s.favorites.len() < MAX_FAVORITES {
                s.favorites.push(FavoriteDatabase {
                    path: path.clone(),
                    kind: "unspecific".into(),
                });
            }
            Ok(())
        })?;
        Ok(())
    }

    pub fn start_screen(&self) -> StartScreen {
        let s = self.load();
        StartScreen {
            favorites: s.favorites.iter().map(|f| symbol(&f.path, &s)).collect(),
            recent: s.recent_databases.iter().map(|p| symbol(p, &s)).collect(),
            folders: s
                .folder_shortcuts
                .iter()
                .map(|p| FolderShortcut {
                    path: p.clone(),
                    name: p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string()),
                    exists: p.is_dir(),
                })
                .collect(),
            kinds: DATABASE_KINDS.to_vec(),
        }
    }

    /// The databases directly in `folder`, by name.
    pub fn folder_databases(&self, folder: &Path) -> Result<FolderListing, AppError> {
        absolute(folder)?;
        let settings = self.load();
        let mut paths = vec![];
        let mut truncated = false;
        let entries = std::fs::read_dir(folder)
            .map_err(|e| AppError::new("io", format!("{}: {e}", folder.display())))?;
        for (i, entry) in entries.enumerate() {
            if i >= MAX_FOLDER_ENTRIES {
                truncated = true;
                break;
            }
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if is_database(&path) && path.is_file() {
                if paths.len() == MAX_FOLDER_DATABASES {
                    truncated = true;
                    break;
                }
                paths.push(path);
            }
        }
        paths.sort_by_key(|p| name_of(p).to_lowercase());
        Ok(FolderListing {
            symbols: paths.iter().map(|p| symbol(p, &settings)).collect(),
            truncated,
        })
    }

    pub fn pin_database(&self, path: &Path) -> Result<(), AppError> {
        absolute(path)?;
        self.update(|s| {
            if !s.favorites.iter().any(|f| f.path == path) {
                if s.favorites.len() >= MAX_FAVORITES {
                    return Err(AppError::new("limitExceeded", "My Databases is full"));
                }
                s.favorites.push(FavoriteDatabase {
                    path: path.to_path_buf(),
                    kind: "unspecific".into(),
                });
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Remove Database Symbol: the files stay.
    pub fn unpin_database(&self, path: &Path) -> Result<(), AppError> {
        self.update(|s| {
            s.favorites.retain(|f| f.path != path);
            Ok(())
        })?;
        Ok(())
    }

    pub fn set_database_kind(&self, path: &Path, kind: &str) -> Result<(), AppError> {
        if !DATABASE_KINDS.contains(&kind) {
            return Err(AppError::new("invalidSettings", "Unknown database type"));
        }
        self.update(|s| {
            let f = s
                .favorites
                .iter_mut()
                .find(|f| f.path == path)
                .ok_or_else(|| AppError::new("notFound", "The database is not in My Databases"))?;
            f.kind = kind.into();
            if let Some(p) = s.database_properties.iter_mut().find(|p| p.path == path) {
                p.kind = kind.into();
            }
            Ok(())
        })?;
        Ok(())
    }

    pub fn forget_recent(&self, path: &Path) -> Result<(), AppError> {
        self.update(|s| {
            s.recent_databases.retain(|p| p != path);
            Ok(())
        })?;
        Ok(())
    }

    pub fn set_database_properties(
        &self,
        mut properties: DatabaseProperties,
    ) -> Result<(), AppError> {
        absolute(&properties.path)?;
        if !is_database(&properties.path) {
            return Err(AppError::new(
                "invalidPath",
                "Choose a .cbh or .2cbh database",
            ));
        }
        properties.name = properties.name.trim().to_string();
        self.update(|s| {
            s.database_properties.retain(|p| p.path != properties.path);
            s.database_properties.push(properties);
            Ok(())
        })?;
        Ok(())
    }

    pub fn set_database_view(&self, view: DatabaseView) -> Result<(), AppError> {
        self.update(|s| {
            s.database_view = view;
            Ok(())
        })?;
        Ok(())
    }

    pub fn add_folder_shortcut(&self, folder: &Path) -> Result<(), AppError> {
        absolute(folder)?;
        if !folder.is_dir() {
            return Err(AppError::new("io", "The folder does not exist"));
        }
        self.update(|s| {
            if !s.folder_shortcuts.iter().any(|p| p == folder) {
                if s.folder_shortcuts.len() >= MAX_FOLDERS {
                    return Err(AppError::new("limitExceeded", "Too many folder shortcuts"));
                }
                s.folder_shortcuts.push(folder.to_path_buf());
            }
            Ok(())
        })?;
        Ok(())
    }

    pub fn remove_folder_shortcut(&self, folder: &Path) -> Result<(), AppError> {
        self.update(|s| {
            s.folder_shortcuts.retain(|p| p != folder);
            Ok(())
        })?;
        Ok(())
    }
}

#[tauri::command]
pub async fn start_screen(app: tauri::AppHandle) -> Result<StartScreen, AppError> {
    use tauri::Manager;
    tauri::async_runtime::spawn_blocking(move || app.state::<Settings>().start_screen())
        .await
        .map_err(|e| AppError::new("state", e.to_string()))
}

#[tauri::command]
pub async fn folder_databases(
    app: tauri::AppHandle,
    folder: PathBuf,
) -> Result<FolderListing, AppError> {
    use tauri::Manager;
    tauri::async_runtime::spawn_blocking(move || app.state::<Settings>().folder_databases(&folder))
        .await
        .map_err(|e| AppError::new("state", e.to_string()))?
}

#[tauri::command]
pub fn pin_database(state: tauri::State<'_, Settings>, path: PathBuf) -> Result<(), AppError> {
    state.pin_database(&path)
}

#[tauri::command]
pub fn unpin_database(state: tauri::State<'_, Settings>, path: PathBuf) -> Result<(), AppError> {
    state.unpin_database(&path)
}

#[tauri::command]
pub fn set_database_kind(
    state: tauri::State<'_, Settings>,
    path: PathBuf,
    kind: String,
) -> Result<(), AppError> {
    state.set_database_kind(&path, &kind)
}

#[tauri::command]
pub fn set_database_properties(
    state: tauri::State<'_, Settings>,
    path: PathBuf,
    properties: PropertyValues,
) -> Result<(), AppError> {
    state.set_database_properties(DatabaseProperties {
        path,
        name: properties.name,
        kind: properties.kind,
        description: properties.description,
    })
}

#[derive(serde::Deserialize)]
pub struct PropertyValues {
    pub name: String,
    pub kind: String,
    pub description: String,
}

#[tauri::command]
pub fn set_database_view(
    state: tauri::State<'_, Settings>,
    view: DatabaseView,
) -> Result<(), AppError> {
    state.set_database_view(view)
}

#[tauri::command]
pub fn forget_recent(state: tauri::State<'_, Settings>, path: PathBuf) -> Result<(), AppError> {
    state.forget_recent(&path)
}

#[tauri::command]
pub fn add_folder_shortcut(
    state: tauri::State<'_, Settings>,
    folder: PathBuf,
) -> Result<(), AppError> {
    state.add_folder_shortcut(&folder)
}

#[tauri::command]
pub fn remove_folder_shortcut(
    state: tauri::State<'_, Settings>,
    folder: PathBuf,
) -> Result<(), AppError> {
    state.remove_folder_shortcut(&folder)
}
