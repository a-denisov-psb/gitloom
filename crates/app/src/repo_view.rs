//! State of one loaded repository tab.

use std::collections::HashSet;
use std::sync::Arc;

use git_core::{DiffRequest, FileChange, FileDiff, GitBackend};

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
    pub changes: Loadable<DetailChanges>,
}

/// Files shown in the details panel of a graph row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailChanges {
    /// Commit against its first parent.
    Commit(Vec<FileChange>),
    /// Stash entry, split into staged, unstaged and untracked parts.
    Stash(git_core::StashChanges),
}

#[derive(Debug, Clone)]
pub struct DiffView {
    pub request: DiffRequest,
    pub diff: Loadable<FileDiff>,
    /// Display rows `(hunk, line)`; `line == u32::MAX` is the hunk header.
    pub rows: Vec<(u32, u32)>,
    pub scroll_id: iced::widget::Id,
    pub scroll: Scroll,
    /// Selected changed lines as `(hunk, line)`.
    pub selection: std::collections::BTreeSet<(u32, u32)>,
    /// Row index of the last plain or Ctrl click (start of a Shift range).
    pub anchor: Option<usize>,
    /// A missing object of a partial clone is being fetched.
    pub fetching: bool,
    /// Objects already fetched for this diff (each is tried once).
    pub fetched: Vec<String>,
}

/// Write actions that would break an operation in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guarded {
    Commit,
    Amend,
    /// Stash, pop and apply (deleting a stash is harmless).
    Stash,
    /// Undo or redo of a commit or amend (moves HEAD).
    MoveHead,
}

/// How a line click changes the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMode {
    /// Plain click: select only this line.
    Replace,
    /// Shift+click: select the range from the anchor.
    Range,
    /// Ctrl/Cmd+click: toggle this line.
    Toggle,
}

/// Marks a hunk header row in [`DiffView::rows`].
pub const HUNK_HEADER: u32 = u32::MAX;

impl DiffView {
    pub fn new(request: DiffRequest) -> Self {
        Self {
            request,
            diff: Loadable::Loading,
            rows: Vec::new(),
            scroll_id: iced::widget::Id::unique(),
            scroll: Scroll::default(),
            selection: std::collections::BTreeSet::new(),
            anchor: None,
            fetching: false,
            fetched: Vec::new(),
        }
    }

    /// Whether lines of this diff can be staged (`Some(false)`) or unstaged (`Some(true)`).
    pub fn staged(&self) -> Option<bool> {
        match self.request.kind {
            git_core::DiffKind::Unstaged | git_core::DiffKind::Untracked => Some(false),
            git_core::DiffKind::Staged => Some(true),
            git_core::DiffKind::Commit(_) | git_core::DiffKind::Stash { .. } => None,
        }
    }

    /// True if display row `row` is a changed (selectable) line.
    fn is_changed(&self, row: usize) -> bool {
        let Loadable::Ready(file) = &self.diff else {
            return false;
        };
        self.rows.get(row).is_some_and(|&(h, l)| {
            l != HUNK_HEADER
                && file
                    .hunks
                    .get(h as usize)
                    .and_then(|hunk| hunk.lines.get(l as usize))
                    .is_some_and(|line| line.kind != git_core::DiffLineKind::Context)
        })
    }

    /// Applies a click on display row `row` to the line selection.
    pub fn click(&mut self, row: usize, mode: ClickMode) {
        // Commit diffs are read-only.
        if self.staged().is_none() || !self.is_changed(row) {
            return;
        }
        let Some(&key) = self.rows.get(row) else {
            return;
        };
        match (mode, self.anchor) {
            (ClickMode::Range, Some(anchor)) => {
                let (from, to) = (anchor.min(row), anchor.max(row));
                for r in from..=to {
                    if self.is_changed(r) {
                        self.selection.insert(self.rows[r]);
                    }
                }
            }
            (ClickMode::Toggle, _) => {
                if !self.selection.remove(&key) {
                    self.selection.insert(key);
                }
                self.anchor = Some(row);
            }
            (ClickMode::Replace | ClickMode::Range, _) => {
                self.selection.clear();
                self.selection.insert(key);
                self.anchor = Some(row);
            }
        }
    }

    /// Stores the loaded diff and flattens its hunks into display rows.
    pub fn set_diff(&mut self, diff: Result<FileDiff, String>) {
        self.rows.clear();
        self.selection.clear();
        self.anchor = None;
        match diff {
            Ok(diff) => {
                for (h, hunk) in diff.hunks.iter().enumerate() {
                    self.rows.push((h as u32, HUNK_HEADER));
                    self.rows
                        .extend((0..hunk.lines.len()).map(|l| (h as u32, l as u32)));
                }
                self.diff = Loadable::Ready(diff);
            }
            Err(error) => self.diff = Loadable::Failed(error),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CenterView {
    Graph,
    Diff(Box<DiffView>),
}

/// Vertical scroll state of the graph list (pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scroll {
    pub offset: f32,
    /// Viewport height; a guess until the first scroll event arrives.
    pub height: f32,
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            offset: 0.0,
            height: 1200.0,
        }
    }
}

/// State of the file watcher of a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchState {
    Starting,
    Running,
    /// Not available; refreshes happen on window focus and F5.
    Failed,
}

/// Background write operation; repeatable so it can be retried.
pub type WriteOp = Arc<
    dyn Fn(&dyn git_core::GitWriter) -> git_core::Result<Option<crate::undo::History>>
        + Send
        + Sync,
>;

/// The most recently started write, kept for "Retry" after a lock error.
#[derive(Clone)]
pub struct LastWrite {
    pub label: String,
    pub op: WriteOp,
    /// The write was a commit whose success clears the message fields.
    pub clears_message: bool,
}

/// A running network operation (fetch, pull, push) of a tab.
#[derive(Debug, Clone)]
pub struct NetworkOp {
    pub label: String,
    /// Latest progress update, if Git reported one.
    pub progress: Option<git_core::Progress>,
    pub cancel: git_core::CancelToken,
    /// Started automatically (auto-fetch); yields to user operations.
    pub background: bool,
}

/// A network command to run, with its display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkRequest {
    pub label: String,
    pub args: Vec<String>,
    pub background: bool,
}

/// Operation waiting for a cancelled background operation to end.
#[derive(Clone)]
pub enum Queued {
    Write(LastWrite),
    Network(NetworkRequest),
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
    /// Commit summary (first line).
    pub summary: String,
    /// Commit description.
    pub description: iced::widget::text_editor::Content,
    /// The fields were filled from `MERGE_MSG` for the current merge.
    pub merge_prefilled: bool,
    /// "Amend previous commit" is checked.
    pub amend: bool,
    /// Runs write operations through the `git` executable.
    pub writer: Arc<dyn git_core::GitWriter>,
    /// Label of the running write operation; one at a time per tab.
    pub busy: Option<String>,
    /// Why write actions are disabled (Git missing or too old), if they are.
    pub write_block: Option<String>,
    pub watcher: WatchState,
    /// Own write running or its refresh pending: watcher events are ignored.
    pub suppress_watch: bool,
    /// A status-only refresh is running.
    pub status_refreshing: bool,
    /// Reload the shown diff once the refresh after a write completes.
    pub reload_diff: bool,
    /// Clear the commit message when the running write (a commit) succeeds.
    pub clear_message_after_write: bool,
    pub last_write: Option<LastWrite>,
    /// Running network operation, shown in the status bar with Cancel.
    pub network: Option<NetworkOp>,
    /// Starts once the running (cancelled) background operation finished.
    pub queued: Option<Queued>,
    /// Undoable actions, most recent last.
    pub undo: Vec<crate::undo::UndoAction>,
    /// Undone actions that can be redone, most recent last.
    pub redo: Vec<crate::undo::UndoAction>,
    /// Identifies this tab's graph scrollable for programmatic scrolling.
    pub graph_scroll_id: iced::widget::Id,
    pub scroll: Scroll,
}

impl RepoView {
    /// Full commit message from summary and description.
    pub fn commit_message(&self) -> String {
        let summary = self.summary.trim();
        let description = self.description.text();
        let description = description.trim();
        if description.is_empty() {
            summary.to_owned()
        } else {
            format!("{summary}\n\n{description}")
        }
    }

    /// Commit HEAD points to, if it is loaded.
    pub fn head_commit(&self) -> Option<&git_core::Commit> {
        self.snapshot
            .head
            .target()
            .and_then(|id| self.snapshot.commit(id))
    }

    /// Message of the HEAD commit as (summary, description).
    fn head_message(&self) -> Option<(String, String)> {
        self.head_commit()
            .map(|c| (c.summary.clone(), c.body.clone()))
    }

    /// Checks or unchecks "Amend previous commit". Checking fills empty
    /// fields with the HEAD message; unchecking clears them if unchanged.
    pub fn set_amend(&mut self, amend: bool) {
        let Some((summary, body)) = self.head_message() else {
            self.amend = false;
            return;
        };
        let current_description = self.description.text();
        if amend {
            if self.summary.trim().is_empty() && current_description.trim().is_empty() {
                self.summary = summary;
                self.description = iced::widget::text_editor::Content::with_text(&body);
            }
        } else if self.summary == summary && current_description.trim() == body.trim() {
            self.summary.clear();
            self.description = iced::widget::text_editor::Content::new();
        }
        self.amend = amend;
    }

    /// Why `action` is disabled by the operation in progress, if it is.
    /// Committing concludes a merge, cherry-pick or revert and stays allowed.
    pub fn state_block(&self, action: Guarded) -> Option<String> {
        let state = &self.snapshot.state;
        let operation = state.operation()?;
        let blocked = match action {
            Guarded::Commit => !state.concluded_by_commit(),
            Guarded::Amend | Guarded::Stash | Guarded::MoveHead => true,
        };
        blocked.then(|| {
            let verb = match action {
                Guarded::Commit => "commit",
                Guarded::Amend => "amend",
                Guarded::Stash => "stash",
                Guarded::MoveHead => "undo or redo commits",
            };
            format!("Cannot {verb} while a {operation} is in progress")
        })
    }

    /// HEAD is already part of its upstream branch (amending rewrites pushed history).
    pub fn head_pushed(&self) -> bool {
        self.snapshot
            .refs
            .iter()
            .find(|r| r.is_head)
            .and_then(|r| r.ahead_behind)
            .is_some_and(|(ahead, _)| ahead == 0)
    }

    /// Fills empty message fields from a merge in progress or the commit template.
    pub fn prefill_message(&mut self) {
        let empty = self.summary.trim().is_empty() && self.description.text().trim().is_empty();
        match &self.snapshot.merge_message {
            Some(message) if !self.merge_prefilled => {
                if empty {
                    let (summary, description) = split_message(&strip_comments(message));
                    self.summary = summary;
                    self.description = iced::widget::text_editor::Content::with_text(&description);
                }
                self.merge_prefilled = true;
            }
            Some(_) => {}
            None => {
                self.merge_prefilled = false;
                if empty {
                    if let Some(template) = &self.snapshot.commit_template {
                        self.description = iced::widget::text_editor::Content::with_text(
                            &strip_comments(template),
                        );
                    }
                }
            }
        }
    }

    pub fn new(backend: Arc<dyn GitBackend>, snapshot: Arc<RepoSnapshot>) -> Self {
        let selection = restore_selection(None, &snapshot);
        let mut cli = git_core::CliBackend::new(backend.path());
        // Gitloom itself answers credential prompts (ADR-019).
        if let Ok(exe) = std::env::current_exe() {
            cli = cli.with_askpass(exe);
        }
        let writer: Arc<dyn git_core::GitWriter> = Arc::new(cli);
        let mut view = Self {
            backend,
            snapshot,
            selection,
            details: None,
            center: CenterView::Graph,
            filter: String::new(),
            collapsed: HashSet::from([Section::Submodules]),
            refreshing: false,
            error: None,
            summary: String::new(),
            description: iced::widget::text_editor::Content::new(),
            merge_prefilled: false,
            amend: false,
            writer,
            busy: None,
            write_block: None,
            watcher: WatchState::Starting,
            suppress_watch: false,
            status_refreshing: false,
            reload_diff: false,
            clear_message_after_write: false,
            last_write: None,
            network: None,
            queued: None,
            undo: Vec::new(),
            redo: Vec::new(),
            graph_scroll_id: iced::widget::Id::unique(),
            scroll: Scroll::default(),
        };
        view.prefill_message();
        view
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
        self.suppress_watch = false;
        self.error = None;
        self.prefill_message();
    }

    /// Selected commit id, if a commit (not WIP) is selected.
    pub fn selected_commit(&self) -> Option<&str> {
        match &self.selection {
            Some(Selection::Commit(id)) => Some(id),
            _ => None,
        }
    }
}

/// Message without `#` comment lines (as Git's editor cleanup would remove them).
pub fn strip_comments(message: &str) -> String {
    let kept: Vec<&str> = message.lines().filter(|l| !l.starts_with('#')).collect();
    kept.join("\n").trim().to_owned()
}

/// First line as summary, the rest (after the blank line) as description.
pub fn split_message(message: &str) -> (String, String) {
    match message.split_once('\n') {
        Some((summary, rest)) => (summary.trim().to_owned(), rest.trim().to_owned()),
        None => (message.trim().to_owned(), String::new()),
    }
}

/// File to select after `path` left `list` (by staging or unstaging):
/// the next one, or the previous one if it was the last (GitKraken behavior).
pub fn next_file_after<'a>(list: &'a [FileChange], path: &str) -> Option<&'a FileChange> {
    let index = list.iter().position(|f| f.path == path)?;
    list.get(index + 1)
        .or_else(|| index.checked_sub(1).and_then(|i| list.get(i)))
}

/// Diff request that shows `file` from the given list.
pub fn diff_request(file: &FileChange, staged: bool) -> DiffRequest {
    let kind = if staged {
        git_core::DiffKind::Staged
    } else if file.kind == git_core::ChangeKind::Untracked {
        git_core::DiffKind::Untracked
    } else {
        git_core::DiffKind::Unstaged
    };
    DiffRequest {
        kind,
        path: file.path.clone(),
        old_path: file.old_path.clone(),
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

    fn change(path: &str) -> FileChange {
        FileChange {
            path: path.into(),
            kind: ChangeKind::Modified,
            old_path: None,
        }
    }

    fn loaded_diff() -> DiffView {
        use git_core::{DiffHunk, DiffLine, DiffLineKind};
        let line = |kind| DiffLine {
            kind,
            old_line: None,
            new_line: None,
            content: String::new(),
            missing_newline: false,
            crlf: false,
        };
        let mut view = DiffView::new(DiffRequest {
            kind: git_core::DiffKind::Unstaged,
            path: "f".into(),
            old_path: None,
        });
        view.set_diff(Ok(FileDiff {
            old_path: Some("f".into()),
            new_path: Some("f".into()),
            binary: false,
            too_large: false,
            hunks: vec![DiffHunk {
                header: "@@".into(),
                old_start: 1,
                old_count: 3,
                new_start: 1,
                new_count: 3,
                // Rows: 0 header, 1 context, 2 removed, 3 added, 4 added, 5 context.
                lines: vec![
                    line(DiffLineKind::Context),
                    line(DiffLineKind::Removed),
                    line(DiffLineKind::Added),
                    line(DiffLineKind::Added),
                    line(DiffLineKind::Context),
                ],
            }],
        }));
        view
    }

    #[test]
    fn line_selection_clicks() {
        let mut view = loaded_diff();
        view.click(2, ClickMode::Replace);
        assert_eq!(view.selection.iter().copied().collect::<Vec<_>>(), [(0, 1)]);
        view.click(4, ClickMode::Range);
        assert_eq!(
            view.selection.iter().copied().collect::<Vec<_>>(),
            [(0, 1), (0, 2), (0, 3)]
        );
        view.click(3, ClickMode::Toggle);
        assert_eq!(
            view.selection.iter().copied().collect::<Vec<_>>(),
            [(0, 1), (0, 3)]
        );
        // Context lines and headers are not selectable.
        view.click(1, ClickMode::Replace);
        view.click(0, ClickMode::Replace);
        assert_eq!(view.selection.len(), 2);
        view.click(3, ClickMode::Replace);
        assert_eq!(view.selection.iter().copied().collect::<Vec<_>>(), [(0, 2)]);
    }

    #[test]
    fn message_helpers() {
        assert_eq!(
            strip_comments("Merge branch 'x'\n\n# Conflicts:\n#\ta.txt\n"),
            "Merge branch 'x'"
        );
        assert_eq!(
            split_message("Summary\n\nBody line\nmore"),
            ("Summary".to_owned(), "Body line\nmore".to_owned())
        );
        assert_eq!(split_message("Only"), ("Only".to_owned(), String::new()));
    }

    #[test]
    fn next_file_after_staging() {
        let list = [change("a"), change("b"), change("c")];
        assert_eq!(next_file_after(&list, "a").unwrap().path, "b");
        assert_eq!(next_file_after(&list, "b").unwrap().path, "c");
        // The last one falls back to the previous file.
        assert_eq!(next_file_after(&list, "c").unwrap().path, "b");
        assert!(next_file_after(&list[..1], "a").is_none());
        assert!(next_file_after(&list, "missing").is_none());
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
