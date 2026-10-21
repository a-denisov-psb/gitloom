use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use git_core::{DiffRequest, FileChange, GitBackend, GixBackend};
use iced::widget::pane_grid;
use iced::{event, keyboard, window, Element, Event, Subscription, Task};

use crate::repo_view::{CenterView, CommitDetails, Loadable, RepoView, Section, Selection};
use crate::snapshot::RepoSnapshot;
use crate::ui;

/// Window focus events closer together than this trigger one refresh.
const FOCUS_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Sidebar,
    Center,
    Panel,
}

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
    DiffClosed,
    Retry(TabId),
    Refresh,
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
}

pub struct App {
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub panes: pane_grid::State<PaneKind>,
    pub status: String,
    next_tab: u64,
    focus_seq: u64,
}

impl App {
    /// Starts with one tab per path given on the command line, or a start tab.
    pub fn new(paths: Vec<PathBuf>) -> (Self, Task<Message>) {
        use pane_grid::{Axis, Configuration};

        let panes = pane_grid::State::with_configuration(Configuration::Split {
            axis: Axis::Vertical,
            ratio: 0.18,
            a: Box::new(Configuration::Pane(PaneKind::Sidebar)),
            b: Box::new(Configuration::Split {
                axis: Axis::Vertical,
                ratio: 0.72,
                a: Box::new(Configuration::Pane(PaneKind::Center)),
                b: Box::new(Configuration::Pane(PaneKind::Panel)),
            }),
        });

        let mut app = Self {
            tabs: Vec::new(),
            active: 0,
            panes,
            status: String::new(),
            next_tab: 0,
            focus_seq: 0,
        };
        let tasks: Vec<_> = paths
            .into_iter()
            .map(|path| app.open_in_new_tab(path))
            .collect();
        if app.tabs.is_empty() {
            app.add_tab(TabState::Start);
        }
        (app, Task::batch(tasks))
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

    /// Loads the changed files of the selected commit if not loaded yet.
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
        match message {
            Message::TabSelected(id) => {
                if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                    self.active = index;
                }
            }
            Message::TabClosed(id) => {
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
                    return self.select(Selection::Commit(target));
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
                tab.state = match result {
                    Ok(opened) => {
                        TabState::Ready(Box::new(RepoView::new(opened.backend, opened.snapshot)))
                    }
                    Err(error) => TabState::Failed { path, error },
                };
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
        event::listen_with(global_event)
    }

    pub fn view(&self) -> Element<'_, Message> {
        ui::view(self)
    }
}

/// Maps window and keyboard events that no widget handled.
fn global_event(event: Event, status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::F5),
            ..
        }) if status == event::Status::Ignored => Some(Message::Refresh),
        _ => None,
    }
}

/// Runs blocking work (Git, file I/O) on tokio's blocking pool.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| format!("Background task failed: {e}"))?
}
