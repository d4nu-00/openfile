use crate::commands::AppError;
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};
pub fn database_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    ext.starts_with("cb")
        || ext.starts_with("2cb")
        || [
            "cib", "cit", "cib2", "cit2", "cko", "2lgd", "2lhd", "2lbd", "flags",
        ]
        .contains(&ext.as_str())
}
pub fn reject_source_destination(
    destination: &Path,
    sources: &[std::path::PathBuf],
) -> Result<(), AppError> {
    if database_extension(destination) {
        return Err(AppError::new(
            "sourceDestination",
            "Cannot save PGN into a database companion",
        ));
    }
    if destination.exists() {
        for source in sources {
            if same_source(destination, source)? {
                return Err(AppError::new(
                    "sourceDestination",
                    "Destination aliases a read-only database source",
                ));
            }
        }
    }
    Ok(())
}
pub(crate) fn same_source(a: &Path, b: &Path) -> Result<bool, AppError> {
    let a_meta = std::fs::metadata(a).map_err(io_error)?;
    let b_meta = match std::fs::metadata(b) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(io_error(e)),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(a_meta.dev() == b_meta.dev() && a_meta.ino() == b_meta.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = (a_meta, b_meta);
        same_file::is_same_file(a, b).map_err(io_error)
    }
}
fn io_error(e: io::Error) -> AppError {
    AppError::new("io", e.to_string())
}
pub fn atomic_pgn(path: &Path, text: &str) -> Result<(), AppError> {
    atomic_pgn_with_writer(path, |file| file.write_all(text.as_bytes()))
}
pub fn atomic_pgn_with_writer(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), AppError> {
    if database_extension(path) {
        return Err(AppError::new(
            "sourceDestination",
            "Refusing a database-family extension",
        ));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
    write(temp.as_file_mut()).map_err(io_error)?;
    temp.as_file_mut().flush().map_err(io_error)?;
    if path.exists() {
        temp.as_file()
            .set_permissions(std::fs::metadata(path).map_err(io_error)?.permissions())
            .map_err(io_error)?;
    }
    temp.as_file().sync_all().map_err(io_error)?;
    temp.persist(path).map_err(|e| io_error(e.error))?;
    Ok(())
}
