use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use git_core::{DiffRequest, FileChange, GitBackend, GixBackend};
use iced::widget::pane_grid;
use iced::{event, keyboard, window, Element, Event, Subscription, Task};

use crate::overlay::{ContextMenu, MenuEntry, Modal};
use crate::panes::Panes;
use crate::repo_view::{
    CenterView, CommitDetails, Guarded, LastWrite, Loadable, RepoView, Section, Selection,
    WatchState, WriteOp,
};
use crate::session::{self, Session};
use crate::shortcuts::{self, Shortcut};
use crate::snapshot::RepoSnapshot;
use crate::ui;
use crate::undo::{History, UndoAction};

/// Window focus events closer together than this trigger one refresh.
const FOCUS_DEBOUNCE: Duration = Duration::from_millis(500);
/// Text inputs; keyboard shortcuts pause while one of them has focus.
pub const FILTER_INPUT: &str = "sidebar-filter";
pub const START_PATH_INPUT: &str = "start-path";
pub const COMMIT_SUMMARY_INPUT: &str = "commit-summary";

/// File system events are collected for this long before they are classified.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(300);

/// Session changes are written after this quiet period.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

/// What a discard affects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscardTarget {
    File(git_core::FileChange),
    /// All unstaged changes and untracked files.
    All,
    /// Hunk `index` of the shown diff.
    Hunk(usize),
    /// Selected lines of the shown diff.
    Lines,
}

/// Which list a file row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileArea {
    Unstaged,
    Staged,
    /// Files of a commit (read-only).
    Commit,
}

/// Stable tab identity; async results are routed by it, not by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(u64);

pub enum TabState {
    Start,
    Loading { path: PathBuf },
    Ready(Box<RepoView>),
    Failed { path: PathBuf, error: String },
}

pub struct Tab {
    pub id: TabId,
    pub state: TabState,
    /// Incremented per load; results of older loads are dropped.
    generation: u64,
}

impl Tab {
    pub fn title(&self) -> String {
        match &self.state {
            TabState::Start => "New Tab".into(),
            TabState::Ready(view) => view.name().to_owned(),
            TabState::Loading { path } | TabState::Failed { path, .. } => {
                path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                )
            }
        }
    }

    pub fn view(&self) -> Option<&RepoView> {
        match &self.state {
            TabState::Ready(view) => Some(view),
            _ => None,
        }
    }

    /// Repository path shown in this tab (root once loaded).
    pub fn path(&self) -> Option<&std::path::Path> {
        match &self.state {
            TabState::Start => None,
            TabState::Ready(view) => Some(view.backend.path()),
            TabState::Loading { path } | TabState::Failed { path, .. } => Some(path),
        }
    }

    fn view_mut(&mut self) -> Option<&mut RepoView> {
        match &mut self.state {
            TabState::Ready(view) => Some(view),
            _ => None,
        }
    }
}

/// Backend and first snapshot of a freshly opened repository.
#[derive(Clone)]
pub struct Opened {
    backend: Arc<dyn GitBackend>,
    snapshot: Arc<RepoSnapshot>,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("path", &self.backend.path())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(TabId),
    TabClosed(TabId),
    TabOpened,
    PaneResized(pane_grid::ResizeEvent),
    FilterChanged(String),
    SectionToggled(Section),
    RefClicked(String),
    WipSelected,
    CommitSelected(String),
    DiffOpened(DiffRequest),
    DiffLoaded {
        tab: TabId,
        request: DiffRequest,
        result: Result<git_core::FileDiff, git_core::Error>,
    },
    /// Fetching a blob missing from a partial clone finished.
    ObjectFetched {
        tab: TabId,
        request: DiffRequest,
        result: Result<(), String>,
    },
    DiffScrolled {
        offset: f32,
        height: f32,
    },
    /// Click on a diff line (display row index).
    DiffLineClicked(usize),
    /// Stage or unstage hunk `index` of the shown diff.
    ApplyHunk(usize),
    /// Stage or unstage the selected lines of the shown diff.
    ApplySelectedLines,
    ModifiersChanged(keyboard::Modifiers),
    GraphScrolled {
        offset: f32,
        height: f32,
    },
    DiffClosed,
    Retry(TabId),
    Refresh,
    Shortcut(Shortcut),
    /// A shortcut after checking whether a text input has focus.
    ShortcutChecked {
        shortcut: Shortcut,
        text_input_focused: bool,
    },
    /// Show the native folder dialog.
    OpenRepository,
    FolderPicked(Option<PathBuf>),
    /// Path typed on the start screen (fallback when no folder dialog is available).
    StartPathChanged(String),
    StartPathSubmitted,
    WindowFocused,
    FocusRefreshDue(u64),
    RepoOpened {
        tab: TabId,
        generation: u64,
        result: Result<Opened, String>,
    },
    SnapshotLoaded {
        tab: TabId,
        generation: u64,
        result: Result<Arc<RepoSnapshot>, String>,
    },
    CommitChangesLoaded {
        tab: TabId,
        id: String,
        result: Result<Vec<FileChange>, String>,
    },
    SessionLoaded(session::Loaded),
    WatcherStarted(TabId),
    WatcherFailed {
        tab: TabId,
        error: String,
    },
    /// Classified file system events of a tab.
    FsChanged {
        tab: TabId,
        status: bool,
        refs: bool,
    },
    /// Result of comparing refs and HEAD with the snapshot.
    RefsChecked {
        tab: TabId,
        changed: Result<bool, String>,
    },
    StatusLoaded {
        tab: TabId,
        generation: u64,
        result: Result<git_core::WorkingTree, String>,
    },
    /// Button `index` of the open dialog was pressed.
    ModalAction(usize),
    ModalClosed,
    /// Entry `index` of the open context menu was chosen.
    MenuSelected(usize),
    MenuClosed,
    /// Right click on a file row.
    FileContextMenu {
        file: git_core::FileChange,
        area: FileArea,
        position: iced::Point,
    },
    Stage(git_core::FileChange),
    Unstage(git_core::FileChange),
    StageAll,
    UnstageAll,
    CopyToClipboard(String),
    WindowResized(iced::Size),
    GitDetected(git_core::GitAvailability),
    DismissNotice,
    /// A write operation finished (success or Git error).
    WriteFinished {
        tab: TabId,
        label: String,
        result: Result<Option<History>, String>,
    },
    /// Run the last write of the tab again (after a lock error).
    RetryWrite(TabId),
    Undo,
    Redo,
    /// Toolbar "Stash": stash all changes including untracked files.
    StashPush,
    StashPop(String),
    StashApply(String),
    /// Ask before deleting a stash.
    StashDrop(String),
    StashDropConfirmed(String),
    StashContextMenu {
        id: String,
        position: iced::Point,
    },
    SummaryChanged(String),
    DescriptionEdited(iced::widget::text_editor::Action),
    /// Commit the staged changes with the entered message.
    CommitRequested,
    AmendToggled(bool),
    /// Ask before discarding (file, all, hunk or selected lines).
    Discard(DiscardTarget),
    /// Confirmed in the dialog.
    DiscardConfirmed(DiscardTarget),
    /// Append the file to `.gitignore`.
    IgnoreFile(String),
    /// Show the file in the system file manager.
    OpenFileLocation(String),
    /// Transient message for the status bar.
    StatusText(String),
    SaveDue(u64),
    Saved(Result<(), String>),
    OpenRecent(PathBuf),
    RemoveRecent(TabId),
    CloseRequested(window::Id),
    Exit,
}

pub struct App {
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub panes: Panes,
    pub status: String,
    /// Content of the path input on the start screen.
    pub start_path: String,
    /// Recent repositories and the last persisted state.
    pub session: Session,
    /// Result of looking for `git`; `None` until detected.
    pub git: Option<git_core::GitAvailability>,
    /// App-wide one-time notice (e.g. Git missing), dismissible.
    pub notice: Option<String>,
    /// Open modal dialog; blocks the rest of the window.
    pub modal: Option<Modal>,
    pub context_menu: Option<ContextMenu>,
    /// Current window size, used to keep menus inside the window.
    pub window_size: iced::Size,
    /// Keyboard modifiers held right now (for Shift/Ctrl clicks).
    modifiers: keyboard::Modifiers,
    /// Where the session is stored; `None` disables persistence (tests).
    session_path: Option<PathBuf>,
    /// The session file was read; before that nothing is written.
    session_loaded: bool,
    /// Paths from the command line, opened once the session is restored.
    pending_paths: Vec<PathBuf>,
    save_seq: u64,
    next_tab: u64,
    focus_seq: u64,
    /// Tab and center content (graph or diff) visible after the last update;
    /// a change means the scrollable was recreated and needs its offset restored.
    shown_center: Option<(TabId, bool)>,
}

impl App {
    /// Restores the session in the background, then opens `paths` from the command line.
    pub fn new(paths: Vec<PathBuf>, session_path: Option<PathBuf>) -> (Self, Task<Message>) {
        let mut app = Self {
            tabs: Vec::new(),
            active: 0,
            panes: Panes::new(Default::default()),
            status: String::new(),
            start_path: String::new(),
            session: Session::default(),
            git: None,
            notice: None,
            modal: None,
            context_menu: None,
            window_size: iced::Size::new(1400.0, 860.0),
            modifiers: keyboard::Modifiers::default(),
            session_path: session_path.clone(),
            session_loaded: false,
            pending_paths: paths,
            save_seq: 0,
            next_tab: 0,
            focus_seq: 0,
            shown_center: None,
        };
        app.add_tab(TabState::Start);
        let task = match session_path {
            Some(path) => Task::perform(blocking(move || Ok(session::load(&path))), |loaded| {
                Message::SessionLoaded(loaded.unwrap_or_else(|error| session::Loaded {
                    session: Session::default(),
                    notice: Some(error),
                }))
            }),
            None => Task::done(Message::SessionLoaded(session::Loaded {
                session: Session::default(),
                notice: None,
            })),
        };
        let detect = Task::perform(blocking(|| Ok(git_core::detect_git())), |result| {
            Message::GitDetected(result.unwrap_or(git_core::GitAvailability::Missing))
        });
        (app, Task::batch([task, detect]))
    }

    /// Why write actions are blocked, if they are (unknown counts as blocked).
    fn write_block(&self) -> Option<String> {
        match &self.git {
            Some(git) => git.reason(),
            None => Some("Checking for Git…".into()),
        }
    }

    /// True (with the reason in the status bar) if the operation in progress
    /// in the active tab forbids `action`.
    fn state_blocked(&mut self, action: Guarded) -> bool {
        match self
            .active_tab()
            .and_then(Tab::view)
            .and_then(|v| v.state_block(action))
        {
            Some(reason) => {
                self.status = reason;
                true
            }
            None => false,
        }
    }

    /// Runs a write operation in the background, one at a time per tab, and
    /// refreshes the tab afterwards.
    fn run_write(
        &mut self,
        tab: TabId,
        label: impl Into<String>,
        op: impl Fn(&dyn git_core::GitWriter) -> git_core::Result<()> + Send + Sync + 'static,
    ) -> Task<Message> {
        self.run_undoable(tab, label, move |git| op(git).map(|()| None))
    }

    /// Like [`Self::run_write`]; the operation may return an undo entry.
    /// Operations are repeatable so a failure caused by a lock can be retried.
    fn run_undoable(
        &mut self,
        tab: TabId,
        label: impl Into<String>,
        op: impl Fn(&dyn git_core::GitWriter) -> git_core::Result<Option<History>>
            + Send
            + Sync
            + 'static,
    ) -> Task<Message> {
        self.start_write(tab, label.into(), Arc::new(op))
    }

    fn start_write(&mut self, tab: TabId, label: String, op: WriteOp) -> Task<Message> {
        if let Some(reason) = self.write_block() {
            self.status = reason;
            return Task::none();
        }
        let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
            return Task::none();
        };
        if let Some(running) = &view.busy {
            let message = format!("Please wait: {running} is still running");
            self.status = message;
            return Task::none();
        }
        view.busy = Some(label.clone());
        view.suppress_watch = true;
        view.last_write = Some(LastWrite {
            label: label.clone(),
            op: Arc::clone(&op),
            clears_message: false,
        });
        let writer = Arc::clone(&view.writer);
        Task::perform(
            blocking(move || op(writer.as_ref()).map_err(|e| e.to_string())),
            move |result| Message::WriteFinished {
                tab,
                label: label.clone(),
                result,
            },
        )
    }

    /// Applies the loaded session: restores tabs and panes, then opens pending paths.
    fn restore(&mut self, loaded: session::Loaded) -> Task<Message> {
        let session::Loaded { session, notice } = loaded;
        self.session_loaded = true;
        if let Some(notice) = notice {
            self.status = notice;
        }
        self.panes.set_ratios(session.panes);
        let mut tasks = Vec::new();
        if !session.tabs.is_empty() {
            // Replace the untouched start tab with the restored tabs.
            if self.tabs.len() == 1 && matches!(self.tabs[0].state, TabState::Start) {
                self.tabs.clear();
            }
            for path in &session.tabs {
                tasks.push(self.open_in_new_tab(path.clone()));
            }
            self.active = session.active.min(self.tabs.len().saturating_sub(1));
        }
        self.session = session;
        for path in std::mem::take(&mut self.pending_paths) {
            tasks.push(self.open_path(path));
        }
        Task::batch(tasks)
    }

    /// Session as it should be on disk for the current state.
    fn current_session(&self) -> Session {
        let mut tabs = Vec::new();
        let mut active = 0;
        for (index, tab) in self.tabs.iter().enumerate() {
            if let Some(path) = tab.path() {
                if index == self.active {
                    active = tabs.len();
                }
                tabs.push(path.to_path_buf());
            }
        }
        Session {
            version: session::VERSION,
            recent: self.session.recent.clone(),
            tabs,
            active,
            panes: self.panes.ratios,
        }
    }

    /// Schedules a debounced save when the persisted state changed.
    fn persist(&mut self) -> Task<Message> {
        if !self.session_loaded {
            return Task::none();
        }
        let current = self.current_session();
        if current == self.session {
            return Task::none();
        }
        self.session = current;
        if self.session_path.is_none() {
            return Task::none();
        }
        self.save_seq += 1;
        let seq = self.save_seq;
        Task::perform(tokio::time::sleep(SAVE_DEBOUNCE), move |()| {
            Message::SaveDue(seq)
        })
    }

    fn save_task(&self) -> Task<Message> {
        let Some(path) = self.session_path.clone() else {
            return Task::done(Message::Saved(Ok(())));
        };
        let json = session::to_json(&self.session);
        Task::perform(
            blocking(move || session::save(&path, &json).map_err(|e| e.to_string())),
            Message::Saved,
        )
    }

    pub fn title(&self) -> String {
        match self.active_tab().and_then(Tab::view) {
            Some(view) => format!("{} – Gitloom", view.name()),
            None => "Gitloom".into(),
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    fn active_view_mut(&mut self) -> Option<&mut RepoView> {
        self.tabs.get_mut(self.active).and_then(Tab::view_mut)
    }

    fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    fn add_tab(&mut self, state: TabState) -> TabId {
        self.next_tab += 1;
        let id = TabId(self.next_tab);
        self.tabs.push(Tab {
            id,
            state,
            generation: 0,
        });
        self.active = self.tabs.len() - 1;
        id
    }

    fn open_in_new_tab(&mut self, path: PathBuf) -> Task<Message> {
        let id = self.add_tab(TabState::Start);
        self.open_in_tab(id, path)
    }

    /// Opens `path`: switches to a tab that already shows it, reuses the
    /// active start tab, or opens a new tab.
    fn open_path(&mut self, path: PathBuf) -> Task<Message> {
        if let Some(index) = self.tabs.iter().position(|t| match &t.state {
            TabState::Ready(view) => same_repo(view.backend.path(), &path),
            TabState::Loading { path: p } | TabState::Failed { path: p, .. } => p == &path,
            TabState::Start => false,
        }) {
            self.active = index;
            return Task::none();
        }
        match self.active_tab() {
            Some(tab) if matches!(tab.state, TabState::Start) => {
                let id = tab.id;
                self.open_in_tab(id, path)
            }
            _ => self.open_in_new_tab(path),
        }
    }

    fn close_tab(&mut self, id: TabId) {
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(index);
            if self.active > index || self.active >= self.tabs.len() {
                self.active = self.active.saturating_sub(1);
            }
            if self.tabs.is_empty() {
                self.add_tab(TabState::Start);
            }
        }
    }

    fn shortcut(&mut self, shortcut: Shortcut) -> Task<Message> {
        // An open dialog or menu only reacts to Esc and Enter.
        let overlay_open = self.modal.is_some() || self.context_menu.is_some();
        if overlay_open && !matches!(shortcut, Shortcut::Close | Shortcut::Confirm) {
            return Task::none();
        }
        let count = self.tabs.len();
        match shortcut {
            Shortcut::NewTab => {
                self.add_tab(TabState::Start);
            }
            Shortcut::CloseTab => {
                if let Some(id) = self.active_tab().map(|t| t.id) {
                    self.close_tab(id);
                }
            }
            Shortcut::NextTab => self.active = (self.active + 1) % count.max(1),
            Shortcut::PreviousTab => self.active = (self.active + count.max(1) - 1) % count.max(1),
            Shortcut::SwitchTab(index) => {
                if index < count {
                    self.active = index;
                }
            }
            Shortcut::OpenRepository => return self.update(Message::OpenRepository),
            Shortcut::Refresh => return self.update(Message::Refresh),
            Shortcut::ToggleSidebar => self.panes.toggle_sidebar(),
            Shortcut::TogglePanel => self.panes.toggle_panel(),
            Shortcut::Navigate(step) => {
                let target = self.active_tab().and_then(Tab::view).and_then(|view| {
                    crate::nav::navigate(&view.snapshot, view.selection.as_ref(), step)
                });
                if let Some(selection) = target {
                    let task = self.select(selection);
                    return Task::batch([task, self.reveal_selection()]);
                }
            }
            Shortcut::StageSelected => {
                if let Some((file, false)) = self.selected_file() {
                    return self.stage_file(file, true);
                }
            }
            Shortcut::UnstageSelected => {
                if let Some((file, true)) = self.selected_file() {
                    return self.stage_file(file, false);
                }
            }
            Shortcut::Undo => return self.undo_redo(true),
            Shortcut::Redo => return self.undo_redo(false),
            Shortcut::Commit => return self.commit(false),
            Shortcut::StageAllAndCommit => return self.commit(true),
            Shortcut::FocusSummary => {
                // The commit box belongs to the WIP row.
                if let Some(view) = self.active_view_mut() {
                    if view.snapshot.has_wip {
                        view.selection = Some(Selection::Wip);
                        view.center = CenterView::Graph;
                    }
                }
                return iced::widget::operation::focus(COMMIT_SUMMARY_INPUT);
            }
            Shortcut::StageAll => return self.update(Message::StageAll),
            Shortcut::UnstageAll => return self.update(Message::UnstageAll),
            Shortcut::FocusFilter => {
                if !self.panes.sidebar_visible {
                    self.panes.toggle_sidebar();
                }
                return iced::widget::operation::focus(FILTER_INPUT);
            }
            Shortcut::Close => {
                if self.modal.take().is_some() || self.context_menu.take().is_some() {
                    return Task::none();
                }
                if let Some(view) = self.active_view_mut() {
                    view.center = CenterView::Graph;
                }
            }
            Shortcut::Confirm => {
                if let Some(modal) = self.modal.take() {
                    if let Some(message) = modal.default_action() {
                        return self.update(message);
                    }
                }
            }
        }
        Task::none()
    }

    /// Opens the repository containing `path` in tab `id`.
    fn open_in_tab(&mut self, id: TabId, path: PathBuf) -> Task<Message> {
        let Some(tab) = self.tab_mut(id) else {
            return Task::none();
        };
        tab.generation += 1;
        let generation = tab.generation;
        tab.state = TabState::Loading { path: path.clone() };
        Task::perform(
            blocking(move || {
                let backend: Arc<dyn GitBackend> =
                    Arc::new(GixBackend::discover(&path).map_err(|e| e.to_string())?);
                let snapshot = Arc::new(RepoSnapshot::load(backend.as_ref())?);
                Ok(Opened { backend, snapshot })
            }),
            move |result| Message::RepoOpened {
                tab: id,
                generation,
                result,
            },
        )
    }

    /// Reloads the snapshot of tab `id` unless a load is already running.
    fn refresh(&mut self, id: TabId) -> Task<Message> {
        let Some(tab) = self.tab_mut(id) else {
            return Task::none();
        };
        let TabState::Ready(view) = &mut tab.state else {
            return Task::none();
        };
        if view.refreshing {
            return Task::none();
        }
        view.refreshing = true;
        tab.generation += 1;
        let generation = tab.generation;
        let backend = Arc::clone(&view.backend);
        Task::perform(
            blocking(move || RepoSnapshot::load(backend.as_ref()).map(Arc::new)),
            move |result| Message::SnapshotLoaded {
                tab: id,
                generation,
                result,
            },
        )
    }

    /// Stages (or unstages) one file in the active tab. If its diff is open,
    /// the selection moves to the next file of the same list.
    fn stage_file(&mut self, file: git_core::FileChange, stage: bool) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let mut paths = vec![file.path.clone()];
        paths.extend(file.old_path.clone());
        let label = format!("{} {}", if stage { "Stage" } else { "Unstage" }, file.path);
        let write = self.run_write(tab, label, move |git| {
            if stage {
                git.stage(&paths)
            } else {
                git.unstage(&paths)
            }
        });

        // Only move on if the write was accepted and this file is the one shown.
        let Some(view) = self.active_view_mut() else {
            return write;
        };
        if view.busy.is_none() {
            return write;
        }
        let shown = matches!(&view.center, CenterView::Diff(diff)
            if diff.request.path == file.path
                && matches!(diff.request.kind, git_core::DiffKind::Staged) != stage);
        if !shown {
            return write;
        }
        let tree = &view.snapshot.working_tree;
        let list = if stage { &tree.unstaged } else { &tree.staged };
        let next = crate::repo_view::next_file_after(list, &file.path)
            .map(|next| crate::repo_view::diff_request(next, !stage));
        match next {
            Some(request) => Task::batch([write, self.update(Message::DiffOpened(request))]),
            None => {
                view.center = CenterView::Graph;
                write
            }
        }
    }

    /// Stages (or unstages) the lines of the shown diff for which `pick`
    /// is true, then reloads the diff at the same position.
    fn apply_diff_lines(
        &mut self,
        pick: impl Fn(&crate::repo_view::DiffView, usize, usize) -> bool,
    ) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        let CenterView::Diff(diff) = &view.center else {
            return Task::none();
        };
        let (Some(staged), Loadable::Ready(file)) = (diff.staged(), &diff.diff) else {
            return Task::none();
        };
        let direction = if staged {
            git_core::PatchDirection::Unstage
        } else {
            git_core::PatchDirection::Stage
        };
        let Some(patch) = git_core::build_patch(file, direction, |h, l| pick(diff, h, l)) else {
            return Task::none();
        };
        let label = format!(
            "{} lines of {}",
            if staged { "Unstage" } else { "Stage" },
            diff.request.path
        );
        let task = self.run_write(tab, label, move |git| git.apply_cached(&patch, staged));
        if let Some(view) = self.active_view_mut() {
            if view.busy.is_some() {
                view.reload_diff = true;
            }
        }
        task
    }

    /// Commits the staged changes (optionally staging everything first).
    fn commit(&mut self, stage_all: bool) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        if let Some(reason) = commit_blocker(view, stage_all) {
            self.status = reason;
            return Task::none();
        }
        let message = view.commit_message();
        let amend = view.amend;
        let label = if amend { "Amend" } else { "Commit" };
        let task = self.run_undoable(tab, label, move |git| {
            if stage_all {
                git.stage_all()?;
            }
            let before = git_core::head_commit(git)?;
            git.commit(&message, amend)?;
            let after = git_core::head_commit(git)?
                .ok_or_else(|| git_core::Error::Backend("HEAD missing after commit".into()))?;
            Ok(Some(History::Push(UndoAction::Commit {
                amend,
                before,
                after,
            })))
        });
        if let Some(view) = self.active_view_mut() {
            if view.busy.is_some() {
                view.clear_message_after_write = true;
            }
        }
        task
    }

    /// Undoes (or redoes) the most recent action of the active tab. A failed
    /// attempt drops the entry and explains why.
    fn undo_redo(&mut self, undo: bool) -> Task<Message> {
        if let Some(reason) = self.write_block() {
            self.status = reason;
            return Task::none();
        }
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        if view.busy.is_some() {
            return Task::none();
        }
        let stack = if undo { &view.undo } else { &view.redo };
        if let Some(UndoAction::Commit { .. }) = stack.last() {
            if let Some(reason) = view.state_block(Guarded::MoveHead) {
                self.status = reason;
                return Task::none();
            }
        }
        let stack = if undo { &mut view.undo } else { &mut view.redo };
        let Some(action) = stack.pop() else {
            return Task::none();
        };
        let root = view.backend.path().to_path_buf();
        let label = format!("{} {}", if undo { "Undo" } else { "Redo" }, action.label());
        self.run_undoable(tab, label, move |git| {
            let next = match action.clone() {
                UndoAction::Commit {
                    amend,
                    before,
                    after,
                } => {
                    if undo {
                        git_core::move_head(git, Some(&after), before.as_deref(), "gitloom: undo")?;
                    } else {
                        git_core::move_head(git, before.as_deref(), Some(&after), "gitloom: redo")?;
                    }
                    UndoAction::Commit {
                        amend,
                        before,
                        after,
                    }
                }
                UndoAction::Discard { label, snapshot } => {
                    // Restoring returns the replaced state, which reverts the restore.
                    let replaced = git_core::restore_snapshot(git, &root, &snapshot)?;
                    UndoAction::Discard {
                        label,
                        snapshot: replaced,
                    }
                }
            };
            Ok(Some(if undo {
                History::Undone(next)
            } else {
                History::Redone(next)
            }))
        })
    }

    /// Runs a confirmed discard in the active tab and records it for undo.
    fn discard(&mut self, target: DiscardTarget) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        let root = view.backend.path().to_path_buf();
        match target {
            DiscardTarget::File(file) => {
                let label = format!("Discard {}", file.path);
                let undo_label = label.clone();
                // A discarded file whose diff is open closes the diff.
                if matches!(&view.center, CenterView::Diff(d) if d.request.path == file.path) {
                    view.center = CenterView::Graph;
                }
                self.run_undoable(tab, label, move |git| {
                    let snapshot =
                        git_core::discard_files(git, &root, std::slice::from_ref(&file))?;
                    Ok(Some(History::Push(UndoAction::Discard {
                        label: undo_label.clone(),
                        snapshot,
                    })))
                })
            }
            DiscardTarget::All => {
                let unstaged = view.snapshot.working_tree.unstaged.clone();
                if matches!(&view.center, CenterView::Diff(d) if d.staged() == Some(false)) {
                    view.center = CenterView::Graph;
                }
                self.run_undoable(tab, "Discard all changes", move |git| {
                    let snapshot = git_core::discard_all(git, &root, &unstaged)?;
                    Ok(Some(History::Push(UndoAction::Discard {
                        label: "Discard all changes".into(),
                        snapshot,
                    })))
                })
            }
            DiscardTarget::Hunk(index) => self.discard_diff_lines(tab, move |_, h, _| h == index),
            DiscardTarget::Lines => self.discard_diff_lines(tab, |diff, h, l| {
                diff.selection.contains(&(h as u32, l as u32))
            }),
        }
    }

    /// Reverts lines of the shown unstaged diff in the working tree.
    fn discard_diff_lines(
        &mut self,
        tab: TabId,
        pick: impl Fn(&crate::repo_view::DiffView, usize, usize) -> bool,
    ) -> Task<Message> {
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        let CenterView::Diff(diff) = &view.center else {
            return Task::none();
        };
        let (Some(false), Loadable::Ready(file)) = (diff.staged(), &diff.diff) else {
            return Task::none();
        };
        let selected: std::collections::HashSet<(usize, usize)> = file
            .hunks
            .iter()
            .enumerate()
            .flat_map(|(h, hunk)| (0..hunk.lines.len()).map(move |l| (h, l)))
            .filter(|&(h, l)| pick(diff, h, l))
            .collect();
        if selected.is_empty() {
            return Task::none();
        }
        let file = file.clone();
        let path = diff.request.path.clone();
        let root = view.backend.path().to_path_buf();
        let backend = Arc::clone(&view.backend);
        let label = format!("Discard lines of {path}");
        let undo_label = label.clone();
        let task = self.run_undoable(tab, label, move |git| {
            let content = backend
                .worktree_content(&path)?
                .ok_or_else(|| git_core::Error::Backend(format!("{path} no longer exists")))?;
            let snapshot = git_core::discard_lines(git, &root, &file, &content, |h, l| {
                selected.contains(&(h, l))
            })?;
            Ok(Some(History::Push(UndoAction::Discard {
                label: undo_label.clone(),
                snapshot,
            })))
        });
        if let Some(view) = self.active_view_mut() {
            if view.busy.is_some() {
                view.reload_diff = true;
            }
        }
        task
    }

    /// Reloads the shown diff after a write, keeping its scroll position.
    fn reload_diff(&mut self, tab: TabId) -> Task<Message> {
        let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
            return Task::none();
        };
        view.reload_diff = false;
        let tree = view.snapshot.working_tree.clone();
        let backend = Arc::clone(&view.backend);
        let CenterView::Diff(diff) = &mut view.center else {
            return Task::none();
        };
        // A partly staged new file is tracked now.
        if diff.request.kind == git_core::DiffKind::Untracked
            && tree
                .unstaged
                .iter()
                .any(|f| f.path == diff.request.path && f.kind != git_core::ChangeKind::Untracked)
        {
            diff.request.kind = git_core::DiffKind::Unstaged;
        }
        load_diff(tab, backend, diff.request.clone())
    }

    /// File whose diff is open in the active tab, with the list it belongs to.
    fn selected_file(&self) -> Option<(git_core::FileChange, bool)> {
        let view = self.active_tab()?.view()?;
        let CenterView::Diff(diff) = &view.center else {
            return None;
        };
        let tree = &view.snapshot.working_tree;
        match diff.request.kind {
            git_core::DiffKind::Staged => tree
                .staged
                .iter()
                .find(|f| f.path == diff.request.path)
                .map(|f| (f.clone(), true)),
            git_core::DiffKind::Unstaged | git_core::DiffKind::Untracked => tree
                .unstaged
                .iter()
                .find(|f| f.path == diff.request.path)
                .map(|f| (f.clone(), false)),
            git_core::DiffKind::Commit(_) => None,
        }
    }

    /// Reloads only the working tree status of tab `id` (watcher, window focus).
    fn refresh_status(&mut self, id: TabId) -> Task<Message> {
        let Some(tab) = self.tab_mut(id) else {
            return Task::none();
        };
        let TabState::Ready(view) = &mut tab.state else {
            return Task::none();
        };
        if view.refreshing || view.status_refreshing {
            return Task::none();
        }
        view.status_refreshing = true;
        tab.generation += 1;
        let generation = tab.generation;
        let backend = Arc::clone(&view.backend);
        Task::perform(
            blocking(move || backend.working_tree().map_err(|e| e.to_string())),
            move |result| Message::StatusLoaded {
                tab: id,
                generation,
                result,
            },
        )
    }

    /// Compares HEAD, refs and stashes with the snapshot; a full reload follows
    /// only if something moved.
    fn check_refs(&mut self, id: TabId) -> Task<Message> {
        let Some(view) = self.tab_mut(id).and_then(Tab::view_mut) else {
            return Task::none();
        };
        let backend = Arc::clone(&view.backend);
        let snapshot = Arc::clone(&view.snapshot);
        Task::perform(
            blocking(move || {
                let head = backend.head().map_err(|e| e.to_string())?;
                let refs = backend.references().map_err(|e| e.to_string())?;
                let stashes = backend.stashes().map_err(|e| e.to_string())?;
                let state = backend.state().map_err(|e| e.to_string())?;
                Ok(refs_changed(&snapshot, &head, &refs, &stashes) || state != snapshot.state)
            }),
            move |changed| Message::RefsChecked { tab: id, changed },
        )
    }

    /// Loads the changed files of the selected commit in the active tab if not loaded yet.
    fn load_details(&mut self) -> Task<Message> {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return Task::none();
        };
        let tab_id = tab.id;
        let Some(view) = tab.view_mut() else {
            return Task::none();
        };
        let Some(id) = view.selected_commit().map(str::to_owned) else {
            return Task::none();
        };
        if view.details.as_ref().is_some_and(|d| d.id == id) {
            return Task::none();
        }
        view.details = Some(CommitDetails {
            id: id.clone(),
            changes: Loadable::Loading,
        });
        let backend = Arc::clone(&view.backend);
        let commit = id.clone();
        Task::perform(
            blocking(move || backend.commit_changes(&commit).map_err(|e| e.to_string())),
            move |result| Message::CommitChangesLoaded {
                tab: tab_id,
                id: id.clone(),
                result,
            },
        )
    }

    fn select(&mut self, selection: Selection) -> Task<Message> {
        if let Some(view) = self.active_view_mut() {
            view.selection = Some(selection);
            view.center = CenterView::Graph;
        }
        self.load_details()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        // Keep each tab's reason for disabled write actions current.
        let block = self.write_block();
        for tab in &mut self.tabs {
            if let Some(view) = tab.view_mut() {
                if view.write_block != block {
                    view.write_block.clone_from(&block);
                }
            }
        }
        // The active tab may have changed or finished loading; details load lazily.
        let details = self.load_details();
        let scroll = self.restore_scroll();
        Task::batch([task, details, scroll, self.persist()])
    }

    /// Re-applies the stored offset when a different graph or diff became visible.
    fn restore_scroll(&mut self) -> Task<Message> {
        let shown = self.active_tab().and_then(|tab| {
            let view = tab.view()?;
            let (is_diff, id, offset) = match &view.center {
                CenterView::Graph => (false, &view.graph_scroll_id, view.scroll.offset),
                CenterView::Diff(diff) => (true, &diff.scroll_id, diff.scroll.offset),
            };
            let task = (offset > 0.0).then(|| scroll_by_id(id.clone(), offset));
            Some(((tab.id, is_diff), task))
        });
        let key = shown.as_ref().map(|(key, _)| *key);
        if key == self.shown_center {
            return Task::none();
        }
        self.shown_center = key;
        shown.and_then(|(_, task)| task).unwrap_or_else(Task::none)
    }

    /// Scrolls the graph so the selected row is visible.
    fn reveal_selection(&mut self) -> Task<Message> {
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        let Some(row) = crate::repo_view::selection_row(view.selection.as_ref(), &view.snapshot)
        else {
            return Task::none();
        };
        match crate::ui::reveal_offset(row, view.scroll.offset, view.scroll.height) {
            Some(offset) => {
                view.scroll.offset = offset;
                scroll_to(view, offset)
            }
            None => Task::none(),
        }
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(id) => {
                if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                    self.active = index;
                }
            }
            Message::TabClosed(id) => self.close_tab(id),
            Message::TabOpened => {
                self.add_tab(TabState::Start);
            }
            Message::PaneResized(event) => self.panes.resize(event.split, event.ratio),
            Message::SessionLoaded(loaded) => return self.restore(loaded),
            Message::GitDetected(git) => {
                self.notice = git.reason();
                self.git = Some(git);
            }
            Message::DismissNotice => self.notice = None,
            Message::StatusText(text) => self.status = text,
            Message::ModalAction(index) => {
                let action = self.modal.take().and_then(|m| m.action(index));
                if let Some(message) = action {
                    return self.update(message);
                }
            }
            Message::ModalClosed => self.modal = None,
            Message::MenuSelected(index) => {
                let action = self.context_menu.take().and_then(|m| m.select(index));
                if let Some(message) = action {
                    return self.update(message);
                }
            }
            Message::MenuClosed => self.context_menu = None,
            Message::FileContextMenu {
                file,
                area,
                position,
            } => {
                let writable = self.write_block().is_none();
                let write_item =
                    |label: &str, shortcut: Option<&str>, message: Message| MenuEntry::Item {
                        label: label.into(),
                        shortcut: shortcut.map(str::to_owned),
                        enabled: writable,
                        message: Box::new(message),
                    };
                let mut entries = Vec::new();
                match area {
                    FileArea::Unstaged => {
                        entries.push(write_item("Stage", Some("S"), Message::Stage(file.clone())));
                        entries.push(write_item(
                            "Discard changes",
                            None,
                            Message::Discard(DiscardTarget::File(file.clone())),
                        ));
                        entries.push(MenuEntry::Separator);
                        entries.push(write_item(
                            "Ignore",
                            None,
                            Message::IgnoreFile(file.path.clone()),
                        ));
                    }
                    FileArea::Staged => {
                        entries.push(write_item(
                            "Unstage",
                            Some("U"),
                            Message::Unstage(file.clone()),
                        ));
                        entries.push(MenuEntry::Separator);
                    }
                    FileArea::Commit => {}
                }
                if area != FileArea::Commit {
                    entries.push(MenuEntry::item(
                        "Open file location",
                        Message::OpenFileLocation(file.path.clone()),
                    ));
                }
                entries.push(MenuEntry::item(
                    "Copy file path",
                    Message::CopyToClipboard(file.path),
                ));
                self.context_menu = Some(ContextMenu::new(position, entries));
            }
            Message::Stage(file) => return self.stage_file(file, true),
            Message::Unstage(file) => return self.stage_file(file, false),
            Message::StageAll => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.run_write(tab, "Stage all changes", |git| git.stage_all());
                }
            }
            Message::UnstageAll => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.run_write(tab, "Unstage all changes", |git| git.unstage_all());
                }
            }
            Message::CopyToClipboard(text) => {
                self.status = "Copied to the clipboard".into();
                return iced::clipboard::write(text);
            }
            Message::WindowResized(size) => self.window_size = size,
            Message::ModifiersChanged(modifiers) => self.modifiers = modifiers,
            Message::DiffLineClicked(row) => {
                let mode = if self.modifiers.shift() {
                    crate::repo_view::ClickMode::Range
                } else if self.modifiers.command() {
                    crate::repo_view::ClickMode::Toggle
                } else {
                    crate::repo_view::ClickMode::Replace
                };
                if let Some(view) = self.active_view_mut() {
                    if let CenterView::Diff(diff) = &mut view.center {
                        diff.click(row, mode);
                    }
                }
            }
            Message::ApplyHunk(index) => return self.apply_diff_lines(move |_, h, _| h == index),
            Message::ApplySelectedLines => {
                return self
                    .apply_diff_lines(|diff, h, l| diff.selection.contains(&(h as u32, l as u32)))
            }
            Message::WriteFinished { tab, label, result } => {
                if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
                    view.busy = None;
                    match &result {
                        Ok(Some(History::Push(action))) => {
                            view.undo.push(action.clone());
                            view.redo.clear();
                        }
                        Ok(Some(History::Undone(action))) => view.redo.push(action.clone()),
                        Ok(Some(History::Redone(action))) => view.undo.push(action.clone()),
                        Ok(None) | Err(_) => {}
                    }
                    let clears_message = std::mem::take(&mut view.clear_message_after_write);
                    if let (Some(last), Err(error)) = (&mut view.last_write, &result) {
                        if git_core::locked_file(error).is_some() {
                            last.clears_message = clears_message;
                        }
                    }
                    // The message stays after a failed commit so nothing is lost.
                    if clears_message && result.is_ok() {
                        view.summary.clear();
                        view.description = iced::widget::text_editor::Content::new();
                        view.amend = false;
                    }
                }
                if let Err(error) = result {
                    self.modal = Some(match git_core::locked_file(&error) {
                        Some(lock) => Modal::locked(&label, lock, Message::RetryWrite(tab)),
                        None => Modal::error(format!("{label} failed"), error),
                    });
                }
                return self.refresh(tab);
            }
            Message::RetryWrite(tab) => {
                let Some(last) = self
                    .tab_mut(tab)
                    .and_then(Tab::view_mut)
                    .and_then(|v| v.last_write.clone())
                else {
                    return Task::none();
                };
                let task = self.start_write(tab, last.label, last.op);
                if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
                    if view.busy.is_some() {
                        view.clear_message_after_write = last.clears_message;
                    }
                }
                return task;
            }
            Message::Discard(target) => {
                let (title, text) = match &target {
                    DiscardTarget::File(file) => (
                        "Discard changes".to_string(),
                        format!("Discard all changes to {}? This cannot be undone outside Gitloom.", file.path),
                    ),
                    DiscardTarget::All => (
                        "Discard all changes".to_string(),
                        "Discard all unstaged changes and delete all untracked files? Staged changes are kept.".to_string(),
                    ),
                    DiscardTarget::Hunk(_) => (
                        "Discard hunk".to_string(),
                        "Discard the changes of this hunk?".to_string(),
                    ),
                    DiscardTarget::Lines => (
                        "Discard lines".to_string(),
                        "Discard the selected lines?".to_string(),
                    ),
                };
                self.modal = Some(Modal::confirm(
                    title,
                    text,
                    "Discard",
                    true,
                    Message::DiscardConfirmed(target),
                ));
            }
            Message::DiscardConfirmed(target) => return self.discard(target),
            Message::Undo => return self.undo_redo(true),
            Message::Redo => return self.undo_redo(false),
            Message::StashPush | Message::StashPop(_) | Message::StashApply(_)
                if self.state_blocked(Guarded::Stash) => {}
            Message::StashPush => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.run_write(tab, "Stash", |git| git.stash_push(None));
                }
            }
            Message::StashPop(id) => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.run_write(tab, "Pop stash", move |git| git.stash_pop(&id));
                }
            }
            Message::StashApply(id) => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.run_write(tab, "Apply stash", move |git| git.stash_apply(&id));
                }
            }
            Message::StashDrop(id) => {
                let name = self
                    .active_tab()
                    .and_then(Tab::view)
                    .and_then(|v| v.snapshot.stashes.iter().find(|s| s.id == id))
                    .map_or_else(|| "this stash".to_owned(), |s| format!("\"{}\"", s.message));
                self.modal = Some(Modal::confirm(
                    "Delete stash",
                    format!("Delete {name}? Its changes are lost."),
                    "Delete",
                    true,
                    Message::StashDropConfirmed(id),
                ));
            }
            Message::StashDropConfirmed(id) => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    if let Some(view) = self.active_view_mut() {
                        if view.selected_commit() == Some(id.as_str()) {
                            view.selection = None;
                        }
                    }
                    return self.run_write(tab, "Delete stash", move |git| git.stash_drop(&id));
                }
            }
            Message::StashContextMenu { id, position } => {
                let writable = self.write_block().is_none();
                let restorable = writable
                    && self
                        .active_tab()
                        .and_then(Tab::view)
                        .is_some_and(|v| v.state_block(Guarded::Stash).is_none());
                let item = |label: &str, enabled: bool, message: Message| MenuEntry::Item {
                    label: label.into(),
                    shortcut: None,
                    enabled,
                    message: Box::new(message),
                };
                self.context_menu = Some(ContextMenu::new(
                    position,
                    vec![
                        item("Apply stash", restorable, Message::StashApply(id.clone())),
                        item("Pop stash", restorable, Message::StashPop(id.clone())),
                        MenuEntry::Separator,
                        item("Delete stash", writable, Message::StashDrop(id)),
                    ],
                ));
            }
            Message::SummaryChanged(summary) => {
                if let Some(view) = self.active_view_mut() {
                    view.summary = summary;
                }
            }
            Message::DescriptionEdited(action) => {
                if let Some(view) = self.active_view_mut() {
                    view.description.perform(action);
                }
            }
            Message::CommitRequested => return self.commit(false),
            Message::AmendToggled(true) if self.state_blocked(Guarded::Amend) => {}
            Message::AmendToggled(amend) => {
                if let Some(view) = self.active_view_mut() {
                    view.set_amend(amend);
                }
            }
            Message::IgnoreFile(path) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let Some(root) = self
                    .active_tab()
                    .and_then(Tab::path)
                    .map(|p| p.to_path_buf())
                else {
                    return Task::none();
                };
                return self.run_write(tab, format!("Ignore {path}"), move |_| {
                    append_to_gitignore(&root, &path)
                });
            }
            Message::OpenFileLocation(path) => {
                if let Some(root) = self
                    .active_tab()
                    .and_then(Tab::path)
                    .map(|p| p.to_path_buf())
                {
                    return Task::perform(
                        blocking(move || reveal_in_file_manager(&root.join(&path))),
                        |result| Message::StatusText(result.err().unwrap_or_default()),
                    );
                }
            }
            Message::SaveDue(seq) => {
                if seq == self.save_seq {
                    return self.save_task();
                }
            }
            Message::Saved(Ok(())) => {}
            Message::Saved(Err(error)) => {
                self.status = format!("Could not save the session: {error}");
            }
            Message::OpenRecent(path) => return self.open_path(path),
            Message::RemoveRecent(id) => {
                if let Some(path) = self.tabs.iter().find(|t| t.id == id).and_then(Tab::path) {
                    let path = path.to_path_buf();
                    self.session.remove_recent(&path);
                    self.close_tab(id);
                }
            }
            Message::CloseRequested(window) => {
                // Write the latest state before leaving.
                if self.session_loaded {
                    self.session = self.current_session();
                }
                return self
                    .save_task()
                    .chain(window::close(window))
                    .chain(Task::done(Message::Exit));
            }
            Message::Exit => return iced::exit(),
            Message::FilterChanged(filter) => {
                if let Some(view) = self.active_view_mut() {
                    view.filter = filter;
                }
            }
            Message::SectionToggled(section) => {
                if let Some(view) = self.active_view_mut() {
                    if !view.collapsed.remove(&section) {
                        view.collapsed.insert(section);
                    }
                }
            }
            Message::RefClicked(name) => {
                let target = self.active_tab().and_then(Tab::view).and_then(|view| {
                    view.snapshot
                        .refs
                        .iter()
                        .find(|r| r.name == name)
                        .map(|r| r.target.clone())
                });
                if let Some(target) = target {
                    let task = self.select(Selection::Commit(target));
                    return Task::batch([task, self.reveal_selection()]);
                }
            }
            Message::WipSelected => return self.select(Selection::Wip),
            Message::CommitSelected(id) => {
                let task = self.select(Selection::Commit(id));
                return Task::batch([task, self.reveal_selection()]);
            }
            Message::DiffOpened(request) => {
                let Some(tab) = self.tabs.get_mut(self.active) else {
                    return Task::none();
                };
                let tab_id = tab.id;
                let Some(view) = tab.view_mut() else {
                    return Task::none();
                };
                view.center =
                    CenterView::Diff(Box::new(crate::repo_view::DiffView::new(request.clone())));
                return load_diff(tab_id, Arc::clone(&view.backend), request);
            }
            Message::DiffLoaded {
                tab,
                request,
                result,
            } => {
                let can_fetch = self.write_block().is_none();
                let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
                    return Task::none();
                };
                let writer = Arc::clone(&view.writer);
                let CenterView::Diff(diff) = &mut view.center else {
                    return Task::none();
                };
                if diff.request != request {
                    return Task::none();
                }
                match result {
                    // A blob of a partial clone: Git fetches it, then reload.
                    // Each object is fetched once, so a repeated miss fails.
                    Err(git_core::Error::MissingObject {
                        id,
                        partial_clone: true,
                    }) if can_fetch && !diff.fetched.contains(&id) => {
                        diff.fetched.push(id.clone());
                        diff.fetching = true;
                        return Task::perform(
                            blocking(move || writer.fetch_object(&id).map_err(|e| e.to_string())),
                            move |result| Message::ObjectFetched {
                                tab,
                                request: request.clone(),
                                result,
                            },
                        );
                    }
                    result => diff.set_diff(result.map_err(|e| e.to_string())),
                }
            }
            Message::ObjectFetched {
                tab,
                request,
                result,
            } => {
                let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
                    return Task::none();
                };
                let backend = Arc::clone(&view.backend);
                let CenterView::Diff(diff) = &mut view.center else {
                    return Task::none();
                };
                if diff.request != request {
                    return Task::none();
                }
                diff.fetching = false;
                match result {
                    Ok(()) => return load_diff(tab, backend, request),
                    Err(error) => {
                        diff.set_diff(Err(git_core::Error::MissingObject {
                            id: String::new(),
                            partial_clone: true,
                        }
                        .to_string()));
                        self.modal = Some(Modal::error("Fetching missing content failed", error));
                    }
                }
            }
            Message::DiffScrolled { offset, height } => {
                if let Some(view) = self.active_view_mut() {
                    if let CenterView::Diff(diff) = &mut view.center {
                        diff.scroll = crate::repo_view::Scroll { offset, height };
                    }
                }
            }
            Message::GraphScrolled { offset, height } => {
                if let Some(view) = self.active_view_mut() {
                    view.scroll = crate::repo_view::Scroll { offset, height };
                }
            }
            Message::DiffClosed => {
                if let Some(view) = self.active_view_mut() {
                    view.center = CenterView::Graph;
                }
            }
            Message::Retry(id) => {
                let path = self
                    .tabs
                    .iter()
                    .find(|t| t.id == id)
                    .and_then(|t| match &t.state {
                        TabState::Failed { path, .. } => Some(path.clone()),
                        _ => None,
                    });
                return match path {
                    Some(path) => self.open_in_tab(id, path),
                    None => self.refresh(id),
                };
            }
            Message::Shortcut(shortcut) if shortcut.works_in_text_input() => {
                return self.shortcut(shortcut)
            }
            Message::Shortcut(shortcut) => {
                // Typing in the filter must not move the selection.
                use iced::widget::operation::is_focused;
                return is_focused(FILTER_INPUT).then(move |filter| {
                    is_focused(START_PATH_INPUT).then(move |path| {
                        is_focused(COMMIT_SUMMARY_INPUT).map(move |summary| {
                            Message::ShortcutChecked {
                                shortcut,
                                text_input_focused: filter || path || summary,
                            }
                        })
                    })
                });
            }
            Message::ShortcutChecked {
                shortcut,
                text_input_focused,
            } => {
                if !text_input_focused {
                    return self.shortcut(shortcut);
                }
            }
            Message::OpenRepository => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Open repository")
                        .pick_folder(),
                    |folder| Message::FolderPicked(folder.map(|f| f.path().to_path_buf())),
                );
            }
            Message::FolderPicked(Some(path)) => return self.open_path(path),
            Message::FolderPicked(None) => {}
            Message::StartPathChanged(path) => self.start_path = path,
            Message::StartPathSubmitted => {
                let input = self.start_path.trim();
                if !input.is_empty() {
                    let path = PathBuf::from(input);
                    let path = std::path::absolute(&path).unwrap_or(path);
                    self.start_path.clear();
                    return self.open_path(path);
                }
            }
            Message::Refresh => {
                if let Some(id) = self.active_tab().map(|t| t.id) {
                    return self.refresh(id);
                }
            }
            Message::WindowFocused => {
                self.focus_seq += 1;
                let seq = self.focus_seq;
                return Task::perform(tokio::time::sleep(FOCUS_DEBOUNCE), move |()| {
                    Message::FocusRefreshDue(seq)
                });
            }
            Message::FocusRefreshDue(seq) => {
                if seq == self.focus_seq {
                    // With a healthy watcher only the status can be stale.
                    let target = self
                        .active_tab()
                        .and_then(|t| t.view().map(|v| (t.id, v.watcher)));
                    return match target {
                        Some((id, WatchState::Running)) => self.refresh_status(id),
                        _ => self.update(Message::Refresh),
                    };
                }
            }
            Message::WatcherStarted(tab) => {
                if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
                    view.watcher = WatchState::Running;
                }
            }
            Message::WatcherFailed { tab, error } => {
                if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
                    if view.watcher != WatchState::Failed {
                        view.watcher = WatchState::Failed;
                        self.notice = Some(format!(
                            "Automatic refresh is unavailable for {} ({error}). Changes appear when the window regains focus or with F5.",
                            view.name()
                        ));
                    }
                }
            }
            Message::FsChanged { tab, status, refs } => {
                let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
                    return Task::none();
                };
                if view.suppress_watch || view.busy.is_some() {
                    return Task::none();
                }
                if refs {
                    return self.check_refs(tab);
                }
                if status {
                    return self.refresh_status(tab);
                }
            }
            Message::RefsChecked { tab, changed } => match changed {
                Ok(true) => return self.refresh(tab),
                // Index changes often come with ref events (commit, checkout).
                Ok(false) => return self.refresh_status(tab),
                Err(_) => return self.refresh(tab),
            },
            Message::StatusLoaded {
                tab,
                generation,
                result,
            } => {
                let Some(tab) = self.tab_mut(tab) else {
                    return Task::none();
                };
                let current = tab.generation == generation;
                if let Some(view) = tab.view_mut() {
                    view.status_refreshing = false;
                    if current {
                        match result.and_then(|tree| view.snapshot.with_working_tree(tree)) {
                            Ok(snapshot) => view.apply_snapshot(Arc::new(snapshot)),
                            Err(error) => view.error = Some(error),
                        }
                    }
                }
            }
            Message::RepoOpened {
                tab,
                generation,
                result,
            } => {
                let Some(tab) = self.tab_mut(tab) else {
                    return Task::none();
                };
                if tab.generation != generation {
                    return Task::none();
                }
                let path = match &tab.state {
                    TabState::Loading { path } => path.clone(),
                    _ => return Task::none(),
                };
                let id = tab.id;
                tab.state = match result {
                    Ok(opened) => {
                        TabState::Ready(Box::new(RepoView::new(opened.backend, opened.snapshot)))
                    }
                    Err(error) => TabState::Failed { path, error },
                };
                if let Some(root) = self
                    .tab_mut(id)
                    .and_then(|t| t.view())
                    .map(|v| v.backend.path().to_path_buf())
                {
                    self.session.touch_recent(&root, unix_now());
                }
                // A subfolder of an already open repository: keep the existing tab.
                let root = self
                    .tab_mut(id)
                    .and_then(|t| t.view())
                    .map(|v| v.backend.path().to_path_buf());
                if let Some(root) = root {
                    if let Some(existing) = self.tabs.iter().position(|t| {
                        t.id != id && t.view().is_some_and(|v| same_repo(v.backend.path(), &root))
                    }) {
                        let existing_id = self.tabs[existing].id;
                        self.close_tab(id);
                        if let Some(index) = self.tabs.iter().position(|t| t.id == existing_id) {
                            self.active = index;
                        }
                        return Task::none();
                    }
                }
                return self.load_details();
            }
            Message::SnapshotLoaded {
                tab,
                generation,
                result,
            } => {
                let Some(tab) = self.tab_mut(tab) else {
                    return Task::none();
                };
                if tab.generation != generation {
                    return Task::none();
                }
                let id = tab.id;
                let mut reload = false;
                if let Some(view) = tab.view_mut() {
                    match result {
                        Ok(snapshot) => view.apply_snapshot(snapshot),
                        Err(error) => {
                            view.refreshing = false;
                            view.error = Some(error);
                        }
                    }
                    reload = view.reload_diff;
                }
                if reload {
                    return Task::batch([self.reload_diff(id), self.load_details()]);
                }
                return self.load_details();
            }
            Message::CommitChangesLoaded { tab, id, result } => {
                if let Some(details) = self
                    .tab_mut(tab)
                    .and_then(Tab::view_mut)
                    .and_then(|v| v.details.as_mut())
                    .filter(|d| d.id == id)
                {
                    details.changes = match result {
                        Ok(changes) => Loadable::Ready(changes),
                        Err(error) => Loadable::Failed(error),
                    };
                }
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // One file watcher per loaded repository, keyed by tab and root.
        let watchers = self.tabs.iter().filter_map(|tab| {
            let view = tab.view()?;
            Some(Subscription::run_with(
                (tab.id.0, view.backend.path().to_path_buf()),
                watch_repository,
            ))
        });
        Subscription::batch(
            [
                event::listen_with(global_event),
                window::close_requests().map(Message::CloseRequested),
            ]
            .into_iter()
            .chain(watchers),
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        ui::view(self)
    }
}

fn scroll_to(view: &RepoView, offset: f32) -> Task<Message> {
    scroll_by_id(view.graph_scroll_id.clone(), offset)
}

fn scroll_by_id(id: iced::widget::Id, offset: f32) -> Task<Message> {
    iced::widget::operation::scroll_to(
        id,
        iced::widget::operation::AbsoluteOffset {
            x: None,
            y: Some(offset),
        },
    )
}

/// Loads the diff of `request` in the background.
fn load_diff(tab: TabId, backend: Arc<dyn GitBackend>, request: DiffRequest) -> Task<Message> {
    let load = request.clone();
    Task::perform(
        blocking(move || Ok(backend.file_diff(&load))),
        move |result: Result<_, String>| Message::DiffLoaded {
            tab,
            request: request.clone(),
            result: result.unwrap_or_else(|e| Err(git_core::Error::Backend(e))),
        },
    )
}

/// Maps window events and keyboard shortcuts no widget handled.
fn global_event(event: Event, status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
        Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
            if status == event::Status::Ignored =>
        {
            shortcuts::shortcut(&key, modifiers).map(Message::Shortcut)
        }
        _ => None,
    }
}

/// Why committing is not possible right now, if it is not.
pub fn commit_blocker(view: &RepoView, stage_all: bool) -> Option<String> {
    let tree = &view.snapshot.working_tree;
    let merging = view.snapshot.merge_message.is_some();
    let has_changes = !tree.staged.is_empty() || (stage_all && !tree.unstaged.is_empty());
    let state = if view.amend {
        Guarded::Amend
    } else {
        Guarded::Commit
    };
    if let Some(reason) = view.state_block(state) {
        Some(reason)
    } else if !has_changes && !merging && !view.amend {
        // Amending may only change the message.
        Some("Stage files to commit".into())
    } else if view.summary.trim().is_empty() {
        Some("Enter a commit summary".into())
    } else {
        None
    }
}

/// True if HEAD, any ref target or the stash list differ from `snapshot`.
fn refs_changed(
    snapshot: &RepoSnapshot,
    head: &git_core::Head,
    refs: &[git_core::Reference],
    stashes: &[git_core::Stash],
) -> bool {
    let key = |r: &git_core::Reference| (r.name.clone(), r.kind, r.target.clone());
    let old: std::collections::HashSet<_> = snapshot.refs.iter().map(key).collect();
    let new: std::collections::HashSet<_> = refs.iter().map(key).collect();
    let stash_ids = |s: &[git_core::Stash]| s.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    snapshot.head != *head || old != new || stash_ids(&snapshot.stashes) != stash_ids(stashes)
}

/// False for pure access events: reading files (our own status and refs
/// loads included) must not trigger a refresh, or refreshes would loop.
fn changes_content(kind: &notify::EventKind) -> bool {
    !matches!(kind, notify::EventKind::Access(_))
}

/// Whether the watcher callback passes `event` on. Access events are
/// dropped right there: a batch of them would still be classified, and
/// classifying reads `.git/info/exclude`, which produces the next access
/// event (endless refresh checks with constant CPU load).
fn forward_event(event: &notify::Result<notify::Event>) -> bool {
    match event {
        Ok(event) => changes_content(&event.kind),
        // Errors (lost events) force a full refresh.
        Err(_) => true,
    }
}

/// Watches one repository and emits classified, debounced change messages.
fn watch_repository(data: &(u64, PathBuf)) -> impl iced::futures::Stream<Item = Message> {
    let (tab, root) = (TabId(data.0), data.1.clone());
    iced::stream::channel(
        16,
        async move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
            use iced::futures::{SinkExt as _, StreamExt as _};
            use notify::Watcher as _;

            let (sender, mut events) = iced::futures::channel::mpsc::unbounded();
            let watcher = notify::recommended_watcher(move |event| {
                if forward_event(&event) {
                    let _ = sender.unbounded_send(event);
                }
            });
            let mut watcher = match watcher {
                Ok(watcher) => watcher,
                Err(error) => {
                    let error = error.to_string();
                    let _ = output.send(Message::WatcherFailed { tab, error }).await;
                    return;
                }
            };
            if let Err(error) = watcher.watch(&root, notify::RecursiveMode::Recursive) {
                let error = error.to_string();
                let _ = output.send(Message::WatcherFailed { tab, error }).await;
                return;
            }
            let discover_root = root.clone();
            let backend = match blocking(move || {
                GixBackend::discover(&discover_root).map_err(|e| e.to_string())
            })
            .await
            {
                Ok(backend) => Arc::new(backend),
                Err(error) => {
                    let _ = output.send(Message::WatcherFailed { tab, error }).await;
                    return;
                }
            };
            let _ = output.send(Message::WatcherStarted(tab)).await;

            while let Some(first) = events.next().await {
                let mut paths = Vec::new();
                let mut overflow = false;
                let mut collect = |event: notify::Result<notify::Event>| match event {
                    Ok(event) if changes_content(&event.kind) => paths.extend(event.paths),
                    Ok(_) => {}
                    // Lost events (e.g. queue overflow): treat everything as changed.
                    Err(_) => overflow = true,
                };
                collect(first);
                tokio::time::sleep(WATCH_DEBOUNCE).await;
                while let Ok(event) = events.try_recv() {
                    collect(event);
                }
                let changes = if overflow {
                    git_core::ChangeSet {
                        status: true,
                        refs: true,
                    }
                } else {
                    let backend = Arc::clone(&backend);
                    blocking(move || Ok(backend.classify_changes(&paths)))
                        .await
                        .unwrap_or(git_core::ChangeSet {
                            status: true,
                            refs: true,
                        })
                };
                if !changes.is_empty() {
                    let message = Message::FsChanged {
                        tab,
                        status: changes.status,
                        refs: changes.refs,
                    };
                    if output.send(message).await.is_err() {
                        break;
                    }
                }
            }
            drop(watcher);
        },
    )
}

/// Appends `path` (anchored at the repository root, special characters
/// escaped) to the root `.gitignore`.
fn append_to_gitignore(root: &std::path::Path, path: &str) -> git_core::Result<()> {
    use std::io::Write as _;
    let file = root.join(".gitignore");
    let existing = std::fs::read(&file).unwrap_or_default();
    let mut entry = String::new();
    if !existing.is_empty() && !existing.ends_with(b"\n") {
        entry.push('\n');
    }
    entry.push_str(&gitignore_pattern(path));
    entry.push('\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .and_then(|mut f| f.write_all(entry.as_bytes()))
        .map_err(|e| git_core::Error::Backend(format!("could not update .gitignore: {e}")))
}

/// Pattern matching exactly `path` relative to the repository root.
fn gitignore_pattern(path: &str) -> String {
    let mut pattern = String::from("/");
    for c in path.chars() {
        if matches!(c, '\\' | '*' | '?' | '[' | '!' | '#') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    // Trailing spaces are ignored by Git unless escaped.
    if pattern.ends_with(' ') {
        let trimmed = pattern.trim_end_matches(' ').len();
        let spaces = pattern.len() - trimmed;
        pattern.truncate(trimmed);
        pattern.push_str(&"\\ ".repeat(spaces));
    }
    pattern
}

/// Opens the system file manager at `path` (selecting it where supported).
fn reveal_in_file_manager(path: &std::path::Path) -> Result<(), String> {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("explorer");
        c.arg(format!("/select,{}", path.display()));
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-R").arg(path);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path.parent().unwrap_or(path));
        c
    };
    command
        .spawn()
        .map(drop)
        .map_err(|e| format!("Could not open the file manager: {e}"))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// True if `path` is `root` or inside it (compared without touching the file system).
fn same_repo(root: &std::path::Path, path: &std::path::Path) -> bool {
    path.starts_with(root)
}

/// Runs blocking work (Git, file I/O) on tokio's blocking pool.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| format!("Background task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_tabs(count: usize) -> App {
        let (mut app, _) = App::new(Vec::new(), None);
        for _ in 1..count {
            app.add_tab(TabState::Start);
        }
        app
    }

    #[test]
    fn starts_with_one_start_tab() {
        let (app, _) = App::new(Vec::new(), None);
        assert_eq!(app.tabs.len(), 1);
        assert!(matches!(app.tabs[0].state, TabState::Start));
        assert_eq!(app.title(), "Gitloom");
    }

    #[test]
    fn command_line_paths_open_in_own_tabs() {
        let (mut app, _) = App::new(vec![PathBuf::from("/a"), PathBuf::from("/b")], None);
        let _ = app.update(Message::SessionLoaded(session::Loaded {
            session: Session::default(),
            notice: None,
        }));
        assert_eq!(app.tabs.len(), 2);
        assert!(app
            .tabs
            .iter()
            .all(|t| matches!(t.state, TabState::Loading { .. })));
        assert_eq!(app.active, 1);
    }

    #[test]
    fn new_and_close_tab() {
        let mut app = app_with_tabs(1);
        let _ = app.update(checked(Shortcut::NewTab, false));
        assert_eq!((app.tabs.len(), app.active), (2, 1));
        let _ = app.update(checked(Shortcut::CloseTab, false));
        assert_eq!((app.tabs.len(), app.active), (1, 0));
        // Closing the last tab leaves a fresh start tab.
        let _ = app.update(checked(Shortcut::CloseTab, false));
        assert_eq!(app.tabs.len(), 1);
    }

    #[test]
    fn closing_a_tab_before_the_active_one_keeps_it_active() {
        let mut app = app_with_tabs(3);
        let active_id = app.tabs[2].id;
        let first = app.tabs[0].id;
        let _ = app.update(Message::TabClosed(first));
        assert_eq!(app.tabs[app.active].id, active_id);
    }

    #[test]
    fn cycle_and_switch_tabs() {
        let mut app = app_with_tabs(3);
        assert_eq!(app.active, 2);
        let _ = app.update(checked(Shortcut::NextTab, false));
        assert_eq!(app.active, 0);
        let _ = app.update(checked(Shortcut::PreviousTab, false));
        assert_eq!(app.active, 2);
        let _ = app.update(checked(Shortcut::SwitchTab(1), false));
        assert_eq!(app.active, 1);
        let _ = app.update(checked(Shortcut::SwitchTab(7), false));
        assert_eq!(app.active, 1);
    }

    #[test]
    fn opening_reuses_the_active_start_tab() {
        let mut app = app_with_tabs(1);
        let _ = app.update(Message::FolderPicked(Some(PathBuf::from("/repo"))));
        assert_eq!(app.tabs.len(), 1);
        assert!(matches!(app.tabs[0].state, TabState::Loading { .. }));

        let _ = app.update(Message::FolderPicked(Some(PathBuf::from("/other"))));
        assert_eq!((app.tabs.len(), app.active), (2, 1));
    }

    #[test]
    fn stale_results_are_dropped() {
        let mut app = app_with_tabs(1);
        let _ = app.update(Message::FolderPicked(Some(PathBuf::from("/repo"))));
        let id = app.tabs[0].id;
        let _ = app.update(Message::RepoOpened {
            tab: id,
            generation: 0,
            result: Err("old".into()),
        });
        assert!(matches!(app.tabs[0].state, TabState::Loading { .. }));
        let _ = app.update(Message::RepoOpened {
            tab: id,
            generation: 1,
            result: Err("not a repository".into()),
        });
        assert!(
            matches!(&app.tabs[0].state, TabState::Failed { error, .. } if error == "not a repository")
        );
    }

    #[test]
    fn typed_path_opens_in_start_tab() {
        let mut app = app_with_tabs(1);
        let _ = app.update(Message::StartPathChanged("  /some/repo ".into()));
        let _ = app.update(Message::StartPathSubmitted);
        assert!(app.start_path.is_empty());
        assert!(
            // On Windows the path gets the current drive.
            matches!(&app.tabs[0].state, TabState::Loading { path } if *path == std::path::absolute("/some/repo").unwrap())
        );
    }

    fn loaded(session: Session) -> Message {
        Message::SessionLoaded(session::Loaded {
            session,
            notice: None,
        })
    }

    #[test]
    fn restores_tabs_active_tab_and_panes() {
        let (mut app, _) = App::new(Vec::new(), None);
        let session = Session {
            tabs: vec![
                PathBuf::from("/a"),
                PathBuf::from("/b"),
                PathBuf::from("/c"),
            ],
            active: 1,
            panes: crate::panes::PaneRatios {
                sidebar: 0.3,
                center: 0.6,
            },
            ..Session::default()
        };
        let _ = app.update(loaded(session.clone()));
        let paths: Vec<_> = app.tabs.iter().filter_map(Tab::path).collect();
        assert_eq!(
            paths,
            [
                std::path::Path::new("/a"),
                std::path::Path::new("/b"),
                std::path::Path::new("/c")
            ]
        );
        assert_eq!(app.active, 1);
        assert_eq!(app.panes.ratios, session.panes);
        assert_eq!(app.current_session(), session);
    }

    #[test]
    fn command_line_path_already_in_session_is_not_duplicated() {
        let (mut app, _) = App::new(vec![PathBuf::from("/b")], None);
        let _ = app.update(loaded(Session {
            tabs: vec![PathBuf::from("/a"), PathBuf::from("/b")],
            active: 0,
            ..Session::default()
        }));
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active, 1);
    }

    #[test]
    fn session_tracks_tabs_and_start_tabs_are_not_stored() {
        let (mut app, _) = App::new(Vec::new(), None);
        let _ = app.update(loaded(Session::default()));
        let _ = app.update(Message::FolderPicked(Some(PathBuf::from("/repo"))));
        let _ = app.update(checked(Shortcut::NewTab, false));
        assert_eq!(app.session.tabs, [PathBuf::from("/repo")]);
        assert_eq!(app.session.active, 0);
    }

    #[test]
    fn remove_from_recent_closes_the_failed_tab() {
        let (mut app, _) = App::new(Vec::new(), None);
        let mut session = Session {
            tabs: vec![PathBuf::from("/gone")],
            ..Session::default()
        };
        session.touch_recent(std::path::Path::new("/gone"), 1);
        let _ = app.update(loaded(session));
        let id = app.tabs[0].id;
        let _ = app.update(Message::RepoOpened {
            tab: id,
            generation: 1,
            result: Err("Not found".into()),
        });
        let _ = app.update(Message::RemoveRecent(id));
        assert!(app.session.recent.is_empty());
        assert!(matches!(app.tabs[0].state, TabState::Start));
    }

    #[test]
    fn switching_tabs_requests_details_of_the_active_tab() {
        use git_core::{Commit, Head, Log, WorkingTree};

        let (mut app, _) = App::new(Vec::new(), None);
        let _ = app.update(loaded(Session::default()));
        let commit = Commit {
            id: "c1".into(),
            parents: Vec::new(),
            summary: String::new(),
            body: String::new(),
            author: String::new(),
            email: String::new(),
            author_time: 0,
            commit_time: 0,
        };
        let snapshot = RepoSnapshot::build(
            Log {
                commits: vec![commit],
                truncated: false,
            },
            Head::Detached {
                target: "c1".into(),
            },
            Vec::new(),
            Vec::new(),
            WorkingTree::default(),
        )
        .unwrap();
        let backend: Arc<dyn GitBackend> = Arc::new(git_core::MockBackend::new("mock"));
        let first = app.tabs[0].id;
        app.tabs[0].state = TabState::Ready(Box::new(RepoView::new(backend, Arc::new(snapshot))));
        let _ = app.update(Message::TabOpened);
        assert!(app.tabs[0].view().unwrap().details.is_none());

        let _ = app.update(Message::TabSelected(first));
        let details = app.tabs[0].view().unwrap().details.as_ref().unwrap();
        assert_eq!(details.id, "c1");
        assert_eq!(details.changes, Loadable::Loading);
    }

    /// App with one ready tab showing commits `ids` (linear, newest first).
    fn ready_app(ids: &[&str]) -> App {
        use git_core::{Commit, Head, Log, WorkingTree};

        let (mut app, _) = App::new(Vec::new(), None);
        let _ = app.update(loaded(Session::default()));
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
        let snapshot = RepoSnapshot::build(
            Log {
                commits,
                truncated: false,
            },
            Head::Detached {
                target: ids[0].into(),
            },
            Vec::new(),
            Vec::new(),
            WorkingTree::default(),
        )
        .unwrap();
        let backend: Arc<dyn GitBackend> = Arc::new(git_core::MockBackend::new("mock"));
        app.tabs[0].state = TabState::Ready(Box::new(RepoView::new(backend, Arc::new(snapshot))));
        app
    }

    fn selected(app: &App) -> Option<Selection> {
        app.tabs[app.active]
            .view()
            .and_then(|v| v.selection.clone())
    }

    fn checked(shortcut: Shortcut, text_input_focused: bool) -> Message {
        Message::ShortcutChecked {
            shortcut,
            text_input_focused,
        }
    }

    #[test]
    fn keyboard_moves_the_selection() {
        use crate::nav::Move;
        let mut app = ready_app(&["c", "b", "a"]);
        assert_eq!(selected(&app), Some(Selection::Commit("c".into())));
        let _ = app.update(checked(Shortcut::Navigate(Move::Next), false));
        assert_eq!(selected(&app), Some(Selection::Commit("b".into())));
        let _ = app.update(checked(Shortcut::Navigate(Move::Last), false));
        assert_eq!(selected(&app), Some(Selection::Commit("a".into())));
        let _ = app.update(checked(Shortcut::Navigate(Move::Child), false));
        assert_eq!(selected(&app), Some(Selection::Commit("b".into())));
    }

    #[test]
    fn shortcuts_pause_while_typing() {
        use crate::nav::Move;
        let mut app = ready_app(&["c", "b"]);
        let _ = app.update(checked(Shortcut::Navigate(Move::Next), true));
        assert_eq!(selected(&app), Some(Selection::Commit("c".into())));
        let _ = app.update(checked(Shortcut::NewTab, true));
        assert_eq!(app.tabs.len(), 1);
    }

    #[test]
    fn escape_closes_the_diff() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::DiffOpened(DiffRequest {
            kind: git_core::DiffKind::Staged,
            path: "f".into(),
            old_path: None,
        }));
        assert!(matches!(
            app.tabs[0].view().unwrap().center,
            CenterView::Diff(_)
        ));
        let _ = app.update(Message::Shortcut(Shortcut::Close));
        assert!(matches!(
            app.tabs[0].view().unwrap().center,
            CenterView::Graph
        ));
    }

    fn sample_diff(lines: usize) -> git_core::FileDiff {
        use git_core::{DiffHunk, DiffLine, DiffLineKind};
        git_core::FileDiff {
            old_path: Some("f".into()),
            new_path: Some("f".into()),
            binary: false,
            too_large: false,
            hunks: vec![DiffHunk {
                header: "@@ -1 +1 @@".into(),
                old_start: 1,
                old_count: 1,
                new_start: 1,
                new_count: 1,
                lines: (0..lines)
                    .map(|i| DiffLine {
                        kind: DiffLineKind::Added,
                        old_line: None,
                        new_line: Some(i as u32 + 1),
                        content: format!("line {i}"),
                        missing_newline: false,
                        crlf: false,
                    })
                    .collect(),
            }],
        }
    }

    #[test]
    fn diff_loads_in_the_background_and_ignores_stale_results() {
        let mut app = ready_app(&["c"]);
        let request = |path: &str| DiffRequest {
            kind: git_core::DiffKind::Commit("c".into()),
            path: path.into(),
            old_path: None,
        };
        let _ = app.update(Message::DiffOpened(request("f")));
        let tab = app.tabs[0].id;
        let diff_state = |app: &App| match &app.tabs[0].view().unwrap().center {
            CenterView::Diff(diff) => (diff.diff.clone(), diff.rows.len()),
            CenterView::Graph => panic!("diff closed"),
        };
        assert_eq!(diff_state(&app).0, Loadable::Loading);

        // A result for another file (opened earlier) is dropped.
        let _ = app.update(Message::DiffLoaded {
            tab,
            request: request("other"),
            result: Ok(sample_diff(1)),
        });
        assert_eq!(diff_state(&app).0, Loadable::Loading);

        let _ = app.update(Message::DiffLoaded {
            tab,
            request: request("f"),
            result: Ok(sample_diff(3)),
        });
        let (diff, rows) = diff_state(&app);
        assert!(matches!(diff, Loadable::Ready(_)));
        // One header row plus three lines.
        assert_eq!(rows, 4);
    }

    /// Staging app showing a loaded unstaged diff of "a" with three added lines.
    fn app_with_open_diff(kind: git_core::DiffKind) -> App {
        let mut app = staging_app(&["a"], &[]);
        let request = DiffRequest {
            kind,
            path: "a".into(),
            old_path: None,
        };
        let _ = app.update(Message::DiffOpened(request.clone()));
        let tab = app.tabs[0].id;
        let _ = app.update(Message::DiffLoaded {
            tab,
            request,
            result: Ok(sample_diff(3)),
        });
        app
    }

    fn open_diff(app: &App) -> &crate::repo_view::DiffView {
        match &app.tabs[0].view().unwrap().center {
            CenterView::Diff(diff) => diff,
            CenterView::Graph => panic!("no diff"),
        }
    }

    #[test]
    fn line_clicks_follow_the_modifier_keys() {
        let mut app = app_with_open_diff(git_core::DiffKind::Unstaged);
        let _ = app.update(Message::DiffLineClicked(1));
        let _ = app.update(Message::ModifiersChanged(keyboard::Modifiers::SHIFT));
        let _ = app.update(Message::DiffLineClicked(3));
        assert_eq!(open_diff(&app).selection.len(), 3);
        let _ = app.update(Message::ModifiersChanged(keyboard::Modifiers::COMMAND));
        let _ = app.update(Message::DiffLineClicked(2));
        let selected: Vec<_> = open_diff(&app).selection.iter().copied().collect();
        assert_eq!(selected, [(0, 0), (0, 2)]);
    }

    #[test]
    fn applying_lines_reloads_the_diff_after_the_refresh() {
        let mut app = app_with_open_diff(git_core::DiffKind::Unstaged);
        let _ = app.update(Message::DiffLineClicked(2));
        let _ = app.update(Message::ApplySelectedLines);
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Stage lines of a"));
        assert!(view.reload_diff);

        let tab = app.tabs[0].id;
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Stage lines of a".into(),
            result: Ok(None),
        });
        let generation = app.tabs[0].generation;
        let snapshot = Arc::clone(&app.tabs[0].view().unwrap().snapshot);
        let _ = app.update(Message::SnapshotLoaded {
            tab,
            generation,
            result: Ok(snapshot),
        });
        let view = app.tabs[0].view().unwrap();
        assert!(!view.reload_diff, "diff reload was requested");
        assert!(
            matches!(view.center, CenterView::Diff(_)),
            "diff stays open"
        );
    }

    #[test]
    fn hunk_staging_is_not_offered_for_commit_diffs() {
        let mut app = app_with_open_diff(git_core::DiffKind::Commit("c".into()));
        let _ = app.update(Message::ApplyHunk(0));
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        let _ = app.update(Message::DiffLineClicked(1));
        assert!(open_diff(&app).selection.is_empty());
    }

    #[test]
    fn diff_errors_are_shown() {
        let mut app = ready_app(&["c"]);
        let request = DiffRequest {
            kind: git_core::DiffKind::Staged,
            path: "f".into(),
            old_path: None,
        };
        let _ = app.update(Message::DiffOpened(request.clone()));
        let tab = app.tabs[0].id;
        let _ = app.update(Message::DiffLoaded {
            tab,
            request,
            result: Err(git_core::Error::Backend("boom".into())),
        });
        assert!(matches!(
            &app.tabs[0].view().unwrap().center,
            CenterView::Diff(diff) if diff.diff == Loadable::Failed("Git error: boom".into())
        ));
    }

    fn missing(id: &str) -> Result<git_core::FileDiff, git_core::Error> {
        Err(git_core::Error::MissingObject {
            id: id.into(),
            partial_clone: true,
        })
    }

    fn shown_diff_view(app: &App) -> &crate::repo_view::DiffView {
        match &app.tabs[0].view().unwrap().center {
            CenterView::Diff(diff) => diff,
            CenterView::Graph => panic!("diff closed"),
        }
    }

    #[test]
    fn missing_partial_clone_content_is_fetched_once() {
        let mut app = app_with_open_diff(git_core::DiffKind::Staged);
        let tab = app.tabs[0].id;
        let request = shown_diff_view(&app).request.clone();
        let _ = app.update(Message::DiffOpened(request.clone()));
        let _ = app.update(Message::DiffLoaded {
            tab,
            request: request.clone(),
            result: missing("abc"),
        });
        let diff = shown_diff_view(&app);
        assert!(diff.fetching, "shows 'Fetching missing content…'");
        assert_eq!(diff.diff, Loadable::Loading);

        let _ = app.update(Message::ObjectFetched {
            tab,
            request: request.clone(),
            result: Ok(()),
        });
        assert!(!shown_diff_view(&app).fetching);
        // Still missing after the fetch: no endless loop.
        let _ = app.update(Message::DiffLoaded {
            tab,
            request,
            result: missing("abc"),
        });
        assert_eq!(
            shown_diff_view(&app).diff,
            Loadable::Failed("Content not available locally (partial clone)".into())
        );
    }

    #[test]
    fn failed_fetch_shows_the_git_error() {
        let mut app = app_with_open_diff(git_core::DiffKind::Staged);
        let tab = app.tabs[0].id;
        let request = shown_diff_view(&app).request.clone();
        let _ = app.update(Message::DiffOpened(request.clone()));
        let _ = app.update(Message::DiffLoaded {
            tab,
            request: request.clone(),
            result: missing("abc"),
        });
        let _ = app.update(Message::ObjectFetched {
            tab,
            request,
            result: Err("fatal: unable to access remote".into()),
        });
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Fetching missing content failed");
        assert!(matches!(shown_diff_view(&app).diff, Loadable::Failed(_)));
    }

    #[test]
    fn without_git_missing_content_is_explained() {
        let mut app = app_with_open_diff(git_core::DiffKind::Staged);
        let _ = app.update(Message::GitDetected(git_core::GitAvailability::Missing));
        let tab = app.tabs[0].id;
        let request = shown_diff_view(&app).request.clone();
        let _ = app.update(Message::DiffOpened(request.clone()));
        let _ = app.update(Message::DiffLoaded {
            tab,
            request,
            result: missing("abc"),
        });
        assert_eq!(
            shown_diff_view(&app).diff,
            Loadable::Failed("Content not available locally (partial clone)".into())
        );
    }

    #[test]
    fn toggling_panels_by_shortcut() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(checked(Shortcut::ToggleSidebar, false));
        assert!(!app.panes.sidebar_visible);
        let _ = app.update(checked(Shortcut::TogglePanel, false));
        assert!(!app.panes.panel_visible);
        let _ = app.update(checked(Shortcut::FocusFilter, false));
        assert!(app.panes.sidebar_visible);
    }

    fn git_available() -> Message {
        Message::GitDetected(git_core::GitAvailability::Available(
            git_core::MIN_GIT_VERSION,
        ))
    }

    #[test]
    fn missing_git_shows_notice_and_blocks_writes() {
        let mut app = ready_app(&["c"]);
        // Any update syncs the reason into the tabs.
        let _ = app.update(Message::DismissNotice);
        assert!(
            app.tabs[0].view().unwrap().write_block.is_some(),
            "unknown until detected"
        );
        let _ = app.update(Message::GitDetected(git_core::GitAvailability::Missing));
        let reason = app.tabs[0].view().unwrap().write_block.clone().unwrap();
        assert!(reason.contains("not found"));
        assert_eq!(app.notice.as_deref(), Some(reason.as_str()));
        let _ = app.update(Message::DismissNotice);
        assert!(app.notice.is_none());

        let tab = app.tabs[0].id;
        let _ = app.run_write(tab, "Stage", |_| Ok(()));
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert!(app.status.contains("not found"));
    }

    #[test]
    fn one_write_at_a_time_per_tab() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(git_available());
        assert!(app.notice.is_none());
        assert!(app.tabs[0].view().unwrap().write_block.is_none());

        let tab = app.tabs[0].id;
        let _ = app.run_write(tab, "Stage file", |_| Ok(()));
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Stage file")
        );
        let _ = app.run_write(tab, "Commit", |_| Ok(()));
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Stage file")
        );
        assert!(app.status.contains("Stage file is still running"));

        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Stage file".into(),
            result: Err("boom".into()),
        });
        let view = app.tabs[0].view().unwrap();
        assert!(view.busy.is_none());
        assert!(view.refreshing, "a write is followed by a refresh");
        let modal = app.modal.as_ref().expect("error dialog");
        assert_eq!(modal.title, "Stage file failed");
        assert_eq!(modal.details.as_deref(), Some("boom"));
    }

    fn file(path: &str) -> git_core::FileChange {
        git_core::FileChange {
            path: path.into(),
            kind: git_core::ChangeKind::Modified,
            old_path: None,
        }
    }

    /// Ready app with git available and the given unstaged/staged files.
    fn staging_app(unstaged: &[&str], staged: &[&str]) -> App {
        let mut app = ready_app(&["c"]);
        let _ = app.update(git_available());
        let tree = git_core::WorkingTree {
            unstaged: unstaged.iter().map(|p| file(p)).collect(),
            staged: staged.iter().map(|p| file(p)).collect(),
        };
        let view = app.tabs[0].view_mut().unwrap();
        view.snapshot = Arc::new(view.snapshot.with_working_tree(tree).unwrap());
        app
    }

    fn shown_diff(app: &App) -> Option<DiffRequest> {
        match &app.tabs[0].view().unwrap().center {
            CenterView::Diff(diff) => Some(diff.request.clone()),
            CenterView::Graph => None,
        }
    }

    #[test]
    fn staging_the_shown_file_moves_to_the_next_unstaged_file() {
        let mut app = staging_app(&["a", "b", "c"], &[]);
        let _ = app.update(Message::DiffOpened(crate::repo_view::diff_request(
            &file("b"),
            false,
        )));
        let _ = app.update(checked(Shortcut::StageSelected, false));
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Stage b"));
        let shown = shown_diff(&app).unwrap();
        assert_eq!(
            (shown.path.as_str(), shown.kind),
            ("c", git_core::DiffKind::Unstaged)
        );
    }

    #[test]
    fn staging_the_last_unstaged_file_closes_the_diff() {
        let mut app = staging_app(&["only"], &[]);
        let _ = app.update(Message::DiffOpened(crate::repo_view::diff_request(
            &file("only"),
            false,
        )));
        let _ = app.update(Message::Stage(file("only")));
        assert!(shown_diff(&app).is_none());
    }

    #[test]
    fn staging_another_file_keeps_the_shown_diff() {
        let mut app = staging_app(&["a", "b"], &[]);
        let _ = app.update(Message::DiffOpened(crate::repo_view::diff_request(
            &file("a"),
            false,
        )));
        let _ = app.update(Message::Stage(file("b")));
        assert_eq!(shown_diff(&app).unwrap().path, "a");
    }

    #[test]
    fn unstaging_moves_within_the_staged_list() {
        let mut app = staging_app(&[], &["x", "y"]);
        let _ = app.update(Message::DiffOpened(crate::repo_view::diff_request(
            &file("y"),
            true,
        )));
        let _ = app.update(checked(Shortcut::UnstageSelected, false));
        let shown = shown_diff(&app).unwrap();
        assert_eq!(
            (shown.path.as_str(), shown.kind),
            ("x", git_core::DiffKind::Staged)
        );
        // S does nothing for a staged file.
        let _ = app.update(Message::WriteFinished {
            tab: app.tabs[0].id,
            label: "Unstage y".into(),
            result: Ok(None),
        });
        let _ = app.update(checked(Shortcut::StageSelected, false));
        assert!(app.tabs[0].view().unwrap().busy.is_none());
    }

    #[test]
    fn unstaged_file_menu_offers_stage() {
        let mut app = staging_app(&["a"], &[]);
        let _ = app.update(Message::FileContextMenu {
            file: file("a"),
            area: FileArea::Unstaged,
            position: iced::Point::ORIGIN,
        });
        let menu = app.context_menu.clone().unwrap();
        assert!(matches!(menu.select(0), Some(Message::Stage(f)) if f.path == "a"));
        assert!(menu.select(2).is_none(), "separator");
        let _ = app.update(Message::MenuSelected(0));
        assert_eq!(app.tabs[0].view().unwrap().busy.as_deref(), Some("Stage a"));
    }

    #[test]
    fn commit_needs_staged_changes_and_a_summary() {
        let mut app = staging_app(&["a"], &[]);
        let _ = app.update(Message::SummaryChanged("Fix".into()));
        let _ = app.update(Message::CommitRequested);
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert_eq!(app.status, "Stage files to commit");

        // Ctrl+Shift+Enter stages everything first.
        let _ = app.update(Message::Shortcut(Shortcut::StageAllAndCommit));
        assert_eq!(app.tabs[0].view().unwrap().busy.as_deref(), Some("Commit"));

        let mut app = staging_app(&[], &["a"]);
        let _ = app.update(Message::CommitRequested);
        assert_eq!(app.status, "Enter a commit summary");
    }

    #[test]
    fn commit_message_is_kept_on_failure_and_cleared_on_success() {
        let mut app = staging_app(&[], &["a"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::SummaryChanged("Add feature".into()));
        let _ = app.update(Message::DescriptionEdited(
            iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Paste(
                Arc::new("Details".into()),
            )),
        ));
        assert_eq!(
            app.tabs[0].view().unwrap().commit_message(),
            "Add feature\n\nDetails"
        );
        let _ = app.update(Message::Shortcut(Shortcut::Commit));
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Commit".into(),
            result: Err("pre-commit hook failed".into()),
        });
        assert_eq!(app.tabs[0].view().unwrap().summary, "Add feature");
        assert!(app.modal.is_some());
        app.modal = None;

        let _ = app.update(Message::Shortcut(Shortcut::Commit));
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Commit".into(),
            result: Ok(None),
        });
        let view = app.tabs[0].view().unwrap();
        assert!(view.summary.is_empty());
        assert!(view.description.text().trim().is_empty());
    }

    #[test]
    fn merge_message_prefills_and_allows_committing_without_staged_files() {
        let mut app = staging_app(&[], &[]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.merge_message = Some("Merge branch 'x'\n\n# Conflicts:\n#\ta.txt\n".into());
        view.apply_snapshot(Arc::new(snapshot));
        assert_eq!(view.summary, "Merge branch 'x'");
        assert!(commit_blocker(view, false).is_none());
    }

    /// Staging app with `state` in progress and a staged file "a".
    fn app_in_state(state: git_core::RepoState) -> App {
        let mut app = staging_app(&[], &["a"]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        if state.concluded_by_commit() {
            snapshot.merge_message = Some("Merge branch 'x'\n".into());
        }
        snapshot.state = state;
        view.apply_snapshot(Arc::new(snapshot));
        view.summary = "Summary".into();
        app
    }

    fn merging() -> git_core::RepoState {
        git_core::RepoState::Merging {
            merge_head: "x".into(),
        }
    }

    #[test]
    fn merge_in_progress_allows_commit_but_blocks_amend_and_stash() {
        let mut app = app_in_state(merging());
        let view = app.tabs[0].view().unwrap();
        assert!(commit_blocker(view, false).is_none(), "commit concludes it");
        assert!(view.state_block(Guarded::Amend).is_some());

        let _ = app.update(Message::StashPush);
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert_eq!(app.status, "Cannot stash while a merge is in progress");
        let _ = app.update(Message::AmendToggled(true));
        assert!(!app.tabs[0].view().unwrap().amend);
        let _ = app.update(Message::StashDropConfirmed("s".into()));
        assert!(
            app.tabs[0].view().unwrap().busy.is_some(),
            "deleting a stash is harmless"
        );
    }

    #[test]
    fn rebase_in_progress_blocks_committing() {
        let app = app_in_state(git_core::RepoState::Rebasing {
            branch: Some("topic".into()),
            onto: None,
            step: None,
        });
        let view = app.tabs[0].view().unwrap();
        assert_eq!(
            commit_blocker(view, false).as_deref(),
            Some("Cannot commit while a rebase is in progress")
        );
    }

    #[test]
    fn undoing_a_commit_is_blocked_but_a_discard_is_not() {
        let mut app = app_in_state(merging());
        app.tabs[0].view_mut().unwrap().undo.push(commit_action());
        let _ = app.update(Message::Undo);
        let view = app.tabs[0].view().unwrap();
        assert!(view.busy.is_none());
        assert_eq!(view.undo.len(), 1, "entry is kept");

        let view = app.tabs[0].view_mut().unwrap();
        view.undo.push(UndoAction::Discard {
            label: "Discard a".into(),
            snapshot: git_core::DiscardSnapshot::default(),
        });
        let _ = app.update(Message::Undo);
        assert!(app.tabs[0].view().unwrap().busy.is_some());
    }

    #[test]
    fn lock_errors_offer_retry_of_the_same_operation() {
        let mut app = staging_app(&[], &["a"]);
        let tab = app.tabs[0].id;
        app.tabs[0].view_mut().unwrap().summary = "Summary".into();
        let _ = app.update(Message::CommitRequested);
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Commit".into(),
            result: Err(
                "git commit failed (exit code 128): fatal: Unable to create \
                         '/r/.git/index.lock': File exists."
                    .into(),
            ),
        });
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.details.as_deref(), Some("/r/.git/index.lock"));
        assert!(matches!(modal.default_action(), Some(Message::RetryWrite(t)) if t == tab));
        assert_eq!(app.tabs[0].view().unwrap().summary, "Summary", "kept");

        let _ = app.update(Message::RetryWrite(tab));
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Commit"));
        assert!(view.clear_message_after_write, "a successful retry clears");
    }

    #[test]
    fn template_fills_an_empty_description() {
        let mut app = staging_app(&[], &["a"]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.commit_template = Some("Ticket: \n# help text\n".into());
        view.apply_snapshot(Arc::new(snapshot));
        assert_eq!(view.description.text().trim(), "Ticket:");
    }

    /// Staging app whose HEAD commit "c" has a message.
    fn amend_app() -> App {
        let mut app = staging_app(&[], &[]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        let mut commits = (*snapshot.commits).clone();
        commits[0].summary = "Old summary".into();
        commits[0].body = "Old body".into();
        snapshot.commits = Arc::new(commits);
        view.apply_snapshot(Arc::new(snapshot));
        app
    }

    #[test]
    fn amend_fills_and_clears_the_message() {
        let mut app = amend_app();
        let _ = app.update(Message::AmendToggled(true));
        let view = app.tabs[0].view().unwrap();
        assert!(view.amend);
        assert_eq!(view.commit_message(), "Old summary\n\nOld body");
        assert!(commit_blocker(view, false).is_none(), "message-only amend");

        let _ = app.update(Message::CommitRequested);
        assert_eq!(app.tabs[0].view().unwrap().busy.as_deref(), Some("Amend"));

        let mut app = amend_app();
        let _ = app.update(Message::AmendToggled(true));
        let _ = app.update(Message::AmendToggled(false));
        let view = app.tabs[0].view().unwrap();
        assert!(
            view.summary.is_empty(),
            "unchanged prefill is removed again"
        );
    }

    #[test]
    fn amend_keeps_an_edited_message() {
        let mut app = amend_app();
        let _ = app.update(Message::SummaryChanged("New summary".into()));
        let _ = app.update(Message::AmendToggled(true));
        assert_eq!(app.tabs[0].view().unwrap().summary, "New summary");
        let _ = app.update(Message::AmendToggled(false));
        assert_eq!(app.tabs[0].view().unwrap().summary, "New summary");
    }

    #[test]
    fn pushed_head_is_detected() {
        let mut app = amend_app();
        let view = app.tabs[0].view_mut().unwrap();
        assert!(!view.head_pushed(), "no upstream");
        let mut snapshot = (*view.snapshot).clone();
        snapshot.refs = vec![git_core::Reference {
            name: "main".into(),
            kind: git_core::RefKind::LocalBranch,
            target: "c".into(),
            is_head: true,
            upstream: Some("origin/main".into()),
            ahead_behind: Some((0, 0)),
        }];
        view.apply_snapshot(Arc::new(snapshot));
        assert!(view.head_pushed());
    }

    #[test]
    fn stash_menu_and_confirmed_delete() {
        let mut app = staging_app(&["a"], &[]);
        let _ = app.update(Message::StashContextMenu {
            id: "s1".into(),
            position: iced::Point::ORIGIN,
        });
        let menu = app.context_menu.clone().unwrap();
        assert!(matches!(menu.select(0), Some(Message::StashApply(id)) if id == "s1"));
        assert!(matches!(menu.select(1), Some(Message::StashPop(id)) if id == "s1"));
        let _ = app.update(Message::MenuSelected(3));
        // Deleting asks first.
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Delete stash")
        );
    }

    #[test]
    fn toolbar_stash_runs_a_write() {
        let mut app = staging_app(&["a"], &[]);
        let _ = app.update(Message::StashPush);
        assert_eq!(app.tabs[0].view().unwrap().busy.as_deref(), Some("Stash"));
    }

    fn commit_action() -> UndoAction {
        UndoAction::Commit {
            amend: false,
            before: Some("c".into()),
            after: "d123456789".into(),
        }
    }

    #[test]
    fn undo_moves_the_action_to_redo() {
        let mut app = staging_app(&[], &[]);
        let tab = app.tabs[0].id;
        app.tabs[0].view_mut().unwrap().undo.push(commit_action());
        let _ = app.update(checked(Shortcut::Undo, false));
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Undo Commit d123456"));
        assert!(view.undo.is_empty(), "popped while running");

        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Undo Commit d123456".into(),
            result: Ok(Some(History::Undone(commit_action()))),
        });
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.redo, [commit_action()]);

        // Redo puts it back without clearing other redo entries.
        let _ = app.update(checked(Shortcut::Redo, false));
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Redo Commit d123456".into(),
            result: Ok(Some(History::Redone(commit_action()))),
        });
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.undo, [commit_action()]);
        assert!(view.redo.is_empty());
    }

    #[test]
    fn failed_undo_drops_the_entry_and_explains() {
        let mut app = staging_app(&[], &[]);
        let tab = app.tabs[0].id;
        app.tabs[0].view_mut().unwrap().undo.push(commit_action());
        let _ = app.update(Message::Undo);
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Undo Commit d123456".into(),
            result: Err("Git error: Cannot undo: the repository changed".into()),
        });
        let view = app.tabs[0].view().unwrap();
        assert!(view.undo.is_empty() && view.redo.is_empty());
        let modal = app.modal.as_ref().unwrap();
        assert!(modal
            .details
            .as_deref()
            .unwrap()
            .contains("the repository changed"));
    }

    #[test]
    fn new_actions_clear_the_redo_stack() {
        let mut app = staging_app(&[], &[]);
        let tab = app.tabs[0].id;
        app.tabs[0].view_mut().unwrap().redo.push(commit_action());
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Commit".into(),
            result: Ok(Some(History::Push(commit_action()))),
        });
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.undo.len(), 1);
        assert!(view.redo.is_empty());
    }

    #[test]
    fn undo_while_busy_keeps_the_entry() {
        let mut app = staging_app(&["a"], &[]);
        app.tabs[0].view_mut().unwrap().undo.push(commit_action());
        let _ = app.update(Message::StashPush);
        let _ = app.update(Message::Undo);
        assert_eq!(app.tabs[0].view().unwrap().undo.len(), 1);
    }

    #[test]
    fn every_discard_asks_first() {
        for target in [
            DiscardTarget::File(file("a")),
            DiscardTarget::All,
            DiscardTarget::Hunk(0),
            DiscardTarget::Lines,
        ] {
            let mut app = staging_app(&["a"], &[]);
            let _ = app.update(Message::Discard(target.clone()));
            let modal = app.modal.as_ref().expect("confirmation");
            assert_eq!(modal.buttons[1].kind, crate::overlay::ButtonKind::Danger);
            assert!(
                app.tabs[0].view().unwrap().busy.is_none(),
                "nothing runs yet"
            );
            assert!(
                matches!(modal.default_action(), Some(Message::DiscardConfirmed(t)) if t == target)
            );
        }
    }

    #[test]
    fn confirmed_file_discard_runs_and_records_undo() {
        let mut app = staging_app(&["a", "b"], &[]);
        let _ = app.update(Message::DiffOpened(crate::repo_view::diff_request(
            &file("a"),
            false,
        )));
        let _ = app.update(Message::Discard(DiscardTarget::File(file("a"))));
        let _ = app.update(Message::Shortcut(Shortcut::Confirm));
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Discard a"));
        assert!(
            matches!(view.center, CenterView::Graph),
            "diff of the discarded file closes"
        );

        let tab = app.tabs[0].id;
        let undo = UndoAction::Discard {
            label: "Discard a".into(),
            snapshot: git_core::DiscardSnapshot::default(),
        };
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Discard a".into(),
            result: Ok(Some(History::Push(undo.clone()))),
        });
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.undo, [undo]);
        assert!(view.redo.is_empty());
    }

    #[test]
    fn unstaged_menu_has_discard_ignore_and_location() {
        let mut app = staging_app(&["a"], &[]);
        let _ = app.update(Message::FileContextMenu {
            file: file("a"),
            area: FileArea::Unstaged,
            position: iced::Point::ORIGIN,
        });
        let menu = app.context_menu.clone().unwrap();
        let labels: Vec<_> = menu
            .entries
            .iter()
            .map(|e| match e {
                MenuEntry::Item { label, .. } => label.as_str(),
                MenuEntry::Separator => "-",
            })
            .collect();
        assert_eq!(
            labels,
            [
                "Stage",
                "Discard changes",
                "-",
                "Ignore",
                "Open file location",
                "Copy file path"
            ]
        );
    }

    #[test]
    fn gitignore_patterns_are_anchored_and_escaped() {
        assert_eq!(gitignore_pattern("build/out.log"), "/build/out.log");
        assert_eq!(gitignore_pattern("a*b?[c]"), "/a\\*b\\?\\[c]");
        assert_eq!(gitignore_pattern("#note!"), "/\\#note\\!");
        assert_eq!(gitignore_pattern("trail  "), "/trail\\ \\ ");
    }

    #[test]
    fn appending_to_gitignore_keeps_existing_lines() {
        let dir = std::env::temp_dir().join(format!("gitloom-ignore-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".gitignore"), "target/").unwrap();
        append_to_gitignore(&dir, "notes.txt").unwrap();
        append_to_gitignore(&dir, "x*.tmp").unwrap();
        let content = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert_eq!(content, "target/\n/notes.txt\n/x\\*.tmp\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_context_menu_copies_the_path() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::FileContextMenu {
            file: file("src/main.rs"),
            area: FileArea::Commit,
            position: iced::Point::new(10.0, 20.0),
        });
        assert!(app.context_menu.is_some());
        // Other shortcuts are ignored while the menu is open.
        let _ = app.update(checked(Shortcut::NewTab, false));
        assert_eq!(app.tabs.len(), 1);
        let _ = app.update(Message::MenuSelected(0));
        assert!(app.context_menu.is_none());
        assert_eq!(app.status, "Copied to the clipboard");
    }

    #[test]
    fn escape_closes_menu_then_dialog_then_diff() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::DiffOpened(DiffRequest {
            kind: git_core::DiffKind::Staged,
            path: "f".into(),
            old_path: None,
        }));
        app.modal = Some(Modal::error("Failed", "details"));
        let _ = app.update(Message::Shortcut(Shortcut::Close));
        assert!(app.modal.is_none());
        assert!(matches!(
            app.tabs[0].view().unwrap().center,
            CenterView::Diff(_)
        ));
        let _ = app.update(Message::Shortcut(Shortcut::Close));
        assert!(matches!(
            app.tabs[0].view().unwrap().center,
            CenterView::Graph
        ));
    }

    #[test]
    fn enter_confirms_the_default_button() {
        let mut app = ready_app(&["c"]);
        app.modal = Some(Modal::confirm(
            "Toggle?",
            "Hide the sidebar?",
            "Hide",
            false,
            checked(Shortcut::ToggleSidebar, false),
        ));
        let _ = app.update(Message::Shortcut(Shortcut::Confirm));
        assert!(app.modal.is_none());
        assert!(!app.panes.sidebar_visible);

        // Cancel only closes.
        app.modal = Some(Modal::confirm(
            "Toggle?",
            "Show it again?",
            "Show",
            false,
            checked(Shortcut::ToggleSidebar, false),
        ));
        let _ = app.update(Message::ModalAction(0));
        assert!(app.modal.is_none());
        assert!(!app.panes.sidebar_visible);
    }

    fn dirty_tree() -> git_core::WorkingTree {
        git_core::WorkingTree {
            unstaged: vec![git_core::FileChange {
                path: "f".into(),
                kind: git_core::ChangeKind::Modified,
                old_path: None,
            }],
            staged: Vec::new(),
        }
    }

    #[test]
    fn watcher_status_change_updates_the_wip_row() {
        let mut app = ready_app(&["c", "b"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::WatcherStarted(tab));
        let _ = app.update(Message::FsChanged {
            tab,
            status: true,
            refs: false,
        });
        let view = app.tabs[0].view().unwrap();
        assert!(view.status_refreshing);
        assert!(!view.refreshing, "no full reload for a status change");
        let generation = app.tabs[0].generation;

        let _ = app.update(Message::StatusLoaded {
            tab,
            generation,
            result: Ok(dirty_tree()),
        });
        let view = app.tabs[0].view().unwrap();
        assert!(!view.status_refreshing);
        assert!(view.snapshot.has_wip);
        assert_eq!(view.snapshot.layout.len(), 3);
    }

    #[test]
    fn stale_status_results_are_dropped() {
        let mut app = ready_app(&["c"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::FsChanged {
            tab,
            status: true,
            refs: false,
        });
        let _ = app.update(Message::StatusLoaded {
            tab,
            generation: 0,
            result: Ok(dirty_tree()),
        });
        assert!(!app.tabs[0].view().unwrap().snapshot.has_wip);
    }

    #[test]
    fn watcher_events_are_ignored_during_own_writes() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(git_available());
        let tab = app.tabs[0].id;
        let _ = app.run_write(tab, "Stage", |_| Ok(()));
        let _ = app.update(Message::FsChanged {
            tab,
            status: true,
            refs: true,
        });
        assert!(!app.tabs[0].view().unwrap().status_refreshing);
    }

    #[test]
    fn unchanged_refs_only_refresh_the_status() {
        let mut app = ready_app(&["c"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::RefsChecked {
            tab,
            changed: Ok(false),
        });
        let view = app.tabs[0].view().unwrap();
        assert!(view.status_refreshing && !view.refreshing);

        let mut app = ready_app(&["c"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::RefsChecked {
            tab,
            changed: Ok(true),
        });
        assert!(app.tabs[0].view().unwrap().refreshing);
    }

    #[test]
    fn refs_comparison() {
        let app = ready_app(&["c", "b"]);
        let snapshot = &app.tabs[0].view().unwrap().snapshot;
        let head = snapshot.head.clone();
        assert!(!refs_changed(snapshot, &head, &[], &[]));
        let moved = git_core::Head::Detached { target: "b".into() };
        assert!(refs_changed(snapshot, &moved, &[], &[]));
        let new_ref = git_core::Reference {
            name: "main".into(),
            kind: git_core::RefKind::LocalBranch,
            target: "c".into(),
            is_head: false,
            upstream: None,
            ahead_behind: None,
        };
        assert!(refs_changed(snapshot, &head, &[new_ref], &[]));
        let stash = git_core::Stash {
            index: 0,
            message: "WIP".into(),
            id: "s".into(),
            base: Some("c".into()),
            time: 0,
            author: String::new(),
        };
        assert!(refs_changed(snapshot, &head, &[], &[stash]));
    }

    #[test]
    fn focus_refresh_depends_on_the_watcher() {
        let mut app = ready_app(&["c"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::WatcherStarted(tab));
        let _ = app.update(Message::FocusRefreshDue(app.focus_seq));
        let view = app.tabs[0].view().unwrap();
        assert!(view.status_refreshing && !view.refreshing);

        let mut app = ready_app(&["c"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::WatcherFailed {
            tab,
            error: "inotify limit".into(),
        });
        assert!(app.notice.as_deref().unwrap().contains("inotify limit"));
        let _ = app.update(Message::FocusRefreshDue(app.focus_seq));
        assert!(app.tabs[0].view().unwrap().refreshing);
    }

    #[test]
    fn access_events_do_not_trigger_refreshes() {
        use notify::event::{AccessKind, AccessMode, CreateKind, ModifyKind};
        use notify::EventKind;
        assert!(!changes_content(&EventKind::Access(AccessKind::Open(
            AccessMode::Any
        ))));
        assert!(!changes_content(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(changes_content(&EventKind::Create(CreateKind::File)));
        assert!(changes_content(&EventKind::Modify(ModifyKind::Any)));
        assert!(changes_content(&EventKind::Any));

        let event = |kind| Ok(notify::Event::new(kind));
        assert!(!forward_event(&event(EventKind::Access(AccessKind::Open(
            notify::event::AccessMode::Any
        )))));
        assert!(forward_event(&event(EventKind::Modify(ModifyKind::Any))));
        assert!(forward_event(&Err(notify::Error::generic(
            "queue overflow"
        ))));
    }

    #[test]
    fn subfolders_belong_to_the_open_repository() {
        let root = std::path::Path::new("/work/repo");
        assert!(same_repo(root, std::path::Path::new("/work/repo")));
        assert!(same_repo(root, std::path::Path::new("/work/repo/src/ui")));
        assert!(!same_repo(root, std::path::Path::new("/work/repository")));
        assert!(!same_repo(root, std::path::Path::new("/work")));
    }
}
