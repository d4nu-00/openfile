use crate::CbhError;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
pub const EXTENSIONS: &[&str] = &[
    "cbh", "cbg", "cbp", "cbt", "cba", "cbc", "cbe", "cbs", "cbl",
];
pub struct CompanionSet {
    pub format: crate::DatabaseFormat,
    pub paths: HashMap<String, PathBuf>,
}
impl CompanionSet {
    pub fn discover(path: &Path) -> Result<Self, CbhError> {
        let stem = path
            .file_stem()
            .ok_or(CbhError::InvalidHeader)?
            .as_encoded_bytes();
        let extension = path.extension().map(|s| s.as_encoded_bytes());
        let (format, head) = match extension {
            Some(e) if e.eq_ignore_ascii_case(b"cbh") => (crate::DatabaseFormat::Classic, "cbh"),
            Some(e) if e.eq_ignore_ascii_case(b"2cbh") => (crate::DatabaseFormat::New, "2cbh"),
            _ => {
                return Err(CbhError::Unsupported(
                    "Choose a .cbh or .2cbh database file".into(),
                ));
            }
        };
        let mut paths = HashMap::new();
        for entry in std::fs::read_dir(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )? {
            let p = entry?.path();
            if !p
                .file_stem()
                .is_some_and(|s| s.as_encoded_bytes().eq_ignore_ascii_case(stem))
            {
                continue;
            }
            let Some(ext) = p
                .extension()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
            else {
                continue;
            };
            // Retain unparsed family members for source provenance and safe exports.
            // Only EXTENSIONS are presented as supported parser companions.
            if !(ext.starts_with("cb")
                || ext.starts_with("2cb")
                || matches!(
                    ext.as_str(),
                    "cib"
                        | "cit"
                        | "cib2"
                        | "cit2"
                        | "cko"
                        | "2lgd"
                        | "2lhd"
                        | "2lbd"
                        | "2lid"
                        | "2lcd"
                        | "flags"
                ))
                || !p.is_file()
            {
                continue;
            }
            if paths.insert(ext, p).is_some() {
                return Err(CbhError::AmbiguousCompanion);
            }
        }
        if !paths.contains_key(head) {
            return Err(CbhError::InvalidHeader);
        }
        Ok(Self { format, paths })
    }
    pub fn get(&self, extension: &str) -> Option<&Path> {
        self.paths
            .get(&extension.to_ascii_lowercase())
            .map(PathBuf::as_path)
    }
}
pub fn choose_candidate<'a>(
    stem: &str,
    extension: &str,
    candidates: &'a [&str],
) -> Result<Option<&'a str>, CbhError> {
    let wanted = format!("{stem}.{extension}");
    let mut found = None;
    for &name in candidates {
        if name.eq_ignore_ascii_case(&wanted) {
            if found.is_some() {
                return Err(CbhError::AmbiguousCompanion);
            }
            found = Some(name);
        }
    }
    Ok(found)
}

/// The files `Database::open` reads from a classic database.
pub(crate) const CLASSIC_READ: &[&str] = &[
    "cbh", "cbg", "cbp", "cbt", "cba", "cbc", "cbs", "cbe", "cbj",
];

/// A file of a database whose contents are not on this computer, such as
/// a file in iCloud Drive that macOS removed to save space ("Optimize Mac
/// Storage"). Reading it waits until macOS has downloaded the whole file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudFile {
    pub path: PathBuf,
    pub bytes: u64,
}

/// The files of the database at `path` that opening it would read but
/// that are not on this computer yet. `stat` reports this without
/// starting a download. Always empty outside macOS.
pub fn not_downloaded(path: &Path) -> Result<Vec<CloudFile>, CbhError> {
    let files = CompanionSet::discover(path)?;
    let read: Vec<&str> = match files.format {
        crate::DatabaseFormat::Classic => CLASSIC_READ.to_vec(),
        crate::DatabaseFormat::New => crate::v2::names::DECODED_COMPANIONS.to_vec(),
    };
    let mut missing = vec![];
    for extension in read {
        let Some(file) = files.get(extension) else {
            continue;
        };
        let Ok(meta) = std::fs::metadata(file) else {
            continue;
        };
        if is_dataless(&meta) {
            missing.push(CloudFile {
                path: file.to_path_buf(),
                bytes: meta.len(),
            });
        }
    }
    Ok(missing)
}

#[cfg(target_os = "macos")]
fn is_dataless(meta: &std::fs::Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    // SF_DATALESS in <sys/stat.h>.
    meta.st_flags() & 0x4000_0000 != 0
}
#[cfg(not(target_os = "macos"))]
fn is_dataless(_: &std::fs::Metadata) -> bool {
    false
}
