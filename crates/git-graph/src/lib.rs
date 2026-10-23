//! Pure commit graph algorithms: topological ordering and lane layout.
//!
//! The crate knows nothing about Git object ids. The caller maps its commits
//! to positions in an input slice and passes plain [`GraphCommit`] values.

mod layout;
mod topo;

#[cfg(test)]
mod test_support;

pub use layout::{
    layout, layout_with, ColorId, LaneHint, LaneHints, Layout, LayoutOptions, Row, Segment, Wip,
};
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
    /// The row order is not a permutation of the input commits.
    InvalidOrder,
    /// The graph needs more lanes than fit into a `u16`.
    TooManyLanes,
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
            Self::InvalidOrder => {
                f.write_str("row order does not contain every commit exactly once")
            }
            Self::TooManyLanes => f.write_str("graph exceeds the supported number of lanes"),
        }
    }
}

impl std::error::Error for Error {}

/// Checks that the input fits into [`Id`] and that every id equals its position.
fn check_ids(commits: &[GraphCommit]) -> Result<(), Error> {
    if u32::try_from(commits.len()).is_err() {
        return Err(Error::TooManyCommits(commits.len()));
    }
    match commits
        .iter()
        .enumerate()
        .find(|(position, commit)| commit.id.index() != *position)
    {
        Some((position, commit)) => Err(Error::IdMismatch {
            position,
            id: commit.id,
        }),
        None => Ok(()),
    }
}
