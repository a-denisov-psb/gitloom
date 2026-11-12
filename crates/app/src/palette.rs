//! Command palette state (M5-06): a list of entries of one mode (actions,
//! repositories, branches, files), filtered by the fuzzy query. Pure; the
//! app builds the entries and runs the chosen target.

use std::path::PathBuf;

use crate::actions::ActionId;
use crate::app::CheckoutTarget;

/// At most this many matches are shown (the rest is reachable by typing).
pub const MAX_SHOWN: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Actions,
    OpenRepository,
    CheckoutBranch,
    /// Identities for the active repository (M8-04).
    ChooseIdentity,
    /// Tracked files at HEAD, for the file history.
    Files,
    /// Remotes with a website (M12-03).
    RemoteWebsites,
}

impl Mode {
    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Actions => "Type a command",
            Self::OpenRepository => "Open repository: type a name or path",
            Self::CheckoutBranch => "Checkout branch: type a branch name",
            Self::ChooseIdentity => "Choose identity: type a name or email",
            Self::Files => "File history: type a file path",
            Self::RemoteWebsites => "Open remote in browser: type a remote name",
        }
    }
}

/// What an entry does when chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Action(ActionId),
    /// Switch the palette to another list.
    Mode(Mode),
    OpenRepo(PathBuf),
    /// Folder dialog.
    Browse,
    Checkout(CheckoutTarget),
    File(String),
    /// An identity menu entry (M8-04).
    Identity(crate::app::IdentityChoice),
    /// A web page (M12-03).
    Link(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub label: String,
    /// Right-aligned text: shortcut, path or branch kind.
    pub detail: Option<String>,
    /// Why the entry cannot run; it is listed anyway.
    pub disabled: Option<String>,
    pub target: Target,
}

impl Entry {
    pub fn new(label: impl Into<String>, target: Target) -> Self {
        Self {
            label: label.into(),
            detail: None,
            disabled: None,
            target,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn disabled(mut self, reason: Option<String>) -> Self {
        self.disabled = reason;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Palette {
    pub mode: Mode,
    pub query: String,
    entries: Vec<Entry>,
    /// Indices into `entries`, best match first, at most [`MAX_SHOWN`].
    matches: Vec<usize>,
    /// Index into `matches`.
    pub selected: usize,
    /// The list is still being loaded (files).
    pub loading: bool,
    /// The last edit emptied the query: the Backspace that did it must not
    /// also leave the sub-list.
    emptied: bool,
}

impl Palette {
    pub fn new(mode: Mode, entries: Vec<Entry>) -> Self {
        let mut palette = Self {
            mode,
            query: String::new(),
            entries,
            matches: Vec::new(),
            selected: 0,
            loading: false,
            emptied: false,
        };
        palette.refilter();
        palette
    }

    /// A list that is still loading.
    pub fn loading(mode: Mode) -> Self {
        Self {
            loading: true,
            ..Self::new(mode, Vec::new())
        }
    }

    pub fn set_entries(&mut self, entries: Vec<Entry>) {
        self.entries = entries;
        self.loading = false;
        self.refilter();
    }

    pub fn set_query(&mut self, query: String) {
        self.emptied = !self.query.is_empty() && query.is_empty();
        self.query = query;
        self.refilter();
    }

    fn refilter(&mut self) {
        self.matches =
            crate::fuzzy::rank(&self.query, self.entries.iter().map(|e| e.label.as_str()));
        self.matches.truncate(MAX_SHOWN);
        self.selected = 0;
    }

    /// Shown entries, best first.
    pub fn shown(&self) -> impl Iterator<Item = &Entry> {
        self.matches.iter().filter_map(|&i| self.entries.get(i))
    }

    pub fn shown_count(&self) -> usize {
        self.matches.len()
    }

    /// ↑ / ↓ with wrap-around.
    pub fn move_selection(&mut self, down: bool) {
        let count = self.matches.len();
        if count == 0 {
            return;
        }
        self.selected = if down {
            (self.selected + 1) % count
        } else {
            (self.selected + count - 1) % count
        };
    }

    pub fn select(&mut self, index: usize) {
        if index < self.matches.len() {
            self.selected = index;
        }
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.matches
            .get(self.selected)
            .and_then(|&i| self.entries.get(i))
    }

    /// Target of the selected entry, unless it is disabled.
    pub fn chosen(&self) -> Option<Target> {
        self.selected_entry()
            .filter(|e| e.disabled.is_none())
            .map(|e| e.target.clone())
    }

    /// Backspace in the query field: true if it should return to the action
    /// list (sub-list with an empty query that was already empty).
    pub fn leave_on_backspace(&mut self) -> bool {
        if std::mem::take(&mut self.emptied) {
            return false;
        }
        self.query.is_empty() && self.mode != Mode::Actions
    }
}

/// Registered actions with the recently used ones first (most recent first),
/// the rest in registry order.
pub fn order_recent(all: &[ActionId], recent: &[ActionId]) -> Vec<ActionId> {
    let mut ordered: Vec<ActionId> = recent
        .iter()
        .copied()
        .filter(|id| all.contains(id))
        .collect();
    ordered.extend(all.iter().copied().filter(|id| !recent.contains(id)));
    ordered
}

/// Remembers `id` as most recently used (at most `limit` entries).
pub fn touch_recent(recent: &mut Vec<ActionId>, id: ActionId, limit: usize) {
    recent.retain(|r| *r != id);
    recent.insert(0, id);
    recent.truncate(limit);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<Entry> {
        vec![
            Entry::new("Fetch all", Target::Action(ActionId::FetchAll)).detail("Ctrl+L"),
            Entry::new("Create branch", Target::Action(ActionId::CreateBranch)),
            Entry::new("Search commits", Target::Action(ActionId::SearchCommits))
                .disabled(Some("Not available yet".into())),
            Entry::new("Checkout branch…", Target::Mode(Mode::CheckoutBranch)),
        ]
    }

    fn labels(palette: &Palette) -> Vec<&str> {
        palette.shown().map(|e| e.label.as_str()).collect()
    }

    #[test]
    fn filter_ranks_and_resets_the_selection() {
        let mut palette = Palette::new(Mode::Actions, entries());
        assert_eq!(palette.shown_count(), 4);
        palette.move_selection(true);
        palette.set_query("cb".into());
        assert_eq!(labels(&palette), ["Create branch", "Checkout branch…"]);
        assert_eq!(palette.selected, 0);
        palette.set_query("zzz".into());
        assert_eq!(palette.shown_count(), 0);
        assert!(palette.chosen().is_none());
    }

    #[test]
    fn selection_wraps_around() {
        let mut palette = Palette::new(Mode::Actions, entries());
        palette.move_selection(false);
        assert_eq!(palette.selected, 3);
        palette.move_selection(true);
        assert_eq!(palette.selected, 0);
        palette.select(2);
        assert_eq!(palette.selected, 2);
        palette.select(9);
        assert_eq!(palette.selected, 2, "out of range is ignored");
    }

    #[test]
    fn disabled_entries_cannot_run() {
        let mut palette = Palette::new(Mode::Actions, entries());
        palette.set_query("search".into());
        assert_eq!(labels(&palette), ["Search commits"]);
        assert!(palette.chosen().is_none());
        palette.set_query("fetch".into());
        assert_eq!(palette.chosen(), Some(Target::Action(ActionId::FetchAll)));
    }

    #[test]
    fn backspace_on_an_empty_query_leaves_a_sub_list() {
        let mut palette = Palette::new(Mode::CheckoutBranch, entries());
        palette.set_query("m".into());
        assert!(!palette.leave_on_backspace(), "query not empty");
        // The Backspace that empties the query stays in the list …
        palette.set_query(String::new());
        assert!(!palette.leave_on_backspace());
        // … the next one leaves it.
        assert!(palette.leave_on_backspace());
        // The action list itself is never left.
        let mut actions = Palette::new(Mode::Actions, entries());
        assert!(!actions.leave_on_backspace());
    }

    #[test]
    fn loading_list_gets_entries_later() {
        let mut palette = Palette::loading(Mode::Files);
        assert!(palette.loading);
        palette.set_query("r".into());
        palette.set_entries(vec![
            Entry::new("README.md", Target::File("README.md".into())),
            Entry::new("src/main.rs", Target::File("src/main.rs".into())),
        ]);
        assert!(!palette.loading);
        // The query typed while loading applies to the loaded list.
        assert_eq!(labels(&palette), ["README.md", "src/main.rs"]);
    }

    #[test]
    fn recent_actions_come_first() {
        let all = [ActionId::FetchAll, ActionId::CreateBranch, ActionId::Undo];
        let mut recent = Vec::new();
        touch_recent(&mut recent, ActionId::Undo, 10);
        touch_recent(&mut recent, ActionId::CreateBranch, 10);
        touch_recent(&mut recent, ActionId::Undo, 10);
        assert_eq!(recent, [ActionId::Undo, ActionId::CreateBranch]);
        assert_eq!(
            order_recent(&all, &recent),
            [ActionId::Undo, ActionId::CreateBranch, ActionId::FetchAll]
        );
        touch_recent(&mut recent, ActionId::FetchAll, 2);
        assert_eq!(recent.len(), 2);
    }

    /// Acceptance M5-06: filtering 100 000 file paths < 30 ms (release).
    /// `cargo test --release -p gitloom palette_filter_speed -- --ignored --nocapture`
    #[test]
    #[ignore = "timing; run in release"]
    fn palette_filter_speed() {
        let dirs = [
            "crates", "src", "docs", "tests", "assets", "ui", "core", "lib",
        ];
        let entries: Vec<Entry> = (0..100_000)
            .map(|i| {
                let path = format!(
                    "{}/{}/module_{i}/file_{}.rs",
                    dirs[i % dirs.len()],
                    dirs[(i / 7) % dirs.len()],
                    i % 97
                );
                Entry::new(path.clone(), Target::File(path))
            })
            .collect();
        let mut palette = Palette::new(Mode::Files, entries);
        let mut worst = std::time::Duration::ZERO;
        for query in ["m", "mod", "src/ui", "core module 42", "file_96.rs", "xyz"] {
            let start = std::time::Instant::now();
            palette.set_query(query.into());
            let elapsed = start.elapsed();
            eprintln!("{query:>16}: {elapsed:?}, {} shown", palette.shown_count());
            worst = worst.max(elapsed);
        }
        assert!(worst.as_millis() < 30, "worst {worst:?}");
    }

    #[test]
    fn many_entries_are_capped() {
        let entries = (0..1000)
            .map(|i| Entry::new(format!("file{i}.rs"), Target::File(format!("file{i}.rs"))))
            .collect();
        let palette = Palette::new(Mode::Files, entries);
        assert_eq!(palette.shown_count(), MAX_SHOWN);
    }
}
