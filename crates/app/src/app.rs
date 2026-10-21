use std::collections::HashSet;
use std::time::{Duration, Instant};

use git_core::{
    Commit, FileChange, GitBackend, MockBackend, RefKind, Reference, Stash, WorkingTree,
};
use iced::widget::{pane_grid, text_editor};
use iced::{Element, Task};

use crate::ui;

const LOG_LIMIT: usize = 5_000;
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Sidebar,
    Center,
    Panel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Local,
    Remote,
    Tags,
    Stashes,
    Submodules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Wip,
    Commit(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CenterView {
    Graph,
    Diff(DiffTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffTarget {
    pub path: String,
    pub source: DiffSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffSource {
    Unstaged,
    Staged,
    Commit(usize),
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

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(usize),
    TabClosed(usize),
    TabOpened,
    Toolbar(ToolbarAction),
    PickerOpened(Picker),
    PaneResized(pane_grid::ResizeEvent),
    FilterChanged(String),
    SectionToggled(Section),
    /// Single click selects the target commit, double click checks out.
    RefClicked(String),
    StashClicked(usize),
    WipSelected,
    CommitSelected(usize),
    DiffOpened(DiffTarget),
    DiffClosed,
    Stage(String),
    Unstage(String),
    StageAll,
    UnstageAll,
    SummaryChanged(String),
    DescriptionEdited(text_editor::Action),
    CommitRequested,
}

/// One open repository tab.
pub struct Repo {
    pub name: String,
    pub commits: Vec<Commit>,
    pub refs: Vec<Reference>,
    pub stashes: Vec<Stash>,
    pub working_tree: WorkingTree,
    /// Changed files of the selected commit.
    pub commit_changes: Vec<FileChange>,
    pub selection: Selection,
    pub center: CenterView,
    pub filter: String,
    pub collapsed: HashSet<Section>,
    pub summary: String,
    pub description: text_editor::Content,
    backend: Box<dyn GitBackend>,
}

impl Repo {
    pub fn open(backend: Box<dyn GitBackend>) -> git_core::Result<Self> {
        let working_tree = backend.working_tree()?;
        let selection = if working_tree.is_empty() {
            Selection::Commit(0)
        } else {
            Selection::Wip
        };

        let mut repo = Self {
            name: backend.name().to_string(),
            commits: backend.log(LOG_LIMIT)?.commits,
            refs: backend.references()?,
            stashes: backend.stashes()?,
            working_tree,
            commit_changes: Vec::new(),
            selection: Selection::Wip,
            center: CenterView::Graph,
            filter: String::new(),
            collapsed: HashSet::from([Section::Submodules]),
            summary: String::new(),
            description: text_editor::Content::new(),
            backend,
        };
        repo.select(selection);
        Ok(repo)
    }

    pub fn head(&self) -> Option<&Reference> {
        self.refs.iter().find(|r| r.is_head)
    }

    pub fn refs_at<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Reference> + 'a {
        self.refs.iter().filter(move |r| r.target == id)
    }

    fn select(&mut self, selection: Selection) {
        self.selection = selection;
        self.commit_changes = match selection {
            Selection::Commit(index) => self
                .commits
                .get(index)
                .and_then(|c| self.backend.commit_changes(&c.id).ok())
                .unwrap_or_default(),
            Selection::Wip => Vec::new(),
        };
    }

    fn select_ref(&mut self, name: &str) {
        let index = self
            .refs
            .iter()
            .find(|r| r.name == name)
            .and_then(|r| self.commits.iter().position(|c| c.id == r.target));
        if let Some(index) = index {
            self.select(Selection::Commit(index));
        }
    }

    // TODO: route staging through the write backend once it exists.
    fn stage(&mut self, path: &str) {
        move_file(
            &mut self.working_tree.unstaged,
            &mut self.working_tree.staged,
            path,
        );
    }

    fn unstage(&mut self, path: &str) {
        move_file(
            &mut self.working_tree.staged,
            &mut self.working_tree.unstaged,
            path,
        );
    }
}

fn move_file(from: &mut Vec<FileChange>, to: &mut Vec<FileChange>, path: &str) {
    if let Some(pos) = from.iter().position(|f| f.path == path) {
        to.push(from.remove(pos));
        to.sort_by(|a, b| a.path.cmp(&b.path));
    }
}

pub struct App {
    pub repos: Vec<Repo>,
    pub active: usize,
    pub panes: pane_grid::State<PaneKind>,
    pub status: String,
    last_click: Option<(String, Instant)>,
    opened: usize,
}

impl App {
    pub fn new() -> Self {
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
            repos: Vec::new(),
            active: 0,
            panes,
            status: "Ready".into(),
            last_click: None,
            opened: 0,
        };
        app.open_sample();
        app
    }

    pub fn title(&self) -> String {
        match self.repo() {
            Some(repo) => format!("{} - Gitloom", repo.name),
            None => "Gitloom".into(),
        }
    }

    pub fn repo(&self) -> Option<&Repo> {
        self.repos.get(self.active)
    }

    fn repo_mut(&mut self) -> Option<&mut Repo> {
        self.repos.get_mut(self.active)
    }

    // TODO: replace with a folder picker and the real backend.
    fn open_sample(&mut self) {
        self.opened += 1;
        let name = format!("sample-repo-{}", self.opened);
        match Repo::open(Box::new(MockBackend::new(name))) {
            Ok(repo) => {
                self.repos.push(repo);
                self.active = self.repos.len() - 1;
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn is_double_click(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let double = matches!(
            &self.last_click,
            Some((last, at)) if last == key && now.duration_since(*at) < DOUBLE_CLICK
        );
        self.last_click = (!double).then(|| (key.to_string(), now));
        double
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(index) => self.active = index,
            Message::TabClosed(index) => {
                if index < self.repos.len() {
                    self.repos.remove(index);
                    if self.active >= index && self.active > 0 {
                        self.active -= 1;
                    }
                }
            }
            Message::TabOpened => self.open_sample(),
            Message::Toolbar(action) => {
                self.status = format!("{}: not implemented yet", action.label());
            }
            Message::PickerOpened(picker) => {
                self.status = format!("{picker:?} picker: not implemented yet");
            }
            Message::PaneResized(event) => self.panes.resize(event.split, event.ratio),
            Message::FilterChanged(filter) => {
                if let Some(repo) = self.repo_mut() {
                    repo.filter = filter;
                }
            }
            Message::SectionToggled(section) => {
                if let Some(repo) = self.repo_mut() {
                    if !repo.collapsed.remove(&section) {
                        repo.collapsed.insert(section);
                    }
                }
            }
            Message::RefClicked(name) => {
                if self.is_double_click(&name) {
                    let kind = self
                        .repo()
                        .and_then(|r| r.refs.iter().find(|x| x.name == name))
                        .map(|r| r.kind);
                    self.status = match kind {
                        Some(RefKind::RemoteBranch) => {
                            format!("Checkout {name} as local branch: not implemented yet")
                        }
                        _ => format!("Checkout {name}: not implemented yet"),
                    };
                } else if let Some(repo) = self.repo_mut() {
                    repo.center = CenterView::Graph;
                    repo.select_ref(&name);
                }
            }
            Message::StashClicked(index) => {
                self.status = format!("Show stash@{{{index}}}: not implemented yet");
            }
            Message::WipSelected => {
                if let Some(repo) = self.repo_mut() {
                    repo.select(Selection::Wip);
                }
            }
            Message::CommitSelected(index) => {
                if let Some(repo) = self.repo_mut() {
                    repo.select(Selection::Commit(index));
                }
            }
            Message::DiffOpened(target) => {
                if let Some(repo) = self.repo_mut() {
                    repo.center = CenterView::Diff(target);
                }
            }
            Message::DiffClosed => {
                if let Some(repo) = self.repo_mut() {
                    repo.center = CenterView::Graph;
                }
            }
            Message::Stage(path) => {
                if let Some(repo) = self.repo_mut() {
                    repo.stage(&path);
                }
            }
            Message::Unstage(path) => {
                if let Some(repo) = self.repo_mut() {
                    repo.unstage(&path);
                }
            }
            Message::StageAll => {
                if let Some(repo) = self.repo_mut() {
                    let paths: Vec<_> = repo
                        .working_tree
                        .unstaged
                        .iter()
                        .map(|f| f.path.clone())
                        .collect();
                    paths.iter().for_each(|p| repo.stage(p));
                }
            }
            Message::UnstageAll => {
                if let Some(repo) = self.repo_mut() {
                    let paths: Vec<_> = repo
                        .working_tree
                        .staged
                        .iter()
                        .map(|f| f.path.clone())
                        .collect();
                    paths.iter().for_each(|p| repo.unstage(p));
                }
            }
            Message::SummaryChanged(summary) => {
                if let Some(repo) = self.repo_mut() {
                    repo.summary = summary;
                }
            }
            Message::DescriptionEdited(action) => {
                if let Some(repo) = self.repo_mut() {
                    repo.description.perform(action);
                }
            }
            Message::CommitRequested => {
                self.status = "Commit: not implemented yet (write backend pending)".into();
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        ui::view(self)
    }
}
