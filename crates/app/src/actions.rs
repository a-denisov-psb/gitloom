//! Registry of user actions (M5-01): label, category, context, default
//! shortcut and availability of every action in one table. Keyboard
//! dispatch, the shortcut list (Ctrl+/) and the command palette are built
//! from it. Shortcuts follow `docs/UX-PARITY.md`: Ctrl on Windows/Linux,
//! ⌘ on macOS.

use std::sync::LazyLock;

use iced::keyboard::{key::Named, Key, Modifiers};

use crate::nav::Move;

/// A user action. Existing keyboard handling dispatches on it (also known
/// as `shortcuts::Shortcut`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionId {
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
    /// Zero-based tab index (Ctrl+1 … Ctrl+9).
    SwitchTab(usize),
    OpenRepository,
    Refresh,
    ToggleSidebar,
    TogglePanel,
    Navigate(Move),
    FocusFilter,
    /// S / U: stage or unstage the file whose diff is shown.
    StageSelected,
    UnstageSelected,
    StageAll,
    UnstageAll,
    /// Ctrl+L: fetch all remotes.
    FetchAll,
    /// Shift+↑/↓: extend the commit selection by one row.
    ExtendSelection(Move),
    /// Ctrl+B: new branch at the selected commit.
    CreateBranch,
    /// Ctrl+Z.
    Undo,
    /// Ctrl+Y or Ctrl+Shift+Z.
    Redo,
    /// Ctrl+Enter: commit staged changes.
    Commit,
    /// Ctrl+Shift+Enter: stage everything and commit.
    StageAllAndCommit,
    /// Ctrl+Shift+M: focus the commit summary.
    FocusSummary,
    /// Esc: close the dialog, the menu or the diff view (in that order).
    Close,
    /// Enter: confirm the default button of an open dialog.
    Confirm,
    /// P / R / F / D in the interactive rebase panel (S arrives as
    /// `StageSelected` and means squash there).
    TodoKey(char),
    /// Alt+↑ / Alt+↓: move the selected row of the interactive rebase panel.
    Reorder {
        up: bool,
    },
    /// Ctrl+/: list of all keyboard shortcuts.
    ShortcutList,
    CommandPalette,
    SearchCommits,
    /// Ctrl+Shift+O: palette list of repositories.
    OpenRepoPalette,
    /// Ctrl+Shift+H: palette list of files for the file history.
    FileHistory,
    /// Palette list of branches to check out.
    CheckoutBranch,
    OpenTerminal,
    OpenFileManager,
    ExternalDiff,
    Fullscreen,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Settings,
    /// About dialog (palette and Preferences).
    About,
    /// Toolbar actions in the palette (M6-07: every action by keyboard).
    Pull,
    Push,
    Stash,
    PopStash,
    /// Shift+F10 / Menu key: context menu of the selected commit.
    ContextMenu,
    /// Diff: Hunk / Inline / Split view (M7-01).
    DiffView(crate::settings::DiffMode),
    /// Opens Preferences → Gitflow (M7-07).
    InitializeGitflow,
    /// Start feature / release / hotfix (M7-08).
    FlowStart(git_core::FlowKind),
}

/// Group in the shortcut list; the sections of `docs/UX-PARITY.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    RepoActions,
    Navigation,
    PaletteAndSearch,
    Interface,
    Tabs,
}

impl Category {
    pub const ALL: [Self; 5] = [
        Self::RepoActions,
        Self::Navigation,
        Self::PaletteAndSearch,
        Self::Interface,
        Self::Tabs,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::RepoActions => "Repo actions",
            Self::Navigation => "Navigation",
            Self::PaletteAndSearch => "Command palette and search",
            Self::Interface => "Interface",
            Self::Tabs => "Tabs",
        }
    }
}

/// Where a shortcut applies. A shortcut must be unique within its context;
/// global and text field shortcuts apply everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// Anywhere, but not while a text field has focus.
    Global,
    /// The commit graph.
    Graph,
    /// The staging area and the diff of a working copy file.
    Staging,
    /// Also while a text field has focus.
    TextField,
    /// The interactive rebase panel (P/R/S/F/D, Alt+↑/↓).
    RebasePanel,
}

impl Context {
    #[cfg(test)]
    fn overlaps(self, other: Self) -> bool {
        let everywhere = |c: Self| matches!(c, Self::Global | Self::TextField);
        self == other || everywhere(self) || everywhere(other)
    }
}

/// What an action needs to be enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requires {
    Nothing,
    /// An open repository in the active tab.
    Repository,
}

/// Modifier that has to be held besides Shift and Alt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primary {
    None,
    /// Ctrl on Windows/Linux, ⌘ on macOS.
    Command,
    /// Ctrl on every platform (Ctrl+Tab).
    Control,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordKey {
    /// Lower case character.
    Char(char),
    Named(Named),
}

/// A key with its modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub key: ChordKey,
    pub primary: Primary,
    pub shift: bool,
    pub alt: bool,
    /// Only on macOS (⌘↑ / ⌘↓).
    pub mac_only: bool,
}

const fn chord(key: ChordKey) -> Chord {
    Chord {
        key,
        primary: Primary::None,
        shift: false,
        alt: false,
        mac_only: false,
    }
}

const fn ch(c: char) -> Chord {
    chord(ChordKey::Char(c))
}

const fn named(n: Named) -> Chord {
    chord(ChordKey::Named(n))
}

impl Chord {
    const fn cmd(mut self) -> Self {
        self.primary = Primary::Command;
        self
    }

    const fn ctrl(mut self) -> Self {
        self.primary = Primary::Control;
        self
    }

    const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    const fn mac(mut self) -> Self {
        self.mac_only = true;
        self
    }

    fn matches(&self, key: &Key, modifiers: Modifiers, mac: bool) -> bool {
        if self.mac_only && !mac {
            return false;
        }
        // `Modifiers::command` is fixed at compile time; `mac` keeps the
        // macOS mapping testable on every platform.
        let command = if mac {
            modifiers.logo()
        } else {
            modifiers.control()
        };
        let primary = match self.primary {
            Primary::None => !modifiers.control() && !modifiers.logo(),
            Primary::Command => command,
            Primary::Control => modifiers.control(),
        };
        let key = match (self.key, key.as_ref()) {
            // Upper case is accepted too (e.g. Caps Lock).
            (ChordKey::Char(c), Key::Character(s)) => {
                let mut chars = s.chars().flat_map(char::to_lowercase);
                chars.next() == Some(c) && chars.next().is_none()
            }
            (ChordKey::Named(n), Key::Named(m)) => n == m,
            _ => false,
        };
        key && primary && modifiers.shift() == self.shift && modifiers.alt() == self.alt
    }

    /// Display text: "Ctrl+Shift+S" on Windows/Linux, "⇧⌘S" on macOS.
    pub fn label(&self, mac: bool) -> String {
        let key = match self.key {
            ChordKey::Char(' ') => "Space".to_owned(),
            ChordKey::Char(c) => c.to_uppercase().collect(),
            ChordKey::Named(n) => named_label(n, mac).to_owned(),
        };
        if mac {
            let mut out = String::new();
            if self.primary == Primary::Control {
                out.push('⌃');
            }
            if self.alt {
                out.push('⌥');
            }
            if self.shift {
                out.push('⇧');
            }
            if self.primary == Primary::Command {
                out.push('⌘');
            }
            out + &key
        } else {
            let mut parts = Vec::new();
            if self.primary != Primary::None {
                parts.push("Ctrl");
            }
            if self.shift {
                parts.push("Shift");
            }
            if self.alt {
                parts.push("Alt");
            }
            parts.push(&key);
            parts.join("+")
        }
    }
}

fn named_label(key: Named, mac: bool) -> &'static str {
    match key {
        Named::ArrowUp => "↑",
        Named::ArrowDown => "↓",
        Named::Enter if mac => "↩",
        Named::Enter => "Enter",
        Named::Escape => "Esc",
        Named::Home => "Home",
        Named::End => "End",
        Named::Tab => "Tab",
        Named::F5 => "F5",
        Named::F10 => "F10",
        Named::F11 => "F11",
        Named::ContextMenu => "Menu",
        _ => "?",
    }
}

/// One entry of the registry.
#[derive(Debug, Clone)]
pub struct Action {
    pub id: ActionId,
    pub label: String,
    pub category: Category,
    pub context: Context,
    /// Default shortcuts; the first one is shown in menus.
    pub chords: Vec<Chord>,
    pub requires: Requires,
}

impl Action {
    /// Shortcut text of the first chord for the current platform.
    pub fn shortcut_label(&self, mac: bool) -> Option<String> {
        self.chords
            .iter()
            .find(|c| !c.mac_only || mac)
            .map(|c| c.label(mac))
    }

    /// All shortcuts for the current platform, e.g. "↓, J".
    pub fn shortcut_labels(&self, mac: bool) -> String {
        self.chords
            .iter()
            .filter(|c| !c.mac_only || mac)
            .map(|c| c.label(mac))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// True on macOS (⌘ instead of Ctrl).
pub const MAC: bool = cfg!(target_os = "macos");

static REGISTRY: LazyLock<Vec<Action>> = LazyLock::new(table);

/// All registered actions.
pub fn registry() -> &'static [Action] {
    &REGISTRY
}

/// The registry entry of `id`.
pub fn find(id: ActionId) -> Option<&'static Action> {
    registry().iter().find(|a| a.id == id)
}

impl ActionId {
    /// Shortcut text of the action on this platform, e.g. "Ctrl+B" or "⌘B".
    pub fn shortcut_text(self) -> Option<String> {
        find(self).and_then(|a| a.shortcut_label(MAC))
    }

    /// Shortcuts that still work while a text input has focus
    /// (Enter only acts while a dialog is open).
    pub fn works_in_text_input(self) -> bool {
        find(self).is_some_and(|a| a.context == Context::TextField)
    }
}

/// Maps a key press to an action. `modified_key` (Shift applied) is tried
/// for punctuation the layout only reaches with Shift, e.g. Ctrl+/ on a
/// German keyboard (Ctrl+Shift+7) or Ctrl++.
pub fn action_for_key(key: &Key, modified_key: &Key, modifiers: Modifiers) -> Option<ActionId> {
    lookup(key, modified_key, modifiers, MAC)
}

fn lookup(key: &Key, modified_key: &Key, modifiers: Modifiers, mac: bool) -> Option<ActionId> {
    find_chord(key, modifiers, mac).or_else(|| match modified_key.as_ref() {
        Key::Character(s)
            if modifiers.shift()
                && s.chars().all(|c| !c.is_alphanumeric())
                && modified_key != key =>
        {
            find_chord(modified_key, modifiers.difference(Modifiers::SHIFT), mac)
        }
        _ => None,
    })
}

fn find_chord(key: &Key, modifiers: Modifiers, mac: bool) -> Option<ActionId> {
    // The rebase panel reuses S and the arrows of other contexts; its own
    // entries only answer keys nothing else claims.
    let matching = |panel: bool| {
        registry()
            .iter()
            .filter(move |a| (a.context == Context::RebasePanel) == panel)
            .find(|a| a.chords.iter().any(|c| c.matches(key, modifiers, mac)))
            .map(|a| a.id)
    };
    matching(false).or_else(|| matching(true))
}

fn table() -> Vec<Action> {
    use ActionId as A;
    use Category::*;
    use Context::*;

    let mut actions = Vec::new();
    let mut add = |id, label: &str, category, context, chords: &[Chord]| {
        actions.push(Action {
            id,
            label: label.to_owned(),
            category,
            context,
            chords: chords.to_vec(),
            requires: Requires::Nothing,
        });
    };

    // Repo actions
    add(
        A::CreateBranch,
        "Create branch",
        RepoActions,
        Global,
        &[ch('b').cmd()],
    );
    add(
        A::FetchAll,
        "Fetch all",
        RepoActions,
        Global,
        &[ch('l').cmd()],
    );
    add(
        A::StageSelected,
        "Stage current file",
        RepoActions,
        Staging,
        &[ch('s')],
    );
    add(
        A::StageAll,
        "Stage all files",
        RepoActions,
        Global,
        &[ch('s').cmd().shift()],
    );
    add(
        A::UnstageSelected,
        "Unstage current file",
        RepoActions,
        Staging,
        &[ch('u')],
    );
    add(
        A::UnstageAll,
        "Unstage all files",
        RepoActions,
        Global,
        &[ch('u').cmd().shift()],
    );
    add(
        A::Commit,
        "Commit staged files",
        RepoActions,
        TextField,
        &[named(Named::Enter).cmd()],
    );
    add(
        A::StageAllAndCommit,
        "Stage all and commit",
        RepoActions,
        TextField,
        &[named(Named::Enter).cmd().shift()],
    );
    add(
        A::FocusSummary,
        "Focus commit message",
        RepoActions,
        TextField,
        &[ch('m').cmd().shift()],
    );
    add(
        A::OpenRepository,
        "Open repository…",
        RepoActions,
        Global,
        &[ch('o').cmd()],
    );
    add(
        A::CheckoutBranch,
        "Checkout branch…",
        RepoActions,
        Global,
        &[],
    );
    add(A::Pull, "Pull", RepoActions, Global, &[]);
    add(A::Push, "Push", RepoActions, Global, &[]);
    add(A::Stash, "Stash", RepoActions, Global, &[]);
    add(A::PopStash, "Pop stash", RepoActions, Global, &[]);
    add(
        A::Refresh,
        "Reload repository",
        RepoActions,
        Global,
        &[named(Named::F5)],
    );
    add(
        A::ExternalDiff,
        "Open in external diff/merge tool",
        RepoActions,
        Global,
        &[ch('d').cmd()],
    );
    for (key, label) in [
        ('p', "Interactive rebase: pick"),
        ('r', "Interactive rebase: reword"),
        ('s', "Interactive rebase: squash"),
        ('f', "Interactive rebase: fixup"),
        ('d', "Interactive rebase: drop"),
    ] {
        add(A::TodoKey(key), label, RepoActions, RebasePanel, &[ch(key)]);
    }
    add(
        A::Reorder { up: true },
        "Interactive rebase: move commit up",
        RepoActions,
        RebasePanel,
        &[named(Named::ArrowUp).alt()],
    );
    add(
        A::Reorder { up: false },
        "Interactive rebase: move commit down",
        RepoActions,
        RebasePanel,
        &[named(Named::ArrowDown).alt()],
    );

    // Navigation
    add(
        A::Navigate(Move::Next),
        "Next commit (older)",
        Navigation,
        Graph,
        &[named(Named::ArrowDown), ch('j')],
    );
    add(
        A::Navigate(Move::Previous),
        "Previous commit (newer)",
        Navigation,
        Graph,
        &[named(Named::ArrowUp), ch('k')],
    );
    add(
        A::Navigate(Move::FirstParent),
        "Next commit in branch (first parent)",
        Navigation,
        Graph,
        &[ch('j').shift()],
    );
    add(
        A::Navigate(Move::Child),
        "Previous commit in branch (child)",
        Navigation,
        Graph,
        &[ch('k').shift()],
    );
    add(
        A::Navigate(Move::First),
        "First commit",
        Navigation,
        Graph,
        &[named(Named::Home).cmd(), named(Named::ArrowUp).cmd().mac()],
    );
    add(
        A::Navigate(Move::Last),
        "Last commit",
        Navigation,
        Graph,
        &[named(Named::End).cmd(), named(Named::ArrowDown).cmd().mac()],
    );
    add(
        A::ExtendSelection(Move::Next),
        "Extend selection down",
        Navigation,
        Graph,
        &[named(Named::ArrowDown).shift()],
    );
    add(
        A::ExtendSelection(Move::Previous),
        "Extend selection up",
        Navigation,
        Graph,
        &[named(Named::ArrowUp).shift()],
    );
    add(A::Undo, "Undo", Navigation, Global, &[ch('z').cmd()]);
    add(
        A::Redo,
        "Redo",
        Navigation,
        Global,
        &[ch('y').cmd(), ch('z').cmd().shift()],
    );

    add(
        A::ContextMenu,
        "Open context menu of the selected commit",
        Navigation,
        Global,
        &[named(Named::F10).shift(), named(Named::ContextMenu)],
    );

    // Command palette and search
    add(
        A::CommandPalette,
        "Command palette",
        PaletteAndSearch,
        Global,
        &[ch('p').cmd()],
    );
    add(
        A::SearchCommits,
        "Search commits",
        PaletteAndSearch,
        Global,
        &[ch('f').cmd()],
    );
    add(
        A::OpenRepoPalette,
        "Open recent repository…",
        PaletteAndSearch,
        Global,
        &[ch('o').cmd().shift()],
    );
    add(
        A::FileHistory,
        "File history and blame",
        PaletteAndSearch,
        Global,
        &[ch('h').cmd().shift()],
    );
    add(
        A::FocusFilter,
        "Focus left panel filter",
        PaletteAndSearch,
        Global,
        &[ch('f').cmd().alt()],
    );
    add(
        A::OpenTerminal,
        "Open repository in terminal",
        PaletteAndSearch,
        Global,
        &[ch('t').alt()],
    );
    add(
        A::OpenFileManager,
        "Open repository in file manager",
        PaletteAndSearch,
        Global,
        &[ch('o').alt()],
    );

    add(
        A::InitializeGitflow,
        "Initialize Gitflow",
        RepoActions,
        Global,
        &[],
    );
    for kind in git_core::FlowKind::ALL {
        add(
            A::FlowStart(kind),
            &format!("Start {}", kind.noun()),
            RepoActions,
            Global,
            &[],
        );
    }

    // Interface
    for mode in crate::settings::DiffMode::ALL {
        add(
            A::DiffView(mode),
            &format!("Diff: {} view", mode.label()),
            Interface,
            Global,
            &[],
        );
    }
    add(
        A::Fullscreen,
        "Toggle fullscreen",
        Interface,
        Global,
        &[named(Named::F11)],
    );
    add(
        A::ZoomIn,
        "Zoom in",
        Interface,
        Global,
        &[ch('=').cmd(), ch('+').cmd(), ch('=').cmd().shift()],
    );
    add(A::ZoomOut, "Zoom out", Interface, Global, &[ch('-').cmd()]);
    add(
        A::ZoomReset,
        "Reset zoom",
        Interface,
        Global,
        &[ch('0').cmd()],
    );
    add(
        A::ToggleSidebar,
        "Toggle left panel",
        Interface,
        Global,
        &[ch('j').cmd()],
    );
    add(
        A::TogglePanel,
        "Toggle commit detail panel",
        Interface,
        Global,
        &[ch('k').cmd()],
    );
    add(
        A::Close,
        "Close dialog or current panel",
        Interface,
        TextField,
        &[named(Named::Escape)],
    );
    add(
        A::Confirm,
        "Confirm dialog",
        Interface,
        TextField,
        &[named(Named::Enter)],
    );
    add(
        A::ShortcutList,
        "Keyboard shortcuts",
        Interface,
        Global,
        &[ch('/').cmd()],
    );
    add(
        A::Settings,
        "Preferences",
        Interface,
        Global,
        &[ch(',').cmd()],
    );
    add(
        A::About,
        &format!("About {}", git_core::identity::DISPLAY_NAME),
        Interface,
        Global,
        &[],
    );

    // Tabs
    add(A::NewTab, "New tab", Tabs, Global, &[ch('t').cmd()]);
    add(A::CloseTab, "Close tab", Tabs, Global, &[ch('w').cmd()]);
    for n in 1..=9u8 {
        let digit = char::from(b'0' + n);
        add(
            A::SwitchTab(usize::from(n - 1)),
            &format!("Switch to tab {n}"),
            Tabs,
            Global,
            &[ch(digit).cmd()],
        );
    }
    add(
        A::NextTab,
        "Next tab",
        Tabs,
        Global,
        &[named(Named::Tab).ctrl()],
    );
    add(
        A::PreviousTab,
        "Previous tab",
        Tabs,
        Global,
        &[named(Named::Tab).ctrl().shift()],
    );

    for action in &mut actions {
        if matches!(
            action.id,
            A::FetchAll
                | A::CreateBranch
                | A::StageSelected
                | A::StageAll
                | A::UnstageSelected
                | A::UnstageAll
                | A::Commit
                | A::StageAllAndCommit
                | A::FocusSummary
                | A::Refresh
                | A::Navigate(_)
                | A::ExtendSelection(_)
                | A::Undo
                | A::Redo
                | A::FocusFilter
                | A::CheckoutBranch
                | A::Pull
                | A::Push
                | A::Stash
                | A::PopStash
                | A::ContextMenu
                | A::SearchCommits
                | A::FileHistory
                | A::OpenTerminal
                | A::OpenFileManager
                | A::ExternalDiff
                | A::InitializeGitflow
                | A::FlowStart(_)
        ) {
            action.requires = Requires::Repository;
        }
    }
    actions
}

/// Rows of the shortcut list: actions with a shortcut whose label, category
/// or shortcut text contains `filter` (case-insensitive), grouped by category.
pub fn shortcut_rows(filter: &str, mac: bool) -> Vec<(Category, Vec<&'static Action>)> {
    let filter = filter.trim().to_lowercase();
    Category::ALL
        .into_iter()
        .filter_map(|category| {
            let rows: Vec<_> = registry()
                .iter()
                .filter(|a| a.category == category && a.shortcut_label(mac).is_some())
                .filter(|a| {
                    filter.is_empty()
                        || a.label.to_lowercase().contains(&filter)
                        || category.title().to_lowercase().contains(&filter)
                        || a.shortcut_labels(mac).to_lowercase().contains(&filter)
                })
                .collect();
            (!rows.is_empty()).then_some((category, rows))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: &str) -> Key {
        Key::Character(c.into())
    }

    #[test]
    fn no_duplicate_shortcut_per_context() {
        for mac in [false, true] {
            let chords: Vec<_> = registry()
                .iter()
                .flat_map(|a| a.chords.iter().map(move |c| (a, c)))
                .filter(|(_, c)| !c.mac_only || mac)
                .collect();
            for (i, (a, c)) in chords.iter().enumerate() {
                for (b, d) in &chords[i + 1..] {
                    let same = c.key == d.key
                        && c.shift == d.shift
                        && c.alt == d.alt
                        // Ctrl and ⌘ are the same key except on macOS.
                        && (c.primary == d.primary
                            || (!mac && c.primary != Primary::None && d.primary != Primary::None));
                    assert!(
                        !(same && a.context.overlaps(b.context)),
                        "{} and {} share {} (mac: {mac})",
                        a.label,
                        b.label,
                        c.label(mac)
                    );
                }
            }
        }
    }

    /// The key event a chord stands for.
    fn press(chord: &Chord, mac: bool) -> (Key, Modifiers) {
        let key = match chord.key {
            ChordKey::Char(c) => Key::Character(c.to_string().into()),
            ChordKey::Named(n) => Key::Named(n),
        };
        let mut modifiers = Modifiers::empty();
        match chord.primary {
            Primary::None => {}
            Primary::Command if mac => modifiers |= Modifiers::LOGO,
            Primary::Command | Primary::Control => modifiers |= Modifiers::CTRL,
        }
        if chord.shift {
            modifiers |= Modifiers::SHIFT;
        }
        if chord.alt {
            modifiers |= Modifiers::ALT;
        }
        (key, modifiers)
    }

    #[test]
    fn every_shortcut_dispatches_to_its_action() {
        for mac in [false, true] {
            for action in registry() {
                for chord in action.chords.iter().filter(|c| !c.mac_only || mac) {
                    let (key, modifiers) = press(chord, mac);
                    let expected = match action.id {
                        // S reaches the rebase panel as "stage current file".
                        ActionId::TodoKey('s') => ActionId::StageSelected,
                        id => id,
                    };
                    assert_eq!(
                        lookup(&key, &key, modifiers, mac),
                        Some(expected),
                        "{} ({}, mac: {mac})",
                        action.label,
                        chord.label(mac)
                    );
                }
            }
        }
    }

    #[test]
    fn every_action_is_registered_once() {
        let mut ids: Vec<_> = registry().iter().map(|a| a.id).collect();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count);
        for id in [
            ActionId::ShortcutList,
            ActionId::CommandPalette,
            ActionId::Settings,
            ActionId::SwitchTab(8),
            ActionId::TodoKey('d'),
        ] {
            assert!(find(id).is_some(), "{id:?}");
        }
    }

    #[test]
    fn mac_mapping() {
        let stage_all = ch('s').cmd().shift();
        assert_eq!(stage_all.label(false), "Ctrl+Shift+S");
        assert_eq!(stage_all.label(true), "⇧⌘S");
        assert_eq!(named(Named::Tab).ctrl().label(true), "⌃Tab");
        assert_eq!(ch('t').alt().label(true), "⌥T");
        assert_eq!(ch('t').alt().label(false), "Alt+T");
        assert_eq!(named(Named::Enter).cmd().label(true), "⌘↩");

        let first = find(ActionId::Navigate(Move::First)).map(|a| a.shortcut_labels(true));
        assert_eq!(first.as_deref(), Some("⌘Home, ⌘↑"));
        let first = find(ActionId::Navigate(Move::First)).map(|a| a.shortcut_labels(false));
        assert_eq!(first.as_deref(), Some("Ctrl+Home"));

        // ⌘↑ jumps to the first commit on macOS only.
        let up = Key::Named(Named::ArrowUp);
        assert_eq!(
            lookup(&up, &up, Modifiers::LOGO, true),
            Some(ActionId::Navigate(Move::First))
        );
        assert_eq!(lookup(&up, &up, Modifiers::LOGO, false), None);
        // Ctrl+Tab stays Ctrl on macOS.
        let tab = Key::Named(Named::Tab);
        assert_eq!(
            lookup(&tab, &tab, Modifiers::CTRL, true),
            Some(ActionId::NextTab)
        );
        assert_eq!(
            lookup(&key("b"), &key("b"), Modifiers::LOGO, true),
            Some(ActionId::CreateBranch)
        );
    }

    #[test]
    fn shifted_punctuation_uses_the_modified_key() {
        // German layout: "/" is Shift+7.
        assert_eq!(
            lookup(
                &key("7"),
                &key("/"),
                Modifiers::CTRL | Modifiers::SHIFT,
                false
            ),
            Some(ActionId::ShortcutList)
        );
        // US layout: "+" is Shift+=.
        assert_eq!(
            lookup(
                &key("="),
                &key("+"),
                Modifiers::CTRL | Modifiers::SHIFT,
                false
            ),
            Some(ActionId::ZoomIn)
        );
        // Letters keep their Shift: Shift+S is not S.
        assert_eq!(lookup(&key("s"), &key("S"), Modifiers::SHIFT, false), None);
    }

    #[test]
    fn shortcut_rows_filter_and_group() {
        let all = shortcut_rows("", false);
        assert_eq!(all.len(), Category::ALL.len());
        assert_eq!(all[0].0, Category::RepoActions);
        let fetch = shortcut_rows("fetch", false);
        assert_eq!(fetch.len(), 1);
        assert_eq!(fetch[0].1[0].id, ActionId::FetchAll);
        // By shortcut text.
        let by_key = shortcut_rows("ctrl+l", false);
        assert!(by_key[0].1.iter().any(|a| a.id == ActionId::FetchAll));
        // By category: the whole group.
        let tabs = shortcut_rows("tabs", false);
        assert!(tabs
            .iter()
            .any(|(c, rows)| *c == Category::Tabs && rows.len() == 13));
        assert!(shortcut_rows("no such action", false).is_empty());
    }
}
