use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use git_core::{DiffRequest, GitBackend, GixBackend};
use iced::widget::pane_grid;
use iced::{event, keyboard, window, Element, Event, Subscription, Task};

use crate::overlay::{ContextMenu, Form, FormAction, FormField, MenuEntry, Modal};
use crate::panes::Panes;
use crate::repo_view::{
    CenterView, CommitDetails, DetailChanges, Guarded, LastWrite, Loadable, NetworkKind, NetworkOp,
    NetworkRequest, Queued, RepoView, Section, Selection, WatchState, WriteOp,
};
use crate::session::{self, Session};
use crate::settings::{self, Settings};
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
pub const SHORTCUT_FILTER_INPUT: &str = "shortcut-filter";

/// File system events are collected for this long before they are classified.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(300);

/// Session changes are written after this quiet period.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);
/// Short status bar messages (zoom level) disappear after this time.
const STATUS_FLASH: Duration = Duration::from_secs(2);
/// Settings changes are written after this quiet period.
const SETTINGS_SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

/// What a discard affects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscardTarget {
    File(git_core::FileChange),
    /// All unstaged changes and untracked files.
    All,
    /// Staged changes too: index and working tree back to HEAD.
    Everything,
    /// Hunk `index` of the shown diff.
    Hunk(usize),
    /// Selected lines of the shown diff.
    Lines,
}

/// What to check out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckoutTarget {
    /// Local branch by name.
    Local(String),
    /// Remote branch by full name (`origin/topic`): its local tracking branch.
    Remote(String),
    /// Commit id (detached HEAD).
    Commit(String),
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
    /// A branch drag started on `name`.
    BranchDragStart {
        name: String,
        kind: git_core::RefKind,
    },
    BranchDragEnter {
        name: String,
        kind: git_core::RefKind,
    },
    BranchDragLeave {
        name: String,
        kind: git_core::RefKind,
    },
    /// Left button released over branch `name`: drops a dragged branch.
    BranchDropped {
        name: String,
        kind: git_core::RefKind,
        position: iced::Point,
    },
    /// Left button released anywhere: a drag not dropped on a branch ends.
    BranchDragEnd,
    /// Cherry-pick the commits in this order (oldest first) onto HEAD.
    CherryPick(Vec<String>),
    /// "Revert commit": a merge commit asks for the parent first.
    RevertStart(String),
    Revert {
        id: String,
        /// 1-based parent a merge commit is reverted against.
        mainline: Option<usize>,
    },
    /// Keep the empty result of a cherry-pick or revert step as a commit.
    CommitEmpty,
    /// "Reset to this commit": asks for the mode.
    ResetStart(String),
    /// Reset HEAD to `id`; a hard reset runs only once `confirmed`.
    Reset {
        id: String,
        mode: git_core::ResetMode,
        confirmed: bool,
    },
    /// "Edit commit message": opens the message dialog for commit `id`.
    RewordStart(String),
    /// "Squash N commits" for the multi-selection: opens the message dialog.
    SquashSelected,
    CommitMessageEdited(iced::widget::text_editor::Action),
    CommitMessageClosed,
    CommitMessageSubmit,
    /// Change the message of commit `id` (amend for HEAD, else a rebase).
    RewordCommit {
        id: String,
        message: String,
        autostash: bool,
    },
    /// Squash the contiguous commits `ids` into one with `message`.
    SquashCommits {
        ids: Vec<String>,
        message: String,
        autostash: bool,
    },
    /// Open the interactive rebase panel for the commits after `base`.
    RebasePanelOpened(String),
    RebaseLoaded {
        tab: TabId,
        base: String,
        result: Result<Vec<(String, String)>, String>,
    },
    RebaseSetAction {
        index: usize,
        action: git_core::TodoAction,
    },
    RebaseDragStart(usize),
    RebaseDragOver(usize),
    RebaseDragEnd,
    RebaseMessageEdited {
        id: String,
        action: iced::widget::text_editor::Action,
    },
    /// "Start rebase" in the panel.
    RebaseStart,
    /// Runs the prepared interactive rebase (also the "Stash and rebase" retry).
    RunInteractiveRebase {
        base: String,
        input: git_core::EditorInput,
        autostash: bool,
    },
    /// Open the conflict view for a conflicted file.
    ConflictOpened(String),
    ConflictLoaded {
        tab: TabId,
        path: String,
        result: Result<git_core::LoadedConflict, String>,
    },
    ConflictToggleLine {
        hunk: usize,
        side: git_core::Side,
        line: usize,
    },
    ConflictToggleHunk {
        hunk: usize,
        side: git_core::Side,
    },
    ConflictTakeAll(git_core::Side),
    ConflictOutputEdited(iced::widget::text_editor::Action),
    ConflictResetOutput,
    /// Save the output as the resolved file and stage it.
    ConflictSave,
    /// Take the whole file from one side (binary, delete/modify).
    ConflictTakeFile(git_core::Side),
    Retry(TabId),
    Refresh,
    Shortcut(Shortcut),
    SettingsLoaded(settings::Loaded),
    /// The status bar message number `seq` may disappear now.
    StatusExpired(u64),
    /// Zoom buttons in Preferences → Appearance.
    ZoomTo(u32),
    /// Light or dark mode of the operating system (start and changes).
    SystemTheme(iced::theme::Mode),
    ThemeChosen(settings::ThemeChoice),
    /// Open the settings view (Ctrl+, , gear button, palette).
    SettingsOpened,
    SettingsClosed,
    SettingsCategory(settings::Category),
    /// Typed auto-fetch interval (minutes).
    AutoFetchInput(String),
    SettingsSaveDue(u64),
    SettingsSaved(Result<(), String>),
    /// Filter of the keyboard shortcut list changed.
    ShortcutFilterChanged(String),
    ShortcutListClosed,
    /// Esc taken by a focused text field.
    EscapeCaptured,
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
        result: Result<DetailChanges, String>,
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
    /// Progress of the tab's running network operation.
    NetworkProgress {
        tab: TabId,
        progress: git_core::Progress,
    },
    /// Status bar "Cancel" of the tab's network operation.
    CancelNetwork(TabId),
    /// "Fetch all" (Pull dropdown, Ctrl+L) in the active tab.
    FetchAll,
    /// Pull dropdown opened at the cursor.
    PullMenu(iced::Point),
    /// Toolbar Pull (`None`: the current mode) or a mode from the dropdown,
    /// which becomes the button's mode.
    Pull(Option<git_core::PullMode>),
    /// Merge the branch with full reference name `source` into HEAD.
    /// `autostash`: stash local changes first and re-apply them afterwards.
    Merge {
        source: String,
        autostash: bool,
    },
    /// Rebase HEAD onto the branch with full reference name `onto`;
    /// unless `confirmed`, merge commits that would be flattened are
    /// pointed out first (M4-13).
    Rebase {
        onto: String,
        autostash: bool,
        confirmed: bool,
    },
    /// Fast-forward local `branch` (`None`: the checked-out branch) to the
    /// branch with full reference name `source`.
    FastForward {
        branch: Option<String>,
        source: String,
        autostash: bool,
    },
    /// Continue the operation in progress with the commit panel's message
    /// (banner button; the commit button does the same).
    ContinueOperation,
    /// Skip the current commit of a rebase or cherry-pick.
    SkipOperation,
    /// Abort the operation in progress (banner, failed pull dialog).
    AbortOperation,
    /// Double click on a branch, or "Checkout this commit" (asks first).
    Checkout(CheckoutTarget),
    CheckoutConfirmed(CheckoutTarget),
    /// Checkout after the detached HEAD check (nothing would be lost).
    CheckoutChecked(CheckoutTarget),
    /// Result of looking for commits only reachable from a detached HEAD.
    DetachedChecked {
        tab: TabId,
        target: CheckoutTarget,
        result: Result<Vec<String>, String>,
    },
    /// Local changes blocked the checkout: stash them, then check out.
    StashAndCheckout(CheckoutTarget),
    /// Open the inline new-branch input at commit `Some(id)` or at the
    /// selection (`None`: selected commit, or HEAD for the WIP row).
    NewBranchStart(Option<String>),
    NewBranchInput(String),
    NewBranchSubmit,
    /// Right click on a branch (sidebar entry or graph pill).
    BranchContextMenu {
        name: String,
        kind: git_core::RefKind,
        position: iced::Point,
    },
    /// Inline rename of a local branch.
    RenameStart(String),
    RenameInput(String),
    RenameSubmit,
    /// Run the rename; `remote` also renames the upstream branch.
    RenameConfirmed {
        remote: bool,
    },
    /// Ask before deleting a local branch.
    DeleteBranch(String),
    DeleteBranchConfirmed {
        name: String,
        force: bool,
    },
    /// Ask before deleting a remote branch (full name, e.g. `origin/topic`).
    DeleteRemoteBranch(String),
    DeleteRemoteBranchConfirmed(String),
    /// Form to choose the upstream branch of a local branch.
    SetUpstreamStart(String),
    UnsetUpstream(String),
    /// "+" in the remote section: form for name and URL.
    AddRemoteStart,
    /// Right click on a remote in the sidebar.
    RemoteContextMenu {
        name: String,
        position: iced::Point,
    },
    FetchRemote(String),
    EditRemoteUrl(String),
    RenameRemoteStart(String),
    /// Ask before removing a remote.
    RemoveRemote(String),
    RemoveRemoteConfirmed(String),
    /// Form for a new tag at commit `id`.
    CreateTagStart(String),
    /// Push a tag (asks for the remote if there are several).
    PushTag(String),
    /// Ask before deleting a local tag.
    DeleteTag(String),
    DeleteTagConfirmed(String),
    /// Ask before deleting a tag on a remote.
    DeleteRemoteTag(String),
    DeleteRemoteTagConfirmed {
        remote: String,
        tag: String,
    },
    /// Right click on a commit row.
    CommitContextMenu {
        id: String,
        position: iced::Point,
    },
    /// Fast-forward a branch that is not checked out to its upstream.
    PullBranch(String),
    /// Push a branch that is not checked out (form without upstream).
    PushBranch(String),
    /// Toolbar Push; `force` uses `--force-with-lease` (after confirmation).
    Push {
        force: bool,
    },
    /// Ask before a force push.
    ForcePushRequested,
    /// Input of field `field` of the open form changed.
    FormInput {
        field: usize,
        value: String,
    },
    FormSubmit,
    FormClosed,
    /// The auto-fetch timer with sequence number `seq` fired.
    AutoFetchDue(u64),
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
    /// User preferences (ADR-023).
    pub settings: Settings,
    /// Open settings view; replaces the tab content.
    pub settings_page: Option<settings::Page>,
    settings_path: Option<PathBuf>,
    settings_save_seq: u64,
    /// Light or dark mode of the operating system (`None` if unknown).
    pub system_theme: iced::theme::Mode,
    /// Result of looking for `git`; `None` until detected.
    pub git: Option<git_core::GitAvailability>,
    /// App-wide one-time notice (e.g. Git missing), dismissible.
    pub notice: Option<String>,
    /// Open modal dialog; blocks the rest of the window.
    pub modal: Option<Modal>,
    /// Open form dialog (inputs); below a modal, above everything else.
    pub form: Option<Form>,
    /// Open commit message dialog (reword, squash).
    pub message_dialog: Option<crate::overlay::MessageDialog>,
    pub context_menu: Option<ContextMenu>,
    /// Open keyboard shortcut list (Ctrl+/) with its filter text.
    pub shortcut_list: Option<String>,
    /// Last reported window size, in logical pixels of `window_scale`;
    /// use [`App::window_size`].
    window_size: iced::Size,
    /// Scale factor when `window_size` was reported.
    window_scale: f32,
    /// Number of the last flashed status message.
    status_seq: u64,
    /// Mode of the toolbar Pull button (persisted in the session).
    pub pull_mode: git_core::PullMode,
    /// When the pending auto-fetch timer fires, and its sequence number
    /// (older timers are ignored).
    auto_fetch_at: Option<std::time::Instant>,
    auto_fetch_seq: u64,
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
            settings: Settings::default(),
            settings_page: None,
            settings_path: session_path
                .as_deref()
                .map(|path| path.with_file_name(settings::FILE_NAME)),
            settings_save_seq: 0,
            system_theme: iced::theme::Mode::None,
            git: None,
            notice: None,
            modal: None,
            message_dialog: None,
            form: None,
            context_menu: None,
            shortcut_list: None,
            window_size: iced::Size::new(1400.0, 860.0),
            window_scale: 1.0,
            status_seq: 0,
            pull_mode: git_core::PullMode::default(),
            auto_fetch_at: None,
            auto_fetch_seq: 0,
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
        let settings_path = app.settings_path.clone();
        let task = match session_path {
            Some(path) => Task::perform(
                blocking(move || {
                    // Settings first: they may take values over from the
                    // session file (ADR-023).
                    let settings = settings_path.map(|p| settings::load(&p, Some(&path)));
                    Ok((settings, session::load(&path)))
                }),
                |loaded| loaded,
            )
            .then(|loaded| {
                let (settings, session) = loaded.unwrap_or_else(|error| {
                    (
                        None,
                        session::Loaded {
                            session: Session::default(),
                            notice: Some(error),
                        },
                    )
                });
                let session = Task::done(Message::SessionLoaded(session));
                match settings {
                    Some(settings) => Task::done(Message::SettingsLoaded(settings)).chain(session),
                    None => session,
                }
            }),
            None => Task::done(Message::SessionLoaded(session::Loaded {
                session: Session::default(),
                notice: None,
            })),
        };
        let detect = Task::perform(blocking(|| Ok(git_core::detect_git())), |result| {
            Message::GitDetected(result.unwrap_or(git_core::GitAvailability::Missing))
        });
        let system_theme = iced::system::theme().map(Message::SystemTheme);
        (app, Task::batch([task, detect, system_theme]))
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
        if view.busy.is_some() {
            let pending = LastWrite {
                label: label.clone(),
                op,
                clears_message: false,
            };
            self.status = yield_or_wait(view, Queued::Write(pending));
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

    /// Runs a network command (fetch, pull, push) in tab `tab` with progress
    /// and Cancel in the status bar. Network operations are write operations
    /// (ADR-020); a background one (auto-fetch) yields to user operations.
    pub fn run_network(&mut self, tab: TabId, request: NetworkRequest) -> Task<Message> {
        if let Some(reason) = self.write_block() {
            if !request.background {
                self.status = reason;
            }
            return Task::none();
        }
        let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
            return Task::none();
        };
        if view.busy.is_some() {
            // Auto-fetch never waits; it runs again at its next interval.
            if !request.background {
                self.status = yield_or_wait(view, Queued::Network(request));
            }
            return Task::none();
        }
        let cancel = git_core::CancelToken::new();
        // An old "Cancelled" or failure notice would be misleading now.
        if !request.background {
            self.status.clear();
        }
        let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
            return Task::none();
        };
        view.busy = Some(request.label.clone());
        view.suppress_watch = true;
        view.network = Some(NetworkOp {
            label: request.label.clone(),
            kind: request.kind,
            progress: None,
            cancel: cancel.clone(),
            background: request.background,
        });
        let writer = Arc::clone(&view.writer);
        Task::run(network_stream(tab, writer, request, cancel), |message| {
            message
        })
    }

    /// Fetches all remotes of tab `tab`; `background` for auto-fetch.
    fn fetch(&mut self, tab: TabId, background: bool) -> Task<Message> {
        let task = self.run_network(tab, crate::fetch::fetch_all(background));
        if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
            if view.network.is_some() || view.queued.is_some() {
                view.last_fetch = std::time::Instant::now();
            }
        }
        task
    }

    /// Opens the inline new-branch input (see [`Message::NewBranchStart`]).
    fn start_new_branch(&mut self, at: Option<String>) -> Task<Message> {
        if let Some(reason) = self.write_block() {
            self.status = reason;
            return Task::none();
        }
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        let snapshot = Arc::clone(&view.snapshot);
        let at = at.or_else(|| view.selected_commit().map(str::to_owned));
        let (start, row) = match at {
            Some(id) if snapshot.is_stash(&id) => {
                self.status = "Branches cannot start at a stash".into();
                return Task::none();
            }
            Some(id) => (id.clone(), Some(id)),
            None => match snapshot.head.target() {
                // The WIP row stands for HEAD.
                Some(head) if snapshot.has_wip => (head.to_owned(), None),
                Some(head) => (head.to_owned(), Some(head.to_owned())),
                None => {
                    self.status = "Create the first commit before creating branches".into();
                    return Task::none();
                }
            },
        };
        view.new_branch = Some(crate::repo_view::NewBranch {
            start,
            row,
            name: String::new(),
            error: None,
            submitted: false,
        });
        iced::widget::operation::focus(crate::ui::NEW_BRANCH_INPUT)
    }

    /// Validates the inline rename; a branch with an upstream asks whether
    /// the remote branch is renamed too.
    fn submit_rename(&mut self) -> Task<Message> {
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        let names = local_branch_names(&view.snapshot);
        let snapshot = Arc::clone(&view.snapshot);
        let Some(rename) = view.rename.as_mut() else {
            return Task::none();
        };
        let name = rename.name.trim().to_owned();
        if name == rename.old {
            view.rename = None;
            return Task::none();
        }
        if let Some(problem) = rename_problem(&rename.old, &name, &names) {
            rename.error = Some(problem);
            return Task::none();
        }
        rename.name = name.clone();
        let old = rename.old.clone();
        let upstream = snapshot
            .refs
            .iter()
            .find(|r| r.kind == git_core::RefKind::LocalBranch && r.name == old)
            .and_then(|r| r.upstream.clone());
        match upstream {
            Some(upstream) => {
                self.modal = Some(Modal {
                    title: "Rename branch".into(),
                    text: format!(
                        "{old} tracks {upstream}. Rename the remote branch to {name} too? \
                         Others using {upstream} would have to switch."
                    ),
                    details: None,
                    buttons: vec![
                        crate::overlay::DialogButton {
                            label: "Cancel".into(),
                            kind: crate::overlay::ButtonKind::Secondary,
                            message: None,
                        },
                        crate::overlay::DialogButton {
                            label: "Local only".into(),
                            kind: crate::overlay::ButtonKind::Secondary,
                            message: Some(Message::RenameConfirmed { remote: false }),
                        },
                        crate::overlay::DialogButton {
                            label: "Rename remote too".into(),
                            kind: crate::overlay::ButtonKind::Primary,
                            message: Some(Message::RenameConfirmed { remote: true }),
                        },
                    ],
                    default: Some(1),
                });
                Task::none()
            }
            None => self.rename_branch(false),
        }
    }

    /// Runs the inline rename; `remote` renames the upstream branch afterwards.
    fn rename_branch(&mut self, remote: bool) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        let Some(rename) = view.rename.clone() else {
            return Task::none();
        };
        let upstream = view
            .snapshot
            .refs
            .iter()
            .find(|r| r.kind == git_core::RefKind::LocalBranch && r.name == rename.old)
            .and_then(|r| r.upstream.clone());
        let follow_up = upstream
            .filter(|_| remote)
            .and_then(|up| {
                up.split_once('/')
                    .map(|(r, b)| (r.to_owned(), b.to_owned()))
            })
            .map(|(remote, old_branch)| NetworkRequest {
                label: format!("Rename {remote}/{old_branch}"),
                kind: NetworkKind::Push,
                args: git_core::rename_remote_branch_args(&remote, &rename.name, &old_branch),
                background: false,
            });
        let (old, new) = (rename.old.clone(), rename.name.clone());
        let task = self.run_undoable(tab, format!("Rename branch {old}"), move |git| {
            git.rename_branch(&old, &new)?;
            Ok(Some(History::Push(UndoAction::Ref(
                git_core::RefChange::RenameBranch {
                    old: old.clone(),
                    new: new.clone(),
                },
            ))))
        });
        if let Some(view) = self.active_view_mut() {
            if view.busy.is_some() {
                if let Some(rename) = view.rename.as_mut() {
                    rename.submitted = true;
                }
                view.then_network = follow_up;
            }
        }
        task
    }

    /// Creates the branch typed in the inline input and checks it out.
    fn create_branch(&mut self) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_view_mut() else {
            return Task::none();
        };
        let names = local_branch_names(&view.snapshot);
        let Some(input) = view.new_branch.as_mut() else {
            return Task::none();
        };
        let name = input.name.trim().to_owned();
        if let Some(problem) = branch_name_problem(&name, &names) {
            input.error = Some(problem);
            return Task::none();
        }
        let start = input.start.clone();
        let label = format!("Create branch {name}");
        let task = self.run_undoable(tab, label, move |git| {
            let from = git_core::head_position(git)?;
            git.create_branch(&name, &start)?;
            Ok(from.map(|from| {
                History::Push(UndoAction::Ref(git_core::RefChange::CreateBranch {
                    name: name.clone(),
                    target: start.clone(),
                    from,
                }))
            }))
        });
        if let Some(view) = self.active_view_mut() {
            let running = view.busy.is_some();
            if let Some(input) = view.new_branch.as_mut() {
                input.submitted = running;
            }
        }
        task
    }

    /// Checks out `target` in the active tab; `stash_first` stashes local
    /// changes (including untracked files) before, and leaves the stash.
    fn checkout(&mut self, target: CheckoutTarget, stash_first: bool) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_tab().and_then(Tab::view) else {
            return Task::none();
        };
        let snapshot = &view.snapshot;
        let local_exists = |name: &str| {
            snapshot
                .refs
                .iter()
                .any(|r| r.kind == git_core::RefKind::LocalBranch && r.name == name)
        };
        // (label, local branch to switch to, remote ref to track, commit to detach at)
        let (label, switch, track, detach) = match &target {
            CheckoutTarget::Local(name) => {
                if snapshot.head.branch() == Some(name.as_str()) {
                    self.status = format!("Already on {name}");
                    return Task::none();
                }
                (format!("Checkout {name}"), Some(name.clone()), None, None)
            }
            CheckoutTarget::Remote(full) => {
                let local = full
                    .split_once('/')
                    .map_or(full.as_str(), |(_, branch)| branch)
                    .to_owned();
                if snapshot.head.branch() == Some(local.as_str()) {
                    self.status = format!("Already on {local}");
                    return Task::none();
                }
                // An existing branch with the same name is reused (GitKraken).
                if local_exists(&local) {
                    (format!("Checkout {local}"), Some(local), None, None)
                } else {
                    (
                        format!("Checkout {full}"),
                        None,
                        Some((local, full.clone())),
                        None,
                    )
                }
            }
            CheckoutTarget::Commit(id) => (
                format!("Checkout {}", &id[..id.len().min(7)]),
                None,
                None,
                Some(id.clone()),
            ),
        };
        let stash_message = format!("Gitloom: before {}", label.to_lowercase());
        let task = self.run_undoable(tab, label, move |git| {
            if stash_first {
                git.stash_push(Some(&stash_message))?;
            }
            let from = git_core::head_position(git)?;
            match (&switch, &track, &detach) {
                (Some(branch), _, _) => git.switch_branch(branch)?,
                (_, Some((local, remote)), _) => git.switch_tracking(local, remote)?,
                (_, _, Some(id)) => git.switch_detached(id)?,
                _ => {}
            }
            // A new tracking branch stays when the checkout is undone.
            let to = git_core::head_position(git)?;
            Ok(from.zip(to).map(|(from, to)| {
                History::Push(UndoAction::Ref(git_core::RefChange::Checkout { from, to }))
            }))
        });
        if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
            if view.busy.is_some() {
                view.pending_checkout = Some(target);
            }
        }
        task
    }

    /// Pushes the current branch of tab `tab` to its upstream; without one,
    /// a form asks for remote and branch name and sets the upstream.
    fn push(&mut self, tab: TabId, force: bool) -> Task<Message> {
        let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
            return Task::none();
        };
        let snapshot = Arc::clone(&view.snapshot);
        let Some((local, upstream)) = snapshot.head_upstream() else {
            self.modal = Some(Modal::info(
                "Cannot push",
                "HEAD is detached. Create or check out a branch to push.",
            ));
            return Task::none();
        };
        match upstream {
            Some((remote, branch)) => {
                let request = NetworkRequest {
                    label: if force { "Force push" } else { "Push" }.into(),
                    kind: NetworkKind::Push,
                    args: git_core::push_args(&remote, local, &branch, false, force),
                    background: false,
                };
                self.run_network(tab, request)
            }
            None => self.push_setup_form(tab, local.to_owned()),
        }
    }

    /// Upstream of local branch `local` as `(remote, branch)`.
    fn branch_upstream(&self, local: &str) -> Option<(String, String)> {
        let snapshot = &self.active_tab()?.view()?.snapshot;
        let upstream = snapshot
            .refs
            .iter()
            .find(|r| r.kind == git_core::RefKind::LocalBranch && r.name == local)?
            .upstream
            .clone()?;
        let (remote, branch) = upstream.split_once('/')?;
        Some((remote.to_owned(), branch.to_owned()))
    }

    /// Form for the first push of `local`: remote and remote branch name.
    fn push_setup_form(&mut self, tab: TabId, local: String) -> Task<Message> {
        let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
            return Task::none();
        };
        let remotes = view.snapshot.remotes.clone();
        if remotes.is_empty() {
            self.modal = Some(Modal::info(
                "Cannot push",
                "This repository has no remote. Add one with \"+\" in the REMOTE section.",
            ));
            return Task::none();
        }
        let remote = if remotes.iter().any(|r| r == "origin") {
            "origin".to_owned()
        } else {
            remotes[0].clone()
        };
        self.form = Some(Form {
            title: format!("Push {local}"),
            text: format!("{local} has no upstream branch yet. Push it and set the upstream:"),
            fields: vec![
                FormField::choice("Remote", remote, remotes),
                FormField::text("Branch name on the remote", local.clone()),
            ],
            submit: "Push".into(),
            action: FormAction::PushSetUpstream { tab, local },
            error: None,
        });
        iced::widget::operation::focus(crate::ui::overlays::FORM_FIRST_INPUT)
    }

    /// Form to choose the upstream of `local` among the remote branches.
    fn set_upstream_form(&mut self, local: String) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_tab().and_then(Tab::view) else {
            return Task::none();
        };
        let remote_branches: Vec<String> = view
            .snapshot
            .refs
            .iter()
            .filter(|r| r.kind == git_core::RefKind::RemoteBranch)
            .map(|r| r.name.clone())
            .collect();
        if remote_branches.is_empty() {
            self.modal = Some(Modal::info(
                "No remote branches",
                "There is no remote branch to track. Fetch a remote, or push the branch to \
                 create it on the remote.",
            ));
            return Task::none();
        }
        let current = view
            .snapshot
            .refs
            .iter()
            .find(|r| r.kind == git_core::RefKind::LocalBranch && r.name == local)
            .and_then(|r| r.upstream.clone());
        let same_name = format!("origin/{local}");
        // A gone upstream (deleted on the remote) is no valid choice.
        let default = current
            .filter(|c| remote_branches.contains(c))
            .or_else(|| remote_branches.iter().find(|b| **b == same_name).cloned())
            .unwrap_or_else(|| remote_branches[0].clone());
        self.form = Some(Form {
            title: format!("Set upstream of {local}"),
            text: "Pull and push of this branch use the upstream branch.".into(),
            fields: vec![FormField::choice(
                "Upstream branch",
                default,
                remote_branches,
            )],
            submit: "Set upstream".into(),
            action: FormAction::SetUpstream { tab, local },
            error: None,
        });
        Task::none()
    }

    /// Push (`delete == false`) or remote delete of tag `tag`: one remote
    /// runs directly (delete after a confirmation), several ask in a form.
    fn tag_remote_action(&mut self, tag: String, delete: bool) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let remotes = self
            .active_tab()
            .and_then(Tab::view)
            .map(|v| v.snapshot.remotes.clone())
            .unwrap_or_default();
        match remotes.as_slice() {
            [] => {
                self.modal = Some(Modal::info(
                    "No remote",
                    "This repository has no remote to push tags to.",
                ));
                Task::none()
            }
            [remote] if delete => {
                self.modal = Some(Modal::confirm(
                    "Delete remote tag",
                    format!("Delete the tag {tag} on {remote}?"),
                    "Delete",
                    true,
                    Message::DeleteRemoteTagConfirmed {
                        remote: remote.clone(),
                        tag,
                    },
                ));
                Task::none()
            }
            [remote] => self.run_network(tab, push_tag_request(remote, &tag)),
            _ => {
                let default = if remotes.iter().any(|r| r == "origin") {
                    "origin".to_owned()
                } else {
                    remotes[0].clone()
                };
                let (title, submit, action) = if delete {
                    (
                        format!("Delete tag {tag} on a remote"),
                        "Delete",
                        FormAction::DeleteRemoteTag { tab, tag },
                    )
                } else {
                    (
                        format!("Push tag {tag}"),
                        "Push",
                        FormAction::PushTag { tab, tag },
                    )
                };
                self.form = Some(Form {
                    title,
                    text: String::new(),
                    fields: vec![FormField::choice("Remote", default, remotes)],
                    submit: submit.into(),
                    action,
                    error: None,
                });
                Task::none()
            }
        }
    }

    /// Validates and runs the open form.
    fn submit_form(&mut self) -> Task<Message> {
        let Some(action) = self.form.as_ref().map(|f| f.action.clone()) else {
            return Task::none();
        };
        let remotes: Vec<String> = match &action {
            FormAction::AddRemote { tab } | FormAction::RenameRemote { tab, .. } => self
                .tab_mut(*tab)
                .and_then(|t| t.view())
                .map(|v| v.snapshot.remotes.clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let Some(form) = self.form.as_mut() else {
            return Task::none();
        };
        match form.action.clone() {
            FormAction::SetUpstream { tab, local } => {
                let upstream = form.value(0).to_owned();
                self.form = None;
                self.run_write(tab, format!("Set upstream of {local}"), move |git| {
                    git.set_upstream(&local, &upstream)
                })
            }
            FormAction::AddRemote { tab } => {
                let (name, url) = (form.value(0).to_owned(), form.value(1).to_owned());
                if let Some(problem) = remote_name_problem(&name, &remotes) {
                    form.error = Some(problem);
                    return Task::none();
                }
                if url.is_empty() {
                    form.error = Some("Enter the URL of the remote repository".into());
                    return Task::none();
                }
                self.form = None;
                self.run_write(tab, format!("Add remote {name}"), move |git| {
                    git.add_remote(&name, &url)
                })
            }
            FormAction::SetRemoteUrl { tab, name } => {
                let url = form.value(0).to_owned();
                if url.is_empty() {
                    form.error = Some("Enter the URL of the remote repository".into());
                    return Task::none();
                }
                self.form = None;
                self.run_write(tab, format!("Edit URL of {name}"), move |git| {
                    git.set_remote_url(&name, &url)
                })
            }
            FormAction::RenameRemote { tab, old } => {
                let new = form.value(0).to_owned();
                if new == old {
                    self.form = None;
                    return Task::none();
                }
                let others: Vec<String> = remotes.into_iter().filter(|r| *r != old).collect();
                if let Some(problem) = remote_name_problem(&new, &others) {
                    form.error = Some(problem);
                    return Task::none();
                }
                self.form = None;
                self.run_write(tab, format!("Rename remote {old}"), move |git| {
                    git.rename_remote(&old, &new)
                })
            }
            FormAction::CreateTag { tab, target } => {
                let name = form.value(0).to_owned();
                let message = form.value(1).to_owned();
                let tags: Vec<String> = self
                    .tab_mut(tab)
                    .and_then(|t| t.view())
                    .map(|v| {
                        v.snapshot
                            .refs
                            .iter()
                            .filter(|r| r.kind == git_core::RefKind::Tag)
                            .map(|r| r.name.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                let Some(form) = self.form.as_mut() else {
                    return Task::none();
                };
                if let Some(problem) = git_core::check_branch_name(&name) {
                    form.error = Some(
                        problem
                            .replace("Branch names", "Tag names")
                            .replace("branch name", "tag name"),
                    );
                    return Task::none();
                }
                if tags.contains(&name) {
                    form.error = Some(format!("A tag named \"{name}\" already exists"));
                    return Task::none();
                }
                self.form = None;
                self.run_undoable(tab, format!("Create tag {name}"), move |git| {
                    let message = (!message.is_empty()).then(|| message.clone());
                    git.create_tag(&name, &target, message.as_deref())?;
                    Ok(Some(History::Push(UndoAction::Ref(
                        git_core::RefChange::CreateTag {
                            name: name.clone(),
                            target: target.clone(),
                            message,
                        },
                    ))))
                })
            }
            FormAction::PushTag { tab, tag } => {
                let remote = form.value(0).to_owned();
                self.form = None;
                self.run_network(tab, push_tag_request(&remote, &tag))
            }
            FormAction::DeleteRemoteTag { tab, tag } => {
                let remote = form.value(0).to_owned();
                self.form = None;
                self.run_network(tab, delete_remote_tag_request(&remote, &tag))
            }
            FormAction::RevertMerge { tab: _, id } => {
                // "2: abc1234 summary" → parent 2.
                let mainline = form
                    .value(0)
                    .split(':')
                    .next()
                    .and_then(|n| n.trim().parse::<usize>().ok());
                self.form = None;
                self.merge(Message::Revert { id, mainline })
            }
            FormAction::PushSetUpstream { tab, local } => {
                let remote = form.value(0).to_owned();
                let branch = form.value(1).to_owned();
                if remote.is_empty() || branch.is_empty() {
                    form.error = Some("Choose a remote and enter a branch name.".into());
                    return Task::none();
                }
                self.form = None;
                let is_head = self
                    .tab_mut(tab)
                    .and_then(|t| t.view())
                    .is_some_and(|v| v.snapshot.head.branch() == Some(local.as_str()));
                self.run_network(
                    tab,
                    NetworkRequest {
                        label: if is_head {
                            "Push".into()
                        } else {
                            format!("Push {local}")
                        },
                        kind: if is_head {
                            NetworkKind::Push
                        } else {
                            NetworkKind::PushBranch
                        },
                        args: git_core::push_args(&remote, &local, &branch, true, false),
                        background: false,
                    },
                )
            }
        }
    }

    /// Pulls the current branch of tab `tab` in the current pull mode.
    fn pull(&mut self, tab: TabId) -> Task<Message> {
        let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) else {
            return Task::none();
        };
        if let Some((local, None)) = view.snapshot.head_upstream() {
            let local = local.to_owned();
            self.modal = Some(Modal {
                title: "Cannot pull".into(),
                text: format!(
                    "{local} has no upstream branch, so there is nothing to pull from. \
                     Set an upstream branch, or push the branch first."
                ),
                details: None,
                buttons: vec![
                    crate::overlay::DialogButton {
                        label: "Close".into(),
                        kind: crate::overlay::ButtonKind::Secondary,
                        message: None,
                    },
                    crate::overlay::DialogButton {
                        label: "Set upstream".into(),
                        kind: crate::overlay::ButtonKind::Primary,
                        message: Some(Message::SetUpstreamStart(local)),
                    },
                ],
                default: Some(1),
            });
            return Task::none();
        }
        if let Some(problem) = pull_blocker(&view.snapshot) {
            self.modal = Some(Modal::info("Cannot pull", problem));
            return Task::none();
        }
        let request = NetworkRequest {
            label: self.pull_mode.label().into(),
            ..crate::fetch::pull(self.pull_mode)
        };
        let task = self.run_network(tab, request);
        if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
            if view.network.is_some() {
                view.last_fetch = std::time::Instant::now();
            }
        }
        task
    }

    /// Starts an auto-fetch in every tab where one is due.
    fn auto_fetch(&mut self, now: std::time::Instant) -> Task<Message> {
        let interval = self.settings.general.auto_fetch_minutes;
        let due: Vec<TabId> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                let state = auto_fetch_state(tab.view()?);
                crate::fetch::auto_fetch_due(now, interval, state).then_some(tab.id)
            })
            .collect();
        Task::batch(due.into_iter().map(|tab| self.fetch(tab, true)))
    }

    /// Applies the loaded session: restores tabs and panes, then opens pending paths.
    fn restore(&mut self, loaded: session::Loaded) -> Task<Message> {
        let session::Loaded { session, notice } = loaded;
        self.session_loaded = true;
        if let Some(notice) = notice {
            self.status = notice;
        }
        self.pull_mode = session.pull_mode.into();
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
            pull_mode: self.pull_mode.into(),
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

    /// Theme of the window: the choice in the settings; "System" follows
    /// the operating system and falls back to Dark when it is unknown.
    pub fn theme(&self) -> iced::Theme {
        crate::theme::app_theme(self.light_theme())
    }

    pub fn light_theme(&self) -> bool {
        match self.settings.appearance.theme {
            settings::ThemeChoice::Dark => false,
            settings::ThemeChoice::Light => true,
            settings::ThemeChoice::System => self.system_theme == iced::theme::Mode::Light,
        }
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
        let overlay_open = self.modal.is_some()
            || self.context_menu.is_some()
            || self.message_dialog.is_some()
            || self.shortcut_list.is_some()
            || self.settings_page.is_some();
        // Zoom and fullscreen change nothing in the overlay's content.
        if overlay_open
            && !matches!(
                shortcut,
                Shortcut::Close
                    | Shortcut::Confirm
                    | Shortcut::ZoomIn
                    | Shortcut::ZoomOut
                    | Shortcut::ZoomReset
                    | Shortcut::Fullscreen
            )
        {
            return Task::none();
        }
        if let Some(reason) = shortcut.static_block() {
            if let Some(action) = crate::actions::find(shortcut) {
                self.status = format!("{}: {reason}", action.label);
            }
            return Task::none();
        }
        if let Some(panel) = self.active_rebase_panel() {
            if rebase_panel_shortcut(panel, shortcut) {
                return Task::none();
            }
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
                    if let Some(view) = self.active_view_mut() {
                        match &selection {
                            Selection::Commit(id) => select_multi(view, id, false, false),
                            Selection::Wip => view.multi.clear(),
                        }
                    }
                    let task = self.select(selection);
                    return Task::batch([task, self.reveal_selection()]);
                }
            }
            Shortcut::ExtendSelection(step) => {
                let target = self.active_tab().and_then(Tab::view).and_then(|view| {
                    crate::nav::navigate(&view.snapshot, view.selection.as_ref(), step)
                });
                // The WIP row and stashes are not part of a multi-selection.
                if let Some(Selection::Commit(id)) = target {
                    let Some(view) = self.active_view_mut() else {
                        return Task::none();
                    };
                    if view.snapshot.is_stash(&id) {
                        return Task::none();
                    }
                    select_multi(view, &id, false, true);
                    let task = self.select(Selection::Commit(id));
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
            Shortcut::FetchAll => return self.update(Message::FetchAll),
            Shortcut::CreateBranch => return self.update(Message::NewBranchStart(None)),
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
            Shortcut::ShortcutList => {
                self.shortcut_list = Some(String::new());
                return iced::widget::operation::focus(SHORTCUT_FILTER_INPUT);
            }
            // Disabled until their task lands (`static_block` above).
            Shortcut::CommandPalette
            | Shortcut::SearchCommits
            | Shortcut::OpenRepoPalette
            | Shortcut::FileHistory
            | Shortcut::OpenTerminal
            | Shortcut::OpenFileManager
            | Shortcut::ExternalDiff => {}
            Shortcut::ZoomIn => {
                return self.set_zoom(settings::zoom_step(
                    self.settings.appearance.zoom_percent,
                    true,
                ))
            }
            Shortcut::ZoomOut => {
                return self.set_zoom(settings::zoom_step(
                    self.settings.appearance.zoom_percent,
                    false,
                ))
            }
            Shortcut::ZoomReset => return self.set_zoom(settings::ZOOM_DEFAULT),
            Shortcut::Fullscreen => {
                return window::latest().and_then(|id| {
                    window::mode(id).then(move |mode| {
                        let next = if mode == window::Mode::Fullscreen {
                            window::Mode::Windowed
                        } else {
                            window::Mode::Fullscreen
                        };
                        window::set_mode(id, next)
                    })
                });
            }
            Shortcut::Settings => return self.open_settings(),
            Shortcut::Close => {
                if self.modal.take().is_some()
                    || self.context_menu.take().is_some()
                    || self.shortcut_list.take().is_some()
                    || self.settings_page.take().is_some()
                    || self.form.take().is_some()
                    || self.message_dialog.take().is_some()
                    || self
                        .active_view_mut()
                        .is_some_and(|v| v.drag.take().is_some())
                    || self
                        .active_view_mut()
                        .is_some_and(|v| v.new_branch.take().is_some() || v.rename.take().is_some())
                {
                    return Task::none();
                }
                if let Some(view) = self.active_view_mut() {
                    view.center = CenterView::Graph;
                }
            }
            // Only meaningful in the interactive rebase panel (handled above).
            Shortcut::TodoKey(_) | Shortcut::Reorder { .. } => {}
            Shortcut::Confirm => {
                if let Some(modal) = self.modal.take() {
                    if let Some(message) = modal.default_action() {
                        return self.update(message);
                    }
                } else if self.form.is_some() {
                    return self.update(Message::FormSubmit);
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
        // During an operation the commit button continues it (`--continue`).
        if let Some(operation) = view.snapshot.state.kind().filter(|_| !amend) {
            let label = format!("Continue {}", operation.command());
            let pending = view.pending_history.clone();
            let task = self.run_undoable(tab, label, move |git| {
                if stage_all {
                    git.stage_all()?;
                }
                git.continue_operation(operation, &message)?;
                complete_pending(git, pending.clone())
            });
            if let Some(view) = self.active_view_mut() {
                if view.busy.is_some() {
                    view.clear_message_after_write = true;
                }
            }
            return task;
        }
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

    /// The interactive rebase panel of the active tab, if it is open.
    fn active_rebase_panel(&mut self) -> Option<&mut crate::rebase_panel::RebasePanel> {
        match &mut self.active_view_mut()?.center {
            CenterView::Rebase(panel) => Some(panel),
            _ => None,
        }
    }

    /// The conflict view of the active tab, if it is open.
    fn active_conflict(&mut self) -> Option<&mut crate::conflict::ConflictView> {
        match &mut self.active_view_mut()?.center {
            CenterView::Conflict(conflict) => Some(conflict),
            _ => None,
        }
    }

    /// Runs a [`Message::Merge`], [`Message::FastForward`] or
    /// [`Message::Rebase`]; local changes that block it lead to the "Stash
    /// and merge" (or rebase) dialog.
    fn merge(&mut self, request: Message) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(view) = self.active_tab().and_then(Tab::view) else {
            return Task::none();
        };
        let head = head_name(&view.snapshot).to_owned();
        let operation = view.snapshot.state.kind();
        let root = view.backend.path().to_path_buf();
        let pending = view.pending_history.clone();
        let short = |source: &str| {
            source
                .strip_prefix("refs/heads/")
                .or_else(|| source.strip_prefix("refs/remotes/"))
                .unwrap_or(source)
                .to_owned()
        };
        let (label, touches_worktree) = match &request {
            Message::Merge { source, .. } => (format!("Merge {} into {head}", short(source)), true),
            Message::Rebase { onto, .. } => (format!("Rebase {head} onto {}", short(onto)), true),
            Message::RunInteractiveRebase { .. } => (format!("Interactive rebase of {head}"), true),
            Message::RewordCommit { id, .. } => {
                (format!("Edit message of {}", &id[..id.len().min(7)]), true)
            }
            Message::SquashCommits { ids, .. } => (format!("Squash {} commits", ids.len()), true),
            Message::CherryPick(ids) => (
                match ids.as_slice() {
                    [id] => format!("Cherry-pick {}", &id[..id.len().min(7)]),
                    _ => format!("Cherry-pick {} commits", ids.len()),
                },
                true,
            ),
            Message::Revert { id, .. } => (format!("Revert {}", &id[..id.len().min(7)]), true),
            Message::CommitEmpty => ("Commit empty".to_string(), false),
            Message::Reset { id, mode, .. } => (
                format!(
                    "{} reset {head} to {}",
                    mode.label(),
                    &id[..id.len().min(7)]
                ),
                true,
            ),
            Message::FastForward {
                branch: None,
                source,
                ..
            } => (format!("Fast-forward {head} to {}", short(source)), true),
            Message::FastForward {
                branch: Some(branch),
                source,
                ..
            } => (format!("Fast-forward {branch} to {}", short(source)), false),
            _ => return Task::none(),
        };
        let guard = if matches!(request, Message::Reset { .. }) {
            Guarded::Reset
        } else {
            Guarded::Merge
        };
        if touches_worktree && self.state_blocked(guard) {
            return Task::none();
        }
        let op = request.clone();
        let entry_label = label.clone();
        let started_label = label.clone();
        let task = self.run_undoable(tab, label, move |git| {
            use git_core::{HistoryUndo, MergeOutcome};
            // A fast-forward of another branch moves only that branch.
            if let Message::FastForward {
                branch: Some(branch),
                source,
                ..
            } = &op
            {
                let reference = format!("refs/heads/{branch}");
                let before = git_core::ref_commit(git, &reference)?;
                git_core::fast_forward_branch(git, branch, source)?;
                let after = git_core::ref_commit(git, &reference)?;
                return Ok(before
                    .zip(after)
                    .filter(|(b, a)| b != a)
                    .map(|(before, after)| {
                        History::Push(UndoAction::History(git_core::HistoryChange {
                            label: entry_label.clone(),
                            kind: HistoryUndo::MoveBranch {
                                name: branch.clone(),
                            },
                            before,
                            after,
                            branch: None,
                        }))
                    }));
            }
            // "Commit empty" completes the cherry-pick or revert that stopped.
            if let Message::CommitEmpty = &op {
                if let Some(operation) = operation {
                    git_core::commit_empty(git, operation)?;
                }
                return complete_pending(git, pending.clone());
            }
            let Some(before) = git_core::head_commit(git)? else {
                return Ok(None);
            };
            let branch = git_core::current_branch(git)?;
            let (outcome, kind) = match &op {
                Message::Merge { source, autostash } => {
                    (git_core::merge(git, source, *autostash)?, HistoryUndo::Keep)
                }
                Message::Rebase {
                    onto, autostash, ..
                } => (git_core::rebase(git, onto, *autostash)?, HistoryUndo::Keep),
                Message::RunInteractiveRebase {
                    base,
                    input,
                    autostash,
                } => (
                    git_core::rebase_interactive(git, base, input, *autostash)?,
                    HistoryUndo::Keep,
                ),
                Message::RewordCommit {
                    id,
                    message,
                    autostash,
                } => (
                    git_core::reword_commit(git, id, message, *autostash)?,
                    HistoryUndo::Keep,
                ),
                Message::SquashCommits {
                    ids,
                    message,
                    autostash,
                } => (
                    git_core::squash_commits(git, ids, message, *autostash)?,
                    HistoryUndo::Keep,
                ),
                Message::CherryPick(ids) => (git_core::cherry_pick(git, ids)?, HistoryUndo::Keep),
                Message::Revert { id, mainline } => {
                    (git_core::revert(git, id, *mainline)?, HistoryUndo::Keep)
                }
                Message::Reset {
                    id,
                    mode: git_core::ResetMode::Hard,
                    ..
                } => (
                    MergeOutcome::Done,
                    HistoryUndo::HardReset(git_core::hard_reset_with_snapshot(git, &root, id)?),
                ),
                Message::Reset { id, mode, .. } => {
                    let index_tree = git_core::index_tree(git)?;
                    git_core::reset(git, id, *mode)?;
                    (
                        MergeOutcome::Done,
                        HistoryUndo::Reset {
                            mode: *mode,
                            index_tree,
                        },
                    )
                }
                Message::FastForward {
                    source, autostash, ..
                } => {
                    git_core::fast_forward_head(git, source, *autostash)?;
                    (MergeOutcome::Done, HistoryUndo::Keep)
                }
                _ => return Ok(None),
            };
            let entry = git_core::HistoryChange {
                label: entry_label.clone(),
                kind,
                before,
                after: String::new(),
                branch,
            };
            match outcome {
                // Stopped: the entry waits for Continue (or goes with Abort).
                MergeOutcome::Conflicts => Ok(Some(History::Started(UndoAction::History(entry)))),
                MergeOutcome::Done => finish_entry(git, entry),
            }
        });
        let picks = matches!(
            request,
            Message::CherryPick(_) | Message::Revert { .. } | Message::CommitEmpty
        );
        // Queued behind a yielding auto-fetch, the next WriteFinished is the
        // fetch's: only a request that started may leave these flags.
        let started = self
            .tab_mut(tab)
            .and_then(Tab::view_mut)
            .is_some_and(|v| v.busy.as_deref() == Some(started_label.as_str()));
        if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
            if picks && started {
                view.check_empty = true;
            }
        }
        if touches_worktree {
            if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
                if started {
                    view.pending_merge = Some(request);
                }
                // The panel's plan runs now (or once the auto-fetch stopped);
                // conflicts show in the banner.
                if view.busy.is_some() && matches!(view.center, CenterView::Rebase(_)) {
                    view.center = CenterView::Graph;
                }
            }
        }
        task
    }

    /// Skips the current commit (`skip`) or aborts the operation in progress.
    /// The message in the commit panel belonged to that step and is cleared.
    fn skip_or_abort(&mut self, skip: bool) -> Task<Message> {
        let Some(tab) = self.active_tab().map(|t| t.id) else {
            return Task::none();
        };
        let Some(operation) = self
            .active_tab()
            .and_then(Tab::view)
            .and_then(|v| v.snapshot.state.kind())
        else {
            return Task::none();
        };
        if skip && !operation.can_skip() {
            return Task::none();
        }
        let verb = if skip { "Skip" } else { "Abort" };
        let pending = self
            .active_tab()
            .and_then(Tab::view)
            .and_then(|v| v.pending_history.clone());
        let label = format!("{verb} {}", operation.command());
        let task = self.run_undoable(tab, label, move |git| {
            if skip {
                git.skip_operation(operation)?;
                complete_pending(git, pending.clone())
            } else {
                // An aborted operation leaves no undo entry.
                git.abort_operation(operation).map(|()| None)
            }
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
        if stack.last().is_some_and(UndoAction::moves_head) {
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
                UndoAction::Ref(change) => {
                    change.apply(git, undo)?;
                    UndoAction::Ref(change)
                }
                UndoAction::History(change) => UndoAction::History(if undo {
                    git_core::undo_history(git, &root, &change)?
                } else {
                    git_core::redo_history(git, &root, &change)?
                }),
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
            DiscardTarget::Everything => {
                let tree = view.snapshot.working_tree.clone();
                if matches!(&view.center, CenterView::Diff(d) if d.staged().is_some()) {
                    view.center = CenterView::Graph;
                }
                self.run_undoable(tab, "Discard all changes", move |git| {
                    let snapshot = git_core::discard_everything(git, &root, &tree)?;
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
            git_core::DiffKind::Commit(_) | git_core::DiffKind::Stash { .. } => None,
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
                let remotes = backend.remotes().map_err(|e| e.to_string())?;
                Ok(refs_changed(&snapshot, &head, &refs, &stashes)
                    || state != snapshot.state
                    || remotes != snapshot.remotes)
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
        let is_stash = view.snapshot.is_stash(&id);
        Task::perform(
            blocking(move || {
                // A stash keeps staged changes and untracked files in extra parents.
                let changes = if is_stash {
                    backend.stash_changes(&commit).map(DetailChanges::Stash)
                } else {
                    backend.commit_changes(&commit).map(DetailChanges::Commit)
                };
                changes.map_err(|e| e.to_string())
            }),
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
        let timer = self.schedule_auto_fetch();
        Task::batch([task, details, scroll, self.persist(), timer])
    }

    /// Starts a timer for the next auto-fetch check unless one is pending
    /// for that time or earlier (see [`crate::fetch::next_check`]).
    fn schedule_auto_fetch(&mut self) -> Task<Message> {
        let now = std::time::Instant::now();
        let tabs: Vec<crate::fetch::TabState> = self
            .tabs
            .iter()
            .filter_map(|tab| tab.view().map(auto_fetch_state))
            .collect();
        let Some(next) =
            crate::fetch::next_check(now, self.settings.general.auto_fetch_minutes, &tabs)
        else {
            self.auto_fetch_at = None;
            return Task::none();
        };
        if self.auto_fetch_at.is_some_and(|at| at <= next) {
            return Task::none();
        }
        self.auto_fetch_at = Some(next);
        self.auto_fetch_seq += 1;
        let seq = self.auto_fetch_seq;
        // Created lazily: the timer needs the runtime that runs the task.
        Task::perform(
            async move { tokio::time::sleep_until(tokio::time::Instant::from_std(next)).await },
            move |()| Message::AutoFetchDue(seq),
        )
    }

    /// Drops the pending auto-fetch timer and plans the next one with the
    /// current interval.
    fn restart_auto_fetch(&mut self) -> Task<Message> {
        self.auto_fetch_at = None;
        self.auto_fetch_seq += 1;
        self.schedule_auto_fetch()
    }

    /// Writes the settings after a short quiet period (typing an interval
    /// changes them per key).
    fn save_settings(&mut self) -> Task<Message> {
        if self.settings_path.is_none() {
            return Task::none();
        }
        self.settings_save_seq += 1;
        let seq = self.settings_save_seq;
        Task::perform(
            async { tokio::time::sleep(SETTINGS_SAVE_DEBOUNCE).await },
            move |()| Message::SettingsSaveDue(seq),
        )
    }

    /// Window scale from the zoom setting (M5-04).
    pub fn scale_factor(&self) -> f32 {
        self.settings.appearance.zoom_percent as f32 / 100.0
    }

    /// Window size in the current zoom's logical pixels (menus are kept
    /// inside it).
    pub fn window_size(&self) -> iced::Size {
        let ratio = self.window_scale / self.scale_factor();
        iced::Size::new(
            self.window_size.width * ratio,
            self.window_size.height * ratio,
        )
    }

    /// Applies a zoom level, stores it and names it in the status bar.
    fn set_zoom(&mut self, percent: u32) -> Task<Message> {
        let percent = settings::clamp_zoom(percent);
        let text = format!("Zoom {percent} %");
        let flash = self.flash_status(text);
        if percent == self.settings.appearance.zoom_percent {
            return flash;
        }
        self.settings.appearance.zoom_percent = percent;
        Task::batch([flash, self.save_settings()])
    }

    /// Shows `text` in the status bar for a moment.
    fn flash_status(&mut self, text: String) -> Task<Message> {
        self.status = text;
        self.status_seq += 1;
        let seq = self.status_seq;
        // Created lazily: the timer needs the runtime that runs the task.
        Task::perform(
            async { tokio::time::sleep(STATUS_FLASH).await },
            move |()| Message::StatusExpired(seq),
        )
    }

    fn open_settings(&mut self) -> Task<Message> {
        self.context_menu = None;
        self.settings_page = Some(settings::Page::new(&self.settings));
        iced::widget::operation::focus(crate::ui::settings::AUTO_FETCH_INPUT)
    }

    /// Re-applies the stored offset when a different graph or diff became visible.
    fn restore_scroll(&mut self) -> Task<Message> {
        let shown = self.active_tab().and_then(|tab| {
            let view = tab.view()?;
            let (is_diff, id, offset) = match &view.center {
                CenterView::Graph => (false, &view.graph_scroll_id, view.scroll.offset),
                CenterView::Diff(diff) => (true, &diff.scroll_id, diff.scroll.offset),
                CenterView::Conflict(_) | CenterView::Rebase(_) => return None,
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
            Message::ShortcutFilterChanged(filter) => {
                if let Some(list) = &mut self.shortcut_list {
                    *list = filter;
                }
            }
            Message::ShortcutListClosed => self.shortcut_list = None,
            Message::SettingsLoaded(loaded) => {
                let settings::Loaded {
                    settings,
                    notice,
                    write,
                } = loaded;
                if let Some(notice) = notice {
                    self.status = notice;
                }
                self.settings = settings;
                if let Some(page) = &mut self.settings_page {
                    *page = settings::Page::new(&self.settings).with_category(page.category);
                }
                let save = if write {
                    self.save_settings()
                } else {
                    Task::none()
                };
                return Task::batch([save, self.restart_auto_fetch()]);
            }
            Message::SettingsOpened => return self.open_settings(),
            Message::SystemTheme(mode) => self.system_theme = mode,
            Message::StatusExpired(seq) => {
                if seq == self.status_seq {
                    self.status.clear();
                }
            }
            Message::ZoomTo(percent) => return self.set_zoom(percent),
            Message::ThemeChosen(choice) => {
                if self.settings.appearance.theme != choice {
                    self.settings.appearance.theme = choice;
                    return self.save_settings();
                }
            }
            Message::SettingsClosed => self.settings_page = None,
            Message::SettingsCategory(category) => {
                if let Some(page) = &mut self.settings_page {
                    page.category = category;
                }
            }
            Message::AutoFetchInput(input) => {
                let Some(page) = &mut self.settings_page else {
                    return Task::none();
                };
                page.error = None;
                match settings::parse_auto_fetch(&input) {
                    Ok(minutes) => {
                        page.auto_fetch_input = input;
                        if minutes != self.settings.general.auto_fetch_minutes {
                            self.settings.general.auto_fetch_minutes = minutes;
                            return Task::batch([self.save_settings(), self.restart_auto_fetch()]);
                        }
                    }
                    Err(error) => {
                        page.auto_fetch_input = input;
                        page.error = Some(error);
                    }
                }
            }
            Message::SettingsSaveDue(seq) => {
                if seq != self.settings_save_seq {
                    return Task::none();
                }
                let Some(path) = self.settings_path.clone() else {
                    return Task::none();
                };
                let json = settings::to_json(&self.settings);
                return Task::perform(
                    blocking(move || settings::save(&path, &json).map_err(|e| e.to_string())),
                    Message::SettingsSaved,
                );
            }
            Message::SettingsSaved(result) => {
                if let Err(error) = result {
                    self.status = format!("Could not save the settings: {error}");
                }
            }
            Message::EscapeCaptured => {
                if self.shortcut_list.take().is_none() {
                    self.settings_page = None;
                }
            }
            Message::FileContextMenu {
                file,
                area,
                position,
            } => {
                let writable = self.write_block().is_none();
                let write_item =
                    |label: &str, shortcut: Option<&str>, message: Message| MenuEntry::Item {
                        hint: None,
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
            Message::WindowResized(size) => {
                self.window_size = size;
                self.window_scale = self.scale_factor();
            }
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
                let mut queued = None;
                // `Some(show_notice)` for an auto-fetch: its errors appear once
                // in the status bar (until a fetch works again), never as a dialog.
                let mut background = None;
                let mut pull = false;
                let mut push = false;
                let mut blocked_checkout = None;
                let mut blocked_merge = None;
                let mut inline_error = false;
                let mut not_merged = None;
                let mut then_network = None;
                if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
                    view.busy = None;
                    if let Some(op) = view.network.take() {
                        // Network operations are not undoable; what was undone
                        // before them cannot be redone safely.
                        if !op.background && result.is_ok() {
                            view.redo.clear();
                        }
                        if op.background {
                            background = Some(result.is_err() && !view.fetch_error_shown);
                            view.fetch_error_shown = result.is_err();
                        }
                        pull = op.kind == NetworkKind::Pull;
                        push = op.kind == NetworkKind::Push;
                    }
                    queued = view.queued.take();
                    // A rename: close the input, or show Git's error there.
                    if let Some(rename) = view.rename.as_mut().filter(|r| r.submitted) {
                        rename.submitted = false;
                        match &result {
                            Ok(_) => view.rename = None,
                            Err(error) => {
                                rename.error = Some(error.clone());
                                inline_error = true;
                            }
                        }
                    }
                    // A plain delete refused because the branch is not merged.
                    if let (Some(name), Err(error)) = (view.pending_delete.take(), &result) {
                        if git_core::branch_not_merged(error) {
                            not_merged = Some(name);
                        }
                    }
                    then_network = view.then_network.take().filter(|_| result.is_ok());
                    // A new branch: close the input, or show Git's error there.
                    if let Some(input) = view.new_branch.as_mut().filter(|i| i.submitted) {
                        input.submitted = false;
                        match &result {
                            Ok(_) => view.new_branch = None,
                            Err(error) => {
                                input.error = Some(error.clone());
                                inline_error = true;
                            }
                        }
                    }
                    let checkout = view.pending_checkout.take();
                    let merge = view.pending_merge.take();
                    if let Err(error) = &result {
                        if git_core::checkout_blocked(error) {
                            blocked_checkout = checkout;
                        }
                        let blocked = match &merge {
                            Some(
                                Message::Rebase { .. }
                                | Message::RunInteractiveRebase { .. }
                                | Message::RewordCommit { .. }
                                | Message::SquashCommits { .. },
                            ) => git_core::rebase_blocked(error),
                            _ => git_core::merge_blocked(error),
                        };
                        if blocked {
                            blocked_merge = merge;
                        }
                    }
                    match &result {
                        Ok(Some(History::Push(action))) => {
                            view.undo.push(action.clone());
                            view.redo.clear();
                        }
                        Ok(Some(History::Undone(action))) => view.redo.push(action.clone()),
                        Ok(Some(History::Redone(action))) => view.undo.push(action.clone()),
                        Ok(Some(History::Started(action))) => {
                            view.pending_history = Some(action.clone());
                        }
                        Ok(None) | Err(_) => {}
                    }
                    // A completed or aborted operation has no entry aside.
                    let completed =
                        matches!(result, Ok(Some(History::Push(UndoAction::History(_)))));
                    let aborted = result.is_ok() && label.starts_with("Abort ");
                    if completed || aborted {
                        view.pending_history = None;
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
                match result {
                    // Cancelled by the user or by a queued operation: no dialog.
                    Err(error) if error == git_core::Error::Cancelled.to_string() => {
                        if queued.is_none() {
                            self.status = format!("{label}: Cancelled");
                        }
                    }
                    Err(_) if inline_error => {}
                    Err(error) if not_merged.is_some() => {
                        if let Some(name) = not_merged {
                            self.modal = Some(not_merged_dialog(error, name));
                        }
                    }
                    Err(error) if blocked_checkout.is_some() => {
                        if let Some(target) = blocked_checkout {
                            self.modal = Some(checkout_blocked_dialog(error, target));
                        }
                    }
                    Err(error) if blocked_merge.is_some() => {
                        if let Some(request) = blocked_merge {
                            self.modal = merge_blocked_dialog(error, request);
                        }
                    }
                    Err(error) if push && git_core::push_rejection(&error).is_some() => {
                        self.modal = Some(push_rejected_dialog(error));
                    }
                    // Decided after the refresh: conflict dialog or error dialog.
                    Err(error) if pull => {
                        if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
                            view.pull_failure = Some(error);
                        }
                    }
                    Err(error) if background.is_some() => {
                        if background == Some(true) {
                            let reason = error.lines().last().unwrap_or_default();
                            self.status = format!("{label} failed: {reason}");
                        }
                    }
                    Err(error) => {
                        self.modal = Some(match git_core::locked_file(&error) {
                            Some(lock) => Modal::locked(&label, lock, Message::RetryWrite(tab)),
                            None => match auth_hint(&error) {
                                Some(hint) => {
                                    Modal::error(format!("{label} failed"), error).with_text(hint)
                                }
                                None => Modal::error(format!("{label} failed"), error),
                            },
                        });
                    }
                    Ok(_) => {}
                }
                let refresh = self.refresh(tab);
                if let Some(request) = then_network {
                    return Task::batch([refresh, self.run_network(tab, request)]);
                }
                let next = match queued {
                    Some(Queued::Write(write)) => {
                        let clears = write.clears_message;
                        let task = self.start_write(tab, write.label, write.op);
                        if let Some(view) = self.tab_mut(tab).and_then(Tab::view_mut) {
                            view.clear_message_after_write = clears && view.busy.is_some();
                        }
                        task
                    }
                    Some(Queued::Network(request)) => self.run_network(tab, request),
                    None => Task::none(),
                };
                return Task::batch([refresh, next]);
            }
            Message::NetworkProgress { tab, progress } => {
                if let Some(op) = self
                    .tab_mut(tab)
                    .and_then(Tab::view_mut)
                    .and_then(|v| v.network.as_mut())
                {
                    op.progress = Some(progress);
                }
            }
            Message::FetchAll => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.fetch(tab, false);
                }
            }
            Message::PullMenu(position) => {
                let writable = self.write_block().is_none();
                let mut entries = vec![
                    MenuEntry::Item {
                        hint: None,
                        label: "Fetch all".into(),
                        shortcut: Shortcut::FetchAll.shortcut_text(),
                        enabled: writable,
                        message: Box::new(Message::FetchAll),
                    },
                    MenuEntry::Separator,
                ];
                for mode in git_core::PullMode::ALL {
                    let mark = if mode == self.pull_mode { "✓ " } else { "" };
                    entries.push(MenuEntry::Item {
                        hint: None,
                        label: format!("{mark}{}", mode.label()),
                        shortcut: None,
                        enabled: writable,
                        message: Box::new(Message::Pull(Some(mode))),
                    });
                }
                self.context_menu = Some(ContextMenu::new(position, entries));
            }
            Message::Pull(mode) => {
                if let Some(mode) = mode {
                    self.pull_mode = mode;
                }
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.pull(tab);
                }
            }
            // Leaving a detached HEAD first checks for commits on no branch.
            Message::Checkout(target) => {
                let Some(view) = self.active_tab().and_then(Tab::view) else {
                    return Task::none();
                };
                let tab = self.tabs[self.active].id;
                if !matches!(view.snapshot.head, git_core::Head::Detached { .. }) {
                    return self.update(Message::CheckoutChecked(target));
                }
                let writer = Arc::clone(&view.writer);
                return Task::perform(
                    blocking(move || {
                        git_core::unreachable_from_branches(writer.as_ref())
                            .map_err(|e| e.to_string())
                    }),
                    move |result| Message::DetachedChecked {
                        tab,
                        target: target.clone(),
                        result,
                    },
                );
            }
            Message::DetachedChecked {
                tab,
                target,
                result,
            } => {
                let head = self
                    .tab_mut(tab)
                    .and_then(|t| t.view())
                    .and_then(|v| v.snapshot.head.target().map(str::to_owned));
                match (result, head) {
                    (Ok(orphans), _) if orphans.is_empty() => {
                        return self.update(Message::CheckoutChecked(target))
                    }
                    (Ok(orphans), Some(head)) => {
                        self.modal = Some(leave_detached_dialog(orphans.len(), head, target));
                    }
                    (Ok(_), None) => return self.update(Message::CheckoutChecked(target)),
                    (Err(error), _) => {
                        self.modal = Some(Modal::error("Checkout failed", error));
                    }
                }
            }
            Message::CheckoutChecked(CheckoutTarget::Commit(id)) => {
                let short = id[..id.len().min(7)].to_owned();
                self.modal = Some(Modal::confirm(
                    "Checkout commit",
                    format!(
                        "Check out {short}? HEAD will be detached: new commits do not belong to \
                         any branch until you create one."
                    ),
                    "Checkout",
                    false,
                    Message::CheckoutConfirmed(CheckoutTarget::Commit(id)),
                ));
            }
            Message::CheckoutChecked(target) | Message::CheckoutConfirmed(target) => {
                return self.checkout(target, false)
            }
            Message::StashAndCheckout(target) => return self.checkout(target, true),
            Message::CommitContextMenu { id, position } => {
                let writable = self.write_block().is_none();
                let message = self
                    .active_tab()
                    .and_then(Tab::view)
                    .and_then(|v| v.snapshot.commit(&id))
                    .map(full_message)
                    .unwrap_or_default();
                let head = self
                    .active_tab()
                    .and_then(Tab::view)
                    .map_or_else(|| "HEAD".to_owned(), |v| head_name(&v.snapshot).to_owned());
                let facts = self
                    .active_tab()
                    .and_then(Tab::view)
                    .map(|view| {
                        let snapshot = &view.snapshot;
                        let clean = snapshot.state.is_clean();
                        crate::menus::CommitFacts {
                            rebase_from: clean
                                && snapshot.head.target().is_some_and(|head| {
                                    head != id && snapshot.is_ancestor(&id, head) == Some(true)
                                }),
                            rewordable: clean && crate::repo_view::rewordable(snapshot, &id),
                            pickable: clean
                                && snapshot.head.target().is_some_and(|head| head != id),
                            revertable: clean && snapshot.head.target().is_some(),
                            resettable: clean
                                && snapshot.head.target().is_some_and(|head| head != id),
                        }
                    })
                    .unwrap_or_default();
                self.context_menu = Some(ContextMenu::new(
                    position,
                    crate::menus::commit(&id, &message, writable, facts, &head),
                ));
            }
            Message::BranchContextMenu {
                name,
                kind,
                position,
            } => {
                let writable = self.write_block().is_none();
                let Some(view) = self.active_tab().and_then(Tab::view) else {
                    return Task::none();
                };
                let entries = match kind {
                    git_core::RefKind::LocalBranch => {
                        let local = view
                            .snapshot
                            .refs
                            .iter()
                            .find(|r| r.kind == kind && r.name == name);
                        let target = local.map_or("", |r| r.target.as_str());
                        crate::menus::local_branch(crate::menus::LocalBranch {
                            name: &name,
                            target,
                            is_head: view.snapshot.head.branch() == Some(name.as_str()),
                            has_upstream: local.is_some_and(|r| r.upstream.is_some()),
                            writable,
                            head: head_name(&view.snapshot),
                            head_can_fast_forward: view.snapshot.head_can_fast_forward(target),
                        })
                    }
                    git_core::RefKind::RemoteBranch => {
                        let snapshot = &view.snapshot;
                        let target = snapshot
                            .refs
                            .iter()
                            .find(|r| r.kind == kind && r.name == name)
                            .map_or("", |r| r.target.as_str());
                        crate::menus::remote_branch(crate::menus::RemoteBranch {
                            name: &name,
                            writable,
                            head: head_name(snapshot),
                            head_can_fast_forward: snapshot.head_can_fast_forward(target),
                            behind_local: behind_local(snapshot, &name, target),
                        })
                    }
                    git_core::RefKind::Tag => crate::menus::tag(&name, writable),
                };
                self.context_menu = Some(ContextMenu::new(position, entries));
            }
            Message::RenameStart(old) => {
                if let Some(view) = self.active_view_mut() {
                    view.rename = Some(crate::repo_view::RenameBranch {
                        name: old.clone(),
                        old,
                        error: None,
                        submitted: false,
                    });
                    return iced::widget::operation::focus(crate::ui::RENAME_INPUT);
                }
            }
            Message::RenameInput(name) => {
                if let Some(view) = self.active_view_mut() {
                    let names = local_branch_names(&view.snapshot);
                    if let Some(rename) = view.rename.as_mut() {
                        rename.error = rename_problem(&rename.old, &name, &names);
                        rename.name = name;
                    }
                }
            }
            Message::RenameSubmit => return self.submit_rename(),
            Message::RenameConfirmed { remote } => return self.rename_branch(remote),
            Message::DeleteBranch(name) => {
                self.modal = Some(Modal::confirm(
                    "Delete branch",
                    format!("Delete the local branch {name}?"),
                    "Delete",
                    true,
                    Message::DeleteBranchConfirmed { name, force: false },
                ));
            }
            Message::DeleteBranchConfirmed { name, force } => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let branch = name.clone();
                let task = self.run_undoable(tab, format!("Delete branch {name}"), move |git| {
                    let target = git_core::ref_commit(git, &format!("refs/heads/{branch}"))?;
                    let upstream = git_core::branch_upstream(git, &branch)?;
                    git.delete_branch(&branch, force)?;
                    Ok(target.map(|target| {
                        History::Push(UndoAction::Ref(git_core::RefChange::DeleteBranch {
                            name: branch.clone(),
                            target,
                            upstream,
                        }))
                    }))
                });
                if let Some(view) = self.active_view_mut() {
                    if view.busy.is_some() && !force {
                        view.pending_delete = Some(name);
                    }
                }
                return task;
            }
            Message::DeleteRemoteBranch(full) => {
                self.modal = Some(Modal::confirm(
                    "Delete remote branch",
                    format!("Delete {full} on the remote? Others may still use it."),
                    "Delete",
                    true,
                    Message::DeleteRemoteBranchConfirmed(full),
                ));
            }
            Message::DeleteRemoteBranchConfirmed(full) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let Some((remote, branch)) = full.split_once('/') else {
                    return Task::none();
                };
                let request = NetworkRequest {
                    label: format!("Delete {full}"),
                    kind: NetworkKind::Push,
                    args: git_core::delete_remote_branch_args(remote, branch),
                    background: false,
                };
                return self.run_network(tab, request);
            }
            Message::SetUpstreamStart(local) => return self.set_upstream_form(local),
            Message::UnsetUpstream(local) => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    let branch = local.clone();
                    return self.run_write(tab, format!("Unset upstream of {local}"), move |git| {
                        git.unset_upstream(&branch)
                    });
                }
            }
            Message::AddRemoteStart => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                self.form = Some(Form {
                    title: "Add remote".into(),
                    text: "Nothing is fetched yet; use Fetch afterwards.".into(),
                    fields: vec![FormField::text("Name", ""), FormField::text("URL", "")],
                    submit: "Add remote".into(),
                    action: FormAction::AddRemote { tab },
                    error: None,
                });
                return iced::widget::operation::focus(crate::ui::overlays::FORM_FIRST_INPUT);
            }
            Message::RemoteContextMenu { name, position } => {
                let writable = self.write_block().is_none();
                self.context_menu = Some(ContextMenu::new(
                    position,
                    crate::menus::remote(&name, writable),
                ));
            }
            Message::FetchRemote(name) => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    let request = NetworkRequest {
                        label: format!("Fetch {name}"),
                        kind: NetworkKind::Fetch,
                        args: git_core::fetch_remote_args(&name),
                        background: false,
                    };
                    return self.run_network(tab, request);
                }
            }
            Message::EditRemoteUrl(name) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let url = self
                    .active_tab()
                    .and_then(Tab::view)
                    .and_then(|v| {
                        v.snapshot
                            .remote_urls
                            .iter()
                            .find(|(n, _)| *n == name)
                            .map(|(_, u)| u.clone())
                    })
                    .unwrap_or_default();
                self.form = Some(Form {
                    title: format!("Edit URL of {name}"),
                    text: String::new(),
                    fields: vec![FormField::text("URL", url)],
                    submit: "Save".into(),
                    action: FormAction::SetRemoteUrl { tab, name },
                    error: None,
                });
                return iced::widget::operation::focus(crate::ui::overlays::FORM_FIRST_INPUT);
            }
            Message::RenameRemoteStart(old) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                self.form = Some(Form {
                    title: format!("Rename remote {old}"),
                    text: "Remote-tracking branches and upstreams follow the new name.".into(),
                    fields: vec![FormField::text("Name", old.clone())],
                    submit: "Rename".into(),
                    action: FormAction::RenameRemote { tab, old },
                    error: None,
                });
                return iced::widget::operation::focus(crate::ui::overlays::FORM_FIRST_INPUT);
            }
            Message::RemoveRemote(name) => {
                self.modal = Some(Modal::confirm(
                    "Remove remote",
                    format!(
                        "Remove the remote {name}? Its remote-tracking branches are removed \
                         too; nothing changes on the server."
                    ),
                    "Remove",
                    true,
                    Message::RemoveRemoteConfirmed(name),
                ));
            }
            Message::RemoveRemoteConfirmed(name) => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    let remote = name.clone();
                    return self.run_write(tab, format!("Remove remote {name}"), move |git| {
                        git.remove_remote(&remote)
                    });
                }
            }
            Message::CreateTagStart(target) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                self.form = Some(Form {
                    title: format!("Create tag at {}", &target[..target.len().min(7)]),
                    text: String::new(),
                    fields: vec![
                        FormField::text("Tag name", ""),
                        FormField::text("Message (optional; makes an annotated tag)", ""),
                    ],
                    submit: "Create tag".into(),
                    action: FormAction::CreateTag { tab, target },
                    error: None,
                });
                return iced::widget::operation::focus(crate::ui::overlays::FORM_FIRST_INPUT);
            }
            Message::PushTag(tag) => return self.tag_remote_action(tag, false),
            Message::DeleteRemoteTag(tag) => return self.tag_remote_action(tag, true),
            Message::DeleteTag(tag) => {
                self.modal = Some(Modal::confirm(
                    "Delete tag",
                    format!("Delete the local tag {tag}? Tags on remotes stay."),
                    "Delete",
                    true,
                    Message::DeleteTagConfirmed(tag),
                ));
            }
            Message::DeleteTagConfirmed(tag) => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    let name = tag.clone();
                    return self.run_undoable(tab, format!("Delete tag {tag}"), move |git| {
                        let target = git_core::ref_commit(git, &format!("refs/tags/{name}"))?;
                        let message = git_core::tag_message(git, &name)?;
                        git.delete_tag(&name)?;
                        Ok(target.map(|target| {
                            History::Push(UndoAction::Ref(git_core::RefChange::DeleteTag {
                                name: name.clone(),
                                target,
                                message,
                            }))
                        }))
                    });
                }
            }
            Message::DeleteRemoteTagConfirmed { remote, tag } => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.run_network(tab, delete_remote_tag_request(&remote, &tag));
                }
            }
            Message::NewBranchStart(at) => return self.start_new_branch(at),
            Message::NewBranchInput(name) => {
                if let Some(view) = self.active_view_mut() {
                    let names: Vec<String> = local_branch_names(&view.snapshot);
                    if let Some(input) = view.new_branch.as_mut() {
                        // Instant feedback while typing; empty input shows no error yet.
                        input.error = (!name.is_empty())
                            .then(|| branch_name_problem(&name, &names))
                            .flatten();
                        input.name = name;
                    }
                }
            }
            Message::NewBranchSubmit => return self.create_branch(),
            Message::PullBranch(local) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let Some((remote, branch)) = self.branch_upstream(&local) else {
                    return Task::none();
                };
                let request = NetworkRequest {
                    label: format!("Pull {local}"),
                    kind: NetworkKind::Fetch,
                    args: git_core::fast_forward_args(&remote, &branch, &local),
                    background: false,
                };
                return self.run_network(tab, request);
            }
            Message::PushBranch(local) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                match self.branch_upstream(&local) {
                    Some((remote, branch)) => {
                        let request = NetworkRequest {
                            label: format!("Push {local}"),
                            kind: NetworkKind::PushBranch,
                            args: git_core::push_args(&remote, &local, &branch, false, false),
                            background: false,
                        };
                        return self.run_network(tab, request);
                    }
                    None => return self.push_setup_form(tab, local),
                }
            }
            Message::Push { force } => {
                if let Some(tab) = self.active_tab().map(|t| t.id) {
                    return self.push(tab, force);
                }
            }
            Message::ForcePushRequested => {
                self.modal = Some(Modal::confirm(
                    "Force push",
                    "Overwrite the remote branch with your local branch? Commits on the remote \
                     that you do not have are lost. The push is refused if the remote changed \
                     since your last fetch (--force-with-lease).",
                    "Force push",
                    true,
                    Message::Push { force: true },
                ));
            }
            Message::FormInput { field, value } => {
                if let Some(field) = self.form.as_mut().and_then(|f| f.fields.get_mut(field)) {
                    field.value = value;
                }
            }
            Message::FormClosed => self.form = None,
            Message::FormSubmit => return self.submit_form(),
            Message::Rebase {
                onto,
                autostash,
                confirmed: false,
            } => {
                let merges = self.active_tab().and_then(Tab::view).map_or(0, |view| {
                    let snapshot = &view.snapshot;
                    let onto_commit = git_core_ref_target(snapshot, &onto);
                    match (snapshot.head.target(), onto_commit) {
                        (Some(head), Some(onto)) => snapshot.merges_between(onto, head),
                        _ => 0,
                    }
                });
                let confirmed = Message::Rebase {
                    onto: onto.clone(),
                    autostash,
                    confirmed: true,
                };
                if merges == 0 {
                    return self.merge(confirmed);
                }
                let head = self
                    .active_tab()
                    .and_then(Tab::view)
                    .map_or_else(|| "HEAD".to_owned(), |v| head_name(&v.snapshot).to_owned());
                let target = onto
                    .strip_prefix("refs/heads/")
                    .or_else(|| onto.strip_prefix("refs/remotes/"))
                    .unwrap_or(&onto);
                self.modal = Some(Modal::confirm(
                    "Rebase flattens merges",
                    format!(
                        "{head} contains {merges} merge commit{} since {target}. A rebase replays \
                         the changes of the merged commits one by one and drops the merge \
                         commits (Git's default). Rebase anyway?",
                        if merges == 1 { "" } else { "s" }
                    ),
                    "Rebase",
                    false,
                    confirmed,
                ));
            }
            Message::Merge { .. } | Message::FastForward { .. } | Message::Rebase { .. } => {
                return self.merge(message);
            }
            Message::ContinueOperation => return self.commit(false),
            Message::SkipOperation => return self.skip_or_abort(true),
            Message::AbortOperation => return self.skip_or_abort(false),
            Message::AutoFetchDue(seq) => {
                if seq == self.auto_fetch_seq {
                    self.auto_fetch_at = None;
                    return self.auto_fetch(std::time::Instant::now());
                }
            }
            Message::CancelNetwork(tab) => {
                if let Some(op) = self
                    .tab_mut(tab)
                    .and_then(Tab::view_mut)
                    .and_then(|v| v.network.as_ref())
                {
                    op.cancel.cancel();
                }
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
            // With staged changes, ask whether they go as well (maintainer decision).
            Message::Discard(DiscardTarget::All)
                if self
                    .active_tab()
                    .and_then(Tab::view)
                    .is_some_and(|v| !v.snapshot.working_tree.staged.is_empty()) =>
            {
                self.modal = Some(Modal::discard_all_with_staged(
                    Message::DiscardConfirmed(DiscardTarget::All),
                    Message::DiscardConfirmed(DiscardTarget::Everything),
                ));
            }
            Message::Discard(target) => {
                let (title, text) = match &target {
                    DiscardTarget::File(file) => (
                        "Discard changes".to_string(),
                        format!(
                            "Discard all changes to {}? This cannot be undone outside Gitloom.",
                            file.path
                        ),
                    ),
                    DiscardTarget::All => (
                        "Discard all changes".to_string(),
                        "Discard all unstaged changes and delete all untracked files?".to_string(),
                    ),
                    DiscardTarget::Everything => (
                        "Discard all changes".to_string(),
                        "Discard all staged and unstaged changes and delete all untracked files?"
                            .to_string(),
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
                let restorable = self
                    .active_tab()
                    .and_then(Tab::view)
                    .is_some_and(|v| v.state_block(Guarded::Stash).is_none());
                self.context_menu = Some(ContextMenu::new(
                    position,
                    crate::menus::stash(&id, writable, restorable),
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
            Message::WipSelected => {
                if let Some(view) = self.active_view_mut() {
                    view.multi.clear();
                }
                return self.select(Selection::Wip);
            }
            Message::CommitSelected(id) => {
                let (toggle, range) = (self.modifiers.command(), self.modifiers.shift());
                if let Some(view) = self.active_view_mut() {
                    select_multi(view, &id, toggle, range);
                }
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
            Message::RewordStart(id) => {
                let message = self
                    .active_tab()
                    .and_then(Tab::view)
                    .and_then(|v| v.snapshot.commit(&id))
                    .map(full_message);
                if let Some(message) = message {
                    self.message_dialog = Some(crate::overlay::MessageDialog::new(
                        format!("Edit message of {}", &id[..id.len().min(7)]),
                        "Save message",
                        &message,
                        crate::overlay::MessageAction::Reword { id },
                    ));
                }
            }
            Message::SquashSelected => {
                let Some(view) = self.active_tab().and_then(Tab::view) else {
                    return Task::none();
                };
                let ids = view.multi.ids().to_vec();
                if !crate::repo_view::squashable(&view.snapshot, &ids) {
                    self.status = "Select commits that follow each other on the current \
                                   branch, without merges"
                        .into();
                    return Task::none();
                }
                // Oldest first, like Git's combined squash message.
                let message = ids
                    .iter()
                    .rev()
                    .filter_map(|id| view.snapshot.commit(id))
                    .map(|c| full_message(c).trim().to_owned())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                self.message_dialog = Some(crate::overlay::MessageDialog::new(
                    format!("Squash {} commits", ids.len()),
                    "Squash",
                    &message,
                    crate::overlay::MessageAction::Squash { ids },
                ));
            }
            Message::CommitMessageEdited(action) => {
                if let Some(dialog) = self.message_dialog.as_mut() {
                    dialog.editor.perform(action);
                }
            }
            Message::CommitMessageClosed => self.message_dialog = None,
            Message::CommitMessageSubmit => {
                let Some(dialog) = self.message_dialog.as_mut() else {
                    return Task::none();
                };
                let message = dialog.editor.text();
                if message.trim().is_empty() {
                    dialog.error = Some("Enter a commit message".into());
                    return Task::none();
                }
                let message = format!("{}\n", message.trim_end());
                let request = match dialog.action.clone() {
                    crate::overlay::MessageAction::Reword { id } => Message::RewordCommit {
                        id,
                        message,
                        autostash: false,
                    },
                    crate::overlay::MessageAction::Squash { ids } => Message::SquashCommits {
                        ids,
                        message,
                        autostash: false,
                    },
                };
                self.message_dialog = None;
                return self.merge(request);
            }
            Message::RewordCommit { .. } | Message::SquashCommits { .. } => {
                return self.merge(message);
            }
            Message::CherryPick(_) | Message::Revert { .. } | Message::CommitEmpty => {
                return self.merge(message);
            }
            Message::BranchDragStart { name, kind } => {
                if let Some(view) = self.active_view_mut() {
                    view.drag = Some(crate::repo_view::BranchDrag {
                        source: (name, kind),
                        target: None,
                    });
                }
            }
            Message::BranchDragEnter { name, kind } => {
                if let Some(drag) = self.active_view_mut().and_then(|v| v.drag.as_mut()) {
                    drag.target = (drag.source != (name.clone(), kind)).then_some((name, kind));
                }
            }
            Message::BranchDragLeave { name, kind } => {
                if let Some(drag) = self.active_view_mut().and_then(|v| v.drag.as_mut()) {
                    if drag.targets(&name, kind) {
                        drag.target = None;
                    }
                }
            }
            Message::BranchDropped {
                name,
                kind,
                position,
            } => {
                let writable = self.write_block().is_none();
                let Some(view) = self.active_view_mut() else {
                    return Task::none();
                };
                let Some(drag) = view.drag.take() else {
                    return Task::none();
                };
                if drag.source == (name.clone(), kind) || kind == git_core::RefKind::Tag {
                    return Task::none();
                }
                let entries = drop_entries(&view.snapshot, &drag.source, (&name, kind), writable);
                self.context_menu = Some(ContextMenu::new(position, entries));
            }
            Message::BranchDragEnd => {
                if let Some(view) = self.active_view_mut() {
                    if view.drag.is_some() {
                        view.drag = None;
                    }
                }
            }
            Message::ResetStart(id) => {
                if self.state_blocked(Guarded::Reset) {
                    return Task::none();
                }
                if let Some(view) = self.active_tab().and_then(Tab::view) {
                    self.modal = Some(reset_dialog(&view.snapshot, &id));
                }
            }
            Message::Reset {
                id,
                mode: git_core::ResetMode::Hard,
                confirmed: false,
            } => {
                if let Some(view) = self.active_tab().and_then(Tab::view) {
                    self.modal = Some(hard_reset_dialog(&view.snapshot, &id));
                }
            }
            Message::Reset { .. } => return self.merge(message),
            Message::RevertStart(id) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let Some(view) = self.active_tab().and_then(Tab::view) else {
                    return Task::none();
                };
                let Some(commit) = view.snapshot.commit(&id) else {
                    return Task::none();
                };
                if !commit.is_merge() {
                    return self.merge(Message::Revert { id, mainline: None });
                }
                let options: Vec<String> = commit
                    .parents
                    .iter()
                    .enumerate()
                    .map(|(i, parent)| {
                        let summary = view
                            .snapshot
                            .commit(parent)
                            .map_or_else(String::new, |c| c.summary.clone());
                        format!("{}: {} {summary}", i + 1, &parent[..parent.len().min(7)])
                    })
                    .collect();
                let first = options.first().cloned().unwrap_or_default();
                self.form = Some(Form {
                    title: format!("Revert merge {}", commit.short_id()),
                    text: "A merge commit is reverted against one of its parents: the \
                           changes brought in from the other parents are undone. Usually \
                           this is parent 1, the branch that was merged into."
                        .into(),
                    fields: vec![crate::overlay::FormField::choice("Parent", first, options)],
                    submit: "Revert".into(),
                    action: FormAction::RevertMerge { tab, id },
                    error: None,
                });
            }
            Message::RebasePanelOpened(base) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                if self.state_blocked(Guarded::Merge) {
                    return Task::none();
                }
                let Some(view) = self.active_view_mut() else {
                    return Task::none();
                };
                let label = view.snapshot.commit(&base).map_or_else(
                    || base[..base.len().min(7)].to_owned(),
                    |c| format!("{} {}", c.short_id(), c.summary),
                );
                let mut panel = crate::rebase_panel::RebasePanel::new(base.clone(), label);
                panel.flattened_merges = view
                    .snapshot
                    .head
                    .target()
                    .map_or(0, |head| view.snapshot.merges_between(&base, head));
                view.center = CenterView::Rebase(Box::new(panel));
                let writer = Arc::clone(&view.writer);
                let id = base.clone();
                return Task::perform(
                    blocking(move || {
                        git_core::rebase_commits(&*writer, &id).map_err(|e| e.to_string())
                    }),
                    move |result| Message::RebaseLoaded {
                        tab,
                        base: base.clone(),
                        result,
                    },
                );
            }
            Message::RebaseLoaded { tab, base, result } => {
                if let Some(CenterView::Rebase(panel)) = self
                    .tab_mut(tab)
                    .and_then(Tab::view_mut)
                    .map(|v| &mut v.center)
                {
                    if panel.base == base {
                        panel.set_loaded(result);
                    }
                }
            }
            Message::RebaseSetAction { index, action } => {
                if let Some(panel) = self.active_rebase_panel() {
                    panel.set_action(index, action);
                }
            }
            Message::RebaseDragStart(index) => {
                if let Some(panel) = self.active_rebase_panel() {
                    panel.drag_start(index);
                }
            }
            Message::RebaseDragOver(index) => {
                if let Some(panel) = self.active_rebase_panel() {
                    panel.drag_over(index);
                }
            }
            Message::RebaseDragEnd => {
                if let Some(panel) = self.active_rebase_panel() {
                    panel.drag_end();
                }
            }
            Message::RebaseMessageEdited { id, action } => {
                if let Some(panel) = self.active_rebase_panel() {
                    panel.edit_message(&id, action);
                }
            }
            Message::RebaseStart => {
                let Some(panel) = self.active_rebase_panel() else {
                    return Task::none();
                };
                match panel.build() {
                    Ok(input) => {
                        let base = panel.base.clone();
                        return self.update(Message::RunInteractiveRebase {
                            base,
                            input,
                            autostash: false,
                        });
                    }
                    Err(problem) => self.status = problem,
                }
            }
            Message::RunInteractiveRebase { .. } => return self.merge(message),
            Message::ConflictOpened(path) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let Some(view) = self.active_view_mut() else {
                    return Task::none();
                };
                let labels = conflict_labels(&view.snapshot);
                view.center = CenterView::Conflict(Box::new(crate::conflict::ConflictView::new(
                    path.clone(),
                    labels,
                )));
                let writer = Arc::clone(&view.writer);
                return Task::perform(
                    blocking(move || {
                        git_core::load_conflict(&*writer, &path)
                            .map(|loaded| (path, loaded))
                            .map_err(|e| e.to_string())
                    }),
                    move |result| match result {
                        Ok((path, loaded)) => Message::ConflictLoaded {
                            tab,
                            path,
                            result: Ok(loaded),
                        },
                        Err(error) => Message::ConflictLoaded {
                            tab,
                            path: String::new(),
                            result: Err(error),
                        },
                    },
                );
            }
            Message::ConflictLoaded { tab, path, result } => {
                if let Some(CenterView::Conflict(conflict)) = self
                    .tab_mut(tab)
                    .and_then(Tab::view_mut)
                    .map(|v| &mut v.center)
                {
                    // A failed load has no path; it belongs to the open view.
                    if path.is_empty() || conflict.path == path {
                        conflict.set_loaded(result);
                    }
                }
            }
            Message::ConflictToggleLine { hunk, side, line } => {
                if let Some(conflict) = self.active_conflict() {
                    conflict.toggle_line(hunk, side, line);
                }
            }
            Message::ConflictToggleHunk { hunk, side } => {
                if let Some(conflict) = self.active_conflict() {
                    conflict.toggle_hunk(hunk, side);
                }
            }
            Message::ConflictTakeAll(side) => {
                if let Some(conflict) = self.active_conflict() {
                    conflict.take_all(side);
                }
            }
            Message::ConflictOutputEdited(action) => {
                if let Some(conflict) = self.active_conflict() {
                    conflict.edit(action);
                }
            }
            Message::ConflictResetOutput => {
                if let Some(conflict) = self.active_conflict() {
                    conflict.reset_output();
                }
            }
            Message::ConflictSave => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let Some(conflict) = self.active_conflict() else {
                    return Task::none();
                };
                let path = conflict.path.clone();
                match conflict.resolved_content() {
                    Ok(content) => {
                        return self.run_write(tab, format!("Resolve {path}"), move |git| {
                            git_core::resolve_conflict(git, &path, content.as_bytes())
                        });
                    }
                    Err(reason) => self.status = reason,
                }
            }
            Message::ConflictTakeFile(side) => {
                let Some(tab) = self.active_tab().map(|t| t.id) else {
                    return Task::none();
                };
                let Some(path) = self.active_conflict().map(|c| c.path.clone()) else {
                    return Task::none();
                };
                return self.run_write(tab, format!("Resolve {path}"), move |git| {
                    git_core::resolve_with_side(git, &path, side)
                });
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
                // Typing in a text field must not move the selection or stage files.
                return iced::advanced::widget::operate(AnyFocused::default()).map(
                    move |text_input_focused| Message::ShortcutChecked {
                        shortcut,
                        text_input_focused,
                    },
                );
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
                let mut pull_failure = None;
                let mut empty_pick = None;
                if let Some(view) = tab.view_mut() {
                    match result {
                        Ok(snapshot) => view.apply_snapshot(snapshot),
                        Err(error) => {
                            view.refreshing = false;
                            view.error = Some(error);
                        }
                    }
                    reload = view.reload_diff;
                    pull_failure = view
                        .pull_failure
                        .take()
                        .map(|error| (error, view.snapshot.state.clone()));
                    // The operation ended outside Gitloom: its entry is gone.
                    if view.busy.is_none() && view.snapshot.state.is_clean() {
                        view.pending_history = None;
                    }
                    if view.busy.is_none() && std::mem::take(&mut view.check_empty) {
                        empty_pick = view.empty_pick().then(|| empty_pick_dialog(&view.snapshot));
                    }
                }
                if let Some((error, state)) = pull_failure {
                    self.modal = Some(pull_failed_dialog(error, &state));
                }
                if let Some(dialog) = empty_pick.flatten() {
                    self.modal = Some(dialog);
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
                iced::system::theme_changes().map(Message::SystemTheme),
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

/// Why `name` cannot name a new remote: invalid or already used.
fn remote_name_problem(name: &str, existing: &[String]) -> Option<String> {
    if name.contains('/') {
        return Some("Remote names cannot contain \"/\"".into());
    }
    if let Some(problem) = git_core::check_branch_name(name) {
        return Some(
            problem
                .replace("Branch names", "Remote names")
                .replace("a branch name", "a remote name"),
        );
    }
    existing
        .iter()
        .any(|r| r == name)
        .then(|| format!("A remote named \"{name}\" already exists"))
}

fn push_tag_request(remote: &str, tag: &str) -> NetworkRequest {
    NetworkRequest {
        label: format!("Push tag {tag}"),
        kind: NetworkKind::Push,
        args: git_core::push_tag_args(remote, tag),
        background: false,
    }
}

fn delete_remote_tag_request(remote: &str, tag: &str) -> NetworkRequest {
    NetworkRequest {
        label: format!("Delete tag {tag} on {remote}"),
        kind: NetworkKind::Push,
        args: git_core::delete_remote_tag_args(remote, tag),
        background: false,
    }
}

fn local_branch_names(snapshot: &RepoSnapshot) -> Vec<String> {
    snapshot
        .refs
        .iter()
        .filter(|r| r.kind == git_core::RefKind::LocalBranch)
        .map(|r| r.name.clone())
        .collect()
}

/// Why `old` cannot be renamed to `name`: invalid or taken by another branch.
fn rename_problem(old: &str, name: &str, existing: &[String]) -> Option<String> {
    let others: Vec<String> = existing.iter().filter(|b| *b != old).cloned().collect();
    branch_name_problem(name.trim(), &others)
}

/// `git branch -d` refused: offer a force delete.
fn not_merged_dialog(error: String, name: String) -> Modal {
    Modal {
        title: "Branch not merged".into(),
        text: format!(
            "{name} has commits that are not merged into the current branch or its \
             upstream. Deleting it loses them (they can only be found in the reflog)."
        ),
        details: Some(error),
        buttons: vec![
            crate::overlay::DialogButton {
                label: "Cancel".into(),
                kind: crate::overlay::ButtonKind::Secondary,
                message: None,
            },
            crate::overlay::DialogButton {
                label: "Force delete".into(),
                kind: crate::overlay::ButtonKind::Danger,
                message: Some(Message::DeleteBranchConfirmed { name, force: true }),
            },
        ],
        default: Some(0),
    }
}

/// Why `name` cannot be used for a new branch: invalid or already taken.
fn branch_name_problem(name: &str, existing: &[String]) -> Option<String> {
    if let Some(problem) = git_core::check_branch_name(name) {
        return Some(problem.to_owned());
    }
    existing
        .iter()
        .any(|b| b == name)
        .then(|| format!("A branch named \"{name}\" already exists"))
}

/// Leaving a detached HEAD with commits on no branch: offer to create a
/// branch at HEAD first (GitKraken warns the same way).
fn leave_detached_dialog(count: usize, head: String, target: CheckoutTarget) -> Modal {
    let (commits, them) = if count == 1 {
        ("1 commit is".to_owned(), "it")
    } else if count >= 100 {
        ("100 or more commits are".to_owned(), "them")
    } else {
        (format!("{count} commits are"), "them")
    };
    let they = if count == 1 { "it" } else { "they" };
    Modal {
        title: "Leave detached HEAD".into(),
        text: format!(
            "{commits} not on any branch or tag. After leaving, {they} can only be found in \
             the reflog. Create a branch to keep {them}?"
        ),
        details: None,
        buttons: vec![
            crate::overlay::DialogButton {
                label: "Cancel".into(),
                kind: crate::overlay::ButtonKind::Secondary,
                message: None,
            },
            crate::overlay::DialogButton {
                label: "Leave anyway".into(),
                kind: crate::overlay::ButtonKind::Danger,
                message: Some(Message::CheckoutChecked(target)),
            },
            crate::overlay::DialogButton {
                label: "Create branch".into(),
                kind: crate::overlay::ButtonKind::Primary,
                message: Some(Message::NewBranchStart(Some(head))),
            },
        ],
        default: Some(2),
    }
}

/// Local changes would be overwritten by the checkout: offer to stash them
/// first (planning decision M3; the stash is not popped automatically).
fn checkout_blocked_dialog(error: String, target: CheckoutTarget) -> Modal {
    Modal {
        title: "Local changes block the checkout".into(),
        text: "Your local changes would be overwritten by the checkout. Stash them \
               (including untracked files) and check out? The stash stays in the list; \
               apply or pop it later."
            .into(),
        details: Some(error),
        buttons: vec![
            crate::overlay::DialogButton {
                label: "Cancel".into(),
                kind: crate::overlay::ButtonKind::Secondary,
                message: None,
            },
            crate::overlay::DialogButton {
                label: "Stash and checkout".into(),
                kind: crate::overlay::ButtonKind::Primary,
                message: Some(Message::StashAndCheckout(target)),
            },
        ],
        default: Some(1),
    }
}

/// Local changes would be overwritten by the merge: offer to stash them,
/// merge and re-apply them (`--autostash`; planning decision M4).
fn merge_blocked_dialog(error: String, request: Message) -> Option<Modal> {
    let operation = if matches!(
        request,
        Message::Rebase { .. }
            | Message::RunInteractiveRebase { .. }
            | Message::RewordCommit { .. }
            | Message::SquashCommits { .. }
    ) {
        "rebase"
    } else {
        "merge"
    };
    let retry = match request {
        Message::Merge { source, .. } => Message::Merge {
            source,
            autostash: true,
        },
        Message::Rebase { onto, .. } => Message::Rebase {
            onto,
            autostash: true,
            confirmed: true,
        },
        Message::RunInteractiveRebase { base, input, .. } => Message::RunInteractiveRebase {
            base,
            input,
            autostash: true,
        },
        Message::RewordCommit { id, message, .. } => Message::RewordCommit {
            id,
            message,
            autostash: true,
        },
        Message::SquashCommits { ids, message, .. } => Message::SquashCommits {
            ids,
            message,
            autostash: true,
        },
        Message::FastForward { branch, source, .. } => Message::FastForward {
            branch,
            source,
            autostash: true,
        },
        _ => return None,
    };
    Some(Modal {
        title: format!("Local changes block the {operation}"),
        text: format!(
            "Your local changes are in the way of the {operation}. Stash them, \
             {operation} and apply them again afterwards? If they conflict with the \
             result, they stay in the stash list."
        ),
        details: Some(error),
        buttons: vec![
            crate::overlay::DialogButton {
                label: "Cancel".into(),
                kind: crate::overlay::ButtonKind::Secondary,
                message: None,
            },
            crate::overlay::DialogButton {
                label: format!("Stash and {operation}"),
                kind: crate::overlay::ButtonKind::Primary,
                message: Some(retry),
            },
        ],
        default: Some(1),
    })
}

/// Menu for dropping branch `source` onto branch `target` (M4-11).
fn drop_entries(
    snapshot: &RepoSnapshot,
    source: &(String, git_core::RefKind),
    target: (&str, git_core::RefKind),
    writable: bool,
) -> Vec<crate::overlay::MenuEntry> {
    use git_core::RefKind;
    let commit_of = |name: &str, kind: RefKind| {
        snapshot
            .refs
            .iter()
            .find(|r| r.name == name && r.kind == kind)
            .map_or("", |r| r.target.as_str())
    };
    let source_commit = commit_of(&source.0, source.1);
    let target_commit = commit_of(target.0, target.1);
    let can_fast_forward = target.1 == RefKind::LocalBranch
        && !target_commit.is_empty()
        && source_commit != target_commit
        && snapshot.is_ancestor(target_commit, source_commit) != Some(false);
    crate::menus::drop_menu(crate::menus::DropFacts {
        source: &source.0,
        source_remote: source.1 == RefKind::RemoteBranch,
        target: target.0,
        target_remote: target.1 == RefKind::RemoteBranch,
        target_commit,
        head: snapshot.head.branch(),
        can_fast_forward,
        writable: writable && snapshot.state.is_clean(),
    })
}

/// "Reset <branch> to this commit": soft, mixed or hard (confirmed next).
fn reset_dialog(snapshot: &RepoSnapshot, id: &str) -> Modal {
    use git_core::ResetMode;
    let head = head_name(snapshot);
    let short = &id[..id.len().min(7)];
    let button = |mode: ResetMode, kind| crate::overlay::DialogButton {
        label: mode.label().into(),
        kind,
        message: Some(Message::Reset {
            id: id.to_owned(),
            mode,
            confirmed: false,
        }),
    };
    Modal {
        title: format!("Reset {head} to {short}"),
        text: "Soft: keep all changes, staged.\n\
               Mixed: keep all changes, unstaged.\n\
               Hard: discard all uncommitted changes (untracked files stay)."
            .into(),
        details: None,
        buttons: vec![
            crate::overlay::DialogButton {
                label: "Cancel".into(),
                kind: crate::overlay::ButtonKind::Secondary,
                message: None,
            },
            button(ResetMode::Soft, crate::overlay::ButtonKind::Secondary),
            button(ResetMode::Mixed, crate::overlay::ButtonKind::Primary),
            button(ResetMode::Hard, crate::overlay::ButtonKind::Danger),
        ],
        default: Some(2),
    }
}

/// A hard reset is always confirmed and lists the changes it discards.
fn hard_reset_dialog(snapshot: &RepoSnapshot, id: &str) -> Modal {
    let head = head_name(snapshot);
    let short = &id[..id.len().min(7)];
    let tree = &snapshot.working_tree;
    let mut lost: Vec<&str> = tree
        .staged
        .iter()
        .chain(&tree.unstaged)
        .filter(|f| f.kind != git_core::ChangeKind::Untracked)
        .map(|f| f.path.as_str())
        .collect();
    lost.sort_unstable();
    lost.dedup();
    let text = if lost.is_empty() {
        format!("Move {head} to {short} and reset the working tree? There are no uncommitted changes to lose.")
    } else {
        format!(
            "Move {head} to {short} and discard the uncommitted changes to {} file{}? \
             Untracked files stay.",
            lost.len(),
            if lost.len() == 1 { "" } else { "s" }
        )
    };
    let details = (!lost.is_empty()).then(|| {
        const SHOWN: usize = 20;
        let mut list = lost
            .iter()
            .take(SHOWN)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        if lost.len() > SHOWN {
            list.push_str(&format!("\n… and {} more", lost.len() - SHOWN));
        }
        list
    });
    Modal {
        title: format!("Hard reset {head}"),
        text,
        details,
        buttons: vec![
            crate::overlay::DialogButton {
                label: "Cancel".into(),
                kind: crate::overlay::ButtonKind::Secondary,
                message: None,
            },
            crate::overlay::DialogButton {
                label: "Hard reset".into(),
                kind: crate::overlay::ButtonKind::Danger,
                message: Some(Message::Reset {
                    id: id.to_owned(),
                    mode: git_core::ResetMode::Hard,
                    confirmed: true,
                }),
            },
        ],
        default: Some(0),
    }
}

/// A cherry-pick or revert step has nothing to commit (its changes are
/// already there): skip it, keep it as an empty commit, or abort.
fn empty_pick_dialog(snapshot: &RepoSnapshot) -> Option<Modal> {
    let operation = snapshot.state.kind()?;
    let (title, what) = match operation {
        git_core::Operation::Revert => ("Empty revert", "Reverting this commit"),
        _ => ("Empty cherry-pick", "Applying this commit"),
    };
    let mut buttons = vec![crate::overlay::DialogButton {
        label: format!("Abort {}", operation.command()),
        kind: crate::overlay::ButtonKind::Secondary,
        message: Some(Message::AbortOperation),
    }];
    if operation.can_skip() {
        buttons.push(crate::overlay::DialogButton {
            label: "Skip".into(),
            kind: crate::overlay::ButtonKind::Secondary,
            message: Some(Message::SkipOperation),
        });
    }
    buttons.push(crate::overlay::DialogButton {
        label: "Commit empty".into(),
        kind: crate::overlay::ButtonKind::Primary,
        message: Some(Message::CommitEmpty),
    });
    let default = buttons.len().checked_sub(2);
    Some(Modal {
        title: title.into(),
        text: format!(
            "{what} changes nothing: its changes are already on {}. Skip it, \
             or keep it as an empty commit?",
            snapshot.branch_label()
        ),
        details: None,
        buttons,
        default,
    })
}

/// Commit a full reference name (`refs/heads/x`, `refs/remotes/o/x`) points
/// to in `snapshot`.
fn git_core_ref_target<'a>(snapshot: &'a RepoSnapshot, reference: &str) -> Option<&'a str> {
    let (kind, name) = if let Some(name) = reference.strip_prefix("refs/heads/") {
        (git_core::RefKind::LocalBranch, name)
    } else {
        (
            git_core::RefKind::RemoteBranch,
            reference.strip_prefix("refs/remotes/")?,
        )
    };
    snapshot
        .refs
        .iter()
        .find(|r| r.kind == kind && r.name == name)
        .map(|r| r.target.as_str())
}

/// Completes a history entry with the current HEAD; no entry if nothing
/// moved (e.g. "Already up to date").
fn finish_entry(
    git: &dyn git_core::GitWriter,
    mut entry: git_core::HistoryChange,
) -> git_core::Result<Option<History>> {
    let Some(after) = git_core::head_commit(git)? else {
        return Ok(None);
    };
    if after == entry.before && !matches!(entry.kind, git_core::HistoryUndo::HardReset(_)) {
        return Ok(None);
    }
    entry.after = after;
    Ok(Some(History::Push(UndoAction::History(entry))))
}

/// After Continue, Skip or "Commit empty": once the operation is complete,
/// the entry kept aside when it stopped is pushed; while it is still in
/// progress the entry stays aside.
fn complete_pending(
    git: &dyn git_core::GitWriter,
    pending: Option<UndoAction>,
) -> git_core::Result<Option<History>> {
    let Some(UndoAction::History(entry)) = pending else {
        return Ok(None);
    };
    if git_core::current_state(git)?.is_clean() {
        finish_entry(git, entry)
    } else {
        Ok(Some(History::Started(UndoAction::History(entry))))
    }
}

/// Summary and body of `commit` as one message.
fn full_message(commit: &git_core::Commit) -> String {
    if commit.body.is_empty() {
        commit.summary.clone()
    } else {
        format!("{}\n\n{}", commit.summary, commit.body)
    }
}

/// Keys of the interactive rebase panel; true if `shortcut` was handled.
fn rebase_panel_shortcut(panel: &mut crate::rebase_panel::RebasePanel, shortcut: Shortcut) -> bool {
    use crate::nav::Move;
    use git_core::TodoAction;
    match shortcut {
        Shortcut::Navigate(Move::Previous) => panel.move_selection(true),
        Shortcut::Navigate(Move::Next) => panel.move_selection(false),
        Shortcut::Reorder { up } => panel.move_selected(up),
        Shortcut::StageSelected => panel.set_selected_action(TodoAction::Squash),
        Shortcut::TodoKey(key) => {
            if let Some(action) = TodoAction::ALL.into_iter().find(|a| a.key() == key) {
                panel.set_selected_action(action);
            }
        }
        _ => return false,
    }
    true
}

/// Marker labels for ours and theirs in the conflict view: the checked-out
/// branch (or HEAD) and the commit being merged, picked or reverted.
fn conflict_labels(snapshot: &RepoSnapshot) -> (String, String) {
    use git_core::RepoState;
    let ours = head_name(snapshot).to_owned();
    let theirs = match &snapshot.state {
        RepoState::Merging { merge_head } => snapshot.commit_name(merge_head),
        RepoState::CherryPicking { commit: Some(id) }
        | RepoState::Reverting { commit: Some(id) } => snapshot.commit_name(id),
        _ => "theirs".into(),
    };
    (ours, theirs)
}

/// Name of the checked-out branch for menu labels, "HEAD" when detached.
fn head_name(snapshot: &RepoSnapshot) -> &str {
    snapshot.head.branch().unwrap_or("HEAD")
}

/// Local branch tracking remote branch `name` (at `target`) that is not
/// checked out and can be fast-forwarded to it.
fn behind_local<'a>(snapshot: &'a RepoSnapshot, name: &str, target: &str) -> Option<&'a str> {
    snapshot
        .refs
        .iter()
        .find(|r| {
            r.kind == git_core::RefKind::LocalBranch
                && !r.is_head
                && r.upstream.as_deref() == Some(name)
                && r.target != target
                && snapshot.is_ancestor(&r.target, target) != Some(false)
        })
        .map(|r| r.name.as_str())
}

/// Dialog for a rejected push: the remote has commits we do not have
/// (offer Pull or a force push), or a force push lost its lease.
fn push_rejected_dialog(error: String) -> Modal {
    if git_core::push_rejection(&error) == Some(git_core::PushRejection::StaleLease) {
        return Modal::error("Force push refused", error).with_text(
            "The remote branch changed since your last fetch, so the force push was refused \
             to keep those commits. Fetch and check the changes first.",
        );
    }
    Modal {
        title: "Push rejected".into(),
        text: "The remote branch has commits that you do not have. Pull them first, or \
               overwrite the remote branch with a force push."
            .into(),
        details: Some(error),
        buttons: vec![
            crate::overlay::DialogButton {
                label: "Cancel".into(),
                kind: crate::overlay::ButtonKind::Secondary,
                message: None,
            },
            crate::overlay::DialogButton {
                label: "Force push".into(),
                kind: crate::overlay::ButtonKind::Danger,
                message: Some(Message::ForcePushRequested),
            },
            crate::overlay::DialogButton {
                label: "Pull".into(),
                kind: crate::overlay::ButtonKind::Primary,
                message: Some(Message::Pull(None)),
            },
        ],
        default: Some(2),
    }
}

/// Why the current branch cannot be pulled, if it cannot.
fn pull_blocker(snapshot: &RepoSnapshot) -> Option<&'static str> {
    match &snapshot.head {
        git_core::Head::Detached { .. } => {
            Some("HEAD is detached. Check out a branch to pull its upstream branch.")
        }
        git_core::Head::Unborn { .. } => Some("The current branch has no commits yet."),
        git_core::Head::Branch { name, .. } => {
            let upstream = snapshot
                .refs
                .iter()
                .find(|r| r.kind == git_core::RefKind::LocalBranch && &r.name == name)
                .and_then(|r| r.upstream.as_ref());
            // "Set upstream" follows with upstream management (M3-10).
            upstream.is_none().then_some(
                "The current branch has no upstream branch, so there is nothing to pull from. \
                 Set an upstream branch first (e.g. by pushing the branch).",
            )
        }
    }
}

/// Dialog after a failed pull: a conflict left a merge or rebase behind
/// (resolution comes with M4, abort is offered), or a plain error.
fn pull_failed_dialog(error: String, state: &git_core::RepoState) -> Modal {
    let operation = match state {
        git_core::RepoState::Merging { .. } => "merge",
        git_core::RepoState::Rebasing { .. } => "rebase",
        _ => {
            return match auth_hint(&error) {
                Some(hint) => Modal::error("Pull failed", error).with_text(hint),
                None => Modal::error("Pull failed", error),
            }
        }
    };
    Modal {
        title: "Pull stopped with conflicts".into(),
        text: format!(
            "The {operation} stopped because of conflicting changes. Resolve the \
             conflicts, stage the files and continue, or abort the {operation} to get \
             back to the state before the pull."
        ),
        details: Some(error),
        buttons: vec![
            crate::overlay::DialogButton {
                label: "Close".into(),
                kind: crate::overlay::ButtonKind::Secondary,
                message: None,
            },
            crate::overlay::DialogButton {
                label: format!("Abort {operation}"),
                kind: crate::overlay::ButtonKind::Danger,
                message: Some(Message::AbortOperation),
            },
        ],
        default: Some(0),
    }
}

/// Updates the multi-selection for a click (or keyboard move) on commit
/// `id`: `toggle` = Ctrl, `range` = Shift, neither = only this commit.
fn select_multi(view: &mut RepoView, id: &str, toggle: bool, range: bool) {
    let snapshot = Arc::clone(&view.snapshot);
    let order = crate::repo_view::selectable_ids(&snapshot);
    if snapshot.is_stash(id) {
        view.multi.clear();
        return;
    }
    // The current single selection is where a Ctrl/Shift selection starts.
    if (toggle || range) && view.multi.is_empty() {
        if let Some(current) = view.selected_commit().map(str::to_owned) {
            if order.contains(&current.as_str()) {
                view.multi.single(&current);
            }
        }
    }
    if range {
        view.multi.range(id, &order);
    } else if toggle {
        view.multi.toggle(id, &order);
    } else {
        view.multi.single(id);
    }
}

/// What the auto-fetch scheduler needs to know about a tab.
fn auto_fetch_state(view: &RepoView) -> crate::fetch::TabState {
    crate::fetch::TabState {
        last_fetch: view.last_fetch,
        busy: view.busy.is_some(),
        operation_in_progress: !view.snapshot.state.is_clean(),
        has_remotes: view
            .snapshot
            .refs
            .iter()
            .any(|r| r.kind == git_core::RefKind::RemoteBranch),
    }
}

/// Hint for authentication failures in Git's output (ADR-019).
fn auth_hint(error: &str) -> Option<&'static str> {
    let lower = error.to_lowercase();
    if lower.contains("host key verification failed") {
        Some("The server's SSH host key was not accepted. Check the host and confirm its key, then try again.")
    } else if lower.contains("permission denied (publickey") {
        Some("The server rejected your SSH key. Check that the key is added to your account and loaded in ssh-agent (or protected by a passphrase Gitloom can ask for).")
    } else if lower.contains("authentication failed")
        || lower.contains("could not read username")
        || lower.contains("could not read password")
        || lower.contains("invalid username or password")
    {
        Some("Authentication failed. Check your credentials or your Git credential helper (e.g. Git Credential Manager); Gitloom does not store passwords.")
    } else {
        None
    }
}

/// Called while a write runs: a running background network operation is
/// cancelled and `next` starts after it; otherwise the user has to wait.
/// Returns the status message.
fn yield_or_wait(view: &mut RepoView, next: Queued) -> String {
    match &view.network {
        Some(op) if op.background => {
            op.cancel.cancel();
            view.queued = Some(next);
            format!("Stopping {}…", op.label)
        }
        _ => {
            let running = view.busy.clone().unwrap_or_default();
            format!("Please wait: {running} is still running")
        }
    }
}

/// Runs a network command off the UI thread and emits its progress
/// updates, then [`Message::WriteFinished`].
fn network_stream(
    tab: TabId,
    writer: Arc<dyn git_core::GitWriter>,
    request: NetworkRequest,
    cancel: git_core::CancelToken,
) -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(
        64,
        async move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
            use iced::futures::{SinkExt as _, StreamExt as _};

            let (sender, mut updates) = iced::futures::channel::mpsc::unbounded();
            let NetworkRequest { label, args, .. } = request;
            let work = tokio::task::spawn_blocking(move || {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                writer
                    .run_network(&args, &cancel, &mut |progress| {
                        let _ = sender.unbounded_send(progress);
                    })
                    .map(|_| None)
                    .map_err(|e| e.to_string())
            });
            // The sender is dropped when the command ends, which ends this loop.
            while let Some(progress) = updates.next().await {
                let _ = output
                    .send(Message::NetworkProgress { tab, progress })
                    .await;
            }
            let result = work
                .await
                .unwrap_or_else(|e| Err(format!("Background task failed: {e}")));
            let _ = output
                .send(Message::WriteFinished { tab, label, result })
                .await;
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

/// Finds out whether any focusable widget (text input or editor) has the
/// keyboard focus. Unlike `operation::is_focused`, which yields nothing when
/// its widget is not in the current view (and would swallow the shortcut),
/// this always produces an answer.
#[derive(Debug, Default)]
struct AnyFocused(bool);

impl iced::advanced::widget::Operation<bool> for AnyFocused {
    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation<bool>),
    ) {
        operate(self);
    }

    fn focusable(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _bounds: iced::Rectangle,
        state: &mut dyn iced::advanced::widget::operation::Focusable,
    ) {
        self.0 |= state.is_focused();
    }

    fn finish(&self) -> iced::advanced::widget::operation::Outcome<bool> {
        iced::advanced::widget::operation::Outcome::Some(self.0)
    }
}

/// Maps window events and keyboard shortcuts no widget handled.
fn global_event(event: Event, status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
        Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            modifiers,
            ..
        }) if status == event::Status::Ignored => {
            shortcuts::shortcut_with_modified(&key, &modified_key, modifiers).map(Message::Shortcut)
        }
        // A focused text field takes the first Esc; overlays with a query
        // field close right away anyway.
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        }) => Some(Message::EscapeCaptured),
        _ => None,
    }
}

/// Why committing is not possible right now, if it is not.
pub fn commit_blocker(view: &RepoView, stage_all: bool) -> Option<String> {
    let tree = &view.snapshot.working_tree;
    // Continuing an operation may commit nothing new (e.g. a resolved rebase step).
    let continues = view.snapshot.state.kind().is_some() && !view.amend;
    let merging = view.snapshot.merge_message.is_some();
    let has_changes = !tree.staged.is_empty() || (stage_all && !tree.unstaged.is_empty());
    let state = if view.amend {
        Guarded::Amend
    } else {
        Guarded::Commit
    };
    let conflicts = view.conflicted_files();
    if let Some(reason) = view.state_block(state) {
        Some(reason)
    } else if continues && !conflicts.is_empty() && !stage_all {
        Some(unresolved_conflicts(&conflicts))
    } else if !has_changes && !continues && !merging && !view.amend {
        // Amending may only change the message.
        Some("Stage files to commit".into())
    } else if view.summary.trim().is_empty() {
        Some("Enter a commit summary".into())
    } else {
        None
    }
}

/// Tooltip for a blocked Continue: which files still have conflicts.
pub fn unresolved_conflicts(paths: &[&str]) -> String {
    const SHOWN: usize = 5;
    let mut list = paths
        .iter()
        .take(SHOWN)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > SHOWN {
        list.push_str(&format!(" and {} more", paths.len() - SHOWN));
    }
    format!("Resolve and stage the conflicted files first: {list}")
}

/// True if HEAD, any ref target or upstream, or the stash list differ from `snapshot`.
fn refs_changed(
    snapshot: &RepoSnapshot,
    head: &git_core::Head,
    refs: &[git_core::Reference],
    stashes: &[git_core::Stash],
) -> bool {
    let key =
        |r: &git_core::Reference| (r.name.clone(), r.kind, r.target.clone(), r.upstream.clone());
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
    fn shortcut_list_opens_filters_and_closes() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(checked(Shortcut::ShortcutList, false));
        assert_eq!(app.shortcut_list.as_deref(), Some(""));
        let _ = app.update(Message::ShortcutFilterChanged("fetch".into()));
        assert_eq!(app.shortcut_list.as_deref(), Some("fetch"));
        // Other shortcuts wait while the list is open; Esc closes it.
        let _ = app.update(checked(Shortcut::NewTab, false));
        assert_eq!(app.tabs.len(), 1);
        let _ = app.update(Message::Shortcut(Shortcut::Close));
        assert!(app.shortcut_list.is_none());
        // Esc in the focused filter field closes it too.
        let _ = app.update(checked(Shortcut::ShortcutList, false));
        let _ = app.update(Message::EscapeCaptured);
        assert!(app.shortcut_list.is_none());
    }

    #[test]
    fn actions_of_later_tasks_explain_why_they_do_nothing() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(checked(Shortcut::CommandPalette, false));
        assert_eq!(app.status, "Command palette: Not available yet");
    }

    #[test]
    fn settings_view_opens_applies_and_closes() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(checked(Shortcut::Settings, false));
        let page = app.settings_page.as_ref().unwrap();
        assert_eq!(page.category, settings::Category::General);
        assert_eq!(page.auto_fetch_input, "5");

        // Valid input applies immediately; invalid input keeps the old value.
        let _ = app.update(Message::AutoFetchInput("12".into()));
        assert_eq!(app.settings.general.auto_fetch_minutes, 12);
        let _ = app.update(Message::AutoFetchInput("12x".into()));
        assert_eq!(app.settings.general.auto_fetch_minutes, 12);
        let page = app.settings_page.as_ref().unwrap();
        assert_eq!(page.auto_fetch_input, "12x");
        assert!(page.error.is_some());
        let _ = app.update(Message::AutoFetchInput("0".into()));
        assert_eq!(app.settings.general.auto_fetch_minutes, 0);
        assert!(app.settings_page.as_ref().unwrap().error.is_none());

        // Other shortcuts wait; Esc returns to the repository.
        let _ = app.update(checked(Shortcut::NewTab, false));
        assert_eq!(app.tabs.len(), 1);
        let _ = app.update(Message::SettingsCategory(settings::Category::Appearance));
        assert_eq!(
            app.settings_page.as_ref().map(|p| p.category),
            Some(settings::Category::Appearance)
        );
        let _ = app.update(Message::Shortcut(Shortcut::Close));
        assert!(app.settings_page.is_none());
        let _ = app.update(Message::SettingsOpened);
        let _ = app.update(Message::EscapeCaptured);
        assert!(app.settings_page.is_none());
    }

    #[test]
    fn zoom_shortcuts_step_persist_and_flash() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(checked(Shortcut::ZoomIn, false));
        assert_eq!(app.settings.appearance.zoom_percent, 110);
        assert!((app.scale_factor() - 1.1).abs() < 1e-6);
        assert_eq!(app.status, "Zoom 110 %");
        let seq = app.status_seq;
        let _ = app.update(Message::StatusExpired(seq));
        assert!(app.status.is_empty());

        for _ in 0..20 {
            let _ = app.update(checked(Shortcut::ZoomIn, false));
        }
        assert_eq!(app.settings.appearance.zoom_percent, 200);
        // Zoom also works while the settings view is open.
        let _ = app.update(Message::SettingsOpened);
        let _ = app.update(checked(Shortcut::ZoomReset, false));
        assert_eq!(app.settings.appearance.zoom_percent, 100);
        for _ in 0..20 {
            let _ = app.update(checked(Shortcut::ZoomOut, false));
        }
        assert_eq!(app.settings.appearance.zoom_percent, 70);
        let _ = app.update(Message::ZoomTo(120));
        assert_eq!(app.settings.appearance.zoom_percent, 120);
        // An older flash does not clear a newer message.
        let old = app.status_seq;
        let _ = app.update(Message::ZoomTo(130));
        let _ = app.update(Message::StatusExpired(old));
        assert_eq!(app.status, "Zoom 130 %");
    }

    #[test]
    fn window_size_follows_the_zoom() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::WindowResized(iced::Size::new(1000.0, 800.0)));
        assert_eq!(app.window_size(), iced::Size::new(1000.0, 800.0));
        let _ = app.update(Message::ZoomTo(200));
        assert_eq!(app.window_size(), iced::Size::new(500.0, 400.0));
        let _ = app.update(Message::WindowResized(iced::Size::new(600.0, 400.0)));
        assert_eq!(app.window_size(), iced::Size::new(600.0, 400.0));
    }

    #[test]
    fn theme_choice_and_system_mode() {
        let mut app = ready_app(&["c"]);
        assert!(!app.light_theme(), "dark by default");
        let _ = app.update(Message::ThemeChosen(settings::ThemeChoice::Light));
        assert!(app.light_theme());
        assert_eq!(app.settings.appearance.theme, settings::ThemeChoice::Light);
        assert_eq!(crate::theme::palette(&app.theme()), &crate::theme::LIGHT);

        let _ = app.update(Message::ThemeChosen(settings::ThemeChoice::System));
        assert!(!app.light_theme(), "unknown system mode falls back to dark");
        let _ = app.update(Message::SystemTheme(iced::theme::Mode::Light));
        assert!(app.light_theme());
        let _ = app.update(Message::SystemTheme(iced::theme::Mode::Dark));
        assert!(!app.light_theme());
    }

    #[test]
    fn auto_fetch_interval_change_restarts_the_timer() {
        let mut app = ready_app(&["c"]);
        let seq = app.auto_fetch_seq;
        let _ = app.update(Message::SettingsOpened);
        let _ = app.update(Message::AutoFetchInput("7".into()));
        assert!(app.auto_fetch_seq > seq, "the pending timer is replaced");
        // A timer of the old interval does nothing.
        let _ = app.update(Message::AutoFetchDue(seq));
        assert!(app.tabs[0].view().unwrap().busy.is_none());
    }

    #[test]
    fn loaded_settings_apply_and_report_a_reset_file() {
        let mut app = ready_app(&["c"]);
        let mut loaded = Settings::default();
        loaded.general.auto_fetch_minutes = 30;
        let _ = app.update(Message::SettingsLoaded(settings::Loaded {
            settings: loaded.clone(),
            notice: Some("The settings file was unreadable; defaults are used.".into()),
            write: true,
        }));
        assert_eq!(app.settings, loaded);
        assert!(app.status.contains("defaults are used"));
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
            _ => panic!("diff closed"),
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
            _ => panic!("no diff"),
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
            _ => panic!("diff closed"),
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
            _ => None,
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

    fn rebasing() -> git_core::RepoState {
        git_core::RepoState::Rebasing {
            branch: Some("topic".into()),
            onto: None,
            step: None,
        }
    }

    /// Marks `paths` as conflicted in the working tree of the first tab.
    fn with_conflicts(app: &mut App, paths: &[&str]) {
        let view = app.tabs[0].view_mut().unwrap();
        let mut tree = view.snapshot.working_tree.clone();
        for path in paths {
            tree.unstaged.push(git_core::FileChange {
                path: (*path).into(),
                kind: git_core::ChangeKind::Conflicted,
                old_path: None,
            });
        }
        view.snapshot = Arc::new(view.snapshot.with_working_tree(tree).unwrap());
    }

    #[test]
    fn commit_button_continues_the_operation() {
        for (state, label) in [
            (rebasing(), "Continue rebase"),
            (merging(), "Continue merge"),
            (
                git_core::RepoState::CherryPicking { commit: None },
                "Continue cherry-pick",
            ),
            (
                git_core::RepoState::Reverting { commit: None },
                "Continue revert",
            ),
        ] {
            let mut app = app_in_state(state);
            let view = app.tabs[0].view_mut().unwrap();
            // A resolved step may have nothing staged.
            let tree = git_core::WorkingTree::default();
            view.snapshot = Arc::new(view.snapshot.with_working_tree(tree).unwrap());
            assert!(commit_blocker(view, false).is_none(), "{label}");
            let _ = app.update(Message::CommitRequested);
            let view = app.tabs[0].view().unwrap();
            assert_eq!(view.busy.as_deref(), Some(label));
            assert!(view.clear_message_after_write);
        }
    }

    #[test]
    fn banner_continue_needs_resolved_conflicts_and_a_message() {
        let mut app = app_in_state(rebasing());
        with_conflicts(&mut app, &["b.txt", "c.txt"]);
        let view = app.tabs[0].view().unwrap();
        assert_eq!(
            commit_blocker(view, false).as_deref(),
            Some("Resolve and stage the conflicted files first: b.txt, c.txt")
        );
        let _ = app.update(Message::ContinueOperation);
        assert!(app.tabs[0].view().unwrap().busy.is_none());

        let mut app = app_in_state(rebasing());
        app.tabs[0].view_mut().unwrap().summary.clear();
        let _ = app.update(Message::ContinueOperation);
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert_eq!(app.status, "Enter a commit summary");
    }

    #[test]
    fn blocked_merge_offers_stash_and_merge() {
        let mut app = staging_app(&["a"], &[]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::Merge {
            source: "refs/heads/topic".into(),
            autostash: false,
        });
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Merge topic into HEAD")
        );
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Merge topic into HEAD".into(),
            result: Err("error: Your local changes to the following files would be \
                         overwritten by merge:\n\ta\nPlease commit your changes or stash them"
                .into()),
        });
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Local changes block the merge");
        assert!(matches!(
            modal.action(1),
            Some(Message::Merge { source, autostash: true }) if source == "refs/heads/topic"
        ));
    }

    #[test]
    fn blocked_rebase_offers_stash_and_rebase() {
        let mut app = staging_app(&["a"], &[]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::Rebase {
            onto: "refs/remotes/origin/main".into(),
            autostash: false,
            confirmed: false,
        });
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Rebase HEAD onto origin/main")
        );
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Rebase HEAD onto origin/main".into(),
            result: Err("error: cannot rebase: You have unstaged changes.".into()),
        });
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Local changes block the rebase");
        assert_eq!(modal.buttons[1].label, "Stash and rebase");
        assert!(matches!(
            modal.action(1),
            Some(Message::Rebase { onto, autostash: true, .. }) if onto == "refs/remotes/origin/main"
        ));
    }

    #[test]
    fn merging_is_blocked_during_an_operation_but_fast_forwarding_others_is_not() {
        let mut app = app_in_state(rebasing());
        let _ = app.update(Message::Merge {
            source: "refs/heads/topic".into(),
            autostash: false,
        });
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert_eq!(app.status, "Cannot merge while a rebase is in progress");

        let _ = app.update(Message::FastForward {
            branch: Some("topic".into()),
            source: "refs/remotes/origin/topic".into(),
            autostash: false,
        });
        let view = app.tabs[0].view().unwrap();
        assert_eq!(
            view.busy.as_deref(),
            Some("Fast-forward topic to origin/topic")
        );
        assert!(
            view.pending_merge.is_none(),
            "does not touch the working tree"
        );
    }

    fn conflict_center(app: &App) -> Option<&crate::conflict::ConflictView> {
        match &app.tabs[0].view().unwrap().center {
            CenterView::Conflict(conflict) => Some(conflict),
            _ => None,
        }
    }

    #[test]
    fn conflict_view_opens_loads_and_closes_when_resolved() {
        let mut app = app_in_state(merging());
        with_conflicts(&mut app, &["a.txt"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::ConflictOpened("a.txt".into()));
        let conflict = conflict_center(&app).unwrap();
        assert_eq!(conflict.loaded, Loadable::Loading);
        assert_eq!(conflict.labels, ("HEAD".to_string(), "x".to_string()));

        let text = git_core::ConflictText {
            chunks: vec![git_core::Chunk::Conflict(git_core::ConflictHunk {
                ours: vec!["o\n".into()],
                base: vec![],
                theirs: vec!["t\n".into()],
            })],
            eol: "\n",
        };
        let _ = app.update(Message::ConflictLoaded {
            tab,
            path: "a.txt".into(),
            result: Ok(git_core::LoadedConflict {
                path: "a.txt".into(),
                stages: git_core::ConflictStages::default(),
                content: git_core::ConflictContent::Text(text),
            }),
        });
        // Saving with an open conflict explains instead of writing.
        let _ = app.update(Message::ConflictSave);
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert_eq!(
            app.status,
            "Pick lines for the remaining conflict, or edit the output"
        );
        let _ = app.update(Message::ConflictTakeAll(git_core::Side::Theirs));
        assert_eq!(conflict_center(&app).unwrap().output.text(), "t\n");
        let _ = app.update(Message::ConflictSave);
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Resolve a.txt")
        );

        // The refresh after resolving: no conflict left, the view closes.
        let view = app.tabs[0].view_mut().unwrap();
        view.busy = None;
        let snapshot = view
            .snapshot
            .with_working_tree(git_core::WorkingTree::default());
        view.apply_snapshot(Arc::new(snapshot.unwrap()));
        assert!(conflict_center(&app).is_none());
    }

    #[test]
    fn a_stale_conflict_load_is_ignored() {
        let mut app = app_in_state(merging());
        with_conflicts(&mut app, &["a.txt", "b.txt"]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::ConflictOpened("b.txt".into()));
        let _ = app.update(Message::ConflictLoaded {
            tab,
            path: "a.txt".into(),
            result: Err("old".into()),
        });
        assert_eq!(conflict_center(&app).unwrap().loaded, Loadable::Loading);
    }

    fn rebase_center(app: &App) -> Option<&crate::rebase_panel::RebasePanel> {
        match &app.tabs[0].view().unwrap().center {
            CenterView::Rebase(panel) => Some(panel),
            _ => None,
        }
    }

    fn menu_item_enabled(app: &App, label: &str) -> Option<bool> {
        app.context_menu
            .as_ref()?
            .entries
            .iter()
            .find_map(|e| match e {
                crate::overlay::MenuEntry::Item {
                    label: l, enabled, ..
                } if l == label => Some(*enabled),
                _ => None,
            })
    }

    #[test]
    fn interactive_rebase_is_offered_for_ancestors_of_head() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(git_available());
        let item = "Interactive rebase from here";
        for (id, enabled) in [("a", true), ("b", true), ("c", false)] {
            let _ = app.update(Message::CommitContextMenu {
                id: id.into(),
                position: iced::Point::ORIGIN,
            });
            assert_eq!(menu_item_enabled(&app, item), Some(enabled), "{id}");
        }
    }

    #[test]
    fn rebase_panel_keys_plan_and_start() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(git_available());
        let tab = app.tabs[0].id;
        let _ = app.update(Message::RebasePanelOpened("a".into()));
        assert_eq!(rebase_center(&app).unwrap().loaded, Loadable::Loading);
        let _ = app.update(Message::RebaseLoaded {
            tab,
            base: "a".into(),
            result: Ok(vec![("b".into(), "B\n".into()), ("c".into(), "C\n".into())]),
        });
        // Top row (newest, "c") is selected: S squashes it into "b".
        let _ = app.update(checked(Shortcut::StageSelected, false));
        assert_eq!(
            rebase_center(&app).unwrap().entries[1].action,
            git_core::TodoAction::Squash
        );
        // ↓ selects "b"; S there would leave nothing to meld into.
        let _ = app.update(checked(Shortcut::Navigate(crate::nav::Move::Next), false));
        let _ = app.update(checked(Shortcut::TodoKey('s'), false));
        let _ = app.update(checked(Shortcut::StageSelected, false));
        let _ = app.update(Message::RebaseStart);
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert!(app.status.contains("oldest kept commit"), "{}", app.status);

        let _ = app.update(checked(Shortcut::TodoKey('p'), false));
        let _ = app.update(Message::RebaseStart);
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Interactive rebase of HEAD"));
        assert!(matches!(view.center, CenterView::Graph), "panel closes");
        assert!(matches!(
            &view.pending_merge,
            Some(Message::RunInteractiveRebase { base, input, autostash: false })
                if base == "a"
                    && input.sequence.as_deref() == Some("pick b\nsquash c\n")
                    && input.messages == ["B\n\nC\n"]
        ));
    }

    fn paste_into_message_dialog(app: &mut App, text: &str) {
        use iced::widget::text_editor::{Action, Edit};
        let _ = app.update(Message::CommitMessageEdited(Action::Edit(Edit::Paste(
            Arc::new(text.into()),
        ))));
    }

    #[test]
    fn reword_and_squash_rules_follow_the_first_parent_line() {
        let app = ready_app(&["c", "b", "a"]);
        let snapshot = &app.tabs[0].view().unwrap().snapshot;
        assert!(crate::repo_view::rewordable(snapshot, "c"), "HEAD: amend");
        assert!(crate::repo_view::rewordable(snapshot, "b"));
        assert!(!crate::repo_view::rewordable(snapshot, "a"), "root commit");
        let ids = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(crate::repo_view::squashable(snapshot, &ids(&["c", "b"])));
        assert!(
            !crate::repo_view::squashable(snapshot, &ids(&["b", "a"])),
            "root"
        );
        assert!(
            !crate::repo_view::squashable(snapshot, &ids(&["c", "a"])),
            "gap"
        );
        assert!(!crate::repo_view::squashable(snapshot, &ids(&["c"])));
    }

    #[test]
    fn edit_commit_message_runs_a_reword() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(git_available());
        let _ = app.update(Message::RewordStart("b".into()));
        assert_eq!(
            app.message_dialog.as_ref().unwrap().title,
            "Edit message of b"
        );
        let _ = app.update(Message::CommitMessageSubmit);
        assert_eq!(
            app.message_dialog.as_ref().unwrap().error.as_deref(),
            Some("Enter a commit message")
        );
        paste_into_message_dialog(&mut app, "New message");
        let _ = app.update(Message::CommitMessageSubmit);
        assert!(app.message_dialog.is_none());
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Edit message of b"));
        assert!(matches!(
            &view.pending_merge,
            Some(Message::RewordCommit { id, message, autostash: false })
                if id == "b" && message == "New message\n"
        ));
    }

    #[test]
    fn squash_selected_needs_a_squashable_selection() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(git_available());
        app.tabs[0].view_mut().unwrap().multi.single("c");
        let _ = app.update(Message::SquashSelected);
        assert!(app.message_dialog.is_none());

        let view = app.tabs[0].view_mut().unwrap();
        let snapshot = Arc::clone(&view.snapshot);
        let order = crate::repo_view::selectable_ids(&snapshot);
        view.multi.range("b", &order);
        let _ = app.update(Message::SquashSelected);
        assert_eq!(
            app.message_dialog.as_ref().unwrap().title,
            "Squash 2 commits"
        );
        paste_into_message_dialog(&mut app, "Both");
        let _ = app.update(Message::CommitMessageSubmit);
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Squash 2 commits"));
        assert!(matches!(
            &view.pending_merge,
            Some(Message::SquashCommits { ids, .. }) if ids.len() == 2
        ));
    }

    /// Ready app with a history given as `(id, parents)`, newest first;
    /// HEAD is detached at the first commit.
    fn app_with_history(history: &[(&str, &[&str])]) -> App {
        use git_core::{Commit, Head, Log, WorkingTree};
        let mut app = ready_app(&["x"]);
        let _ = app.update(git_available());
        let commits = history
            .iter()
            .enumerate()
            .map(|(i, (id, parents))| Commit {
                id: id.to_string(),
                parents: parents.iter().map(|p| p.to_string()).collect(),
                summary: format!("summary {id}"),
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
                target: history[0].0.into(),
            },
            Vec::new(),
            Vec::new(),
            WorkingTree::default(),
        )
        .unwrap();
        app.tabs[0]
            .view_mut()
            .unwrap()
            .apply_snapshot(Arc::new(snapshot));
        app
    }

    #[test]
    fn cherry_pick_and_revert_from_the_commit_menu() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(git_available());
        for (id, pick) in [("c", false), ("b", true)] {
            let _ = app.update(Message::CommitContextMenu {
                id: id.into(),
                position: iced::Point::ORIGIN,
            });
            assert_eq!(
                menu_item_enabled(&app, "Cherry-pick commit"),
                Some(pick),
                "{id}"
            );
            assert_eq!(menu_item_enabled(&app, "Revert commit"), Some(true), "{id}");
        }
        app.context_menu = None;
        let _ = app.update(Message::RevertStart("b".into()));
        assert!(app.form.is_none(), "no parent to choose");
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Revert b")
        );
        assert!(app.tabs[0].view().unwrap().check_empty);
    }

    #[test]
    fn reverting_a_merge_asks_for_the_parent() {
        let mut app = app_with_history(&[("m", &["p1", "p2"]), ("p2", &["p1"]), ("p1", &[])]);
        let _ = app.update(Message::RevertStart("m".into()));
        let form = app.form.as_mut().unwrap();
        assert_eq!(form.value(0), "1: p1 summary p1");
        form.fields[0].value = "2: p2 summary p2".into();
        let _ = app.update(Message::FormSubmit);
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Revert m"));
        assert!(matches!(
            &view.pending_merge,
            Some(Message::Revert { id, mainline: Some(2) }) if id == "m"
        ));
    }

    #[test]
    fn empty_pick_offers_skip_or_commit_empty() {
        for (state, skip) in [
            (git_core::RepoState::CherryPicking { commit: None }, true),
            (git_core::RepoState::Reverting { commit: None }, false),
        ] {
            let mut app = ready_app(&["c", "b", "a"]);
            let _ = app.update(git_available());
            let tab = app.tabs[0].id;
            let _ = app.update(Message::CherryPick(vec!["b".into()]));
            let view = app.tabs[0].view_mut().unwrap();
            assert_eq!(view.busy.as_deref(), Some("Cherry-pick b"));
            view.busy = None;
            let generation = app.tabs[0].generation;
            let view = app.tabs[0].view().unwrap();
            let mut snapshot = (*view.snapshot).clone();
            snapshot.state = state.clone();
            let _ = app.update(Message::SnapshotLoaded {
                tab,
                generation,
                result: Ok(Arc::new(snapshot)),
            });
            let modal = app.modal.clone().unwrap();
            let labels: Vec<_> = modal.buttons.iter().map(|b| b.label.as_str()).collect();
            assert_eq!(labels.contains(&"Skip"), skip, "{state:?}");
            assert_eq!(labels.last(), Some(&"Commit empty"));
            assert!(matches!(
                modal.buttons.last().and_then(|b| b.message.clone()),
                Some(Message::CommitEmpty)
            ));
        }
    }

    #[test]
    fn reset_asks_for_the_mode_and_confirms_hard() {
        let mut app = staging_app(&["u.txt"], &["s.txt"]);
        let _ = app.update(Message::ResetStart("c".into()));
        let modal = app.modal.take().unwrap();
        let labels: Vec<_> = modal.buttons.iter().map(|b| b.label.as_str()).collect();
        assert_eq!(labels, ["Cancel", "Soft", "Mixed", "Hard"]);

        let _ = app.update(modal.action(3).unwrap());
        let modal = app.modal.take().unwrap();
        assert_eq!(modal.title, "Hard reset HEAD");
        assert_eq!(modal.details.as_deref(), Some("s.txt\nu.txt"));
        assert_eq!(modal.default, Some(0), "Cancel is the default");
        assert!(app.tabs[0].view().unwrap().busy.is_none());

        let _ = app.update(modal.action(1).unwrap());
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Hard reset HEAD to c")
        );
    }

    #[test]
    fn soft_reset_runs_directly_and_resets_are_blocked_during_operations() {
        let mut app = staging_app(&[], &[]);
        let _ = app.update(Message::Reset {
            id: "c".into(),
            mode: git_core::ResetMode::Soft,
            confirmed: false,
        });
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Soft reset HEAD to c")
        );

        let mut app = app_in_state(merging());
        let _ = app.update(Message::ResetStart("c".into()));
        assert!(app.modal.is_none());
        assert_eq!(app.status, "Cannot reset while a merge is in progress");
    }

    /// History main (checked out) at "m", feature one commit ahead at "f".
    fn drag_app() -> App {
        let mut app = app_with_history(&[("f", &["m"]), ("m", &[])]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        let branch = |name: &str, target: &str, is_head: bool| git_core::Reference {
            name: name.into(),
            kind: git_core::RefKind::LocalBranch,
            target: target.into(),
            is_head,
            upstream: None,
            ahead_behind: None,
        };
        snapshot.refs = vec![branch("main", "m", true), branch("feature", "f", false)];
        snapshot.head = git_core::Head::Branch {
            name: "main".into(),
            target: "m".into(),
        };
        view.apply_snapshot(Arc::new(snapshot));
        app
    }

    fn local(name: &str) -> (String, git_core::RefKind) {
        (name.into(), git_core::RefKind::LocalBranch)
    }

    #[test]
    fn dragging_a_branch_highlights_targets_and_drops_into_a_menu() {
        let mut app = drag_app();
        let (name, kind) = local("feature");
        let _ = app.update(Message::BranchDragStart {
            name: name.clone(),
            kind,
        });
        // Over itself: no target.
        let _ = app.update(Message::BranchDragEnter {
            name: name.clone(),
            kind,
        });
        assert_eq!(
            app.tabs[0].view().unwrap().drag.as_ref().unwrap().target,
            None
        );
        let _ = app.update(Message::BranchDragEnter {
            name: "main".into(),
            kind,
        });
        let _ = app.update(Message::BranchDragLeave {
            name: name.clone(),
            kind,
        });
        let drag = app.tabs[0].view().unwrap().drag.clone().unwrap();
        assert!(drag.targets("main", kind), "leaving another entry keeps it");

        let _ = app.update(Message::BranchDropped {
            name: "main".into(),
            kind,
            position: iced::Point::ORIGIN,
        });
        let _ = app.update(Message::BranchDragEnd);
        assert!(app.tabs[0].view().unwrap().drag.is_none());
        assert_eq!(
            menu_item_enabled(&app, "Fast-forward main to feature"),
            Some(true)
        );
        assert_eq!(
            menu_item_enabled(&app, "Merge feature into main"),
            Some(true)
        );
        assert_eq!(
            menu_item_enabled(&app, "Rebase feature onto main"),
            Some(false)
        );
    }

    #[test]
    fn a_release_without_a_drag_or_on_the_source_opens_nothing() {
        let mut app = drag_app();
        let (name, kind) = local("feature");
        let _ = app.update(Message::BranchDropped {
            name: "main".into(),
            kind,
            position: iced::Point::ORIGIN,
        });
        assert!(app.context_menu.is_none(), "plain click");
        let _ = app.update(Message::BranchDragStart {
            name: name.clone(),
            kind,
        });
        let _ = app.update(Message::BranchDropped {
            name,
            kind,
            position: iced::Point::ORIGIN,
        });
        assert!(app.context_menu.is_none(), "dropped on itself");
        assert!(app.tabs[0].view().unwrap().drag.is_none());
    }

    fn history_entry(label: &str) -> UndoAction {
        UndoAction::History(git_core::HistoryChange {
            label: label.into(),
            kind: git_core::HistoryUndo::Keep,
            before: "b".into(),
            after: "a".into(),
            branch: Some("main".into()),
        })
    }

    fn finish(app: &mut App, label: &str, result: Result<Option<History>, String>) {
        let tab = app.tabs[0].id;
        app.tabs[0].view_mut().unwrap().busy = Some(label.into());
        let _ = app.update(Message::WriteFinished {
            tab,
            label: label.into(),
            result,
        });
    }

    #[test]
    fn stopped_operation_keeps_its_entry_until_it_completes() {
        let mut app = staging_app(&[], &[]);
        let entry = history_entry("Merge topic into main");
        finish(
            &mut app,
            "Merge topic into main",
            Ok(Some(History::Started(entry.clone()))),
        );
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.pending_history.as_ref(), Some(&entry));
        assert!(view.undo.is_empty());

        // Staging a resolved file does not drop it.
        finish(&mut app, "Stage a.txt", Ok(None));
        assert!(app.tabs[0].view().unwrap().pending_history.is_some());

        finish(
            &mut app,
            "Continue merge",
            Ok(Some(History::Push(entry.clone()))),
        );
        let view = app.tabs[0].view().unwrap();
        assert!(view.pending_history.is_none());
        assert_eq!(view.undo, [entry]);
        assert_eq!(view.undo[0].label(), "Merge topic into main");
    }

    #[test]
    fn aborted_operation_leaves_no_entry() {
        let mut app = staging_app(&[], &[]);
        finish(
            &mut app,
            "Rebase main onto topic",
            Ok(Some(History::Started(history_entry(
                "Rebase main onto topic",
            )))),
        );
        finish(&mut app, "Abort rebase", Ok(None));
        let view = app.tabs[0].view().unwrap();
        assert!(view.pending_history.is_none());
        assert!(view.undo.is_empty());
    }

    #[test]
    fn history_undo_is_blocked_during_an_operation_except_branch_moves() {
        let entry = history_entry("Merge");
        assert!(entry.moves_head());
        let moved = UndoAction::History(git_core::HistoryChange {
            label: "Fast-forward other to main".into(),
            kind: git_core::HistoryUndo::MoveBranch {
                name: "other".into(),
            },
            before: "b".into(),
            after: "a".into(),
            branch: None,
        });
        assert!(!moved.moves_head());

        let mut app = app_in_state(merging());
        app.tabs[0].view_mut().unwrap().undo.push(entry);
        let _ = app.update(Message::Undo);
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert_eq!(app.tabs[0].view().unwrap().undo.len(), 1);
    }

    #[test]
    fn rebasing_over_merges_asks_first() {
        // HEAD "m" merges "s" (side) into "a"; "base" is where "onto" points.
        let mut app = app_with_history(&[
            ("m", &["a", "s"]),
            ("s", &["base"]),
            ("a", &["base"]),
            ("base", &[]),
        ]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.refs = vec![git_core::Reference {
            name: "onto".into(),
            kind: git_core::RefKind::LocalBranch,
            target: "base".into(),
            is_head: false,
            upstream: None,
            ahead_behind: None,
        }];
        assert_eq!(snapshot.merges_between("base", "m"), 1);
        assert_eq!(snapshot.merges_between("a", "m"), 1);
        assert_eq!(snapshot.merges_between("m", "m"), 0);
        view.apply_snapshot(Arc::new(snapshot));

        let _ = app.update(Message::Rebase {
            onto: "refs/heads/onto".into(),
            autostash: false,
            confirmed: false,
        });
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        let modal = app.modal.take().unwrap();
        assert_eq!(modal.title, "Rebase flattens merges");
        let _ = app.update(modal.action(1).unwrap());
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Rebase HEAD onto onto")
        );
    }

    #[test]
    fn operations_started_during_auto_fetch_make_it_yield() {
        let mut app = staging_app(&["a"], &[]);
        let cancel = git_core::CancelToken::default();
        let view = app.tabs[0].view_mut().unwrap();
        view.busy = Some("Auto-fetch".into());
        view.network = Some(crate::repo_view::NetworkOp {
            label: "Auto-fetch".into(),
            kind: crate::repo_view::NetworkKind::Fetch,
            progress: None,
            cancel: cancel.clone(),
            background: true,
        });
        let _ = app.update(Message::Merge {
            source: "refs/heads/topic".into(),
            autostash: false,
        });
        let view = app.tabs[0].view().unwrap();
        assert!(cancel.is_cancelled());
        assert!(view.queued.is_some());
        assert_eq!(app.status, "Stopping Auto-fetch…");
        assert!(
            view.pending_merge.is_none(),
            "the merge has not started yet"
        );
    }

    #[test]
    fn unresolved_conflicts_lists_at_most_five_files() {
        let paths = ["a", "b", "c", "d", "e", "f", "g"];
        assert_eq!(
            unresolved_conflicts(&paths),
            "Resolve and stage the conflicted files first: a, b, c, d, e and 2 more"
        );
    }

    #[test]
    fn skip_only_for_rebase_and_cherry_pick_abort_for_all() {
        for (state, skip) in [
            (rebasing(), Some("Skip rebase")),
            (
                git_core::RepoState::CherryPicking { commit: None },
                Some("Skip cherry-pick"),
            ),
            (merging(), None),
            (git_core::RepoState::Reverting { commit: None }, None),
        ] {
            let mut app = app_in_state(state.clone());
            let _ = app.update(Message::SkipOperation);
            let view = app.tabs[0].view().unwrap();
            assert_eq!(view.busy.as_deref(), skip, "{state:?}");

            let mut app = app_in_state(state.clone());
            let _ = app.update(Message::AbortOperation);
            let view = app.tabs[0].view().unwrap();
            let abort = format!("Abort {}", state.operation().unwrap());
            assert_eq!(view.busy.as_deref(), Some(abort.as_str()));
            assert!(view.clear_message_after_write);
        }
    }

    #[test]
    fn each_new_operation_message_prefills_the_panel() {
        let mut app = staging_app(&[], &[]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.state = rebasing();
        snapshot.merge_message = Some(
            "First step
"
            .into(),
        );
        view.apply_snapshot(Arc::new(snapshot.clone()));
        assert_eq!(view.summary, "First step");
        // Continue cleared the panel; the rebase stopped at the next commit.
        view.summary.clear();
        snapshot.merge_message = Some(
            "Second step
"
            .into(),
        );
        view.apply_snapshot(Arc::new(snapshot.clone()));
        assert_eq!(view.summary, "Second step");
        // The same message does not refill a field the user emptied.
        view.summary.clear();
        view.apply_snapshot(Arc::new(snapshot));
        assert!(view.summary.is_empty());
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
    fn discard_all_asks_about_staged_changes() {
        let mut app = staging_app(&["a"], &["b"]);
        let _ = app.update(Message::Discard(DiscardTarget::All));
        let modal = app.modal.clone().expect("question");
        assert_eq!(modal.buttons.len(), 3);
        assert!(
            matches!(
                modal.default_action(),
                Some(Message::DiscardConfirmed(DiscardTarget::All))
            ),
            "Enter keeps the staged changes"
        );
        assert!(matches!(
            modal.action(2),
            Some(Message::DiscardConfirmed(DiscardTarget::Everything))
        ));
        assert_eq!(modal.buttons[2].kind, crate::overlay::ButtonKind::Danger);

        let _ = app.update(Message::ModalAction(2));
        assert_eq!(
            app.tabs[0].view().unwrap().busy.as_deref(),
            Some("Discard all changes")
        );
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

    /// Regression (M2-01): checking for a focused text field must always
    /// answer, also when no text field is in the view; otherwise every
    /// shortcut that is not allowed in text fields is swallowed.
    #[test]
    fn focus_check_always_answers() {
        use iced::advanced::widget::operation::{Focusable, Outcome};
        use iced::advanced::widget::Operation as _;

        struct Field(bool);
        impl Focusable for Field {
            fn is_focused(&self) -> bool {
                self.0
            }
            fn focus(&mut self) {}
            fn unfocus(&mut self) {}
        }

        let answer = |op: &AnyFocused| match op.finish() {
            Outcome::Some(focused) => Some(focused),
            _ => None,
        };
        let empty = AnyFocused::default();
        assert_eq!(answer(&empty), Some(false), "no text field in the view");

        let mut op = AnyFocused::default();
        op.focusable(None, iced::Rectangle::default(), &mut Field(false));
        op.focusable(None, iced::Rectangle::default(), &mut Field(true));
        op.focusable(None, iced::Rectangle::default(), &mut Field(false));
        assert_eq!(answer(&op), Some(true));
    }

    fn fetch_request(background: bool) -> NetworkRequest {
        crate::fetch::fetch_all(background)
    }

    #[test]
    fn network_operation_shows_progress_and_cancels_quietly() {
        let mut app = staging_app(&["a"], &[]);
        let tab = app.tabs[0].id;
        let _ = app.run_network(tab, fetch_request(false));
        let progress = git_core::Progress {
            phase: "Receiving objects".into(),
            percent: Some(45),
        };
        let _ = app.update(Message::NetworkProgress { tab, progress });
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Fetch"));
        let op = view.network.as_ref().unwrap();
        assert_eq!(
            crate::ui::statusbar::network_label(op),
            "Fetch: Receiving objects 45%"
        );

        let _ = app.update(Message::CancelNetwork(tab));
        assert!(op_cancelled(&app));
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Fetch".into(),
            result: Err(git_core::Error::Cancelled.to_string()),
        });
        let view = app.tabs[0].view().unwrap();
        assert!(view.network.is_none() && view.busy.is_none());
        assert!(app.modal.is_none(), "no error dialog after cancel");
        assert_eq!(app.status, "Fetch: Cancelled");
    }

    fn op_cancelled(app: &App) -> bool {
        app.tabs[0]
            .view()
            .unwrap()
            .network
            .as_ref()
            .is_some_and(|op| op.cancel.is_cancelled())
    }

    #[test]
    fn user_operation_cancels_auto_fetch_and_runs_afterwards() {
        let mut app = staging_app(&["a"], &[]);
        let tab = app.tabs[0].id;
        let _ = app.run_network(tab, fetch_request(true));
        let _ = app.update(Message::Stage(file("a")));
        assert!(op_cancelled(&app), "auto-fetch is stopped");
        assert!(app.tabs[0].view().unwrap().queued.is_some());

        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Fetch".into(),
            result: Err(git_core::Error::Cancelled.to_string()),
        });
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Stage a"), "queued write runs");
        assert!(view.queued.is_none());
        assert!(!app.status.contains("Cancelled"));
    }

    #[test]
    fn auto_fetch_never_interrupts_or_waits() {
        let mut app = staging_app(&["a"], &[]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::Stage(file("a")));
        let _ = app.run_network(tab, fetch_request(true));
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Stage a"));
        assert!(view.network.is_none() && view.queued.is_none());

        // A user fetch while a user write runs has to wait (no queue).
        let _ = app.run_network(tab, fetch_request(false));
        assert!(app.status.starts_with("Please wait"));
        assert!(app.tabs[0].view().unwrap().queued.is_none());
    }

    #[test]
    fn author_line_skips_the_separator_without_email() {
        let with = crate::ui::panel::author_line("a@example.com", 0);
        assert!(with.starts_with("a@example.com · authored "), "{with}");
        let without = crate::ui::panel::author_line("", 0);
        assert!(without.starts_with("authored "), "{without}");
        assert!(!without.contains('·'));
    }

    #[test]
    fn stash_details_keep_their_sections() {
        let mut app = ready_app(&["s"]);
        let tab = app.tabs[0].id;
        let view = app.tabs[0].view_mut().unwrap();
        view.selection = Some(Selection::Commit("s".into()));
        view.details = Some(CommitDetails {
            id: "s".into(),
            changes: Loadable::Loading,
        });
        let untracked = git_core::FileChange {
            path: "new.txt".into(),
            kind: git_core::ChangeKind::Untracked,
            old_path: None,
        };
        let changes = git_core::StashChanges {
            untracked: vec![untracked],
            ..Default::default()
        };
        let _ = app.update(Message::CommitChangesLoaded {
            tab,
            id: "s".into(),
            result: Ok(DetailChanges::Stash(changes.clone())),
        });
        let details = app.tabs[0].view().unwrap().details.clone().unwrap();
        assert_eq!(
            details.changes,
            Loadable::Ready(DetailChanges::Stash(changes))
        );
    }

    #[test]
    fn authentication_errors_get_a_hint() {
        assert!(
            auth_hint("fatal: Authentication failed for 'https://github.com/a/b.git/'")
                .is_some_and(|h| h.contains("credential helper"))
        );
        assert!(auth_hint("git@github.com: Permission denied (publickey).")
            .is_some_and(|h| h.contains("SSH key")));
        assert!(auth_hint("Host key verification failed.").is_some_and(|h| h.contains("host key")));
        assert_eq!(auth_hint("fatal: couldn't find remote ref main"), None);

        let mut app = staging_app(&["a"], &[]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Fetch".into(),
            result: Err("fatal: Authentication failed for 'https://x/'".into()),
        });
        let modal = app.modal.unwrap();
        assert!(modal.text.contains("credential helper"));
        assert_eq!(
            modal.details.as_deref(),
            Some("fatal: Authentication failed for 'https://x/'")
        );
    }

    /// Staging app whose repository has a remote branch.
    fn app_with_remote() -> App {
        let mut app = staging_app(&[], &[]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.refs.push(git_core::Reference {
            name: "origin/main".into(),
            kind: git_core::RefKind::RemoteBranch,
            target: "c".into(),
            is_head: false,
            upstream: None,
            ahead_behind: None,
        });
        view.snapshot = Arc::new(snapshot);
        app
    }

    #[test]
    fn fetch_all_by_shortcut_and_menu() {
        let mut app = app_with_remote();
        let _ = app.update(checked(Shortcut::FetchAll, false));
        assert_eq!(app.tabs[0].view().unwrap().busy.as_deref(), Some("Fetch"));

        let mut app = app_with_remote();
        let _ = app.update(Message::PullMenu(iced::Point::ORIGIN));
        let menu = app.context_menu.clone().unwrap();
        assert!(matches!(menu.select(0), Some(Message::FetchAll)));
        assert!(matches!(
            menu.select(4),
            Some(Message::Pull(Some(git_core::PullMode::Rebase)))
        ));
    }

    #[test]
    fn auto_fetch_runs_when_due() {
        let mut app = app_with_remote();
        let opened = app.tabs[0].view().unwrap().last_fetch;
        let _ = app.auto_fetch(opened + std::time::Duration::from_secs(60));
        assert!(app.tabs[0].view().unwrap().busy.is_none(), "not due yet");

        let later = opened + std::time::Duration::from_secs(5 * 60);
        let _ = app.auto_fetch(later);
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Auto-fetch"));
        assert!(view.network.as_ref().is_some_and(|op| op.background));
        assert!(view.last_fetch > opened);

        // Off when the interval is 0.
        let mut app = app_with_remote();
        app.settings.general.auto_fetch_minutes = 0;
        let _ = app.auto_fetch(later + std::time::Duration::from_secs(3600));
        assert!(app.tabs[0].view().unwrap().busy.is_none());
    }

    #[test]
    fn auto_fetch_errors_are_quiet_and_shown_once() {
        let mut app = app_with_remote();
        let tab = app.tabs[0].id;
        let fail = |app: &mut App| {
            let _ = app.fetch(tab, true);
            let _ = app.update(Message::WriteFinished {
                tab,
                label: "Auto-fetch".into(),
                result: Err(
                    "fatal: unable to access 'https://x/': Could not resolve host: x".into(),
                ),
            });
        };
        fail(&mut app);
        assert!(app.modal.is_none(), "no dialog for auto-fetch");
        assert!(app
            .status
            .starts_with("Auto-fetch failed: fatal: unable to access"));
        app.status.clear();
        fail(&mut app);
        assert!(app.status.is_empty(), "the same failure stays quiet");
    }

    /// Remote app whose `main` tracks `origin/main`.
    fn app_with_upstream() -> App {
        let mut app = app_with_remote();
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.head = git_core::Head::Branch {
            name: "main".into(),
            target: "c".into(),
        };
        snapshot.refs.push(git_core::Reference {
            name: "main".into(),
            kind: git_core::RefKind::LocalBranch,
            target: "c".into(),
            is_head: true,
            upstream: Some("origin/main".into()),
            ahead_behind: Some((0, 1)),
        });
        view.snapshot = Arc::new(snapshot);
        app
    }

    #[test]
    fn pull_uses_the_chosen_mode_and_remembers_it() {
        let mut app = app_with_upstream();
        let _ = app.update(Message::Pull(Some(git_core::PullMode::Rebase)));
        let view = app.tabs[0].view().unwrap();
        assert_eq!(view.busy.as_deref(), Some("Pull (rebase)"));
        assert_eq!(app.pull_mode, git_core::PullMode::Rebase);
        assert_eq!(
            app.current_session().pull_mode,
            crate::session::PullModeSetting::Rebase,
            "persisted"
        );
    }

    #[test]
    fn pull_without_upstream_explains_why() {
        let mut app = app_with_remote();
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.head = git_core::Head::Branch {
            name: "topic".into(),
            target: "c".into(),
        };
        view.snapshot = Arc::new(snapshot);
        let _ = app.update(Message::Pull(None));
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        let modal = app.modal.unwrap();
        assert_eq!(modal.title, "Cannot pull");
        assert!(modal.text.contains("no upstream"));
    }

    #[test]
    fn failed_pull_shows_conflict_or_error_after_refresh() {
        for (state, conflict) in [
            (
                git_core::RepoState::Merging {
                    merge_head: "x".into(),
                },
                true,
            ),
            (git_core::RepoState::Clean, false),
        ] {
            let mut app = app_with_upstream();
            let tab = app.tabs[0].id;
            let _ = app.update(Message::Pull(None));
            let _ = app.update(Message::WriteFinished {
                tab,
                label: "Pull (fast-forward if possible)".into(),
                result: Err("git pull failed".into()),
            });
            assert!(app.modal.is_none(), "decided after the refresh");

            let mut snapshot = (*app.tabs[0].view().unwrap().snapshot).clone();
            snapshot.state = state;
            let generation = app.tabs[0].generation;
            let _ = app.update(Message::SnapshotLoaded {
                tab,
                generation,
                result: Ok(Arc::new(snapshot)),
            });
            let modal = app.modal.clone().unwrap();
            if conflict {
                assert_eq!(modal.title, "Pull stopped with conflicts");
                assert!(matches!(modal.action(1), Some(Message::AbortOperation)));
                assert!(modal.default_action().is_none(), "Enter only closes");
            } else {
                assert_eq!(modal.title, "Pull failed");
            }
        }
    }

    fn running_args(app: &App) -> Vec<String> {
        // The request is consumed by the task; the label tells what started.
        app.tabs[0]
            .view()
            .unwrap()
            .busy
            .clone()
            .into_iter()
            .collect()
    }

    #[test]
    fn push_goes_to_the_upstream() {
        let mut app = app_with_upstream();
        let _ = app.update(Message::Push { force: false });
        assert_eq!(running_args(&app), ["Push"]);
        assert!(app.form.is_none());
    }

    #[test]
    fn first_push_asks_for_remote_and_branch() {
        let mut app = app_with_remote();
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.head = git_core::Head::Branch {
            name: "topic".into(),
            target: "c".into(),
        };
        snapshot.remotes = vec!["backup".into(), "origin".into()];
        view.snapshot = Arc::new(snapshot);

        let _ = app.update(Message::Push { force: false });
        let form = app.form.clone().expect("push form");
        assert_eq!(form.value(0), "origin", "origin preferred");
        assert_eq!(form.value(1), "topic");

        let _ = app.update(Message::FormInput {
            field: 1,
            value: "  ".into(),
        });
        let _ = app.update(Message::FormSubmit);
        assert!(
            app.form.as_ref().unwrap().error.is_some(),
            "empty name refused"
        );
        let _ = app.update(Message::FormInput {
            field: 1,
            value: "feature/topic".into(),
        });
        let _ = app.update(Message::FormSubmit);
        assert!(app.form.is_none());
        assert_eq!(running_args(&app), ["Push"]);
    }

    #[test]
    fn rejected_push_offers_pull_or_force_push() {
        let mut app = app_with_upstream();
        let tab = app.tabs[0].id;
        let _ = app.update(Message::Push { force: false });
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Push".into(),
            result: Err(" ! [rejected]        main -> main (fetch first)".into()),
        });
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Push rejected");
        assert!(matches!(modal.default_action(), Some(Message::Pull(None))));
        assert!(matches!(modal.action(1), Some(Message::ForcePushRequested)));

        let _ = app.update(Message::ModalAction(1));
        let confirm = app.modal.clone().unwrap();
        assert_eq!(confirm.title, "Force push");
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(running_args(&app), ["Force push"]);

        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Force push".into(),
            result: Err(" ! [rejected]        main -> main (stale info)".into()),
        });
        assert_eq!(app.modal.unwrap().title, "Force push refused");
    }

    /// Upstream app with an extra local branch `topic` and remote branch `origin/feature`.
    fn checkout_app() -> App {
        let mut app = app_with_upstream();
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        for (name, kind) in [
            ("topic", git_core::RefKind::LocalBranch),
            ("origin/topic", git_core::RefKind::RemoteBranch),
            ("origin/feature", git_core::RefKind::RemoteBranch),
        ] {
            snapshot.refs.push(git_core::Reference {
                name: name.into(),
                kind,
                target: "c".into(),
                is_head: false,
                upstream: None,
                ahead_behind: None,
            });
        }
        view.snapshot = Arc::new(snapshot);
        app
    }

    #[test]
    fn checkout_targets() {
        let mut app = checkout_app();
        let _ = app.update(Message::Checkout(CheckoutTarget::Local("main".into())));
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert_eq!(app.status, "Already on main");

        let _ = app.update(Message::Checkout(CheckoutTarget::Local("topic".into())));
        assert_eq!(running_args(&app), ["Checkout topic"]);

        // A remote branch whose local branch exists switches to that branch.
        let mut app = checkout_app();
        let _ = app.update(Message::Checkout(CheckoutTarget::Remote(
            "origin/topic".into(),
        )));
        assert_eq!(running_args(&app), ["Checkout topic"]);

        let mut app = checkout_app();
        let _ = app.update(Message::Checkout(CheckoutTarget::Remote(
            "origin/feature".into(),
        )));
        assert_eq!(running_args(&app), ["Checkout origin/feature"]);
    }

    #[test]
    fn commit_checkout_asks_first() {
        let mut app = checkout_app();
        let _ = app.update(Message::CommitContextMenu {
            id: "c".into(),
            position: iced::Point::ORIGIN,
        });
        let menu = app.context_menu.clone().unwrap();
        let _ = app.update(Message::MenuSelected(0));
        assert!(matches!(
            menu.select(0),
            Some(Message::Checkout(CheckoutTarget::Commit(id))) if id == "c"
        ));
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Checkout commit");
        assert!(modal.text.contains("detached"));
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(running_args(&app), ["Checkout c"]);
    }

    #[test]
    fn blocked_checkout_offers_stash_and_checkout() {
        let mut app = checkout_app();
        let tab = app.tabs[0].id;
        let _ = app.update(Message::Checkout(CheckoutTarget::Local("topic".into())));
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Checkout topic".into(),
            result: Err("error: Your local changes to the following files would be overwritten by checkout:\n\ta.txt".into()),
        });
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Local changes block the checkout");
        assert!(matches!(
            modal.default_action(),
            Some(Message::StashAndCheckout(CheckoutTarget::Local(name))) if name == "topic"
        ));
        let _ = app.update(Message::Shortcut(Shortcut::Confirm));
        assert_eq!(running_args(&app), ["Checkout topic"]);
    }

    #[test]
    fn new_branch_input_validates_and_creates() {
        let mut app = checkout_app();
        let tab = app.tabs[0].id;
        let _ = app.update(Message::NewBranchStart(Some("c".into())));
        let input = app.tabs[0].view().unwrap().new_branch.clone().unwrap();
        assert_eq!(
            (input.start.as_str(), input.row.as_deref()),
            ("c", Some("c"))
        );

        let _ = app.update(Message::NewBranchInput("bad name".into()));
        assert!(app.tabs[0]
            .view()
            .unwrap()
            .new_branch
            .as_ref()
            .unwrap()
            .error
            .is_some());
        let _ = app.update(Message::NewBranchInput("topic".into()));
        let _ = app.update(Message::NewBranchSubmit);
        let view = app.tabs[0].view().unwrap();
        assert!(view.busy.is_none(), "duplicate refused");
        assert!(view
            .new_branch
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("already exists"));

        let _ = app.update(Message::NewBranchInput("feature/x".into()));
        let _ = app.update(Message::NewBranchSubmit);
        assert_eq!(running_args(&app), ["Create branch feature/x"]);

        // Git's error stays at the input instead of a dialog.
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Create branch feature/x".into(),
            result: Err("fatal: cannot lock ref".into()),
        });
        assert!(app.modal.is_none());
        let input = app.tabs[0].view().unwrap().new_branch.clone().unwrap();
        assert_eq!(input.error.as_deref(), Some("fatal: cannot lock ref"));

        let _ = app.update(Message::NewBranchSubmit);
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Create branch feature/x".into(),
            result: Ok(None),
        });
        assert!(
            app.tabs[0].view().unwrap().new_branch.is_none(),
            "closed on success"
        );
    }

    #[test]
    fn new_branch_from_wip_starts_at_head_and_esc_cancels() {
        let mut app = checkout_app();
        let view = app.tabs[0].view_mut().unwrap();
        view.snapshot = Arc::new(
            view.snapshot
                .with_working_tree(git_core::WorkingTree {
                    unstaged: vec![file("a")],
                    staged: Vec::new(),
                })
                .unwrap(),
        );
        view.selection = Some(Selection::Wip);
        let _ = app.update(checked(Shortcut::CreateBranch, false));
        let input = app.tabs[0].view().unwrap().new_branch.clone().unwrap();
        assert_eq!((input.start.as_str(), input.row), ("c", None));

        let _ = app.update(Message::Shortcut(Shortcut::Close));
        assert!(app.tabs[0].view().unwrap().new_branch.is_none());
    }

    /// Message of the enabled menu item labelled `label`.
    fn select_label(menu: &ContextMenu, label: &str) -> Option<Message> {
        let index = menu
            .entries
            .iter()
            .position(|e| matches!(e, MenuEntry::Item { label: l, .. } if l == label))?;
        menu.select(index)
    }

    fn branch_menu(app: &mut App, name: &str, kind: git_core::RefKind) -> ContextMenu {
        let _ = app.update(Message::BranchContextMenu {
            name: name.into(),
            kind,
            position: iced::Point::ORIGIN,
        });
        app.context_menu.take().unwrap()
    }

    #[test]
    fn branch_menu_rules() {
        let mut app = checkout_app();
        let head = branch_menu(&mut app, "main", git_core::RefKind::LocalBranch);
        assert!(
            select_label(&head, "Delete").is_none(),
            "the checked-out branch cannot be deleted"
        );
        let other = branch_menu(&mut app, "topic", git_core::RefKind::LocalBranch);
        assert!(
            matches!(select_label(&other, "Delete"), Some(Message::DeleteBranch(n)) if n == "topic")
        );
        let remote = branch_menu(&mut app, "origin/feature", git_core::RefKind::RemoteBranch);
        assert!(
            matches!(select_label(&remote, "Delete on remote"), Some(Message::DeleteRemoteBranch(n)) if n == "origin/feature")
        );
    }

    #[test]
    fn rename_with_upstream_can_rename_the_remote_branch_too() {
        let mut app = checkout_app();
        let tab = app.tabs[0].id;
        let _ = app.update(Message::RenameStart("main".into()));
        let _ = app.update(Message::RenameInput("topic".into()));
        assert!(
            app.tabs[0]
                .view()
                .unwrap()
                .rename
                .as_ref()
                .unwrap()
                .error
                .is_some(),
            "taken"
        );
        let _ = app.update(Message::RenameInput("trunk".into()));
        let _ = app.update(Message::RenameSubmit);
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Rename branch");
        let _ = app.update(Message::ModalAction(2));
        assert_eq!(running_args(&app), ["Rename branch main"]);

        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Rename branch main".into(),
            result: Ok(None),
        });
        let view = app.tabs[0].view().unwrap();
        assert!(view.rename.is_none());
        assert_eq!(
            view.busy.as_deref(),
            Some("Rename origin/main"),
            "remote rename follows"
        );
    }

    #[test]
    fn rename_without_upstream_runs_directly() {
        let mut app = checkout_app();
        let _ = app.update(Message::RenameStart("topic".into()));
        let _ = app.update(Message::RenameInput("topic2".into()));
        let _ = app.update(Message::RenameSubmit);
        assert!(app.modal.is_none());
        assert_eq!(running_args(&app), ["Rename branch topic"]);
    }

    #[test]
    fn unmerged_delete_offers_force_delete() {
        let mut app = checkout_app();
        let tab = app.tabs[0].id;
        let _ = app.update(Message::DeleteBranch("topic".into()));
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(running_args(&app), ["Delete branch topic"]);
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Delete branch topic".into(),
            result: Err("error: the branch 'topic' is not fully merged".into()),
        });
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Branch not merged");
        assert!(modal.default_action().is_none(), "Enter only closes");
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(running_args(&app), ["Delete branch topic"]);
        assert!(
            app.tabs[0].view().unwrap().pending_delete.is_none(),
            "forced"
        );
    }

    #[test]
    fn remote_branch_delete_asks_first() {
        let mut app = checkout_app();
        let _ = app.update(Message::DeleteRemoteBranch("origin/feature".into()));
        assert_eq!(app.modal.clone().unwrap().title, "Delete remote branch");
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(running_args(&app), ["Delete origin/feature"]);
    }

    /// Checkout app with remotes and an existing tag `v1`.
    fn tag_app(remotes: &[&str]) -> App {
        let mut app = checkout_app();
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.remotes = remotes.iter().map(|r| r.to_string()).collect();
        snapshot.refs.push(git_core::Reference {
            name: "v1".into(),
            kind: git_core::RefKind::Tag,
            target: "c".into(),
            is_head: false,
            upstream: None,
            ahead_behind: None,
        });
        view.snapshot = Arc::new(snapshot);
        app
    }

    #[test]
    fn create_tag_form_validates() {
        let mut app = tag_app(&["origin"]);
        let _ = app.update(Message::CreateTagStart("c".into()));
        let input = |app: &mut App, field, value: &str| {
            let _ = app.update(Message::FormInput {
                field,
                value: value.into(),
            });
        };
        input(&mut app, 0, "bad name");
        let _ = app.update(Message::FormSubmit);
        assert!(app
            .form
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .starts_with("Tag names"));
        input(&mut app, 0, "v1");
        let _ = app.update(Message::FormSubmit);
        assert!(app
            .form
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("already exists"));
        input(&mut app, 0, "v2");
        input(&mut app, 1, "Release 2");
        let _ = app.update(Message::FormSubmit);
        assert!(app.form.is_none());
        assert_eq!(running_args(&app), ["Create tag v2"]);
    }

    #[test]
    fn tag_menu_and_remote_actions() {
        let mut app = tag_app(&["origin"]);
        let menu = branch_menu(&mut app, "v1", git_core::RefKind::Tag);
        assert!(matches!(menu.select(0), Some(Message::PushTag(t)) if t == "v1"));
        assert!(matches!(menu.select(4), Some(Message::CopyToClipboard(t)) if t == "v1"));

        // One remote: push runs, remote delete asks.
        let _ = app.update(Message::PushTag("v1".into()));
        assert_eq!(running_args(&app), ["Push tag v1"]);
        let mut app = tag_app(&["origin"]);
        let _ = app.update(Message::DeleteRemoteTag("v1".into()));
        assert_eq!(app.modal.clone().unwrap().title, "Delete remote tag");
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(running_args(&app), ["Delete tag v1 on origin"]);

        // Several remotes: a form asks which one.
        let mut app = tag_app(&["backup", "origin"]);
        let _ = app.update(Message::PushTag("v1".into()));
        assert_eq!(app.form.as_ref().unwrap().value(0), "origin");
        let _ = app.update(Message::FormInput {
            field: 0,
            value: "backup".into(),
        });
        let _ = app.update(Message::FormSubmit);
        assert_eq!(running_args(&app), ["Push tag v1"]);
    }

    fn form_input(app: &mut App, field: usize, value: &str) {
        let _ = app.update(Message::FormInput {
            field,
            value: value.into(),
        });
    }

    #[test]
    fn set_and_unset_upstream() {
        let mut app = tag_app(&["origin"]);
        let menu = branch_menu(&mut app, "topic", git_core::RefKind::LocalBranch);
        assert!(
            select_label(&menu, "Unset upstream").is_none(),
            "topic has no upstream to unset"
        );
        let main = branch_menu(&mut app, "main", git_core::RefKind::LocalBranch);
        assert!(
            matches!(select_label(&main, "Unset upstream"), Some(Message::UnsetUpstream(b)) if b == "main")
        );

        let _ = app.update(Message::SetUpstreamStart("topic".into()));
        let form = app.form.clone().unwrap();
        assert_eq!(
            form.value(0),
            "origin/topic",
            "same-named remote branch preferred"
        );
        let _ = app.update(Message::FormClosed);

        // A gone upstream is not offered as default.
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        for r in &mut snapshot.refs {
            if r.name == "main" {
                r.upstream = Some("origin/gone".into());
            }
        }
        view.snapshot = Arc::new(snapshot);
        let _ = app.update(Message::SetUpstreamStart("main".into()));
        assert_eq!(app.form.clone().unwrap().value(0), "origin/main");
        let _ = app.update(Message::FormClosed);
        let _ = app.update(Message::SetUpstreamStart("topic".into()));
        let _ = app.update(Message::FormSubmit);
        assert_eq!(running_args(&app), ["Set upstream of topic"]);
    }

    #[test]
    fn pull_without_upstream_offers_set_upstream() {
        let mut app = tag_app(&["origin"]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.head = git_core::Head::Branch {
            name: "topic".into(),
            target: "c".into(),
        };
        view.snapshot = Arc::new(snapshot);
        let _ = app.update(Message::Pull(None));
        let modal = app.modal.clone().unwrap();
        assert!(
            matches!(modal.default_action(), Some(Message::SetUpstreamStart(b)) if b == "topic")
        );
    }

    #[test]
    fn add_remote_validates_name_and_url() {
        let mut app = tag_app(&["origin"]);
        let _ = app.update(Message::AddRemoteStart);
        form_input(&mut app, 0, "origin");
        form_input(&mut app, 1, "https://x/y.git");
        let _ = app.update(Message::FormSubmit);
        assert!(app
            .form
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("already exists"));
        form_input(&mut app, 0, "a/b");
        let _ = app.update(Message::FormSubmit);
        assert!(app
            .form
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("/"));
        form_input(&mut app, 0, "backup");
        form_input(&mut app, 1, " ");
        let _ = app.update(Message::FormSubmit);
        assert!(app
            .form
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("URL"));
        form_input(&mut app, 1, "https://x/y.git");
        let _ = app.update(Message::FormSubmit);
        assert_eq!(running_args(&app), ["Add remote backup"]);
    }

    #[test]
    fn remote_menu_actions() {
        let mut app = tag_app(&["origin"]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.remote_urls = vec![("origin".into(), "https://x/origin.git".into())];
        view.snapshot = Arc::new(snapshot);

        let _ = app.update(Message::RemoteContextMenu {
            name: "origin".into(),
            position: iced::Point::ORIGIN,
        });
        let menu = app.context_menu.take().unwrap();
        assert!(matches!(menu.select(0), Some(Message::FetchRemote(r)) if r == "origin"));
        assert!(matches!(menu.select(4), Some(Message::RemoveRemote(r)) if r == "origin"));

        let _ = app.update(Message::EditRemoteUrl("origin".into()));
        assert_eq!(app.form.as_ref().unwrap().value(0), "https://x/origin.git");
        let _ = app.update(Message::FormClosed);

        let _ = app.update(Message::RemoveRemote("origin".into()));
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(running_args(&app), ["Remove remote origin"]);

        let mut app = tag_app(&["origin"]);
        let _ = app.update(Message::FetchRemote("origin".into()));
        assert_eq!(running_args(&app), ["Fetch origin"]);
    }

    #[test]
    fn pull_and_push_of_other_branches() {
        let mut app = tag_app(&["origin"]);
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        for r in &mut snapshot.refs {
            if r.name == "topic" {
                r.upstream = Some("origin/topic".into());
            }
        }
        view.snapshot = Arc::new(snapshot);
        let menu = branch_menu(&mut app, "topic", git_core::RefKind::LocalBranch);
        assert!(
            matches!(select_label(&menu, "Pull"), Some(Message::PullBranch(b)) if b == "topic")
        );

        let _ = app.update(Message::PullBranch("topic".into()));
        assert_eq!(running_args(&app), ["Pull topic"]);
        let mut app = tag_app(&["origin"]);
        let _ = app.update(Message::PushBranch("topic".into()));
        assert!(app.form.is_some(), "no upstream: push form");
    }

    fn checkout_change() -> UndoAction {
        UndoAction::Ref(git_core::RefChange::Checkout {
            from: git_core::HeadPosition::Branch("main".into()),
            to: git_core::HeadPosition::Branch("topic".into()),
        })
    }

    #[test]
    fn ref_changes_are_undoable() {
        let mut app = checkout_app();
        let tab = app.tabs[0].id;
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Checkout topic".into(),
            result: Ok(Some(History::Push(checkout_change()))),
        });
        assert_eq!(app.tabs[0].view().unwrap().undo, [checkout_change()]);
        let _ = app.update(Message::Undo);
        assert_eq!(running_args(&app), ["Undo Checkout topic"]);
    }

    #[test]
    fn checkout_undo_is_blocked_during_a_merge() {
        let mut app = app_in_state(merging());
        app.tabs[0].view_mut().unwrap().undo.push(checkout_change());
        let _ = app.update(Message::Undo);
        assert!(app.tabs[0].view().unwrap().busy.is_none());
        assert!(app.status.contains("merge"));
    }

    #[test]
    fn network_operations_clear_the_redo_stack() {
        let mut app = app_with_upstream();
        let tab = app.tabs[0].id;
        app.tabs[0].view_mut().unwrap().redo.push(checkout_change());
        let _ = app.update(Message::Push { force: false });
        let _ = app.update(Message::WriteFinished {
            tab,
            label: "Push".into(),
            result: Ok(None),
        });
        assert!(app.tabs[0].view().unwrap().redo.is_empty());
    }

    /// Checkout app with HEAD detached at "c".
    fn detached_app() -> App {
        let mut app = checkout_app();
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.head = git_core::Head::Detached { target: "c".into() };
        view.snapshot = Arc::new(snapshot);
        app
    }

    #[test]
    fn leaving_detached_head_checks_for_lost_commits() {
        let mut app = detached_app();
        let tab = app.tabs[0].id;
        let target = CheckoutTarget::Local("topic".into());
        let _ = app.update(Message::Checkout(target.clone()));
        assert!(app.tabs[0].view().unwrap().busy.is_none(), "checked first");

        let _ = app.update(Message::DetachedChecked {
            tab,
            target: target.clone(),
            result: Ok(vec!["d2".into(), "d1".into()]),
        });
        let modal = app.modal.clone().unwrap();
        assert_eq!(modal.title, "Leave detached HEAD");
        assert!(modal.text.starts_with("2 commits are"));
        assert!(
            matches!(modal.default_action(), Some(Message::NewBranchStart(Some(h))) if h == "c")
        );
        let _ = app.update(Message::ModalAction(1));
        assert_eq!(running_args(&app), ["Checkout topic"], "leave anyway");

        // Nothing would be lost: checkout right away.
        let mut app = detached_app();
        let _ = app.update(Message::DetachedChecked {
            tab,
            target,
            result: Ok(Vec::new()),
        });
        assert!(app.modal.is_none());
        assert_eq!(running_args(&app), ["Checkout topic"]);
    }

    #[test]
    fn one_timer_for_the_next_auto_fetch() {
        let mut app = app_with_remote();
        let opened = app.tabs[0].view().unwrap().last_fetch;
        let _ = app.update(Message::Refresh);
        let due = opened + std::time::Duration::from_secs(5 * 60);
        assert_eq!(app.auto_fetch_at, Some(due));
        let seq = app.auto_fetch_seq;

        // Further messages keep the pending timer.
        let _ = app.update(Message::Refresh);
        assert_eq!(app.auto_fetch_seq, seq);
        // A stale timer is ignored.
        let _ = app.update(Message::AutoFetchDue(seq - 1));
        assert!(app.tabs[0].view().unwrap().busy.is_none());

        // Off: no timer.
        app.settings.general.auto_fetch_minutes = 0;
        let _ = app.update(Message::Refresh);
        assert_eq!(app.auto_fetch_at, None);
    }

    fn multi_ids(app: &App) -> Vec<String> {
        app.tabs[0].view().unwrap().multi.ids().to_vec()
    }

    fn click_with(app: &mut App, id: &str, modifiers: keyboard::Modifiers) {
        let _ = app.update(Message::ModifiersChanged(modifiers));
        let _ = app.update(Message::CommitSelected(id.into()));
        let _ = app.update(Message::ModifiersChanged(keyboard::Modifiers::empty()));
    }

    #[test]
    fn ctrl_and_shift_clicks_select_several_commits() {
        let mut app = ready_app(&["e", "d", "c", "b", "a"]);
        click_with(&mut app, "d", keyboard::Modifiers::empty());
        assert_eq!(multi_ids(&app), ["d"]);
        assert!(!app.tabs[0].view().unwrap().is_multi());

        click_with(&mut app, "b", keyboard::Modifiers::COMMAND);
        assert_eq!(multi_ids(&app), ["d", "b"]);
        assert!(app.tabs[0].view().unwrap().is_multi());

        click_with(&mut app, "e", keyboard::Modifiers::SHIFT);
        assert_eq!(
            multi_ids(&app),
            ["e", "d", "c", "b"],
            "range from the anchor b"
        );

        click_with(&mut app, "c", keyboard::Modifiers::empty());
        assert_eq!(multi_ids(&app), ["c"], "plain click resets");
    }

    #[test]
    fn shift_arrows_extend_and_plain_arrows_reset() {
        let mut app = ready_app(&["e", "d", "c", "b", "a"]);
        click_with(&mut app, "d", keyboard::Modifiers::empty());
        let _ = app.update(checked(
            Shortcut::ExtendSelection(crate::nav::Move::Next),
            false,
        ));
        let _ = app.update(checked(
            Shortcut::ExtendSelection(crate::nav::Move::Next),
            false,
        ));
        assert_eq!(multi_ids(&app), ["d", "c", "b"]);
        let _ = app.update(checked(
            Shortcut::ExtendSelection(crate::nav::Move::Previous),
            false,
        ));
        assert_eq!(multi_ids(&app), ["d", "c"]);
        let _ = app.update(checked(Shortcut::Navigate(crate::nav::Move::Next), false));
        assert_eq!(multi_ids(&app), ["b"]);
    }

    #[test]
    fn multi_selection_survives_refresh() {
        let mut app = ready_app(&["e", "d", "c", "b", "a"]);
        click_with(&mut app, "d", keyboard::Modifiers::empty());
        click_with(&mut app, "b", keyboard::Modifiers::SHIFT);
        let view = app.tabs[0].view_mut().unwrap();
        let snapshot = Arc::clone(&view.snapshot);
        view.apply_snapshot(snapshot);
        assert_eq!(multi_ids(&app), ["d", "c", "b"]);
    }

    #[test]
    fn squash_needs_a_contiguous_run_on_head() {
        let app = ready_app(&["e", "d", "c", "b", "a"]);
        let snapshot = &app.tabs[0].view().unwrap().snapshot;
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(crate::repo_view::contiguous_on_head(
            snapshot,
            &ids(&["d", "c"])
        ));
        assert!(crate::repo_view::contiguous_on_head(
            snapshot,
            &ids(&["e", "d", "c"])
        ));
        assert!(
            !crate::repo_view::contiguous_on_head(snapshot, &ids(&["e", "c"])),
            "gap"
        );
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
