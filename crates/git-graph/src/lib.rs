//! Pure commit graph algorithms: topological ordering and (from M1-03) lane layout.
//!
//! The crate knows nothing about Git object ids. The caller maps its commits
//! to positions in an input slice and passes plain [`GraphCommit`] values.

mod topo;

pub use topo::{topo_order, TopoOrder};

use std::fmt;

/// Position of a commit in the input slice.
///
/// Parent ids that are not a valid position (`>= input.len()`) denote parents
/// missing from the input, e.g. in truncated or shallow histories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub u32);

impl Id {
    /// The id as slice index.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A commit as seen by the graph algorithms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCommit {
    /// Must equal the commit's position in the input slice.
    pub id: Id,
    /// Parent ids in Git order (first parent first).
    pub parents: Vec<Id>,
    /// Commit time as Unix timestamp in seconds.
    pub time: i64,
}

/// Invalid input passed to a graph function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// `commits[position].id` does not equal `position`.
    IdMismatch { position: usize, id: Id },
    /// More commits than fit into an [`Id`].
    TooManyCommits(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdMismatch { position, id } => {
                write!(f, "commit at position {position} has id {}", id.0)
            }
            Self::TooManyCommits(count) => {
                write!(f, "{count} commits exceed the supported maximum")
            }
        }
    }
}

impl std::error::Error for Error {}
