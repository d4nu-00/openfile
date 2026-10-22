//! A whole recovery result is published as one directory, exclusively.
use std::path::Path;

pub(super) fn publish(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(from.as_os_str().as_bytes())?;
        let to = std::ffi::CString::new(to.as_os_str().as_bytes())?;
        // SAFETY: both strings are NUL-terminated, flags require an absent target.
        #[cfg(target_os = "macos")]
        let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileW(from: *const u16, to: *const u16) -> i32;
        }
        let from: Vec<u16> = from.as_os_str().encode_wide().collect();
        let to: Vec<u16> = to.as_os_str().encode_wide().collect();
        if from.contains(&0) || to.contains(&0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Recovery paths cannot contain NUL",
            ));
        }
        let from: Vec<_> = from.into_iter().chain([0]).collect();
        let to: Vec<_> = to.into_iter().chain([0]).collect();
        // SAFETY: valid NUL-terminated paths. MoveFileW requires an absent
        // target; std::fs::rename may replace an empty directory on Windows.
        if unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

pub(super) fn sync_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    std::fs::File::open(path)?.sync_all()?;
    #[cfg(windows)]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn publication_refuses_files_and_empty_or_populated_directories() {
        for kind in ["file", "empty", "populated"] {
            let root = tempfile::tempdir().unwrap();
            let from = root.path().join("temporary");
            let to = root.path().join("destination");
            std::fs::create_dir(&from).unwrap();
            std::fs::write(from.join("game"), "verified").unwrap();
            if kind == "file" {
                std::fs::write(&to, "existing").unwrap();
            } else {
                std::fs::create_dir(&to).unwrap();
                if kind == "populated" {
                    std::fs::write(to.join("keep"), "existing").unwrap();
                }
            }
            assert!(super::publish(&from, &to).is_err(), "{kind}");
            assert_eq!(
                std::fs::read_to_string(from.join("game")).unwrap(),
                "verified"
            );
            match kind {
                "file" => assert_eq!(std::fs::read_to_string(&to).unwrap(), "existing"),
                "empty" => assert_eq!(std::fs::read_dir(&to).unwrap().count(), 0),
                _ => assert_eq!(
                    std::fs::read_to_string(to.join("keep")).unwrap(),
                    "existing"
                ),
            }
        }
    }
}
