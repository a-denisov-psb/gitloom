//! File history view (M5-09): the commits that changed one file, replacing
//! the graph; the selected commit's diff of that file on the right.

use git_core::{CancelToken, DiffKind, DiffRequest, FileHistoryEntry};

use crate::repo_view::{DiffView, Loadable};

#[derive(Debug)]
pub struct FileHistoryView {
    /// Path the history was requested for.
    pub path: String,
    /// Revision the history starts at (`HEAD` or a commit).
    pub start: String,
    pub entries: Loadable<Vec<FileHistoryEntry>>,
    /// Index into the entries.
    pub selected: Option<usize>,
    /// Diff of the file in the selected commit.
    pub diff: Option<DiffView>,
    /// Commit to select once the history arrived.
    pub preselect: Option<String>,
    /// Stops the running `git log` when the view goes away.
    pub cancel: CancelToken,
    pub list_scroll_id: iced::widget::Id,
}

impl FileHistoryView {
    pub fn new(path: String, start: String, preselect: Option<String>) -> Self {
        Self {
            path,
            start,
            entries: Loadable::Loading,
            selected: None,
            diff: None,
            preselect,
            cancel: CancelToken::new(),
            list_scroll_id: iced::widget::Id::unique(),
        }
    }

    /// Takes the loaded history; selects the preselected commit or the
    /// newest one. Returns the diff to load.
    pub fn set_entries(&mut self, entries: Vec<FileHistoryEntry>) -> Option<DiffRequest> {
        let index = self
            .preselect
            .take()
            .and_then(|id| entries.iter().position(|e| e.id == id))
            .or_else(|| (!entries.is_empty()).then_some(0));
        self.entries = Loadable::Ready(entries);
        index.and_then(|i| self.select(i))
    }

    /// Selects entry `index`; returns the diff to load (none if it is
    /// already shown).
    pub fn select(&mut self, index: usize) -> Option<DiffRequest> {
        let Loadable::Ready(entries) = &self.entries else {
            return None;
        };
        let entry = entries.get(index)?;
        if self.selected == Some(index) && self.diff.is_some() {
            return None;
        }
        self.selected = Some(index);
        let request = diff_request(entry);
        self.diff = Some(DiffView::new(request.clone()));
        Some(request)
    }

    pub fn selected_entry(&self) -> Option<&FileHistoryEntry> {
        match &self.entries {
            Loadable::Ready(entries) => self.selected.and_then(|i| entries.get(i)),
            _ => None,
        }
    }

    /// ↑ / ↓ in the list (no wrap-around).
    pub fn step(&mut self, down: bool) -> Option<DiffRequest> {
        let Loadable::Ready(entries) = &self.entries else {
            return None;
        };
        let current = self.selected?;
        let next = if down {
            (current + 1 < entries.len()).then_some(current + 1)
        } else {
            current.checked_sub(1)
        }?;
        self.select(next)
    }
}

impl Drop for FileHistoryView {
    fn drop(&mut self) {
        // Closing the view (or replacing it) stops a running `git log`.
        self.cancel.cancel();
    }
}

/// The file's diff in the commit of `entry`, under its path there.
pub fn diff_request(entry: &FileHistoryEntry) -> DiffRequest {
    DiffRequest {
        kind: DiffKind::Commit(entry.id.clone()),
        path: entry.path.clone(),
        old_path: entry.old_path.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, path: &str, old: Option<&str>) -> FileHistoryEntry {
        FileHistoryEntry {
            id: id.into(),
            author: "A".into(),
            email: "a@x".into(),
            time: 0,
            summary: id.into(),
            path: path.into(),
            old_path: old.map(str::to_owned),
            status: if old.is_some() { "R" } else { "M" }.into(),
        }
    }

    #[test]
    fn newest_is_selected_and_its_diff_requested() {
        let mut view = FileHistoryView::new("b.txt".into(), "HEAD".into(), None);
        let request = view
            .set_entries(vec![
                entry("c2", "b.txt", None),
                entry("c1", "b.txt", Some("a.txt")),
            ])
            .unwrap();
        assert_eq!(view.selected, Some(0));
        assert_eq!(request.kind, DiffKind::Commit("c2".into()));
        // The rename commit shows the diff under both names.
        let request = view.step(true).unwrap();
        assert_eq!(request.path, "b.txt");
        assert_eq!(request.old_path.as_deref(), Some("a.txt"));
        assert!(view.step(true).is_none(), "no wrap-around");
        assert!(view.select(1).is_none(), "already shown");
        assert_eq!(
            view.step(false).map(|r| r.kind),
            Some(DiffKind::Commit("c2".into()))
        );
    }

    #[test]
    fn preselected_commit_and_empty_history() {
        let mut view = FileHistoryView::new("a.txt".into(), "c1".into(), Some("c1".into()));
        view.set_entries(vec![entry("c2", "a.txt", None), entry("c1", "a.txt", None)]);
        assert_eq!(view.selected_entry().map(|e| e.id.as_str()), Some("c1"));

        let mut empty = FileHistoryView::new("x".into(), "HEAD".into(), None);
        assert!(empty.set_entries(Vec::new()).is_none());
        assert!(empty.selected.is_none());
    }

    #[test]
    fn dropping_the_view_cancels_the_request() {
        let view = FileHistoryView::new("a.txt".into(), "HEAD".into(), None);
        let cancel = view.cancel.clone();
        drop(view);
        assert!(cancel.is_cancelled());
    }
}
