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

/// Resolves `file` to the path the editor gets. Copies go under `base`.
pub fn resolve(
    git: &dyn GitWriter,
    root: &Path,
    base: &Path,
    file: &CommitFileRef,
) -> Result<Opened, Problem> {
    let sha = short(&file.commit);
    let id = if file.deleted {
        git_core::commit_id(git, &format!("{}^1", file.commit))
            .map_err(read_failed)?
            .ok_or_else(|| Problem::Refused(format!("Commit {sha} has no parent")))?
    } else {
        file.commit.clone()
    };
    let bytes = match git_core::file_at_commit(git, &id, &file.path) {
        Ok(CommitFile::Content(bytes)) => bytes,
        Ok(CommitFile::Missing) => {
            return Err(Problem::Refused(format!(
                "The file is not in commit {}",
                short(&id)
            )))
        }
        Ok(CommitFile::Folder) => {
            return Err(Problem::Refused("A folder has no file content".into()))
        }
        Ok(CommitFile::Submodule) => return Err(Problem::Refused(SUBMODULE.into())),
        Err(error) => return Err(read_failed(error)),
    };
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
    crate::history_copy::write_copy(&path, &bytes).map_err(|error| Problem::Failed {
        text: WRITE_FAILED.into(),
        error,
    })?;
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
    Ok(Opened::Copy { path, text })
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
