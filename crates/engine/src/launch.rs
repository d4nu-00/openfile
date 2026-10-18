//! Why an engine program could not start, in words the user can act on: a
//! folder chosen in place of the program inside it, or (on macOS) a program
//! downloaded from the internet that the system stops before it can answer.
use crate::EngineError;
use std::path::Path;

/// Folder entries looked at when suggesting the program inside a folder.
const MAX_ENTRIES: usize = 500;
/// Programs named in that suggestion.
const MAX_NAMED: usize = 3;

/// Refuses a folder before trying to start it.
pub(crate) fn check_program(path: &Path) -> Result<(), EngineError> {
    if !path.is_dir() {
        return Ok(());
    }
    let folder = display_name(path);
    let programs = programs_in(path);
    Err(EngineError::NotAProgram(match programs.as_slice() {
        [] => format!("“{folder}” is a folder. Choose the engine program inside it."),
        [one] => {
            format!("“{folder}” is a folder. Choose the engine program inside it: “{one}”.")
        }
        many => format!(
            "“{folder}” is a folder. Choose the engine program inside it, such as {}.",
            many.iter()
                .map(|n| format!("“{n}”"))
                .collect::<Vec<_>>()
                .join(" or ")
        ),
    }))
}

/// Replaces a failed start's error with the reason when the system stopped
/// the program: on macOS, a quarantined download that is not notarized is
/// killed as soon as it starts, which otherwise reads as "process exited".
pub(crate) fn explain_failure(path: &Path, error: EngineError) -> EngineError {
    if matches!(error, EngineError::Exited | EngineError::Io(_)) && quarantined(path) {
        return EngineError::Blocked(format!(
            "macOS stopped “{}” as it started, because it was downloaded from the \
             internet and is not notarized. If you trust it, allow it by running this \
             in Terminal, then choose it again:\nxattr -d com.apple.quarantine {}",
            display_name(path),
            shell_quote(&path.to_string_lossy()),
        ));
    }
    error
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.to_string_lossy(), |n| n.to_string_lossy())
        .into_owned()
}

/// Regular files in `folder` that look like programs, by name.
fn programs_in(folder: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return vec![];
    };
    let mut names: Vec<String> = entries
        .take(MAX_ENTRIES)
        .filter_map(Result::ok)
        .filter(|e| {
            let Ok(meta) = e.metadata() else {
                return false;
            };
            meta.is_file() && is_program(&e.path(), &meta)
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.truncate(MAX_NAMED);
    names
}

#[cfg(unix)]
fn is_program(_path: &Path, meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_program(path: &Path, _meta: &std::fs::Metadata) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
}

#[cfg(target_os = "macos")]
fn quarantined(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(file) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: both strings are NUL-terminated and outlive the call; a null
    // buffer of size 0 only asks for the attribute's size.
    let size = unsafe {
        libc::getxattr(
            file.as_ptr(),
            c"com.apple.quarantine".as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            0,
        )
    };
    size >= 0
}

#[cfg(not(target_os = "macos"))]
fn quarantined(_path: &Path) -> bool {
    false
}

/// `text` as one POSIX shell word.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_names_the_program_inside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "text").unwrap();
        let program = dir.path().join(if cfg!(windows) {
            "engine.exe"
        } else {
            "engine"
        });
        std::fs::write(&program, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let message = check_program(dir.path()).unwrap_err().to_string();
        assert!(message.contains("is a folder"), "{message}");
        assert!(message.contains("“engine"), "{message}");
        assert!(!message.contains("README"), "{message}");
        assert!(check_program(&program).is_ok());
    }

    #[test]
    fn other_failures_keep_their_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("engine");
        std::fs::write(&file, "").unwrap();
        assert!(matches!(
            explain_failure(&file, EngineError::Exited),
            EngineError::Exited
        ));
    }

    #[test]
    fn quotes_paths_for_the_shell() {
        assert_eq!(shell_quote("/a b/c"), "'/a b/c'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_quarantined_program_is_explained() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("engine 2");
        std::fs::write(&file, "").unwrap();
        let status = std::process::Command::new("xattr")
            .args(["-w", "com.apple.quarantine", "0081;00000000;Test;"])
            .arg(&file)
            .status()
            .unwrap();
        assert!(status.success());
        let message = explain_failure(&file, EngineError::Exited).to_string();
        assert!(message.contains("macOS stopped “engine 2”"), "{message}");
        assert!(
            message.contains(&format!(
                "xattr -d com.apple.quarantine '{}'",
                file.display()
            )),
            "{message}"
        );
    }
}
