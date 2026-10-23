//! Git abstraction layer.
//!
//! The UI only talks to [`GitBackend`]. Concrete implementations:
//! - [`GixBackend`]: read operations via gitoxide
//! - `MockBackend`: static sample data, only for tests and the `mock` feature
//! - [`CliBackend`]: write operations via the `git` executable

mod cli_backend;
mod diff;
mod discard;
mod gix_backend;
mod history;
#[cfg(any(test, feature = "mock"))]
mod mock;
mod network;
mod patch;
mod refname;
mod state;
mod watch;

pub use cli_backend::{
    branch_not_merged, checkout_blocked, detect_git, CliBackend, GitAvailability, GitOutput,
    GitVersion, GitWriter, RunOptions, ASKPASS_ENV, MIN_GIT_VERSION,
};
pub use discard::{
    discard_all, discard_everything, discard_files, discard_lines, revert_lines, DiscardSnapshot,
    FileSnapshot, IndexSnapshot,
};
pub use gix_backend::GixBackend;
pub use history::{head_commit, move_head, restore_snapshot};
#[cfg(any(test, feature = "mock"))]
pub use mock::MockBackend;
pub use network::{
    delete_remote_branch_args, parse_progress, push_args, push_rejection,
    rename_remote_branch_args, CancelToken, Progress, PullMode, PushRejection,
};
pub use patch::{build_patch, PatchDirection};
pub use refname::check_branch_name;
pub use state::{locked_file, RepoState};
pub use watch::{classify_git_path, ChangeSet};

use std::fmt;
use std::path::Path;

/// Full 40-char hex object id.
pub type Oid = String;

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: Oid,
    pub parents: Vec<Oid>,
    pub summary: String,
    pub body: String,
    /// Author name after applying the mailmap.
    pub author: String,
    /// Author email after applying the mailmap.
    pub email: String,
    /// Author time as Unix timestamp in seconds (UTC); used for display.
    pub author_time: i64,
    /// Committer time as Unix timestamp in seconds (UTC); used for ordering.
    pub commit_time: i64,
}

impl Commit {
    pub fn short_id(&self) -> &str {
        &self.id[..self.id.len().min(7)]
    }

    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// Result of [`GitBackend::log`].
#[derive(Debug, Clone, Default)]
pub struct Log {
    /// Loaded commits, newest commit time first (not topologically sorted).
    pub commits: Vec<Commit>,
    /// True if the history has more commits than the requested limit.
    pub truncated: bool,
}

/// State of `HEAD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Head {
    /// On a local branch (short name, e.g. `main`).
    Branch { name: String, target: Oid },
    /// Pointing directly at a commit.
    Detached { target: Oid },
    /// On a branch without commits yet (new repository).
    Unborn { name: String },
}

impl Head {
    /// Commit `HEAD` points to, if any.
    pub fn target(&self) -> Option<&str> {
        match self {
            Self::Branch { target, .. } | Self::Detached { target } => Some(target),
            Self::Unborn { .. } => None,
        }
    }

    /// Checked-out branch name, if any.
    pub fn branch(&self) -> Option<&str> {
        match self {
            Self::Branch { name, .. } | Self::Unborn { name } => Some(name),
            Self::Detached { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    LocalBranch,
    RemoteBranch,
    Tag,
}

#[derive(Debug, Clone)]
pub struct Reference {
    /// Short name, e.g. `main`, `origin/main`, `v1.0.0`.
    pub name: String,
    pub kind: RefKind,
    /// Commit the reference points to (tags are peeled).
    pub target: Oid,
    /// True for the checked-out local branch.
    pub is_head: bool,
    /// Upstream of a local branch as short remote branch name (`origin/main`).
    pub upstream: Option<String>,
    /// Commits `(ahead, behind)` compared to the upstream, if both exist.
    pub ahead_behind: Option<(usize, usize)>,
}

impl Reference {
    /// Remote name for remote branches (`origin` for `origin/main`).
    pub fn remote(&self) -> Option<&str> {
        match self.kind {
            RefKind::RemoteBranch => self.name.split('/').next(),
            _ => None,
        }
    }

    /// Name without the remote prefix (`main` for `origin/main`).
    pub fn local_name(&self) -> &str {
        match self.kind {
            RefKind::RemoteBranch => self
                .name
                .split_once('/')
                .map_or(self.name.as_str(), |(_, name)| name),
            _ => &self.name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Stash {
    /// Position in the stash list (`stash@{index}`), newest first.
    pub index: usize,
    pub message: String,
    /// Commit of the stash entry.
    pub id: Oid,
    /// Commit the stash was created on (first parent of the stash commit).
    pub base: Option<Oid>,
    /// Creation time as Unix timestamp in seconds.
    pub time: i64,
    pub author: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
    /// Unmerged entry after a failed merge (display only in M1).
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path relative to the repository root (new path for renames).
    pub path: String,
    pub kind: ChangeKind,
    /// Previous path of a renamed file.
    pub old_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkingTree {
    /// Index ↔ working tree changes, untracked files and conflicts, sorted by path.
    pub unstaged: Vec<FileChange>,
    /// HEAD ↔ index changes, sorted by path.
    pub staged: Vec<FileChange>,
}

impl WorkingTree {
    pub fn len(&self) -> usize {
        self.unstaged.len() + self.staged.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Which two versions of a file to compare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffKind {
    /// Commit against its first parent (root commits against the empty tree).
    Commit(Oid),
    /// HEAD against the index.
    Staged,
    /// Index against the working tree.
    Unstaged,
    /// Nothing against the working tree.
    Untracked,
    /// One part of a stash entry (see [`StashChanges`]).
    Stash { id: Oid, part: StashPart },
}

/// Part of a stash entry, like the sections of the working copy view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashPart {
    /// Index (second parent) against HEAD (first parent).
    Staged,
    /// Stashed working tree (the stash commit) against the index.
    Unstaged,
    /// Untracked files (third parent, `git stash -u`) against nothing.
    Untracked,
}

/// Everything a stash entry contains, split like the working copy view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StashChanges {
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    pub untracked: Vec<FileChange>,
}

impl StashChanges {
    /// Number of distinct paths over all parts.
    pub fn file_count(&self) -> usize {
        let mut paths: Vec<&str> = self
            .staged
            .iter()
            .chain(&self.unstaged)
            .chain(&self.untracked)
            .map(|f| f.path.as_str())
            .collect();
        paths.sort_unstable();
        paths.dedup();
        paths.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRequest {
    pub kind: DiffKind,
    /// Path relative to the repository root (new path for renames).
    pub path: String,
    /// Previous path of a renamed file.
    pub old_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Path on the old side, `None` if the file was added.
    pub old_path: Option<String>,
    /// Path on the new side, `None` if the file was deleted.
    pub new_path: Option<String>,
    /// At least one side contains binary data; `hunks` is empty.
    pub binary: bool,
    /// At least one side exceeds 2 MB; `hunks` is empty.
    pub too_large: bool,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// Unified diff header, e.g. `@@ -1,4 +1,5 @@`.
    pub header: String,
    /// First old line (1-based, 0 for an empty range).
    pub old_start: u32,
    pub old_count: u32,
    /// First new line (1-based, 0 for an empty range).
    pub new_start: u32,
    pub new_count: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// 1-based line number in the old version (`None` for added lines).
    pub old_line: Option<u32>,
    /// 1-based line number in the new version (`None` for removed lines).
    pub new_line: Option<u32>,
    /// Line text without the line terminator.
    pub content: String,
    /// Last line of its version without a trailing newline.
    pub missing_newline: bool,
    /// The line ended with `\r\n` (needed to rebuild exact patches).
    pub crlf: bool,
}

#[derive(Debug, Clone)]
pub enum Error {
    NotARepository(String),
    /// Bare repositories are not supported yet.
    BareRepository(String),
    NotFound(String),
    /// An object the repository refers to is not in the object database;
    /// in a partial clone `git` can fetch it from the promisor remote.
    MissingObject {
        id: Oid,
        partial_clone: bool,
    },
    Backend(String),
    /// The user cancelled a running command.
    Cancelled,
    /// A `git` command failed.
    Command {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotARepository(path) => write!(f, "Not a Git repository: {path}"),
            Error::BareRepository(path) => {
                write!(f, "Bare repositories are not supported yet: {path}")
            }
            Error::NotFound(what) => write!(f, "Not found: {what}"),
            Error::MissingObject {
                partial_clone: true,
                ..
            } => write!(f, "Content not available locally (partial clone)"),
            Error::MissingObject { id, .. } => {
                write!(f, "Object {id} is missing from the repository")
            }
            Error::Backend(msg) => write!(f, "Git error: {msg}"),
            Error::Cancelled => write!(f, "Cancelled"),
            Error::Command {
                command,
                code,
                stderr,
            } => {
                match code {
                    Some(code) => write!(f, "{command} failed (exit code {code})")?,
                    None => write!(f, "{command} was terminated")?,
                }
                if !stderr.is_empty() {
                    write!(f, ": {stderr}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Read side of a repository. Write operations (stage, commit, push, ...)
/// will live in a separate trait so read-only backends stay simple.
pub trait GitBackend: Send + Sync {
    /// Display name of the repository (usually the folder name).
    fn name(&self) -> &str;

    /// Root of the working tree.
    fn path(&self) -> &Path;

    /// Commits reachable from HEAD and all branches and tags, at most `limit`.
    fn log(&self, limit: usize) -> Result<Log>;

    fn head(&self) -> Result<Head>;

    /// Local branches, remote branches (without `*/HEAD`) and tags.
    fn references(&self) -> Result<Vec<Reference>>;

    /// Names of the configured remotes, sorted.
    fn remotes(&self) -> Result<Vec<String>>;

    fn stashes(&self) -> Result<Vec<Stash>>;

    fn working_tree(&self) -> Result<WorkingTree>;

    /// Files changed by a commit compared to its first parent, sorted by path.
    fn commit_changes(&self, id: &str) -> Result<Vec<FileChange>>;

    /// Staged, unstaged and untracked changes of the stash commit `id`.
    fn stash_changes(&self, id: &str) -> Result<StashChanges>;

    /// Line diff of one file.
    fn file_diff(&self, request: &DiffRequest) -> Result<FileDiff>;

    /// Content of the configured `commit.template`, if any.
    fn commit_template(&self) -> Result<Option<String>>;

    /// Operation in progress (merge, rebase, cherry-pick, revert).
    fn state(&self) -> Result<RepoState>;

    /// Prepared message (`MERGE_MSG`) of a merge, cherry-pick or revert in
    /// progress, if one is running.
    fn merge_message(&self) -> Result<Option<String>>;

    /// Working tree content of `path` as Git stores it (filters applied);
    /// `None` if the file does not exist.
    fn worktree_content(&self, path: &str) -> Result<Option<Vec<u8>>>;
}
