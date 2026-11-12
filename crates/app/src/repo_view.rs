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
    /// Shown when Git flow is initialized (M7-08).
    Gitflow,
    Submodules,
}

/// Inputs and result of the last [`crate::branch_tree::Groups`] build.
struct SidebarCache {
    snapshot: Arc<RepoSnapshot>,
    filter: String,
    ref_filter: crate::ref_filter::RefFilter,
    groups: Arc<crate::branch_tree::Groups>,
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
    /// How the loaded diff is laid out (from the diff mode when it loaded).
    pub layout: DiffLayout,
    /// Display rows of the layout.
    pub rows: Vec<DiffRow>,
    /// Change blocks of the Inline layout (M7-03) and the row of each
    /// block's bar.
    pub blocks: Vec<crate::diff_blocks::Block>,
    pub block_rows: Vec<usize>,
    /// Scroll to the first change once the diff is loaded (opening the
    /// file or switching the mode, not reloads after staging).
    pub scroll_to_change: bool,
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
    /// Horizontal offset of the code text (M7-16); one for both halves of
    /// a Split diff. Kept when the diff reloads, reset for another file.
    pub h_offset: f32,
    /// Largest offset, as the horizontal scrollbar last reported it
    /// (`None` until it did).
    pub h_max: Option<f32>,
    /// Width of the longest line, measured when the diff loaded or the
    /// editor settings changed.
    pub text_width: f32,
    /// The longest line as shown (tabs expanded).
    pub longest: String,
    pub hscroll_id: iced::widget::Id,
    /// Preferences → Editor values the diff is drawn with (M11-06).
    pub style: crate::editor_style::EditorStyle,
    /// Characters of every display row (word wrap, M11-07).
    pub row_chars: Vec<crate::diff_wrap::RowChars>,
    /// Width of one character in the editor font.
    pub char_width: f32,
    /// Width of the diff as the scrollbar last reported it (0 = unknown).
    pub width: f32,
    /// Syntax highlighting of both sides (M11-09), when it arrived for the
    /// current content (`generation`).
    pub highlight: Option<Arc<crate::highlight::DiffSides>>,
    /// Counts loads, so a late highlighting result of an older load is
    /// ignored.
    pub generation: u64,
}

/// A branch being dragged: its name and kind, and the branch under the
/// pointer (drop target highlight).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchDrag {
    pub source: (String, git_core::RefKind),
    pub target: Option<(String, git_core::RefKind)>,
}

impl BranchDrag {
    /// `name`/`kind` is highlighted as drop target.
    pub fn targets(&self, name: &str, kind: git_core::RefKind) -> bool {
        self.target
            .as_ref()
            .is_some_and(|(n, k)| n == name && *k == kind)
    }
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
    /// Merge or fast-forward.
    Merge,
    /// Reset HEAD to another commit.
    Reset,
    /// Switch to another branch (`git switch` refuses during an operation).
    Checkout,
}

/// A stash pop or apply that stopped in conflicts (M9-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashConflict {
    /// Commit of the stash entry (Git kept it).
    pub id: String,
    /// The stash's label as the STASHES section shows it.
    pub label: String,
}

/// Banner for conflicted files that no Git operation explains (M9-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictBanner {
    /// "Stash '<label>' applied with conflicts. The stash was kept."
    /// `undo`: the pop/apply is the newest undo entry.
    StashConflicts {
        id: String,
        label: String,
        undo: bool,
    },
    /// "Conflicts resolved. The stash '<label>' is still in the list."
    StashResolved { id: String, label: String },
    /// Conflicted files without a running operation (e.g. after a restart).
    Orphaned,
}

impl ConflictBanner {
    pub fn text(&self) -> String {
        match self {
            Self::StashConflicts { label, .. } => {
                format!("Stash '{label}' applied with conflicts. The stash was kept.")
            }
            Self::StashResolved { label, .. } => {
                format!("Conflicts resolved. The stash '{label}' is still in the list.")
            }
            Self::Orphaned => "Conflicted files without a running operation".into(),
        }
    }
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

/// Layout of a loaded diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffLayout {
    /// Hunk headers with "Stage hunk" (3-line context).
    #[default]
    Hunks,
    /// The whole file with a bar above every change block (M7-03).
    Inline,
    /// Old and new side by side, with block bars like Inline (M7-04).
    Split,
}

impl DiffLayout {
    /// Full file with change blocks (Inline and Split).
    pub fn has_blocks(self) -> bool {
        matches!(self, Self::Inline | Self::Split)
    }
}

impl DiffLayout {
    pub fn of(mode: crate::settings::DiffMode) -> Self {
        match mode {
            crate::settings::DiffMode::Hunk => Self::Hunks,
            crate::settings::DiffMode::Inline => Self::Inline,
            crate::settings::DiffMode::Split => Self::Split,
        }
    }
}

/// One display row of a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffRow {
    /// Header of hunk `h` (Hunk layout).
    Header(u32),
    /// Line `l` of hunk `h`.
    Line(u32, u32),
    /// Bar of change block `b` (Inline and Split layouts).
    Block(u32),
    /// Split layout: lines of hunk `h` on the old (left) and new (right)
    /// side; `None` is an empty cell.
    Split(u32, Option<u32>, Option<u32>),
}

/// Reason shown for line, block and hunk actions of a UTF-16 file (M7-17).
pub const UTF16_LINE_HINT: &str =
    "Line staging is not available for UTF-16 files; stage the whole file";

/// Rows kept above a change when jumping to it.
pub const JUMP_MARGIN: usize = 3;

impl DiffView {
    pub fn new(request: DiffRequest) -> Self {
        Self {
            request,
            diff: Loadable::Loading,
            layout: DiffLayout::default(),
            rows: Vec::new(),
            blocks: Vec::new(),
            block_rows: Vec::new(),
            scroll_to_change: true,
            scroll_id: iced::widget::Id::unique(),
            scroll: Scroll::default(),
            selection: std::collections::BTreeSet::new(),
            anchor: None,
            fetching: false,
            fetched: Vec::new(),
            h_offset: 0.0,
            h_max: None,
            text_width: 0.0,
            longest: String::new(),
            hscroll_id: iced::widget::Id::unique(),
            style: crate::editor_style::EditorStyle::default(),
            row_chars: Vec::new(),
            char_width: 0.0,
            width: 0.0,
            highlight: None,
            generation: 0,
        }
    }

    /// Spans of `line` when highlighting is on and its result matches the
    /// current theme.
    pub fn spans_for(&self, line: &git_core::DiffLine) -> Option<&[crate::highlight::Span]> {
        if !self.style.highlight {
            return None;
        }
        self.highlight
            .as_deref()
            .filter(|sides| sides.theme == self.style.code_theme)
            .and_then(|sides| sides.spans_for(line))
    }

    /// Word wrap layout for a diff `width` wide, `None` without wrapping
    /// (or before the diff loaded).
    pub fn wrap_layout(&self, width: f32) -> Option<crate::diff_wrap::WrapLayout> {
        if !self.style.wrap || width <= 0.0 || self.row_chars.len() != self.rows.len() {
            return None;
        }
        let area = crate::diff_hscroll::text_area(self.layout, width, &self.style);
        let columns = crate::diff_wrap::columns(area, self.char_width);
        Some(crate::diff_wrap::WrapLayout::build(
            &self.row_chars,
            columns,
        ))
    }

    /// Top of display row `row` in pixels (wrapped rows count their visual
    /// lines).
    pub fn row_top(&self, row: usize) -> f32 {
        let height = self.style.line_height();
        match self.wrap_layout(self.width) {
            Some(layout) => layout.start(row) as f32 * height,
            None => row as f32 * height,
        }
    }

    /// Row at the scroll offset and the rows that fit into the viewport.
    pub fn rows_at_scroll(&self) -> (usize, usize) {
        let height = self.style.line_height();
        match self.wrap_layout(self.width) {
            Some(layout) => {
                let top = layout.row_at((self.scroll.offset / height).round() as u32);
                let (first, last) =
                    layout.visible(self.scroll.offset, self.scroll.height, height, 0);
                (top, last.saturating_sub(first))
            }
            None => (
                (self.scroll.offset / height).round() as usize,
                (self.scroll.height / height).floor() as usize,
            ),
        }
    }

    /// Characters of every display row as shown (tabs expanded, with the
    /// no-newline note), for word wrap.
    fn count_row_chars(&mut self) {
        let Loadable::Ready(diff) = &self.diff else {
            self.row_chars.clear();
            return;
        };
        let style = self.style;
        let chars = |h: u32, l: Option<u32>| -> usize {
            let Some(line) = l.and_then(|l| {
                diff.hunks
                    .get(h as usize)
                    .and_then(|hunk| hunk.lines.get(l as usize))
            }) else {
                return 0;
            };
            let mut count = style.expand(&line.content).chars().count();
            if line.missing_newline {
                count += crate::diff_hscroll::NO_NEWLINE.chars().count();
            }
            count
        };
        self.row_chars = self
            .rows
            .iter()
            .map(|row| match *row {
                DiffRow::Header(_) | DiffRow::Block(_) => crate::diff_wrap::RowChars::default(),
                DiffRow::Line(h, l) => crate::diff_wrap::RowChars::single(chars(h, Some(l))),
                DiffRow::Split(h, left, right) => {
                    crate::diff_wrap::RowChars::split(chars(h, left), chars(h, right))
                }
            })
            .collect();
    }

    /// Applies new editor settings: tabs, the measured text width and the
    /// horizontal range change with them.
    pub fn restyle(&mut self, style: crate::editor_style::EditorStyle) {
        if style == self.style {
            return;
        }
        self.style = style;
        if let Loadable::Ready(diff) = &self.diff {
            self.longest = crate::diff_hscroll::longest_line(diff, style.tab);
        }
        self.text_width = crate::diff_hscroll::text_width(&self.longest, &style);
        self.char_width = crate::diff_hscroll::char_width(&style);
        self.count_row_chars();
        self.h_max = None;
        self.h_offset = self.h_offset.min(self.text_width);
    }

    /// Why hunks, blocks and lines of this diff cannot be staged, unstaged
    /// or discarded: `git apply` treats UTF-16 files as binary (M7-17).
    pub fn line_staging_block(&self) -> Option<&'static str> {
        match &self.diff {
            Loadable::Ready(file) if file.encoding.is_some() => Some(UTF16_LINE_HINT),
            _ => None,
        }
    }

    /// Moves the code text by `delta` pixels (wheel or touchpad), within
    /// the range the scrollbar reported (or the text width before that).
    pub fn scroll_horizontally(&mut self, delta: f32) -> bool {
        let max = self.h_max.unwrap_or(self.text_width);
        let offset = crate::diff_hscroll::scrolled(self.h_offset, delta, max);
        let changed = offset != self.h_offset;
        self.h_offset = offset;
        changed
    }

    /// Whether lines of this diff can be staged (`Some(false)`) or unstaged (`Some(true)`).
    pub fn staged(&self) -> Option<bool> {
        match self.request.kind {
            git_core::DiffKind::Unstaged | git_core::DiffKind::Untracked => Some(false),
            git_core::DiffKind::Staged => Some(true),
            git_core::DiffKind::Commit(_) | git_core::DiffKind::Stash { .. } => None,
        }
    }

    /// Line `l` of hunk `h` is a changed (selectable) line.
    fn is_changed_line(&self, h: u32, l: u32) -> bool {
        let Loadable::Ready(file) = &self.diff else {
            return false;
        };
        file.hunks
            .get(h as usize)
            .and_then(|hunk| hunk.lines.get(l as usize))
            .is_some_and(|line| line.kind != git_core::DiffLineKind::Context)
    }

    /// Changed lines of display row `row` (both sides of a Split row).
    fn changed_lines(&self, row: usize) -> Vec<(u32, u32)> {
        let keys = match self.rows.get(row) {
            Some(&DiffRow::Line(h, l)) => vec![(h, l)],
            Some(&DiffRow::Split(h, left, right)) => {
                left.into_iter().chain(right).map(|l| (h, l)).collect()
            }
            _ => Vec::new(),
        };
        keys.into_iter()
            .filter(|&(h, l)| self.is_changed_line(h, l))
            .collect()
    }

    /// Applies a click on display row `row` to the line selection.
    pub fn click(&mut self, row: usize, mode: ClickMode) {
        self.click_side(row, None, mode);
    }

    /// Click on one side of a Split row (`right`: new side) or on an
    /// Inline/Hunk row (`None`).
    pub fn click_side(&mut self, row: usize, right: Option<bool>, mode: ClickMode) {
        // Commit diffs are read-only; UTF-16 lines cannot be staged.
        if self.staged().is_none() || self.line_staging_block().is_some() {
            return;
        }
        let key = match (self.rows.get(row), right) {
            (Some(&DiffRow::Line(h, l)), None) => (h, l),
            (Some(&DiffRow::Split(h, left, new)), Some(right)) => {
                match if right { new } else { left } {
                    Some(l) => (h, l),
                    None => return,
                }
            }
            _ => return,
        };
        if !self.is_changed_line(key.0, key.1) {
            return;
        }
        match (mode, self.anchor) {
            (ClickMode::Range, Some(anchor)) => {
                let (from, to) = (anchor.min(row), anchor.max(row));
                for r in from..=to {
                    for key in self.changed_lines(r) {
                        self.selection.insert(key);
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

    /// Top row to scroll to for the previous or next change block, seen
    /// from the scroll position `top` (in rows); `None` at the ends.
    /// `visible` rows fit in the viewport (0 if unknown).
    pub fn change_target(&self, top: usize, visible: usize, next: bool) -> Option<usize> {
        let max_top = if visible > 0 {
            self.rows.len().saturating_sub(visible)
        } else {
            usize::MAX
        };
        let target = |row: usize| row.saturating_sub(JUMP_MARGIN).min(max_top);
        if next {
            self.block_rows
                .iter()
                .map(|&r| target(r))
                .find(|&t| t > top)
        } else {
            self.block_rows
                .iter()
                .rev()
                .map(|&r| target(r))
                .find(|&t| t < top)
        }
    }

    /// Stores the loaded diff and flattens its hunks into display rows.
    pub fn set_diff(&mut self, diff: Result<FileDiff, String>) {
        self.generation += 1;
        self.highlight = None;
        self.rows.clear();
        self.blocks.clear();
        self.block_rows.clear();
        self.selection.clear();
        self.anchor = None;
        match diff {
            Ok(diff) => {
                self.longest = crate::diff_hscroll::longest_line(&diff, self.style.tab);
                self.text_width = crate::diff_hscroll::text_width(&self.longest, &self.style);
                if self.layout.has_blocks() {
                    self.blocks = crate::diff_blocks::blocks(&diff);
                }
                // Read-only diffs (commit, stash) have no bars: navigation
                // jumps to a block's first changed row instead (M7-05).
                let bars = self.staged().is_some();
                let mut next_block = 0;
                for (h, hunk) in diff.hunks.iter().enumerate() {
                    let h = h as u32;
                    if self.layout == DiffLayout::Hunks {
                        self.rows.push(DiffRow::Header(h));
                    }
                    if self.layout == DiffLayout::Split {
                        for (left, right) in crate::diff_blocks::split_rows(&hunk.lines) {
                            if let Some(block) = self.blocks.get(next_block) {
                                let starts = |side: Option<u32>| side == Some(block.first);
                                if block.hunk == h && (starts(left) || starts(right)) {
                                    self.block_rows.push(self.rows.len());
                                    if bars {
                                        self.rows.push(DiffRow::Block(next_block as u32));
                                    }
                                    next_block += 1;
                                }
                            }
                            self.rows.push(DiffRow::Split(h, left, right));
                        }
                        continue;
                    }
                    for l in 0..hunk.lines.len() as u32 {
                        // The bar sits right above the block's first change.
                        if let Some(block) = self.blocks.get(next_block) {
                            if block.hunk == h && block.first == l {
                                self.block_rows.push(self.rows.len());
                                if bars {
                                    self.rows.push(DiffRow::Block(next_block as u32));
                                }
                                next_block += 1;
                            }
                        }
                        self.rows.push(DiffRow::Line(h, l));
                    }
                }
                self.diff = Loadable::Ready(diff);
                self.char_width = crate::diff_hscroll::char_width(&self.style);
                self.count_row_chars();
            }
            Err(error) => self.diff = Loadable::Failed(error),
        }
    }
}

#[derive(Debug)]
pub enum CenterView {
    Graph,
    Diff(Box<DiffView>),
    /// Conflict resolution view of one file (M4-05).
    Conflict(Box<crate::conflict::ConflictView>),
    /// Interactive rebase panel (M4-07), replacing the graph.
    Rebase(Box<crate::rebase_panel::RebasePanel>),
    /// History of one file (M5-09), replacing the graph.
    FileHistory(Box<crate::file_history::FileHistoryView>),
}

impl RepoView {
    /// Applies new editor settings to the diff shown in the center (M11-06).
    pub fn restyle(&mut self, style: crate::editor_style::EditorStyle) {
        if let Some(diff) = self.center.diff_mut() {
            diff.restyle(style);
        }
    }
}

impl CenterView {
    /// The diff shown in the center: the diff view, or the selected
    /// commit's diff in the file history.
    pub fn diff_mut(&mut self) -> Option<&mut DiffView> {
        match self {
            Self::Diff(diff) => Some(diff),
            Self::FileHistory(history) => history.diff.as_mut(),
            Self::Graph | Self::Conflict(_) | Self::Rebase(_) => None,
        }
    }
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

/// What a network operation does (decides how its result is handled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkKind {
    Fetch,
    Pull,
    /// Push of the checked-out branch (rejections offer Pull / force push).
    Push,
    /// Push of another branch: rejections show the plain error.
    PushBranch,
    /// `git submodule update` (M12-06).
    SubmoduleUpdate,
}

/// A running network operation (fetch, pull, push) of a tab.
#[derive(Debug, Clone)]
pub struct NetworkOp {
    pub label: String,
    pub kind: NetworkKind,
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
    pub kind: NetworkKind,
    pub args: Vec<String>,
    pub background: bool,
    /// What to record for undo (M12-08).
    pub undo: NetworkUndo,
}

/// Undo recording of a network request (M12-08): the local part of a pull
/// and a submodule update are undoable; fetch and push are not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkUndo {
    None,
    /// `git pull` of the checked-out branch.
    Pull,
    /// Fast-forward of a branch that is not checked out (`fetch r b:b`).
    PullBranch {
        branch: String,
    },
    /// `git submodule update` in the repository at `root`.
    SubmoduleUpdate {
        root: std::path::PathBuf,
    },
}

/// Inline input for a new branch in the graph (toolbar Branch, Ctrl+B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewBranch {
    /// Commit the branch starts at.
    pub start: String,
    /// Graph row showing the input: a commit id, or `None` for the WIP row.
    pub row: Option<String>,
    pub name: String,
    /// Shown at the input; from instant validation or from Git.
    pub error: Option<String>,
    /// Creation is running; on failure the input stays open with the error.
    pub submitted: bool,
}

/// Inline rename of a local branch in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameBranch {
    pub old: String,
    pub name: String,
    pub error: Option<String>,
    /// The rename is running; on failure the input stays open with the error.
    pub submitted: bool,
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
    /// Commits selected with Ctrl/Shift (M4-01); more than one shows the
    /// multi-selection panel.
    pub multi: crate::selection::MultiSelection,
    pub details: Option<CommitDetails>,
    /// Signature status per commit, requested once when a commit is
    /// selected (M10-05).
    pub signatures:
        std::collections::HashMap<String, Loadable<Option<git_core::signature::Signature>>>,
    /// Open commit search bar above the graph (M5-07).
    pub search: Option<crate::search::SearchState>,
    /// Hidden and soloed branches (M5-08); the snapshot is filtered by it.
    pub ref_filter: crate::ref_filter::RefFilter,
    /// Number of the latest filter request; older results are dropped.
    pub filter_seq: u64,
    /// Conflicted file to show again once the refresh after a merge tool
    /// run arrived (if it is still conflicted).
    pub reopen_conflict: Option<String>,
    pub center: CenterView,
    pub filter: String,
    pub collapsed: HashSet<Section>,
    /// A Git flow finish was started or continued in this session: once
    /// its stopped merge is concluded, the remaining steps run on their
    /// own (M7-10). After a restart the banner asks instead.
    pub flow_finishing: bool,
    /// `next` step of the finish that was last continued automatically
    /// (no second attempt if continuing failed).
    pub flow_resumed_at: Option<usize>,
    /// Collapsed branch folders and remotes of the left panel
    /// ([`crate::branch_tree::folder_key`]); stored per repository.
    pub folders_collapsed: std::collections::BTreeSet<String>,
    /// LOCAL and REMOTE trees for the current snapshot, text filter and
    /// hide/solo state; rebuilt only when one of them changes (M7-14).
    sidebar_cache: std::cell::RefCell<Option<SidebarCache>>,
    /// A refresh is running.
    pub refreshing: bool,
    /// Last refresh failed; the previous snapshot stays visible.
    pub error: Option<String>,
    /// Commit summary (first line).
    pub summary: String,
    /// Commit description.
    pub description: iced::widget::text_editor::Content,
    /// The fields were filled from `MERGE_MSG` for the current merge.
    pub merge_prefilled: Option<String>,
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
    /// Status bar text once the running write succeeds (e.g. "Committing as
    /// … in …", M8-04).
    pub after_write_status: Option<String>,
    pub last_write: Option<LastWrite>,
    /// Last fetch (or when the tab was opened), for auto-fetch.
    pub last_fetch: std::time::Instant,
    /// An auto-fetch error was shown; further ones stay quiet until a fetch works.
    pub fetch_error_shown: bool,
    /// Inline new-branch input, if open.
    pub new_branch: Option<NewBranch>,
    /// Inline branch rename in the sidebar, if open.
    pub rename: Option<RenameBranch>,
    /// Network operation that follows the running write if it succeeds
    /// (e.g. renaming the remote branch after the local rename).
    pub then_network: Option<NetworkRequest>,
    /// Further network requests run one after the other (bulk delete of
    /// remote branches on several remotes, M12-11).
    pub network_chain: std::collections::VecDeque<NetworkRequest>,
    /// Selected rows of the left panel (M12-11).
    pub ref_selection: crate::ref_selection::RefSelection,
    /// Branch of the running plain delete, to offer a force delete if Git
    /// says it is not merged.
    pub pending_delete: Option<String>,
    /// Target of the running checkout, to offer "Stash and checkout" if
    /// local changes block it.
    pub pending_checkout: Option<crate::app::CheckoutTarget>,
    /// The running merge or fast-forward, repeated with `autostash` if
    /// local changes block it.
    pub pending_merge: Option<crate::app::Message>,
    /// A cherry-pick or revert ran: after the next refresh, an empty result
    /// (operation in progress, nothing to commit) opens the empty dialog.
    pub check_empty: bool,
    /// Branch being dragged onto another one (M4-11).
    pub drag: Option<BranchDrag>,
    /// Undo entry of an operation that stopped (conflicts); pushed once it
    /// completes, dropped when it is aborted (M4-12).
    pub pending_history: Option<crate::undo::UndoAction>,
    /// A stash pop or apply that stopped in conflicts (M9-01); in memory
    /// only, the banner offers Drop stash once the conflicts are resolved.
    pub stash_conflict: Option<StashConflict>,
    /// Files that were conflicted while the current operation ran, for
    /// "Resolved Files" (M12-05); `None` when the operation was already
    /// running when the tab opened (then every staged file counts).
    pub operation_conflicts: Option<std::collections::BTreeSet<String>>,
    /// Submodules with their states (M12-06), read after each refresh.
    pub submodules: Vec<git_core::submodule::Submodule>,
    /// HEAD when the submodules were read last; `None` before the first read.
    pub submodules_head: Option<Option<String>>,
    /// Error of a failed pull, shown once the refresh tells whether it
    /// stopped in a conflict (Git reports merge conflicts on stdout).
    pub pull_failure: Option<String>,
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
    /// Horizontal scroll of the graph table (M11-16): offset, the largest
    /// offset the scrollbar reported, and the scrollbar's id.
    pub graph_h_offset: f32,
    pub graph_h_max: Option<f32>,
    pub graph_hscroll_id: iced::widget::Id,
}

impl RepoView {
    /// Full commit message from summary and description.
    pub fn commit_message(&self) -> String {
        let summary = self.summary.trim();
        let description = self.description.text();
        // "Remove comments from commit messages" (M10-06).
        let options = &self.snapshot.template_options;
        let description = if options.remove_comments {
            git_core::commit_template::strip_comment_lines(&description, &options.comment_char)
        } else {
            description
        };
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
        // Git cannot stash or apply a stash while the index has conflicts.
        if action == Guarded::Stash && state.is_clean() && !self.conflicted_files().is_empty() {
            return Some(
                "Cannot stash while files are conflicted: resolve or discard them first".into(),
            );
        }
        let operation = state.operation()?;
        let blocked = match action {
            // Committing continues the operation (M4-03).
            Guarded::Commit => false,
            Guarded::Amend
            | Guarded::Stash
            | Guarded::MoveHead
            | Guarded::Merge
            | Guarded::Reset
            | Guarded::Checkout => true,
        };
        blocked.then(|| {
            let verb = match action {
                Guarded::Commit => "commit",
                Guarded::Amend => "amend",
                Guarded::Stash => "stash",
                Guarded::MoveHead => "undo or redo commits",
                Guarded::Merge => "merge",
                Guarded::Reset => "reset",
                Guarded::Checkout => "check out a branch",
            };
            format!("Cannot {verb} while a {operation} is in progress")
        })
    }

    /// A cherry-pick or revert stopped with nothing to commit: no conflicts
    /// and no staged or unstaged changes (untracked files do not count).
    pub fn empty_pick(&self) -> bool {
        let tree = &self.snapshot.working_tree;
        matches!(
            self.snapshot.state,
            git_core::RepoState::CherryPicking { .. } | git_core::RepoState::Reverting { .. }
        ) && tree.staged.is_empty()
            && tree
                .unstaged
                .iter()
                .all(|f| f.kind == git_core::ChangeKind::Untracked)
    }

    /// What the banner for a stash conflict or for conflicted files
    /// without a running operation shows (M9-01, ADR-044); `None` while Git
    /// runs an operation (its own banner) or nothing is conflicted.
    pub fn conflict_banner(&self) -> Option<ConflictBanner> {
        if !self.snapshot.state.is_clean() || self.snapshot.flow_finish.is_some() {
            return None;
        }
        let conflicted = !self.conflicted_files().is_empty();
        let stash = self
            .stash_conflict
            .as_ref()
            .filter(|s| self.snapshot.stashes.iter().any(|listed| listed.id == s.id));
        match stash {
            Some(stash) if conflicted => Some(ConflictBanner::StashConflicts {
                id: stash.id.clone(),
                label: stash.label.clone(),
                undo: self.undo.last().is_some_and(|action| {
                    matches!(action, crate::undo::UndoAction::Stash(record)
                        if record.stash == stash.id)
                }),
            }),
            Some(stash) => Some(ConflictBanner::StashResolved {
                id: stash.id.clone(),
                label: stash.label.clone(),
            }),
            None if conflicted => Some(ConflictBanner::Orphaned),
            None => None,
        }
    }

    /// The operation in progress (M12-05).
    pub fn operation(&self) -> Option<crate::operation::OperationStatus> {
        crate::operation::status(self)
    }

    /// Keeps the set of files that were conflicted during the operation:
    /// starts when an operation appears, grows while it runs, ends with it.
    fn record_operation_conflicts(&mut self, was_running: bool) {
        if crate::operation::status(self).is_none() {
            self.operation_conflicts = None;
            return;
        }
        let conflicted: Vec<String> = self
            .conflicted_files()
            .into_iter()
            .map(str::to_owned)
            .collect();
        match &mut self.operation_conflicts {
            Some(set) => set.extend(conflicted),
            None if !was_running => {
                self.operation_conflicts = Some(conflicted.into_iter().collect())
            }
            None => {}
        }
    }

    /// Staged files that were conflicted during the operation ("Resolved
    /// Files"); without a record (the operation ran before the tab opened)
    /// every staged file.
    pub fn resolved_files(&self) -> Vec<&git_core::FileChange> {
        if crate::operation::status(self).is_none() {
            return Vec::new();
        }
        let staged = &self.snapshot.working_tree.staged;
        match &self.operation_conflicts {
            Some(set) => staged.iter().filter(|f| set.contains(&f.path)).collect(),
            None => staged.iter().collect(),
        }
    }

    /// Paths with unresolved conflicts, sorted.
    pub fn conflicted_files(&self) -> Vec<&str> {
        self.snapshot
            .working_tree
            .unstaged
            .iter()
            .filter(|f| f.kind == git_core::ChangeKind::Conflicted)
            .map(|f| f.path.as_str())
            .collect()
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
            // Each new message (e.g. the next stopped rebase step) prefills once.
            Some(message) if self.merge_prefilled.as_ref() != Some(message) => {
                if empty {
                    let (summary, description) = split_message(&strip_comments(message));
                    self.summary = summary;
                    self.description = iced::widget::text_editor::Content::with_text(&description);
                }
                self.merge_prefilled = Some(message.clone());
            }
            Some(_) => {}
            None => {
                self.merge_prefilled = None;
                // M10-06: summary line, blank line, description; only while
                // "Apply this template" is on, never while amending.
                let options = &self.snapshot.template_options;
                if empty && options.apply && !self.amend {
                    if let Some(template) = &self.snapshot.commit_template {
                        let text = if options.remove_comments {
                            git_core::commit_template::strip_comment_lines(
                                template,
                                &options.comment_char,
                            )
                        } else {
                            template.clone()
                        };
                        let (summary, description) =
                            git_core::commit_template::split_template(&text);
                        self.summary = summary;
                        self.description =
                            iced::widget::text_editor::Content::with_text(&description);
                    }
                }
            }
        }
    }

    pub fn new(backend: Arc<dyn GitBackend>, snapshot: Arc<RepoSnapshot>) -> Self {
        let selection = restore_selection(None, &snapshot);
        let mut cli = git_core::CliBackend::new(backend.path());
        // Gitloom itself answers credential prompts (ADR-019) and acts as
        // Git's editor (ADR-021).
        if let Ok(exe) = std::env::current_exe() {
            cli = cli.with_askpass(&exe).with_editor(exe);
        }
        let writer: Arc<dyn git_core::GitWriter> = Arc::new(cli);
        let mut view = Self {
            backend,
            snapshot,
            selection,
            search: None,
            ref_filter: Default::default(),
            filter_seq: 0,
            reopen_conflict: None,
            multi: crate::selection::MultiSelection::default(),
            details: None,
            signatures: std::collections::HashMap::new(),
            center: CenterView::Graph,
            filter: String::new(),
            collapsed: HashSet::new(),
            folders_collapsed: Default::default(),
            flow_finishing: false,
            flow_resumed_at: None,
            sidebar_cache: Default::default(),
            refreshing: false,
            error: None,
            summary: String::new(),
            description: iced::widget::text_editor::Content::new(),
            merge_prefilled: None,
            amend: false,
            writer,
            busy: None,
            write_block: None,
            watcher: WatchState::Starting,
            suppress_watch: false,
            status_refreshing: false,
            reload_diff: false,
            clear_message_after_write: false,
            after_write_status: None,
            last_write: None,
            last_fetch: std::time::Instant::now(),
            fetch_error_shown: false,
            new_branch: None,
            rename: None,
            then_network: None,
            network_chain: std::collections::VecDeque::new(),
            ref_selection: crate::ref_selection::RefSelection::default(),
            pending_delete: None,
            pending_checkout: None,
            pending_merge: None,
            check_empty: false,
            drag: None,
            pending_history: None,
            stash_conflict: None,
            operation_conflicts: None,
            submodules: Vec::new(),
            submodules_head: None,
            pull_failure: None,
            network: None,
            queued: None,
            undo: Vec::new(),
            redo: Vec::new(),
            graph_scroll_id: iced::widget::Id::unique(),
            graph_h_offset: 0.0,
            graph_h_max: None,
            graph_hscroll_id: iced::widget::Id::unique(),
            scroll: Scroll::default(),
        };
        view.prefill_message();
        view
    }

    pub fn name(&self) -> &str {
        self.backend.name()
    }

    /// Replaces the snapshot after a refresh and keeps the selection if possible.
    /// LOCAL and REMOTE trees of the left panel (cached, M7-14).
    pub fn sidebar_groups(&self) -> Arc<crate::branch_tree::Groups> {
        let mut cache = self.sidebar_cache.borrow_mut();
        if let Some(cached) = cache.as_ref().filter(|c| {
            Arc::ptr_eq(&c.snapshot, &self.snapshot)
                && c.filter == self.filter
                && c.ref_filter == self.ref_filter
        }) {
            return Arc::clone(&cached.groups);
        }
        let groups = Arc::new(crate::branch_tree::Groups::build(
            &self.snapshot.refs,
            &self.snapshot.remotes,
            &self.filter,
            &self.ref_filter,
        ));
        *cache = Some(SidebarCache {
            snapshot: Arc::clone(&self.snapshot),
            filter: self.filter.clone(),
            ref_filter: self.ref_filter.clone(),
            groups: Arc::clone(&groups),
        });
        groups
    }

    /// Rows of LOCAL, REMOTE, GITFLOW and TAGS in the order the left panel
    /// shows them (M12-11): collapsed sections and folders and the text
    /// filter leave rows out. A Git flow branch counts at its first row.
    pub fn visible_refs(&self) -> Vec<crate::ref_selection::RefKey> {
        use crate::branch_tree::{folder_key, rows, tag_folder_key, Row};
        let needle = self.filter.to_lowercase();
        let filtering = !needle.is_empty();
        let refs = &self.snapshot.refs;
        let mut order: Vec<crate::ref_selection::RefKey> = Vec::new();
        let push_rows = |order: &mut Vec<_>, rows: Vec<Row<'_>>| {
            for row in rows {
                if let Row::Branch { index, .. } = row {
                    if let Some(r) = refs.get(index) {
                        let key = (r.kind, r.name.clone());
                        if !order.contains(&key) {
                            order.push(key);
                        }
                    }
                }
            }
        };
        let open = |section: Section| !self.collapsed.contains(&section);
        if let (Some(config), true) = (&self.snapshot.flow, open(Section::Gitflow)) {
            let branches = refs
                .iter()
                .enumerate()
                .filter(|(_, r)| r.kind == git_core::RefKind::LocalBranch)
                .filter(|(_, r)| needle.is_empty() || r.name.to_lowercase().contains(&needle))
                .map(|(index, r)| (index, r.name.as_str()));
            let (tree, _) = crate::branch_tree::gitflow(branches, config);
            let collapsed = |path: &str| self.folders_collapsed.contains(path);
            push_rows(&mut order, rows(&tree, &collapsed, filtering));
        }
        let groups = self.sidebar_groups();
        if open(Section::Local) {
            let collapsed = |path: &str| self.folders_collapsed.contains(&folder_key(None, path));
            push_rows(&mut order, rows(&groups.local, &collapsed, filtering));
        }
        if open(Section::Remote) {
            for (name, nodes) in &groups.remotes {
                if !filtering && self.folders_collapsed.contains(&folder_key(Some(name), "")) {
                    continue;
                }
                let collapsed = |path: &str| {
                    self.folders_collapsed
                        .contains(&folder_key(Some(name), path))
                };
                push_rows(&mut order, rows(nodes, &collapsed, filtering));
            }
        }
        if open(Section::Tags) {
            let tags: Vec<(usize, &str)> = refs
                .iter()
                .enumerate()
                .filter(|(_, r)| r.kind == git_core::RefKind::Tag)
                .filter(|(_, r)| needle.is_empty() || r.name.to_lowercase().contains(&needle))
                .map(|(index, r)| (index, r.name.as_str()))
                .collect();
            let tree = crate::branch_tree::build(tags);
            let collapsed = |path: &str| self.folders_collapsed.contains(&tag_folder_key(path));
            push_rows(&mut order, rows(&tree, &collapsed, filtering));
        }
        order
    }

    /// Drops selected left panel rows that are no longer shown.
    pub fn prune_ref_selection(&mut self) {
        if self.ref_selection.len() == 0 && self.ref_selection.anchor.is_none() {
            return;
        }
        let order = self.visible_refs();
        self.ref_selection.retain_visible(&order);
    }

    pub fn apply_snapshot(&mut self, snapshot: Arc<RepoSnapshot>) {
        self.selection = restore_selection(self.selection.as_ref(), &snapshot);
        self.multi.retain(&selectable_ids(&snapshot));
        if let Some(details) = &self.details {
            if snapshot.commit(&details.id).is_none() {
                self.details = None;
            }
        }
        let was_running = crate::operation::status(self).is_some();
        self.snapshot = snapshot;
        self.record_operation_conflicts(was_running);
        self.prune_ref_selection();
        self.refreshing = false;
        self.suppress_watch = false;
        self.error = None;
        self.prefill_message();
        // A resolved (or no longer conflicted) file closes its conflict view.
        if let CenterView::Conflict(conflict) = &self.center {
            if !self.conflicted_files().contains(&conflict.path.as_str()) {
                self.center = CenterView::Graph;
            }
        }
    }

    /// More than one commit is selected (Ctrl/Shift).
    pub fn is_multi(&self) -> bool {
        self.multi.len() > 1
    }

    /// Selected commit id, if a commit (not WIP) is selected.
    pub fn selected_commit(&self) -> Option<&str> {
        match &self.selection {
            Some(Selection::Commit(id)) => Some(id),
            _ => None,
        }
    }
}

/// The commits `ids` form an unbroken first-parent chain ending at HEAD's
/// branch history (squash candidates: contiguous, no merges).
pub fn contiguous_on_head(snapshot: &RepoSnapshot, ids: &[String]) -> bool {
    let Some(head) = snapshot.head.target() else {
        return false;
    };
    // Walk HEAD's first-parent chain and look for the selection as one run.
    let mut chain = Vec::new();
    let mut current = Some(head.to_owned());
    while let Some(id) = current {
        let Some(commit) = snapshot.commit(&id) else {
            break;
        };
        chain.push(id.clone());
        current = commit.parents.first().cloned();
        if chain.len() > 10_000 {
            break;
        }
    }
    let Some(start) = chain.iter().position(|id| ids.contains(id)) else {
        return false;
    };
    let run = &chain[start..(start + ids.len()).min(chain.len())];
    run.len() == ids.len()
        && run.iter().all(|id| ids.contains(id))
        && run
            .iter()
            .all(|id| snapshot.commit(id).is_some_and(|c| c.parents.len() <= 1))
}

/// The message of commit `id` can be changed: it is HEAD (amend), or an
/// older commit on HEAD's first-parent line with a parent and no merge
/// between it and HEAD (reworded by an interactive rebase).
pub fn rewordable(snapshot: &RepoSnapshot, id: &str) -> bool {
    let Some(head) = snapshot.head.target() else {
        return false;
    };
    if head == id {
        return true;
    }
    let mut current = head;
    loop {
        let Some(commit) = snapshot.commit(current) else {
            return false;
        };
        if commit.parents.len() > 1 {
            return false;
        }
        if current == id {
            return commit.parents.len() == 1;
        }
        match commit.parents.first() {
            Some(parent) => current = parent,
            None => return false,
        }
    }
}

/// The selected commits can be squashed: more than one, contiguous on
/// HEAD's first-parent line, and rewordable from the oldest of them.
pub fn squashable(snapshot: &RepoSnapshot, ids: &[String]) -> bool {
    ids.len() > 1
        && contiguous_on_head(snapshot, ids)
        && ids
            .iter()
            .filter_map(|id| snapshot.index.get(id).map(|&row| (row, id)))
            .max_by_key(|(row, _)| *row)
            .is_some_and(|(_, oldest)| rewordable(snapshot, oldest))
}

/// Commit ids that can be part of a multi-selection, in graph order
/// (stashes are excluded).
pub fn selectable_ids(snapshot: &RepoSnapshot) -> Vec<&str> {
    snapshot
        .commits
        .iter()
        .map(|c| c.id.as_str())
        .filter(|id| !snapshot.is_stash(id))
        .collect()
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
            encoding: None,
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

    /// Inline diff of a 40-line file with changes at lines 5 and 30.
    fn inline_diff() -> DiffView {
        let old: String = (1..=40).map(|i| format!("line {i}\n")).collect();
        let new = old.replace("line 5\n", "five\n").replace("line 30\n", "");
        let text =
            git_core::text_diff_with(old.as_bytes(), new.as_bytes(), git_core::DiffContext::Full);
        let mut view = DiffView::new(DiffRequest {
            kind: git_core::DiffKind::Unstaged,
            path: "f".into(),
            old_path: None,
        });
        view.layout = DiffLayout::Inline;
        view.set_diff(Ok(FileDiff {
            old_path: Some("f".into()),
            new_path: Some("f".into()),
            binary: false,
            too_large: false,
            encoding: None,
            hunks: text.hunks,
        }));
        view
    }

    #[test]
    fn inline_rows_have_a_bar_above_each_block_and_no_hunk_header() {
        let view = inline_diff();
        // 40 old lines + 1 added = 41 lines, 2 bars.
        assert_eq!(view.rows.len(), 43);
        assert!(!view.rows.iter().any(|r| matches!(r, DiffRow::Header(_))));
        assert_eq!(view.block_rows, [4, 31]);
        assert_eq!(view.rows[4], DiffRow::Block(0));
        assert_eq!(view.rows[5], DiffRow::Line(0, 4), "first change of block 0");
        assert_eq!(view.rows[31], DiffRow::Block(1));
        // Lines of a block can be clicked like in Hunk mode.
        let mut view = view;
        view.click(5, ClickMode::Replace);
        assert_eq!(view.selection.iter().copied().collect::<Vec<_>>(), [(0, 4)]);
        view.click(4, ClickMode::Replace);
        assert_eq!(view.selection.len(), 1, "bars are not selectable");
    }

    #[test]
    fn split_rows_pair_sides_and_keep_the_blocks() {
        let inline = inline_diff();
        let mut view = inline_diff();
        view.layout = DiffLayout::Split;
        let file = match &view.diff {
            Loadable::Ready(file) => Ok(file.clone()),
            _ => unreachable!(),
        };
        view.set_diff(file);
        assert_eq!(view.blocks, inline.blocks, "same blocks as Inline");
        // Line 5 changed: one paired row; line 30 removed: right side empty.
        // 40 old lines + bars - 1 (the changed pair shares a row).
        assert_eq!(view.rows.len(), 42);
        assert_eq!(view.rows[4], DiffRow::Block(0));
        assert_eq!(view.rows[5], DiffRow::Split(0, Some(4), Some(5)));
        assert_eq!(view.rows[30], DiffRow::Block(1));
        assert_eq!(view.rows[31], DiffRow::Split(0, Some(30), None));
        assert_eq!(view.block_rows, [4, 30]);

        // Removed lines are clicked on the left, added ones on the right.
        view.click_side(5, Some(false), ClickMode::Replace);
        assert_eq!(view.selection.iter().copied().collect::<Vec<_>>(), [(0, 4)]);
        view.click_side(5, Some(true), ClickMode::Toggle);
        assert_eq!(
            view.selection.iter().copied().collect::<Vec<_>>(),
            [(0, 4), (0, 5)]
        );
        // Empty and context cells select nothing.
        view.click_side(31, Some(true), ClickMode::Replace);
        view.click_side(6, Some(false), ClickMode::Replace);
        assert_eq!(view.selection.len(), 2);
        // A range covers both sides of the rows in between.
        view.click_side(5, Some(false), ClickMode::Replace);
        view.click_side(31, Some(false), ClickMode::Range);
        assert_eq!(
            view.selection.iter().copied().collect::<Vec<_>>(),
            [(0, 4), (0, 5), (0, 30)]
        );
    }

    #[test]
    fn read_only_diffs_have_no_block_bars() {
        let mut view = inline_diff();
        view.request.kind = git_core::DiffKind::Commit("c".into());
        let file = match &view.diff {
            Loadable::Ready(file) => Ok(file.clone()),
            _ => unreachable!(),
        };
        for layout in [DiffLayout::Inline, DiffLayout::Split] {
            view.layout = layout;
            view.set_diff(file.clone());
            assert!(!view.rows.iter().any(|r| matches!(r, DiffRow::Block(_))));
            assert_eq!(view.blocks.len(), 2, "navigation still knows the blocks");
            // The first changed row of each block.
            assert!(matches!(
                view.rows[view.block_rows[0]],
                DiffRow::Line(0, 4) | DiffRow::Split(0, Some(4), _)
            ));
        }
    }

    #[test]
    fn change_navigation_targets() {
        let view = inline_diff();
        // Jumps keep 3 rows above the bar.
        assert_eq!(view.change_target(0, 0, true), Some(1));
        assert_eq!(view.change_target(1, 0, true), Some(28));
        assert_eq!(view.change_target(28, 0, true), None, "last change");
        assert_eq!(view.change_target(28, 0, false), Some(1));
        assert_eq!(view.change_target(1, 0, false), None, "first change");
        // Near the end the target is where scrolling stops.
        assert_eq!(view.change_target(1, 20, true), Some(23));
        assert_eq!(view.change_target(23, 20, true), None);
    }

    #[test]
    fn hunk_layout_is_unchanged() {
        let view = loaded_diff();
        assert_eq!(view.rows[0], DiffRow::Header(0));
        assert!(view.blocks.is_empty() && view.block_rows.is_empty());
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
