//! File history view (M5-09): the commits that changed one file, replacing
//! the graph; the selected commit's diff of that file on the right.

use git_core::{Blame, CancelToken, DiffKind, DiffRequest, FileHistoryEntry};

use crate::repo_view::{DiffView, Loadable, Scroll};

/// History list with diff, or blame of the file (M5-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryMode {
    History,
    Blame,
}

/// Blame of the file at one revision.
#[derive(Debug)]
pub struct BlameView {
    /// Path blamed (the file's path at `rev`).
    pub path: String,
    /// Commit blamed; `None`: the working copy.
    pub rev: Option<String>,
    pub blame: Loadable<Blame>,
    /// Stops the running `git blame` when this blame is replaced.
    pub cancel: CancelToken,
    pub scroll_id: iced::widget::Id,
    pub scroll: Scroll,
    /// Syntax highlighting of the blamed file (M11-09) and its theme.
    pub highlight: Option<(crate::highlight::CodeTheme, crate::highlight::Outcome)>,
}

impl BlameView {
    /// Spans of line `index` when highlighting for `theme` arrived.
    pub fn spans(
        &self,
        index: usize,
        theme: crate::highlight::CodeTheme,
    ) -> Option<&[crate::highlight::Span]> {
        match &self.highlight {
            Some((t, crate::highlight::Outcome::Highlighted(h))) if *t == theme => {
                h.lines.get(index).map(Vec::as_slice)
            }
            _ => None,
        }
    }

    pub fn new(path: String, rev: Option<String>) -> Self {
        Self {
            path,
            rev,
            blame: Loadable::Loading,
            cancel: CancelToken::new(),
            scroll_id: iced::widget::Id::unique(),
            scroll: Scroll::default(),
            highlight: None,
        }
    }
}

impl Drop for BlameView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

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
    pub mode: HistoryMode,
    pub blame: Option<BlameView>,
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
            mode: HistoryMode::History,
            blame: None,
        }
    }

    /// Blame of the history's own start: the working copy for HEAD, else
    /// the start commit.
    pub fn default_blame(&self) -> BlameView {
        let rev = (self.start != "HEAD").then(|| self.start.clone());
        BlameView::new(self.path.clone(), rev)
    }

    /// Blame at the commit of entry `index` (under its path there).
    pub fn blame_at(&self, index: usize) -> Option<BlameView> {
        let Loadable::Ready(entries) = &self.entries else {
            return None;
        };
        let entry = entries.get(index)?;
        // A deletion has no content to blame.
        (entry.status != "D").then(|| BlameView::new(entry.path.clone(), Some(entry.id.clone())))
    }

    /// Selects the entry of commit `id` (gutter click); false if the commit
    /// is not in the history.
    pub fn select_commit(&mut self, id: &str) -> Option<DiffRequest> {
        let Loadable::Ready(entries) = &self.entries else {
            return None;
        };
        let index = entries.iter().position(|e| e.id == id)?;
        self.selected = None;
        self.select(index)
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
    fn blame_targets() {
        let mut view = FileHistoryView::new("b.txt".into(), "HEAD".into(), None);
        assert_eq!(view.default_blame().rev, None, "HEAD: the working copy");
        let mut deleted = entry("c3", "b.txt", None);
        deleted.status = "D".into();
        view.set_entries(vec![
            deleted,
            entry("c2", "b.txt", None),
            entry("c1", "b.txt", Some("a.txt")),
        ]);
        assert!(view.blame_at(0).is_none(), "nothing to blame in a deletion");
        let at = view.blame_at(2).unwrap();
        assert_eq!((at.path.as_str(), at.rev.as_deref()), ("b.txt", Some("c1")));
        // Gutter click selects the commit in the list.
        assert!(view.select_commit("c2").is_some());
        assert_eq!(view.selected, Some(1));
        assert!(view.select_commit("unknown").is_none());

        let from_commit = FileHistoryView::new("a.txt".into(), "c1".into(), None);
        assert_eq!(from_commit.default_blame().rev.as_deref(), Some("c1"));
    }

    #[test]
    fn dropping_the_view_cancels_the_request() {
        let view = FileHistoryView::new("a.txt".into(), "HEAD".into(), None);
        let cancel = view.cancel.clone();
        drop(view);
        assert!(cancel.is_cancelled());
    }
}
