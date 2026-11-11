//! Clone a repository (M10-02, ADR-045): `git clone --progress` through the
//! network path (progress, cancel, askpass), name derivation from URLs and
//! the target folder rules.
//!
//! Blocking: runs `git` and touches the file system.

use std::path::Path;

use crate::{CancelToken, CliBackend, Error, GitWriter, Progress, Result};

/// Folder name Git would choose for `url`: the last path segment without a
/// trailing `.git` or `/`. Works for HTTPS, `ssh://`, scp-like
/// (`git@host:owner/repo.git`) URLs and local paths (also with `\`).
/// `None` when nothing usable is left.
pub fn repo_name(url: &str) -> Option<String> {
    let mut rest = url.trim();
    loop {
        let trimmed = rest.trim_end_matches(['/', '\\']);
        let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
        if trimmed.len() == rest.len() {
            break;
        }
        rest = trimmed;
    }
    let (scheme, path) = match rest.split_once("://") {
        Some((_, path)) => (true, path),
        None => (false, rest),
    };
    let has_separator = path.contains(['/', '\\']);
    // `https://host[:port]` names a server, not a repository.
    if scheme && !has_separator {
        return None;
    }
    let segment = path.rsplit(['/', '\\']).next().unwrap_or(path);
    // scp-like `host:repo` (or a drive letter `C:`): the part after the colon.
    let segment = match segment.rsplit_once(':') {
        Some((_, name)) if !has_separator => name,
        _ => segment,
    };
    let segment = segment.trim();
    (!segment.is_empty() && segment != "." && segment != "..").then(|| segment.to_owned())
}

/// Why `target` cannot be cloned or initialized into: it must not exist, or
/// be an empty folder.
pub fn target_problem(target: &Path) -> Option<String> {
    if !target.exists() {
        return None;
    }
    if !target.is_dir() {
        return Some(format!("{} exists and is not a folder", target.display()));
    }
    match std::fs::read_dir(target) {
        Ok(mut entries) => entries
            .next()
            .map(|_| format!("{} already exists and is not empty", target.display())),
        Err(e) => Some(format!("{} cannot be read: {e}", target.display())),
    }
}

/// Clones `url` into `target` (`git clone --progress`, with
/// `--recurse-submodules` if `submodules`). `git` runs in the parent folder
/// of `target`. On failure or cancel a folder that did not exist before is
/// removed again; an empty folder that existed stays.
pub fn clone(
    git: &CliBackend,
    url: &str,
    target: &Path,
    submodules: bool,
    cancel: &CancelToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    if url.trim().is_empty() {
        return Err(Error::Backend("enter the URL of the repository".into()));
    }
    if let Some(problem) = target_problem(target) {
        return Err(Error::Backend(problem));
    }
    let existed = target.exists();
    let target_text = target.to_string_lossy().into_owned();
    let mut args = vec!["clone", "--progress"];
    if submodules {
        args.push("--recurse-submodules");
    }
    args.extend(["--", url.trim(), target_text.as_str()]);
    let result = git.run_network(&args, cancel, progress).map(drop);
    if result.is_err() && !existed && target.exists() {
        // Git removes what it created; this catches a cancelled process.
        let _ = std::fs::remove_dir_all(target);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_urls() {
        for (url, name) in [
            ("https://github.com/owner/repo.git", Some("repo")),
            ("https://github.com/owner/repo", Some("repo")),
            ("https://github.com/owner/repo.git/", Some("repo")),
            ("https://user:pw@host:8443/group/sub/repo.git", Some("repo")),
            ("git@github.com:owner/repo.git", Some("repo")),
            ("git@host:repo.git", Some("repo")),
            ("ssh://git@host:2222/owner/repo.git", Some("repo")),
            ("file:///srv/git/project.git", Some("project")),
            ("/srv/git/project.git", Some("project")),
            ("../sibling", Some("sibling")),
            ("C:\\work\\repo\\", Some("repo")),
            ("  repo  ", Some("repo")),
            ("https://github.com", None),
            ("https://host:8080/", None),
            ("", None),
            ("..", None),
        ] {
            assert_eq!(repo_name(url).as_deref(), name, "{url}");
        }
    }

    #[test]
    fn target_rules() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("new");
        assert_eq!(target_problem(&missing), None);
        std::fs::create_dir(&missing).unwrap();
        assert_eq!(target_problem(&missing), None, "empty folder is fine");
        std::fs::write(missing.join("file"), "x").unwrap();
        assert!(target_problem(&missing).unwrap().contains("not empty"));
        assert!(target_problem(&missing.join("file"))
            .unwrap()
            .contains("not a folder"));
    }
}
