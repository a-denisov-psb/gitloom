//! Git abstraction layer.
//!
//! The UI only talks to [`GitBackend`]. Concrete implementations:
//! - [`MockBackend`]: static sample data for UI development
//! - `GixBackend` (planned): read operations via gitoxide
//! - `CliBackend` (planned): write operations via the `git` executable

mod mock;

pub use mock::MockBackend;

use std::fmt;

/// Full 40-char hex object id.
pub type Oid = String;

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: Oid,
    pub parents: Vec<Oid>,
    pub summary: String,
    pub body: String,
    pub author: String,
    pub email: String,
    /// Unix timestamp in seconds (UTC).
    pub time: i64,
}

impl Commit {
    pub fn short_id(&self) -> &str {
        &self.id[..self.id.len().min(7)]
    }

    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
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
    pub target: Oid,
    /// True for the checked-out local branch.
    pub is_head: bool,
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
    pub index: usize,
    pub message: String,
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
    NotFound(String),
    Backend(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotARepository(path) => write!(f, "not a git repository: {path}"),
            Error::NotFound(what) => write!(f, "not found: {what}"),
            Error::Backend(msg) => write!(f, "git backend error: {msg}"),
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

    /// Commits in topological order, newest first.
    fn log(&self, limit: usize) -> Result<Vec<Commit>>;

    fn references(&self) -> Result<Vec<Reference>>;

    fn stashes(&self) -> Result<Vec<Stash>>;

    fn working_tree(&self) -> Result<WorkingTree>;

    /// Files changed by a commit compared to its first parent.
    fn commit_changes(&self, id: &str) -> Result<Vec<FileChange>>;
}
