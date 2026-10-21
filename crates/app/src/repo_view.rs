//! State of one loaded repository tab.

use std::collections::HashSet;
use std::sync::Arc;

use git_core::{DiffRequest, FileChange, GitBackend};

use crate::snapshot::RepoSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Local,
    Remote,
    Tags,
    Stashes,
    Submodules,
}

/// Selected graph row, by commit id so it survives refreshes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Wip,
    Commit(String),
}

/// Data loaded in the background.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loadable<T> {
    Loading,
    Ready(T),
    Failed(String),
}

/// Changed files of the selected commit.
#[derive(Debug, Clone)]
pub struct CommitDetails {
    pub id: String,
    pub changes: Loadable<Vec<FileChange>>,
}

#[derive(Debug, Clone)]
pub struct DiffView {
    pub request: DiffRequest,
}

#[derive(Debug, Clone)]
pub enum CenterView {
    Graph,
    Diff(Box<DiffView>),
}

pub struct RepoView {
    pub backend: Arc<dyn GitBackend>,
    pub snapshot: Arc<RepoSnapshot>,
    pub selection: Option<Selection>,
    pub details: Option<CommitDetails>,
    pub center: CenterView,
    pub filter: String,
    pub collapsed: HashSet<Section>,
    /// A refresh is running.
    pub refreshing: bool,
    /// Last refresh failed; the previous snapshot stays visible.
    pub error: Option<String>,
    /// Empty, read-only commit description until committing arrives (M2).
    pub description: iced::widget::text_editor::Content,
}

impl RepoView {
    pub fn new(backend: Arc<dyn GitBackend>, snapshot: Arc<RepoSnapshot>) -> Self {
        let selection = restore_selection(None, &snapshot);
        Self {
            backend,
            snapshot,
            selection,
            details: None,
            center: CenterView::Graph,
            filter: String::new(),
            collapsed: HashSet::from([Section::Submodules]),
            refreshing: false,
            error: None,
            description: iced::widget::text_editor::Content::new(),
        }
    }

    pub fn name(&self) -> &str {
        self.backend.name()
    }

    /// Replaces the snapshot after a refresh and keeps the selection if possible.
    pub fn apply_snapshot(&mut self, snapshot: Arc<RepoSnapshot>) {
        self.selection = restore_selection(self.selection.as_ref(), &snapshot);
        if let Some(details) = &self.details {
            if snapshot.commit(&details.id).is_none() {
                self.details = None;
            }
        }
        self.snapshot = snapshot;
        self.refreshing = false;
        self.error = None;
    }

    /// Selected commit id, if a commit (not WIP) is selected.
    pub fn selected_commit(&self) -> Option<&str> {
        match &self.selection {
            Some(Selection::Commit(id)) => Some(id),
            _ => None,
        }
    }
}

/// Graph row of a selection, if it is visible in the snapshot.
pub fn selection_row(selection: Option<&Selection>, snapshot: &RepoSnapshot) -> Option<usize> {
    match selection? {
        Selection::Wip => snapshot.has_wip.then_some(0),
        Selection::Commit(id) => snapshot.index.get(id).map(|&i| snapshot.row_of(i)),
    }
}

/// Keeps `previous` if it still exists in `snapshot`; otherwise falls back
/// to the WIP row (if there are changes) or the first commit.
pub fn restore_selection(
    previous: Option<&Selection>,
    snapshot: &RepoSnapshot,
) -> Option<Selection> {
    if let Some(previous) = previous {
        if selection_row(Some(previous), snapshot).is_some() {
            return Some(previous.clone());
        }
    }
    if snapshot.has_wip {
        return Some(Selection::Wip);
    }
    snapshot
        .commits
        .first()
        .map(|c| Selection::Commit(c.id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::{ChangeKind, Commit, Head, Log, WorkingTree};

    fn snapshot(ids: &[&str], dirty: bool) -> RepoSnapshot {
        let commits = ids
            .iter()
            .enumerate()
            .map(|(i, id)| Commit {
                id: id.to_string(),
                parents: ids.get(i + 1).map(|p| p.to_string()).into_iter().collect(),
                summary: String::new(),
                body: String::new(),
                author: String::new(),
                email: String::new(),
                author_time: 100 - i as i64,
                commit_time: 100 - i as i64,
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
        let head = match ids.first() {
            Some(id) => Head::Detached {
                target: id.to_string(),
            },
            None => Head::Unborn {
                name: "main".into(),
            },
        };
        RepoSnapshot::build(
            Log {
                commits,
                truncated: false,
            },
            head,
            Vec::new(),
            Vec::new(),
            working_tree,
        )
        .unwrap()
    }

    fn commit(id: &str) -> Selection {
        Selection::Commit(id.into())
    }

    #[test]
    fn keeps_existing_commit() {
        let snap = snapshot(&["c", "b", "a"], true);
        assert_eq!(
            restore_selection(Some(&commit("b")), &snap),
            Some(commit("b"))
        );
    }

    #[test]
    fn missing_commit_falls_back_to_wip() {
        let snap = snapshot(&["c", "a"], true);
        assert_eq!(
            restore_selection(Some(&commit("b")), &snap),
            Some(Selection::Wip)
        );
    }

    #[test]
    fn missing_commit_without_changes_falls_back_to_first_commit() {
        let snap = snapshot(&["c", "a"], false);
        assert_eq!(
            restore_selection(Some(&commit("b")), &snap),
            Some(commit("c"))
        );
    }

    #[test]
    fn wip_without_changes_falls_back_to_first_commit() {
        let snap = snapshot(&["c"], false);
        assert_eq!(
            restore_selection(Some(&Selection::Wip), &snap),
            Some(commit("c"))
        );
    }

    #[test]
    fn wip_is_kept_while_there_are_changes() {
        let snap = snapshot(&["c"], true);
        assert_eq!(
            restore_selection(Some(&Selection::Wip), &snap),
            Some(Selection::Wip)
        );
    }

    #[test]
    fn initial_selection_prefers_wip() {
        assert_eq!(
            restore_selection(None, &snapshot(&["c"], true)),
            Some(Selection::Wip)
        );
        assert_eq!(
            restore_selection(None, &snapshot(&["c"], false)),
            Some(commit("c"))
        );
    }

    #[test]
    fn empty_clean_repository_has_no_selection() {
        assert_eq!(restore_selection(None, &snapshot(&[], false)), None);
    }

    #[test]
    fn selection_rows_account_for_wip() {
        let snap = snapshot(&["c", "b"], true);
        assert_eq!(selection_row(Some(&Selection::Wip), &snap), Some(0));
        assert_eq!(selection_row(Some(&commit("b")), &snap), Some(2));
        let clean = snapshot(&["c", "b"], false);
        assert_eq!(selection_row(Some(&commit("b")), &clean), Some(1));
        assert_eq!(selection_row(Some(&Selection::Wip), &clean), None);
    }
}
