//! Git abstraction layer.
//!
//! The UI only talks to [`GitBackend`]. Concrete implementations:
//! - [`GixBackend`]: read operations via gitoxide
//! - `MockBackend`: static sample data, only for tests and the `mock` feature
//! - `CliBackend` (planned, M2): write operations via the `git` executable

mod gix_backend;
#[cfg(any(test, feature = "mock"))]
mod mock;

pub use gix_backend::GixBackend;
#[cfg(any(test, feature = "mock"))]
pub use mock::MockBackend;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, Default)]
pub struct WorkingTree {
    pub unstaged: Vec<FileChange>,
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

#[derive(Debug, Clone)]
pub enum Error {
    NotARepository(String),
    /// Bare repositories are not supported yet.
    BareRepository(String),
    NotFound(String),
    Backend(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotARepository(path) => write!(f, "Not a Git repository: {path}"),
            Error::BareRepository(path) => {
                write!(f, "Bare repositories are not supported yet: {path}")
            }
            Error::NotFound(what) => write!(f, "Not found: {what}"),
            Error::Backend(msg) => write!(f, "Git error: {msg}"),
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

    fn stashes(&self) -> Result<Vec<Stash>>;

    fn working_tree(&self) -> Result<WorkingTree>;

    /// Files changed by a commit compared to its first parent.
    fn commit_changes(&self, id: &str) -> Result<Vec<FileChange>>;
}
