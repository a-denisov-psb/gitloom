use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use git_core::{DiffRequest, FileChange, GitBackend, GixBackend};
use iced::widget::pane_grid;
use iced::{event, keyboard, window, Element, Event, Subscription, Task};

use crate::panes::Panes;
use crate::repo_view::{CenterView, CommitDetails, Loadable, RepoView, Section, Selection};
use crate::session::{self, Session};
use crate::shortcuts::{self, Shortcut};
use crate::snapshot::RepoSnapshot;
use crate::ui;

/// Window focus events closer together than this trigger one refresh.
const FOCUS_DEBOUNCE: Duration = Duration::from_millis(500);
/// Session changes are written after this quiet period.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    Undo,
    Redo,
    Pull,
    Push,
    Branch,
    Stash,
    Pop,
    Terminal,
}

impl ToolbarAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Pull => "Pull",
            Self::Push => "Push",
            Self::Branch => "Branch",
            Self::Stash => "Stash",
            Self::Pop => "Pop",
            Self::Terminal => "Terminal",
        }
    }

    /// Placeholder glyphs until a proper icon set is added.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Undo => "↶",
            Self::Redo => "↷",
            Self::Pull => "↓",
            Self::Push => "↑",
            Self::Branch => "⑂",
            Self::Stash => "⊕",
            Self::Pop => "⊖",
            Self::Terminal => "›_",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picker {
    Repository,
    Branch,
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
    Toolbar(ToolbarAction),
    PickerOpened(Picker),
    PaneResized(pane_grid::ResizeEvent),
    FilterChanged(String),
    SectionToggled(Section),
    RefClicked(String),
    WipSelected,
    CommitSelected(String),
    DiffOpened(DiffRequest),
    GraphScrolled {
        offset: f32,
        height: f32,
    },
    DiffClosed,
    Retry(TabId),
    Refresh,
    Shortcut(Shortcut),
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
    /// Where the session is stored; `None` disables persistence (tests).
    session_path: Option<PathBuf>,
    /// The session file was read; before that nothing is written.
    session_loaded: bool,
    /// Paths from the command line, opened once the session is restored.
    pending_paths: Vec<PathBuf>,
    save_seq: u64,
    next_tab: u64,
    focus_seq: u64,
    /// Tab whose graph was visible after the last update; a change means the
    /// scrollable was recreated and needs its offset restored.
    shown_graph: Option<TabId>,
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
            session_path: session_path.clone(),
            session_loaded: false,
            pending_paths: paths,
            save_seq: 0,
            next_tab: 0,
            focus_seq: 0,
            shown_graph: None,
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
        (app, task)
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
        // The active tab may have changed or finished loading; details load lazily.
        let details = self.load_details();
        let scroll = self.restore_scroll();
        Task::batch([task, details, scroll, self.persist()])
    }

    /// Re-applies the stored offset when a different graph became visible.
    fn restore_scroll(&mut self) -> Task<Message> {
        let shown = self.active_tab().and_then(|tab| {
            let view = tab.view()?;
            let task = (view.scroll.offset > 0.0).then(|| scroll_to(view, view.scroll.offset));
            matches!(view.center, CenterView::Graph).then_some((tab.id, task))
        });
        let id = shown.as_ref().map(|(id, _)| *id);
        if id == self.shown_graph {
            return Task::none();
        }
        self.shown_graph = id;
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
            Message::Toolbar(action) => {
                self.status = format!("{}: not implemented yet", action.label());
            }
            Message::PickerOpened(picker) => {
                self.status = format!("{picker:?} picker: not implemented yet");
            }
            Message::PaneResized(event) => self.panes.resize(event.split, event.ratio),
            Message::SessionLoaded(loaded) => return self.restore(loaded),
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
            Message::CommitSelected(id) => return self.select(Selection::Commit(id)),
            Message::DiffOpened(request) => {
                if let Some(view) = self.active_view_mut() {
                    view.center =
                        CenterView::Diff(Box::new(crate::repo_view::DiffView { request }));
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
            Message::Shortcut(shortcut) => return self.shortcut(shortcut),
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
                    return self.update(Message::Refresh);
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
                if let Some(view) = tab.view_mut() {
                    match result {
                        Ok(snapshot) => view.apply_snapshot(snapshot),
                        Err(error) => {
                            view.refreshing = false;
                            view.error = Some(error);
                        }
                    }
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
        Subscription::batch([
            event::listen_with(global_event),
            window::close_requests().map(Message::CloseRequested),
        ])
    }

    pub fn view(&self) -> Element<'_, Message> {
        ui::view(self)
    }
}

fn scroll_to(view: &RepoView, offset: f32) -> Task<Message> {
    iced::widget::operation::scroll_to(
        view.graph_scroll_id.clone(),
        iced::widget::operation::AbsoluteOffset {
            x: None,
            y: Some(offset),
        },
    )
}

/// Maps window events and keyboard shortcuts no widget handled.
fn global_event(event: Event, status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
            if status == event::Status::Ignored =>
        {
            shortcuts::shortcut(&key, modifiers).map(Message::Shortcut)
        }
        _ => None,
    }
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
        let _ = app.update(Message::Shortcut(Shortcut::NewTab));
        assert_eq!((app.tabs.len(), app.active), (2, 1));
        let _ = app.update(Message::Shortcut(Shortcut::CloseTab));
        assert_eq!((app.tabs.len(), app.active), (1, 0));
        // Closing the last tab leaves a fresh start tab.
        let _ = app.update(Message::Shortcut(Shortcut::CloseTab));
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
        let _ = app.update(Message::Shortcut(Shortcut::NextTab));
        assert_eq!(app.active, 0);
        let _ = app.update(Message::Shortcut(Shortcut::PreviousTab));
        assert_eq!(app.active, 2);
        let _ = app.update(Message::Shortcut(Shortcut::SwitchTab(1)));
        assert_eq!(app.active, 1);
        let _ = app.update(Message::Shortcut(Shortcut::SwitchTab(7)));
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
            matches!(&app.tabs[0].state, TabState::Loading { path } if path == std::path::Path::new("/some/repo"))
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
        let _ = app.update(Message::Shortcut(Shortcut::NewTab));
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

    #[test]
    fn subfolders_belong_to_the_open_repository() {
        let root = std::path::Path::new("/work/repo");
        assert!(same_repo(root, std::path::Path::new("/work/repo")));
        assert!(same_repo(root, std::path::Path::new("/work/repo/src/ui")));
        assert!(!same_repo(root, std::path::Path::new("/work/repository")));
        assert!(!same_repo(root, std::path::Path::new("/work")));
    }
}
