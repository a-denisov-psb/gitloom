//! Read-only copies of files as they are in a commit (M19-01, ADR-061).
//!
//! Editors take a path, not content, so a file of a commit is written to
//! `<temp>/gitloom-view/<commit id, 12 characters>/<repo path>` and the
//! editor gets that path. This module only touches the copy folder; it
//! starts no process (ADR-053). All functions do file I/O: call them off
//! the UI thread.

// The entry points use this module from M19-02 on.
#![cfg_attr(not(test), allow(dead_code))]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Folder name of the copies inside the temp folder.
pub const FOLDER: &str = git_core::identity::VIEW_DIR;

/// Length of the commit id in the copy path.
pub const ID_LEN: usize = 12;

/// Copies older than this are deleted at start-up.
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The base folder of all copies (the one place it is defined).
pub fn base_dir() -> PathBuf {
    std::env::temp_dir().join(FOLDER)
}

/// Where the copy of `repo_path` (`/` separated, relative to the repository
/// root) of commit `commit` lives under `base`. Refuses paths that would
/// leave `base` or that this system cannot write, with a readable reason.
pub fn copy_path(base: &Path, commit: &str, repo_path: &str) -> Result<PathBuf, String> {
    copy_path_for(base, commit, repo_path, cfg!(windows))
}

/// [`copy_path`] with the Windows name rules on or off (testable on every
/// system).
pub fn copy_path_for(
    base: &Path,
    commit: &str,
    repo_path: &str,
    windows: bool,
) -> Result<PathBuf, String> {
    let id: String = commit.chars().take(ID_LEN).collect();
    if id.len() < 4 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("\"{commit}\" is not a commit id"));
    }
    if repo_path.is_empty() {
        return Err("The file has no name".into());
    }
    if repo_path.starts_with('/') {
        return Err(format!("{repo_path} is an absolute path"));
    }
    let mut path = base.join(id.to_ascii_lowercase());
    for part in repo_path.split('/') {
        if let Some(reason) = part_problem(part, windows) {
            return Err(format!(
                "{repo_path} cannot be written on this system: {reason}"
            ));
        }
        path.push(part);
    }
    Ok(path)
}

/// Why one path component cannot be written safely, if it cannot.
fn part_problem(part: &str, windows: bool) -> Option<String> {
    if part.is_empty() {
        return Some("it has an empty folder name".into());
    }
    if part == "." || part == ".." {
        return Some(format!("it contains \"{part}\""));
    }
    if part.contains('\0') {
        return Some("it contains a NUL character".into());
    }
    // A drive prefix or a separator inside a name would leave the folder.
    if part.contains('\\') && (windows || cfg!(windows)) {
        return Some("it contains a backslash".into());
    }
    if windows {
        if let Some(c) = part
            .chars()
            .find(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || (*c as u32) < 32)
        {
            return Some(match c {
                c if (c as u32) < 32 => "it contains a control character".into(),
                c => format!("Windows does not allow \"{c}\" in names"),
            });
        }
        if part.ends_with('.') || part.ends_with(' ') {
            return Some(format!("\"{part}\" ends with a dot or a space"));
        }
        let stem = part.split('.').next().unwrap_or(part).trim_end();
        if is_reserved(stem) {
            return Some(format!("\"{part}\" is a reserved name on Windows"));
        }
    }
    None
}

/// Windows device names (`CON`, `NUL`, `COM1`, …), also with an extension.
fn is_reserved(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    ["COM", "LPT"].iter().any(|prefix| {
        upper.strip_prefix(prefix).is_some_and(|rest| {
            matches!(
                rest,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

/// Writes `bytes` to the copy at `path` (folders are created) and makes it
/// read-only. An existing copy with the same bytes is kept; one with other
/// bytes is rewritten.
pub fn write_copy(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let describe = |e: io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(describe)?;
    }
    match fs::read(path) {
        Ok(existing) if existing == bytes => return set_read_only(path).map_err(describe),
        Ok(_) => make_writable(path).map_err(describe)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(describe(e)),
    }
    fs::write(path, bytes).map_err(describe)?;
    set_read_only(path).map_err(describe)
}

fn set_read_only(path: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    if !permissions.readonly() {
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

/// Clears the read-only flag so the copy can be rewritten or deleted.
#[cfg(unix)]
fn make_writable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(permissions.mode() | 0o200);
    fs::set_permissions(path, permissions)
}

/// Clears the read-only flag so the copy can be rewritten or deleted.
#[cfg(not(unix))]
fn make_writable(path: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)
}

/// Deletes copies under `base` last written more than `max_age` before
/// `now`, then the folders that became empty. Best effort: errors (a file
/// in use, a missing folder) are ignored. Returns the number of deleted files.
pub fn clean(base: &Path, max_age: Duration, now: SystemTime) -> usize {
    clean_with(base, max_age, now, &|path| {
        make_writable(path)?;
        fs::remove_file(path)
    })
}

/// [`clean`] with the delete step given (tests make it fail).
fn clean_with(
    base: &Path,
    max_age: Duration,
    now: SystemTime,
    remove: &dyn Fn(&Path) -> io::Result<()>,
) -> usize {
    let Some(limit) = now.checked_sub(max_age) else {
        return 0;
    };
    clean_dir(base, limit, remove)
}

fn clean_dir(dir: &Path, limit: SystemTime, remove: &dyn Fn(&Path) -> io::Result<()>) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut deleted = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        // No symlinks are followed: the copies are plain files and folders.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            deleted += clean_dir(&path, limit, remove);
            // Fails while the folder still has files; that is fine.
            let _ = fs::remove_dir(&path);
        } else {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|modified| modified < limit);
            if old && remove(&path).is_ok() {
                deleted += 1;
            }
        }
    }
    deleted
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn path_keeps_name_and_folders() {
        let base = Path::new("/tmp/gitloom-view");
        let path = copy_path_for(base, ID, "src/deep/Makefile", false).unwrap();
        assert_eq!(
            path,
            base.join("0123456789ab")
                .join("src")
                .join("deep")
                .join("Makefile")
        );
        assert_eq!(path.file_name().unwrap(), "Makefile");
        let short = copy_path_for(base, "ABCDEF1", "a.txt", false).unwrap();
        assert_eq!(short, base.join("abcdef1").join("a.txt"));
    }

    #[test]
    fn unsafe_paths_are_refused() {
        let base = Path::new("/tmp/gitloom-view");
        for path in ["../x", "a/../../x", "/etc/passwd", "a//b", "a/", "", "./a"] {
            assert!(
                copy_path_for(base, ID, path, false).is_err(),
                "{path:?} must be refused"
            );
        }
        assert!(copy_path_for(base, "../../etc", "a", false).is_err());
        assert!(copy_path_for(base, "", "a", false).is_err());
    }

    #[test]
    fn windows_names_are_refused_on_windows() {
        let base = Path::new("base");
        for path in [
            "C:/x",
            "a/CON",
            "nul.txt",
            "Com1.log",
            "lpt9",
            "a<b",
            "what?",
            "star*",
            "pipe|",
            "quote\"",
            "dot.",
            "space ",
            "back\\slash",
            "tab\tname",
        ] {
            let error = copy_path_for(base, ID, path, true).unwrap_err();
            assert!(error.contains("cannot be written"), "{path:?}: {error}");
        }
        for path in ["console.txt", "com10", "aux_x", "COM", "a.b.c"] {
            assert!(copy_path_for(base, ID, path, true).is_ok(), "{path:?}");
        }
        // The same names are fine elsewhere (no backslash rule on Unix).
        if !cfg!(windows) {
            assert!(copy_path_for(base, ID, "what?", false).is_ok());
            assert!(copy_path_for(base, ID, "back\\slash", false).is_ok());
        }
    }

    #[test]
    fn nothing_is_written_outside_the_base_folder() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("view");
        assert!(copy_path(&base, ID, "../escape.txt").is_err());
        assert!(!dir.path().join("escape.txt").exists());
        assert!(!base.exists());
    }

    #[test]
    fn copy_is_read_only_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let path = copy_path(dir.path(), ID, "src/a.rs").unwrap();
        write_copy(&path, b"one\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"one\n");
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        let first = fs::metadata(&path).unwrap().modified().unwrap();
        // Same bytes: the file is kept as it is.
        write_copy(&path, b"one\n").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), first);
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
    }

    #[test]
    fn changed_bytes_rewrite_a_read_only_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = copy_path(dir.path(), ID, "a.txt").unwrap();
        write_copy(&path, b"old").unwrap();
        write_copy(&path, b"new bytes").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new bytes");
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
    }

    #[test]
    fn cleanup_deletes_old_copies_and_keeps_new_ones() {
        let dir = tempfile::tempdir().unwrap();
        let old = copy_path(dir.path(), ID, "old/a.txt").unwrap();
        let new = copy_path(dir.path(), "fedcba9876543210", "b.txt").unwrap();
        write_copy(&old, b"a").unwrap();
        write_copy(&new, b"b").unwrap();
        let day_ago = SystemTime::now() - Duration::from_secs(25 * 60 * 60);
        make_writable(&old).unwrap();
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(day_ago)
            .unwrap();
        assert_eq!(clean(dir.path(), MAX_AGE, SystemTime::now()), 1);
        assert!(!old.exists());
        // The emptied folders are gone too.
        assert!(!dir.path().join("0123456789ab").exists());
        assert!(new.exists());
    }

    #[test]
    fn cleanup_survives_a_file_it_cannot_delete() {
        let dir = tempfile::tempdir().unwrap();
        let stuck = copy_path(dir.path(), ID, "stuck.txt").unwrap();
        let other = copy_path(dir.path(), ID, "other.txt").unwrap();
        write_copy(&stuck, b"a").unwrap();
        write_copy(&other, b"b").unwrap();
        let later = SystemTime::now() + Duration::from_secs(48 * 60 * 60);
        let deleted = clean_with(dir.path(), MAX_AGE, later, &|path| {
            if path.ends_with("stuck.txt") {
                Err(io::Error::other("in use"))
            } else {
                make_writable(path)?;
                fs::remove_file(path)
            }
        });
        assert_eq!(deleted, 1);
        assert!(stuck.exists());
        assert!(!other.exists());
        // A missing base folder is no error either.
        assert_eq!(clean(&dir.path().join("missing"), MAX_AGE, later), 0);
    }
}
