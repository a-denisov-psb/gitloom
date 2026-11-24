//! Which file the external editor opens for a file of a commit (M19-02,
//! ADR-061): the real working tree file when the commit is HEAD and the
//! file is unchanged, otherwise a read-only copy of the commit's version.
//! Runs Git and file I/O: call it in a background task.

use std::path::{Path, PathBuf};

use git_core::{CommitFile, GitWriter};

/// Reason of the disabled entry for a submodule (ADR-061).
pub const SUBMODULE: &str = "A submodule has no file content";

/// Title of the info toast for a copy.
pub const COPY_TITLE: &str = "Opened a read-only copy";

/// A file of a commit to open in the external editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitFileRef {
    /// Full id of the commit.
    pub commit: String,
    /// Path in that commit (`/` separated).
    pub path: String,
    /// Deleted in `commit`: the version of the first parent opens.
    pub deleted: bool,
    /// The part of a stash the file is listed in, when `commit` is a stash
    /// (finding #55): it decides which version opens.
    pub stash: Option<git_core::StashPart>,
}

/// What opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// The working tree file itself (editable, no notice).
    Real(PathBuf),
    /// A read-only copy and the text of the info toast.
    Copy { path: PathBuf, text: String },
}

impl Opened {
    pub fn path(&self) -> &Path {
        match self {
            Self::Real(path) | Self::Copy { path, .. } => path,
        }
    }
}

/// Why nothing opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Refused for a reason (warning toast).
    Refused(String),
    /// Reading or writing failed (error toast): plain text and the raw
    /// error for "Details".
    Failed { text: String, error: String },
}

const READ_FAILED: &str = "Git could not read the file of the commit.";
const WRITE_FAILED: &str = "The read-only copy could not be written to the temp folder.";

fn read_failed(error: git_core::Error) -> Problem {
    Problem::Failed {
        text: READ_FAILED.into(),
        error: error.to_string(),
    }
}

impl Problem {
    /// The toast for this problem (title "Could not open <file name>").
    pub fn into_launch_error(self, path: &str) -> crate::launch::LaunchError {
        let name = path.rsplit('/').next().unwrap_or(path);
        let title = format!("Could not open {name}");
        match self {
            Self::Refused(text) => {
                crate::launch::LaunchError::Refused(crate::launch::Notice::new(title, text))
            }
            Self::Failed { text, error } => {
                crate::launch::LaunchError::Failed { title, text, error }
            }
        }
    }
}

fn short(id: &str) -> &str {
    &id[..id.len().min(7)]
}

/// Reads `path` at `rev` for a copy; `missing` is the refusal when the
/// file is not there.
fn read_content(
    git: &dyn GitWriter,
    rev: &str,
    path: &str,
    missing: impl FnOnce() -> String,
) -> Result<Vec<u8>, Problem> {
    match git_core::file_at_commit(git, rev, path) {
        Ok(CommitFile::Content(bytes)) => Ok(bytes),
        Ok(CommitFile::Missing) => Err(Problem::Refused(missing())),
        Ok(CommitFile::Folder) => Err(Problem::Refused("A folder has no file content".into())),
        Ok(CommitFile::Submodule) => Err(Problem::Refused(SUBMODULE.into())),
        Err(error) => Err(read_failed(error)),
    }
}

/// Writes the copy and builds the notice.
fn copy(path: PathBuf, bytes: &[u8], text: String) -> Result<Opened, Problem> {
    crate::history_copy::write_copy(&path, bytes).map_err(|error| Problem::Failed {
        text: WRITE_FAILED.into(),
        error,
    })?;
    Ok(Opened::Copy { path, text })
}

/// A file of a stash (finding #55): the right side of the diff the user
/// sees, always a copy under `<stash id>/<part>/`. Messages name the stash
/// and its own short id, never "commit" or a parent's id.
fn resolve_stash(
    git: &dyn GitWriter,
    base: &Path,
    file: &CommitFileRef,
    part: git_core::StashPart,
) -> Result<Opened, Problem> {
    use git_core::StashPart;
    let sha = short(&file.commit);
    let (rev, folder) = match part {
        StashPart::Untracked => (format!("{}^3", file.commit), "untracked"),
        StashPart::Staged if file.deleted => (format!("{}^1", file.commit), "staged"),
        StashPart::Unstaged if file.deleted => (format!("{}^1", file.commit), "unstaged"),
        StashPart::Staged => (format!("{}^2", file.commit), "staged"),
        StashPart::Unstaged => (file.commit.clone(), "unstaged"),
    };
    let not_in_stash = || format!("The file is not in stash {sha}");
    let Some(id) = git_core::commit_id(git, &rev).map_err(read_failed)? else {
        return Err(Problem::Refused(if file.deleted {
            format!("Stash {sha} has no base commit")
        } else {
            not_in_stash()
        }));
    };
    let bytes = read_content(git, &id, &file.path, not_in_stash)?;
    let path = crate::history_copy::copy_path_in(base, &file.commit, Some(folder), &file.path)
        .map_err(Problem::Refused)?;
    let text = if file.deleted {
        format!(
            "{} as it was before stash {sha} deleted it. Changes are not saved to the repository.",
            file.path
        )
    } else {
        format!(
            "{} as of stash {sha}. Changes are not saved to the repository.",
            file.path
        )
    };
    copy(path, &bytes, text)
}

/// Resolves `file` to the path the editor gets. Copies go under `base`.
pub fn resolve(
    git: &dyn GitWriter,
    root: &Path,
    base: &Path,
    file: &CommitFileRef,
) -> Result<Opened, Problem> {
    if let Some(part) = file.stash {
        return resolve_stash(git, base, file, part);
    }
    let sha = short(&file.commit);
    let id = if file.deleted {
        git_core::commit_id(git, &format!("{}^1", file.commit))
            .map_err(read_failed)?
            .ok_or_else(|| Problem::Refused(format!("Commit {sha} has no parent")))?
    } else {
        file.commit.clone()
    };
    let bytes = read_content(git, &id, &file.path, || {
        format!("The file is not in commit {}", short(&id))
    })?;
    if !file.deleted {
        let head = git_core::head_commit(git).map_err(read_failed)?;
        if head.as_deref() == Some(id.as_str()) {
            let real = crate::launch::repo_file_path(root, &file.path);
            if std::fs::read(&real).is_ok_and(|current| current == bytes) {
                return Ok(Opened::Real(real));
            }
        }
    }
    let path = crate::history_copy::copy_path(base, &id, &file.path).map_err(Problem::Refused)?;
    let text = if file.deleted {
        format!(
            "{} as it was before {sha} deleted it. Changes are not saved to the repository.",
            file.path
        )
    } else {
        format!(
            "{} as of {sha}. Changes are not saved to the repository.",
            file.path
        )
    };
    copy(path, &bytes, text)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use git_core::CliBackend;

    struct Repo {
        _dir: tempfile::TempDir,
        root: PathBuf,
        base: PathBuf,
        cli: CliBackend,
    }

    impl Repo {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("repo");
            let base = dir.path().join("view");
            std::fs::create_dir_all(&root).unwrap();
            let repo = Self {
                cli: CliBackend::new(&root),
                _dir: dir,
                root,
                base,
            };
            repo.git(&["init", "--quiet", "-b", "main"]);
            // The checkout filters follow the repository, not the runner's
            // global config (Windows runners set `core.autocrlf=true`).
            repo.git(&["config", "core.autocrlf", "false"]);
            repo
        }

        fn git(&self, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
                .args(["-c", "commit.gpgSign=false"])
                .args(args)
                .current_dir(&self.root)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }

        fn write(&self, path: &str, text: &str) {
            let file = self.root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }

        fn commit(&self, path: &str, text: &str) -> String {
            self.write(path, text);
            self.git(&["add", "--", path]);
            self.git(&["commit", "--quiet", "-m", path]);
            self.git(&["rev-parse", "HEAD"])
        }

        fn open(&self, commit: &str, path: &str, deleted: bool) -> Result<Opened, Problem> {
            resolve(
                &self.cli,
                &self.root,
                &self.base,
                &CommitFileRef {
                    commit: commit.into(),
                    path: path.into(),
                    deleted,
                    stash: None,
                },
            )
        }
    }

    fn copy_of(opened: Result<Opened, Problem>) -> (PathBuf, String) {
        match opened {
            Ok(Opened::Copy { path, text }) => (path, text),
            other => panic!("expected a copy, got {other:?}"),
        }
    }

    #[test]
    fn older_commit_opens_a_read_only_copy() {
        let repo = Repo::new();
        let old = repo.commit("src/a.rs", "old\n");
        repo.commit("src/a.rs", "new\n");
        let (path, text) = copy_of(repo.open(&old, "src/a.rs", false));
        assert!(path.starts_with(&repo.base));
        assert_eq!(path.file_name().unwrap(), "a.rs");
        assert!(path.parent().unwrap().ends_with("src"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "old\n");
        assert!(std::fs::metadata(&path).unwrap().permissions().readonly());
        assert_eq!(
            text,
            format!(
                "src/a.rs as of {}. Changes are not saved to the repository.",
                &old[..7]
            )
        );
    }

    #[test]
    fn older_commit_with_the_same_content_is_still_a_copy() {
        let repo = Repo::new();
        let old = repo.commit("a.txt", "same\n");
        repo.commit("b.txt", "other\n");
        copy_of(repo.open(&old, "a.txt", false));
    }

    #[test]
    fn file_of_a_branch_that_is_not_checked_out() {
        // Finding #53.
        let repo = Repo::new();
        repo.commit("base.txt", "base\n");
        repo.git(&["branch", "feature-b"]);
        repo.git(&["switch", "--quiet", "-c", "feature-a"]);
        let added = repo.commit("README.md", "# A\n");
        repo.git(&["switch", "--quiet", "feature-b"]);
        assert!(!repo.root.join("README.md").exists());
        let (path, _) = copy_of(repo.open(&added, "README.md", false));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "# A\n");
    }

    #[test]
    fn head_with_an_unchanged_file_opens_the_real_file() {
        let repo = Repo::new();
        let head = repo.commit("dir/a.txt", "a\n");
        assert_eq!(
            repo.open(&head, "dir/a.txt", false),
            Ok(Opened::Real(crate::launch::repo_file_path(
                &repo.root,
                "dir/a.txt"
            )))
        );
        assert!(!repo.base.exists());
        // Modified in the working tree: the committed version as a copy.
        repo.write("dir/a.txt", "changed\n");
        let (path, _) = copy_of(repo.open(&head, "dir/a.txt", false));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "a\n");
    }

    #[test]
    fn deleted_file_opens_the_parent_version() {
        let repo = Repo::new();
        let parent = repo.commit("a.txt", "before\n");
        repo.git(&["rm", "--quiet", "a.txt"]);
        repo.git(&["commit", "--quiet", "-m", "delete"]);
        let deleting = repo.git(&["rev-parse", "HEAD"]);
        let (path, text) = copy_of(repo.open(&deleting, "a.txt", true));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "before\n");
        assert!(path.starts_with(repo.base.join(&parent[..12])));
        assert_eq!(
            text,
            format!(
                "a.txt as it was before {} deleted it. Changes are not saved to the repository.",
                &deleting[..7]
            )
        );
    }

    #[test]
    fn renamed_file_opens_under_its_path_in_the_commit() {
        let repo = Repo::new();
        repo.commit("old.txt", "text\n");
        repo.git(&["mv", "old.txt", "new.txt"]);
        repo.git(&["commit", "--quiet", "-m", "rename"]);
        let renamed = repo.git(&["rev-parse", "HEAD"]);
        repo.commit("new.txt", "text\nmore\n");
        let (path, _) = copy_of(repo.open(&renamed, "new.txt", false));
        assert_eq!(path.file_name().unwrap(), "new.txt");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "text\n");
    }

    #[test]
    fn missing_file_and_root_deletion_are_refused() {
        let repo = Repo::new();
        let root = repo.commit("a.txt", "a\n");
        assert_eq!(
            repo.open(&root, "b.txt", false),
            Err(Problem::Refused(format!(
                "The file is not in commit {}",
                &root[..7]
            )))
        );
        assert_eq!(
            repo.open(&root, "a.txt", true),
            Err(Problem::Refused(format!(
                "Commit {} has no parent",
                &root[..7]
            )))
        );
    }

    impl Repo {
        fn open_stash(
            &self,
            stash: &str,
            path: &str,
            part: git_core::StashPart,
            deleted: bool,
        ) -> Result<Opened, Problem> {
            resolve(
                &self.cli,
                &self.root,
                &self.base,
                &CommitFileRef {
                    commit: stash.into(),
                    path: path.into(),
                    deleted,
                    stash: Some(part),
                },
            )
        }

        /// Stashes everything (untracked files too); returns the stash id.
        fn stash(&self) -> String {
            self.git(&["stash", "push", "--quiet", "--include-untracked"]);
            self.git(&["rev-parse", "stash@{0}"])
        }
    }

    /// Finding #55: texts of stash files name the stash and its own short
    /// id, never "commit" and never a parent's id.
    fn assert_stash_text(repo: &Repo, text: &str, stash: &str) {
        assert!(text.contains(&format!("stash {}", &stash[..7])), "{text}");
        assert!(!text.to_lowercase().contains("commit"), "{text}");
        for parent in ["^1", "^2", "^3"] {
            if let Ok(id) = std::process::Command::new("git")
                .args([
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{stash}{parent}"),
                ])
                .current_dir(&repo.root)
                .output()
            {
                let id = String::from_utf8_lossy(&id.stdout).trim().to_owned();
                if id.len() >= 7 {
                    assert!(!text.contains(&id[..7]), "{text} names {parent}");
                }
            }
        }
    }

    #[test]
    fn untracked_file_of_a_stash_opens_from_the_third_parent() {
        use git_core::StashPart;
        let repo = Repo::new();
        repo.commit("a.txt", "a\n");
        repo.write("asdf", "new file\n");
        let stash = repo.stash();
        assert!(!repo.root.join("asdf").exists());
        let (path, text) = copy_of(repo.open_stash(&stash, "asdf", StashPart::Untracked, false));
        assert_eq!(
            path,
            repo.base.join(&stash[..12]).join("untracked").join("asdf")
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new file\n");
        assert!(std::fs::metadata(&path).unwrap().permissions().readonly());
        assert_eq!(
            text,
            format!(
                "asdf as of stash {}. Changes are not saved to the repository.",
                &stash[..7]
            )
        );
        assert_stash_text(&repo, &text, &stash);
    }

    #[test]
    fn staged_and_unstaged_parts_open_their_own_versions() {
        use git_core::StashPart;
        let repo = Repo::new();
        repo.commit("a.txt", "1\n");
        repo.write("a.txt", "2\n");
        repo.git(&["add", "a.txt"]);
        repo.write("a.txt", "3\n");
        let stash = repo.stash();
        let (staged, staged_text) =
            copy_of(repo.open_stash(&stash, "a.txt", StashPart::Staged, false));
        let (unstaged, unstaged_text) =
            copy_of(repo.open_stash(&stash, "a.txt", StashPart::Unstaged, false));
        assert_ne!(staged, unstaged);
        assert!(staged.ends_with("staged/a.txt") || staged.ends_with("staged\\a.txt"));
        assert_eq!(std::fs::read_to_string(&staged).unwrap(), "2\n");
        assert_eq!(std::fs::read_to_string(&unstaged).unwrap(), "3\n");
        assert_stash_text(&repo, &staged_text, &stash);
        assert_stash_text(&repo, &unstaged_text, &stash);
    }

    #[test]
    fn only_unstaged_or_only_staged_changes() {
        use git_core::StashPart;
        let repo = Repo::new();
        repo.commit("a.txt", "1\n");
        repo.write("a.txt", "worktree\n");
        let stash = repo.stash();
        let (path, _) = copy_of(repo.open_stash(&stash, "a.txt", StashPart::Unstaged, false));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "worktree\n");

        repo.write("a.txt", "index\n");
        repo.git(&["add", "a.txt"]);
        let stash = repo.stash();
        let (path, _) = copy_of(repo.open_stash(&stash, "a.txt", StashPart::Staged, false));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "index\n");
    }

    #[test]
    fn deleted_files_of_a_stash_open_the_base_version() {
        use git_core::StashPart;
        let repo = Repo::new();
        repo.commit("gone.txt", "unstaged base\n");
        repo.commit("rm.txt", "staged base\n");
        std::fs::remove_file(repo.root.join("gone.txt")).unwrap();
        repo.git(&["rm", "--quiet", "rm.txt"]);
        let stash = repo.stash();
        for (path, part, content) in [
            ("gone.txt", StashPart::Unstaged, "unstaged base\n"),
            ("rm.txt", StashPart::Staged, "staged base\n"),
        ] {
            let (copy, text) = copy_of(repo.open_stash(&stash, path, part, true));
            assert_eq!(std::fs::read_to_string(&copy).unwrap(), content);
            assert!(copy.starts_with(repo.base.join(&stash[..12])));
            assert_eq!(
                text,
                format!(
                    "{path} as it was before stash {} deleted it. Changes are not saved to the repository.",
                    &stash[..7]
                )
            );
            assert_stash_text(&repo, &text, &stash);
        }
    }

    #[test]
    fn a_stash_file_is_never_the_real_file() {
        use git_core::StashPart;
        let repo = Repo::new();
        repo.commit("a.txt", "1\n");
        repo.write("a.txt", "2\n");
        let stash = repo.stash();
        // The working tree gets the stashed content again.
        repo.write("a.txt", "2\n");
        copy_of(repo.open_stash(&stash, "a.txt", StashPart::Unstaged, false));
        // The base is HEAD and equals the working tree.
        repo.write("a.txt", "1\n");
        copy_of(repo.open_stash(&stash, "a.txt", StashPart::Unstaged, true));
    }

    #[test]
    fn refusals_name_the_stash() {
        use git_core::StashPart;
        let repo = Repo::new();
        repo.commit("a.txt", "1\n");
        repo.write("a.txt", "2\n");
        let stash = repo.stash();
        for part in [StashPart::Staged, StashPart::Unstaged, StashPart::Untracked] {
            match repo.open_stash(&stash, "missing.txt", part, false) {
                Err(Problem::Refused(text)) => {
                    assert_eq!(text, format!("The file is not in stash {}", &stash[..7]));
                    assert_stash_text(&repo, &text, &stash);
                }
                other => panic!("{part:?}: {other:?}"),
            }
        }
    }

    #[test]
    fn editor_gets_the_copy_path() {
        let repo = Repo::new();
        let old = repo.commit("a.txt", "a\n");
        repo.commit("a.txt", "b\n");
        let opened = repo.open(&old, "a.txt", false).unwrap();
        let launch = crate::launch::editor(
            crate::launch::Platform::Linux,
            &crate::launch::SystemLookup,
            "code {path}",
            opened.path(),
        )
        .unwrap();
        assert_eq!(launch.program, "code");
        assert_eq!(launch.args, [opened.path().to_string_lossy().into_owned()]);
    }
}
