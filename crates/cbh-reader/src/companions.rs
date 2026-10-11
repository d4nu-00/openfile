use crate::CbhError;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
pub const EXTENSIONS: &[&str] = &[
    "cbh", "cbg", "cbp", "cbt", "cba", "cbc", "cbe", "cbs", "cbl",
];
pub struct CompanionSet {
    pub paths: HashMap<String, PathBuf>,
}
impl CompanionSet {
    pub fn discover(path: &Path) -> Result<Self, CbhError> {
        let stem = path
            .file_stem()
            .ok_or(CbhError::InvalidHeader)?
            .as_encoded_bytes();
        if !path
            .extension()
            .is_some_and(|s| s.as_encoded_bytes().eq_ignore_ascii_case(b"cbh"))
        {
            return Err(CbhError::Unsupported("Choose a classic .cbh file".into()));
        }
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
                    "cib" | "cit" | "cib2" | "cit2" | "cko" | "2lgd" | "2lhd" | "2lbd" | "flags"
                ))
                || !p.is_file()
            {
                continue;
            }
            if paths.insert(ext, p).is_some() {
                return Err(CbhError::AmbiguousCompanion);
            }
        }
        if !paths.contains_key("cbh") {
            return Err(CbhError::InvalidHeader);
        }
        Ok(Self { paths })
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
