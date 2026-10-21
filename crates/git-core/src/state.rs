//! Operations in progress (merge, rebase, cherry-pick, revert), read from
//! the marker files Git keeps in the `.git` directory.
//!
//! Blocking: file system access.

use std::path::Path;

use crate::Oid;

/// Multi-step operation the repository is in the middle of.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RepoState {
    #[default]
    Clean,
    /// `git merge` stopped (conflicts or `--no-commit`).
    Merging { merge_head: Oid },
    /// `git rebase` (interactive or apply backend) stopped.
    Rebasing {
        /// Branch being rebased (short name); `None` for a detached HEAD.
        branch: Option<String>,
        /// Commit the branch is replayed onto.
        onto: Option<Oid>,
        /// Current step and total number of steps, if known.
        step: Option<(usize, usize)>,
    },
    /// `git cherry-pick` stopped; `commit` is the one being picked, if known.
    CherryPicking { commit: Option<Oid> },
    /// `git revert` stopped; `commit` is the one being reverted, if known.
    Reverting { commit: Option<Oid> },
}

impl RepoState {
    pub fn is_clean(&self) -> bool {
        *self == Self::Clean
    }

    /// Short name of the operation, e.g. "merge", for messages.
    pub fn operation(&self) -> Option<&'static str> {
        match self {
            Self::Clean => None,
            Self::Merging { .. } => Some("merge"),
            Self::Rebasing { .. } => Some("rebase"),
            Self::CherryPicking { .. } => Some("cherry-pick"),
            Self::Reverting { .. } => Some("revert"),
        }
    }

    /// A commit concludes the operation (Git uses the prepared `MERGE_MSG`).
    pub fn concluded_by_commit(&self) -> bool {
        matches!(
            self,
            Self::Merging { .. } | Self::CherryPicking { .. } | Self::Reverting { .. }
        )
    }
}

/// Detects the operation in progress in the repository at `git_dir`.
pub fn detect(git_dir: &Path) -> RepoState {
    for dir in ["rebase-merge", "rebase-apply"] {
        let dir = git_dir.join(dir);
        // `git am` also uses rebase-apply; it marks itself with `applying`.
        if dir.is_dir() && !dir.join("applying").exists() {
            return rebase_state(&dir);
        }
    }
    if let Some(merge_head) = read_oid(&git_dir.join("MERGE_HEAD")) {
        return RepoState::Merging { merge_head };
    }
    if git_dir.join("CHERRY_PICK_HEAD").exists() {
        return RepoState::CherryPicking {
            commit: read_oid(&git_dir.join("CHERRY_PICK_HEAD")),
        };
    }
    if git_dir.join("REVERT_HEAD").exists() {
        return RepoState::Reverting {
            commit: read_oid(&git_dir.join("REVERT_HEAD")),
        };
    }
    // A multi-commit cherry-pick or revert between steps has only the
    // sequencer directory; its next instruction names the operation.
    let todo = read_trimmed(&git_dir.join("sequencer").join("todo"));
    match todo.as_deref().and_then(|t| t.split_whitespace().next()) {
        Some("pick" | "p") => RepoState::CherryPicking { commit: None },
        Some("revert") => RepoState::Reverting { commit: None },
        _ => RepoState::Clean,
    }
}

fn rebase_state(dir: &Path) -> RepoState {
    let branch = read_trimmed(&dir.join("head-name"))
        .filter(|name| name != "detached HEAD")
        .map(|name| {
            name.strip_prefix("refs/heads/")
                .map_or(name.clone(), str::to_owned)
        });
    let number = |file: &str| read_trimmed(&dir.join(file)).and_then(|n| n.parse::<usize>().ok());
    // rebase-merge: msgnum/end; rebase-apply: next/last.
    let step = number("msgnum")
        .zip(number("end"))
        .or_else(|| number("next").zip(number("last")));
    RepoState::Rebasing {
        branch,
        onto: read_oid(&dir.join("onto")),
        step,
    }
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// First line of `path` if it is a hex object id.
fn read_oid(path: &Path) -> Option<Oid> {
    let text = read_trimmed(path)?;
    let first = text.lines().next()?.trim();
    (first.len() >= 40 && first.bytes().all(|b| b.is_ascii_hexdigit())).then(|| first.to_owned())
}

/// Path of the lock file named in a Git "Unable to create '….lock': File
/// exists" error, if `message` is one.
pub fn locked_file(message: &str) -> Option<&str> {
    let start = message.find("Unable to create '")? + "Unable to create '".len();
    let rest = &message[start..];
    let end = rest.find("': File exists")?;
    let path = &rest[..end];
    path.ends_with(".lock").then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_errors_are_recognized() {
        let stderr = "fatal: Unable to create '/r/.git/index.lock': File exists.\n\nAnother git process seems to be running";
        assert_eq!(locked_file(stderr), Some("/r/.git/index.lock"));
        assert_eq!(locked_file("fatal: pathspec 'x' did not match"), None);
    }
}
