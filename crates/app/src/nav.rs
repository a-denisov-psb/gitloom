//! Keyboard navigation in the graph (rows: optional WIP row, then commits).

use crate::repo_view::{selection_row, Selection};
use crate::snapshot::RepoSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// One row down (older).
    Next,
    /// One row up (newer).
    Previous,
    /// First parent of the selected commit (WIP → HEAD).
    FirstParent,
    /// Child on the same branch: a commit whose first parent is the selection.
    Child,
    /// Top row.
    First,
    /// Bottom row.
    Last,
}

/// Selection after `step`, or `None` if it cannot move.
pub fn navigate(
    snapshot: &RepoSnapshot,
    current: Option<&Selection>,
    step: Move,
) -> Option<Selection> {
    let rows = snapshot.layout.rows.len();
    if rows == 0 {
        return None;
    }
    let row = selection_row(current, snapshot);
    let target = match (step, row) {
        (Move::First, _) | (Move::Next | Move::Previous, None) => Some(0),
        (Move::Last, _) => Some(rows - 1),
        (Move::Next, Some(row)) => Some((row + 1).min(rows - 1)),
        (Move::Previous, Some(row)) => Some(row.saturating_sub(1)),
        (Move::FirstParent, Some(row)) => first_parent_row(snapshot, row),
        (Move::Child, Some(row)) => child_row(snapshot, row),
        (Move::FirstParent | Move::Child, None) => None,
    }?;
    selection_at(snapshot, target)
}

fn selection_at(snapshot: &RepoSnapshot, row: usize) -> Option<Selection> {
    if snapshot.has_wip && row == 0 {
        return Some(Selection::Wip);
    }
    let index = row - usize::from(snapshot.has_wip);
    snapshot
        .commits
        .get(index)
        .map(|c| Selection::Commit(c.id.clone()))
}

fn commit_at(snapshot: &RepoSnapshot, row: usize) -> Option<usize> {
    if snapshot.has_wip && row == 0 {
        None
    } else {
        Some(row - usize::from(snapshot.has_wip))
    }
}

fn first_parent_row(snapshot: &RepoSnapshot, row: usize) -> Option<usize> {
    let parent = match commit_at(snapshot, row) {
        // The WIP row's parent is HEAD.
        None => snapshot.head.target()?,
        Some(index) => snapshot.commits.get(index)?.parents.first()?.as_str(),
    };
    snapshot.index.get(parent).map(|&i| snapshot.row_of(i))
}

/// Children appear above their parent; prefer one drawn in the same lane.
fn child_row(snapshot: &RepoSnapshot, row: usize) -> Option<usize> {
    let index = commit_at(snapshot, row)?;
    let id = &snapshot.commits.get(index)?.id;
    let lane = snapshot.layout.rows.get(row).map(|r| r.lane);

    let mut nearest = None;
    for child in (0..index).rev() {
        let commit = &snapshot.commits[child];
        if commit.parents.first() == Some(id) {
            let child_row = snapshot.row_of(child);
            if snapshot.layout.rows.get(child_row).map(|r| r.lane) == lane {
                return Some(child_row);
            }
            nearest.get_or_insert(child_row);
        }
    }
    if nearest.is_none() && snapshot.has_wip && snapshot.head.target() == Some(id.as_str()) {
        return Some(0);
    }
    nearest
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::{ChangeKind, Commit, FileChange, Head, Log, WorkingTree};

    /// `spec`: (id, parents) newest first; HEAD is `head`.
    fn snapshot(spec: &[(&str, &[&str])], head: &str, dirty: bool) -> RepoSnapshot {
        let commits = spec
            .iter()
            .enumerate()
            .map(|(i, (id, parents))| Commit {
                id: id.to_string(),
                parents: parents.iter().map(|p| p.to_string()).collect(),
                summary: String::new(),
                body: String::new(),
                author: String::new(),
                email: String::new(),
                author_time: 1000 - i as i64,
                commit_time: 1000 - i as i64,
            })
            .collect();
        let working_tree = WorkingTree {
            unstaged: dirty
                .then(|| FileChange {
                    path: "f".into(),
                    kind: ChangeKind::Modified,
                    old_path: None,
                })
                .into_iter()
                .collect(),
            staged: Vec::new(),
        };
        RepoSnapshot::build(
            Log {
                commits,
                truncated: false,
            },
            Head::Detached {
                target: head.into(),
            },
            Vec::new(),
            Vec::new(),
            working_tree,
        )
        .unwrap()
    }

    fn commit(id: &str) -> Selection {
        Selection::Commit(id.into())
    }

    /// m = merge(d, f); f on a side branch; d, b on main; a root.
    fn merge_history(dirty: bool) -> RepoSnapshot {
        snapshot(
            &[
                ("m", &["d", "f"]),
                ("f", &["b"]),
                ("d", &["b"]),
                ("b", &["a"]),
                ("a", &[]),
            ],
            "m",
            dirty,
        )
    }

    #[test]
    fn next_and_previous_move_one_row_and_stop_at_the_ends() {
        let snap = merge_history(false);
        assert_eq!(
            navigate(&snap, Some(&commit("m")), Move::Next),
            Some(commit("f"))
        );
        assert_eq!(
            navigate(&snap, Some(&commit("f")), Move::Previous),
            Some(commit("m"))
        );
        assert_eq!(
            navigate(&snap, Some(&commit("m")), Move::Previous),
            Some(commit("m"))
        );
        assert_eq!(
            navigate(&snap, Some(&commit("a")), Move::Next),
            Some(commit("a"))
        );
    }

    #[test]
    fn no_selection_starts_at_the_top() {
        let snap = merge_history(true);
        assert_eq!(navigate(&snap, None, Move::Next), Some(Selection::Wip));
        assert_eq!(navigate(&snap, None, Move::FirstParent), None);
    }

    #[test]
    fn wip_row_is_part_of_the_list() {
        let snap = merge_history(true);
        assert_eq!(
            navigate(&snap, Some(&Selection::Wip), Move::Next),
            Some(commit("m"))
        );
        assert_eq!(
            navigate(&snap, Some(&commit("m")), Move::Previous),
            Some(Selection::Wip)
        );
    }

    #[test]
    fn first_and_last() {
        let snap = merge_history(true);
        assert_eq!(
            navigate(&snap, Some(&commit("b")), Move::First),
            Some(Selection::Wip)
        );
        assert_eq!(
            navigate(&snap, Some(&commit("b")), Move::Last),
            Some(commit("a"))
        );
    }

    #[test]
    fn first_parent_follows_the_branch() {
        let snap = merge_history(true);
        assert_eq!(
            navigate(&snap, Some(&Selection::Wip), Move::FirstParent),
            Some(commit("m"))
        );
        assert_eq!(
            navigate(&snap, Some(&commit("m")), Move::FirstParent),
            Some(commit("d"))
        );
        assert_eq!(
            navigate(&snap, Some(&commit("d")), Move::FirstParent),
            Some(commit("b"))
        );
        assert_eq!(navigate(&snap, Some(&commit("a")), Move::FirstParent), None);
    }

    #[test]
    fn child_prefers_the_same_lane() {
        let snap = merge_history(true);
        // b has two first-parent children: f (side lane) and d (main lane).
        assert_eq!(
            navigate(&snap, Some(&commit("b")), Move::Child),
            Some(commit("d"))
        );
        assert_eq!(
            navigate(&snap, Some(&commit("d")), Move::Child),
            Some(commit("m"))
        );
        // f is only the second parent of m: no child on its branch.
        assert_eq!(navigate(&snap, Some(&commit("f")), Move::Child), None);
        // HEAD's child is the WIP row.
        assert_eq!(
            navigate(&snap, Some(&commit("m")), Move::Child),
            Some(Selection::Wip)
        );
        assert_eq!(navigate(&snap, Some(&Selection::Wip), Move::Child), None);
    }

    #[test]
    fn missing_parent_cannot_be_reached() {
        let snap = snapshot(&[("c", &["outside"])], "c", false);
        assert_eq!(navigate(&snap, Some(&commit("c")), Move::FirstParent), None);
    }

    #[test]
    fn empty_repository() {
        let snap = snapshot(&[], "", false);
        assert_eq!(navigate(&snap, None, Move::Next), None);
    }
}
