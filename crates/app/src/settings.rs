//! User preferences in `<config dir>/settings.json`, separate from the
//! session state (ADR-023). Missing or unknown fields fall back to defaults;
//! a broken file is kept as `settings.json.bak`.
//!
//! All functions touching the file system are blocking; call them off the UI thread.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const FILE_NAME: &str = "settings.json";

/// Longest auto-fetch interval accepted in the settings view (one day).
pub const MAX_AUTO_FETCH_MINUTES: u32 = 24 * 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub general: General,
    pub appearance: Appearance,
    pub tools: ExternalTools,
    pub diff: DiffSettings,
    /// Commit identities (M8-03, ADR-041): only name and email are kept
    /// here; repositories store their choice in their own Git config.
    pub identities: Identities,
    /// Preferences → Templates (M10-07, M10-10).
    pub templates: Templates,
    /// Preferences → Editor (M11-06).
    pub editor: EditorSettings,
    /// Sort, Path/Tree and "View all files" of the details' file list (M14-03).
    pub file_list: crate::file_list::FileListOptions,
}

/// Preferences → Editor (M11-06): the code views (diffs, blame, file
/// history, conflict view); the commit message fields are not affected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorSettings {
    /// Font family; empty is "Default" (the platform monospace).
    pub font: String,
    /// 8 … 24.
    pub font_size: u32,
    /// 1 … 8; tabs are expanded for display only.
    pub tab_size: u32,
    pub line_numbers: bool,
    /// Word wrap in the diff views (M11-07).
    pub word_wrap: bool,
    /// Syntax highlighting (M11-08, M11-09).
    pub syntax_highlighting: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font: String::new(),
            font_size: 12,
            tab_size: 4,
            line_numbers: true,
            word_wrap: false,
            syntax_highlighting: true,
        }
    }
}

/// Preferences → Templates (M10-07): texts shared by all repositories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Templates {
    /// Instruction at the top of the commit message prompt.
    pub commit_prompt: String,
    /// "Use stash message template" (M10-10); off: Git's default message.
    pub stash_enabled: bool,
    /// The stash message template (a preset or a custom text).
    pub stash_template: String,
}

impl Default for Templates {
    fn default() -> Self {
        Self {
            commit_prompt: git_core::prompt::DEFAULT_INSTRUCTION.to_owned(),
            stash_enabled: false,
            stash_template: crate::stash_template::PRESETS[0].to_owned(),
        }
    }
}

/// A saved identity (name and email).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedIdentity {
    pub name: String,
    pub email: String,
}

impl SavedIdentity {
    pub fn identity(&self) -> git_core::Identity {
        git_core::Identity {
            name: self.name.clone(),
            email: self.email.clone(),
        }
    }

    pub fn label(&self) -> String {
        self.identity().label()
    }
}

impl From<&git_core::Identity> for SavedIdentity {
    fn from(identity: &git_core::Identity) -> Self {
        Self {
            name: identity.name.clone(),
            email: identity.email.clone(),
        }
    }
}

/// Preferences → Identities (M8-03).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Identities {
    pub list: Vec<SavedIdentity>,
    /// Index of the default identity in `list`.
    pub default: Option<usize>,
    /// "Keep the global Git config updated with the default identity".
    pub sync_global: bool,
}

impl Default for Identities {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            default: None,
            sync_global: true,
        }
    }
}

impl Identities {
    /// The default identity, if any.
    pub fn default_identity(&self) -> Option<&SavedIdentity> {
        self.default.and_then(|index| self.list.get(index))
    }

    /// Why `name`/`email` cannot be saved (as entry `editing`, if given):
    /// invalid, or the same name and email exist already.
    pub fn problem(&self, name: &str, email: &str, editing: Option<usize>) -> Option<String> {
        if let Some(problem) = git_core::identity_problem(name, email) {
            return Some(problem.to_owned());
        }
        let duplicate = self.list.iter().enumerate().any(|(index, saved)| {
            Some(index) != editing && saved.name == name.trim() && saved.email == email.trim()
        });
        duplicate.then(|| "This identity exists already".to_owned())
    }

    /// Removes entry `index`; the default follows its entry (none when the
    /// default itself is removed).
    pub fn remove(&mut self, index: usize) {
        if index >= self.list.len() {
            return;
        }
        self.list.remove(index);
        self.default = match self.default {
            Some(d) if d == index => None,
            Some(d) if d > index => Some(d - 1),
            other => other,
        };
    }

    /// Imports the global identity when the list is empty (first use):
    /// it becomes the first entry and the default. Returns whether it did.
    pub fn import(&mut self, global: &git_core::Identity) -> bool {
        if !self.list.is_empty() {
            return false;
        }
        self.list.push(SavedIdentity::from(global));
        self.default = Some(0);
        true
    }
}

/// Diff view mode (M7-01, ADR-038): one global setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiffMode {
    #[default]
    Hunk,
    Inline,
    Split,
}

impl DiffMode {
    pub const ALL: [Self; 3] = [Self::Hunk, Self::Inline, Self::Split];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hunk => "Hunk",
            Self::Inline => "Inline",
            Self::Split => "Split",
        }
    }

    /// Context the diff of this mode is loaded with.
    pub fn context(self) -> git_core::DiffContext {
        match self {
            Self::Hunk => git_core::DiffContext::Hunks,
            Self::Inline | Self::Split => git_core::DiffContext::Full,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiffSettings {
    pub view_mode: DiffMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: VERSION,
            general: General::default(),
            appearance: Appearance::default(),
            tools: ExternalTools::default(),
            diff: DiffSettings::default(),
            identities: Identities::default(),
            templates: Templates::default(),
            editor: EditorSettings::default(),
            file_list: crate::file_list::FileListOptions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Auto-fetch interval in minutes; 0 turns it off.
    pub auto_fetch_minutes: u32,
    /// Opt-in update check (ADR-030); off by default.
    pub check_updates: bool,
    /// Fetch, auto-fetch and the fetch of pull use `--prune` (GitKraken
    /// "Auto-Prune", M10-08); on by default.
    pub prune_on_fetch: bool,
    /// Repositories inside WSL whose notice was dismissed with "Don't show
    /// again" (M12-14).
    pub wsl_notice_dismissed: Vec<std::path::PathBuf>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            auto_fetch_minutes: crate::fetch::DEFAULT_INTERVAL_MINUTES,
            check_updates: false,
            prune_on_fetch: true,
            wsl_notice_dismissed: Vec::new(),
        }
    }
}

/// `appearance.theme` (ADR-058): a theme id or "system". Stored as one
/// string; old ids are read as their new id (ADR-054, ADR-060).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeChoice {
    /// A theme id of the registry (`crate::themes`); may be unknown.
    Theme(String),
    /// Follow the operating system with `system_dark` / `system_light`.
    System,
}

impl ThemeChoice {
    pub const SYSTEM: &'static str = "system";

    /// A theme by id.
    pub fn theme(id: &str) -> Self {
        Self::Theme(id.to_owned())
    }

    /// The stored string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Theme(id) => id,
            Self::System => Self::SYSTEM,
        }
    }

    /// The choice for a stored string.
    pub fn parse(value: &str) -> Self {
        match value {
            Self::SYSTEM => Self::System,
            id => Self::theme(crate::themes::canonical_id(id)),
        }
    }
}

impl Default for ThemeChoice {
    fn default() -> Self {
        Self::theme(crate::themes::DARK_ID)
    }
}

impl Serialize for ThemeChoice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ThemeChoice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::parse(&value))
    }
}

/// Reads a theme id of `system_dark` / `system_light` through the alias
/// table of old ids (ADR-060).
fn theme_id<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    Ok(crate::themes::canonical_id(&value).to_owned())
}

fn default_system_dark() -> String {
    crate::themes::DARK_ID.to_owned()
}

fn default_system_light() -> String {
    crate::themes::LIGHT_ID.to_owned()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub theme: ThemeChoice,
    /// Theme ids System uses when the operating system reports dark (or
    /// nothing) and light (ADR-058).
    #[serde(deserialize_with = "theme_id")]
    pub system_dark: String,
    #[serde(deserialize_with = "theme_id")]
    pub system_light: String,
    /// Window scale in percent (70–200).
    pub zoom_percent: u32,
    /// Load avatars from Gravatar (ADR-026); off by default.
    pub avatars: bool,
    /// Labels under the toolbar icons (M10-08); on by default.
    pub toolbar_labels: bool,
    /// Widths of the graph table columns (M11-02).
    pub graph_columns: crate::graph_columns::ColumnWidths,
    /// "Show commit author / date and time / SHA in graph" (M11-03).
    pub graph_author: bool,
    pub graph_date: bool,
    pub graph_sha: bool,
    /// Details panel right of the graph or below it (finding #44); set by
    /// the status bar and the palette, not in Preferences.
    pub details_panel: crate::panes::DetailsPanelPosition,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::default(),
            system_dark: default_system_dark(),
            system_light: default_system_light(),
            zoom_percent: ZOOM_DEFAULT,
            avatars: false,
            toolbar_labels: true,
            graph_columns: crate::graph_columns::ColumnWidths::default(),
            graph_author: true,
            graph_date: true,
            graph_sha: true,
            details_panel: crate::panes::DetailsPanelPosition::Right,
        }
    }
}

/// External diff or merge tool.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", from = "ToolChoiceRepr")]
pub enum ToolChoice {
    /// Whatever `diff.tool` / `merge.tool` say.
    #[default]
    GitConfig,
    /// A tool by name, passed as `--tool=<name>`.
    Tool(String),
    None,
    /// An entry of the tool list by its id (M20, ADR-062); its diff or
    /// merge command runs through Git (ADR-055).
    Entry(String),
}

/// How a tool choice is read: the `{"command": …}` selections of settings
/// files before M20 read as "Use Git config" (no migration, ADR-062).
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ToolChoiceRepr {
    GitConfig,
    Tool(String),
    None,
    Entry(String),
    Command(serde::de::IgnoredAny),
}

impl From<ToolChoiceRepr> for ToolChoice {
    fn from(repr: ToolChoiceRepr) -> Self {
        match repr {
            ToolChoiceRepr::GitConfig | ToolChoiceRepr::Command(_) => Self::GitConfig,
            ToolChoiceRepr::Tool(name) => Self::Tool(name),
            ToolChoiceRepr::None => Self::None,
            ToolChoiceRepr::Entry(id) => Self::Entry(id),
        }
    }
}

/// Id of the built-in entry (M20, ADR-062); other entries get `tool-<n>`.
pub const VSCODE_ID: &str = "vscode";
/// Name and commands of the built-in entry: the commands of the former
/// VS Code presets (ADR-055, VS Code docs "merge conflicts").
pub const VSCODE_NAME: &str = "VS Code";
pub const VSCODE_OPEN: &str = "code {path}";
pub const VSCODE_DIFF: &str = "code --wait --diff {local} {remote}";
pub const VSCODE_MERGE: &str = "code --wait --merge {remote} {local} {base} {merged}";

/// What a tool entry is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// External editor (`{path}`).
    Open,
    Diff,
    Merge,
}

impl From<git_core::ToolKind> for Purpose {
    fn from(kind: git_core::ToolKind) -> Self {
        match kind {
            git_core::ToolKind::Diff => Self::Diff,
            git_core::ToolKind::Merge => Self::Merge,
        }
    }
}

/// An entry of the tool list (Preferences → External tools → Manage
/// tools…, M20, ADR-062). An empty command means "not for this purpose".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolEntry {
    /// Stable, never shown: [`VSCODE_ID`] or `tool-<n>`.
    pub id: String,
    pub name: String,
    /// Editor command with `{path}` (appended when missing).
    pub open: String,
    /// Diff command with `{local}` and `{remote}`.
    pub diff: String,
    /// Merge command with `{merged}`, optionally `{local}`, `{remote}`,
    /// `{base}`.
    pub merge: String,
}

impl ToolEntry {
    /// The built-in VS Code entry.
    pub fn vscode() -> Self {
        Self {
            id: VSCODE_ID.to_owned(),
            name: VSCODE_NAME.to_owned(),
            open: VSCODE_OPEN.to_owned(),
            diff: VSCODE_DIFF.to_owned(),
            merge: VSCODE_MERGE.to_owned(),
        }
    }

    pub fn command(&self, purpose: Purpose) -> &str {
        match purpose {
            Purpose::Open => &self.open,
            Purpose::Diff => &self.diff,
            Purpose::Merge => &self.merge,
        }
    }

    pub fn command_mut(&mut self, purpose: Purpose) -> &mut String {
        match purpose {
            Purpose::Open => &mut self.open,
            Purpose::Diff => &mut self.diff,
            Purpose::Merge => &mut self.merge,
        }
    }
}

/// Why fields of a tool entry are not valid (shown under the field).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryProblems {
    pub name: Option<&'static str>,
    pub open: Option<&'static str>,
    pub diff: Option<&'static str>,
    pub merge: Option<&'static str>,
}

impl EntryProblems {
    pub fn any(&self) -> bool {
        *self != Self::default()
    }

    pub fn command(&self, purpose: Purpose) -> Option<&'static str> {
        match purpose {
            Purpose::Open => self.open,
            Purpose::Diff => self.diff,
            Purpose::Merge => self.merge,
        }
    }
}

pub const NAME_EMPTY: &str = "Enter a name";
pub const NAME_TAKEN: &str = "Another tool has this name";
pub const UNCLOSED_QUOTE: &str = "A quote is not closed";
pub const DIFF_PLACEHOLDERS: &str = "The diff command needs {local} and {remote}";
pub const MERGE_PLACEHOLDER: &str = "The merge command needs {merged}";

/// Problems of a command for `purpose`; an empty command has none.
fn command_problem(command: &str, purpose: Purpose) -> Option<&'static str> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    if crate::launch::split_words_for(crate::launch::Platform::current(), command).is_err() {
        return Some(UNCLOSED_QUOTE);
    }
    match purpose {
        Purpose::Open => None,
        Purpose::Diff => (!command.contains("{local}") || !command.contains("{remote}"))
            .then_some(DIFF_PLACEHOLDERS),
        Purpose::Merge => (!command.contains("{merged}")).then_some(MERGE_PLACEHOLDER),
    }
}

/// Validation of entry `index` (ADR-062): name empty or taken
/// (case-insensitive), an unclosed quote, a diff command without
/// `{local}` or `{remote}`, a merge command without `{merged}`.
pub fn entry_problems(entries: &[ToolEntry], index: usize) -> EntryProblems {
    let Some(entry) = entries.get(index) else {
        return EntryProblems::default();
    };
    let name = entry.name.trim();
    let taken = entries
        .iter()
        .enumerate()
        .any(|(i, other)| i != index && other.name.trim().to_lowercase() == name.to_lowercase());
    EntryProblems {
        name: if name.is_empty() {
            Some(NAME_EMPTY)
        } else if taken {
            Some(NAME_TAKEN)
        } else {
            None
        },
        open: command_problem(&entry.open, Purpose::Open),
        diff: command_problem(&entry.diff, Purpose::Diff),
        merge: command_problem(&entry.merge, Purpose::Merge),
    }
}

/// The command of entry `index` for `purpose` can run: not empty and
/// without a problem.
pub fn runnable(entries: &[ToolEntry], index: usize, purpose: Purpose) -> bool {
    entries.get(index).is_some_and(|entry| {
        !entry.command(purpose).trim().is_empty()
            && entry_problems(entries, index).command(purpose).is_none()
    })
}

/// The entry is offered by the picker of `purpose`: runnable and with a
/// valid name.
pub fn offered(entries: &[ToolEntry], index: usize, purpose: Purpose) -> bool {
    runnable(entries, index, purpose) && entry_problems(entries, index).name.is_none()
}

/// Next free id `tool-<n>` (never [`VSCODE_ID`]).
#[allow(dead_code)] // Manage tools (M20-02)
pub fn next_id(entries: &[ToolEntry]) -> String {
    (1..)
        .map(|n| format!("tool-{n}"))
        .find(|id| !entries.iter().any(|e| &e.id == id))
        .unwrap_or_default()
}

/// `wanted`, or `wanted 2`, `wanted 3`, … when the name is taken
/// (case-insensitive): "New tool", "<name> copy", "<name> copy 2".
#[allow(dead_code)] // Manage tools (M20-02)
pub fn unique_name(entries: &[ToolEntry], wanted: &str) -> String {
    let taken = |name: &str| {
        entries
            .iter()
            .any(|e| e.name.trim().to_lowercase() == name.to_lowercase())
    };
    if !taken(wanted) {
        return wanted.to_owned();
    }
    (2..)
        .map(|n| format!("{wanted} {n}"))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

/// "Restore VS Code" (ADR-062): adds the built-in entry at the top when it
/// is missing, otherwise resets its name and commands in place. Other
/// entries and the selections are not touched.
#[allow(dead_code)] // Manage tools (M20-02)
pub fn restore_vscode(entries: &mut Vec<ToolEntry>) {
    match entries.iter_mut().find(|e| e.id == VSCODE_ID) {
        Some(entry) => *entry = ToolEntry::vscode(),
        None => entries.insert(0, ToolEntry::vscode()),
    }
}

/// Index of the entry shown in Manage tools: `selected` when it exists,
/// else the first entry.
pub fn shown_tool(entries: &[ToolEntry], selected: Option<&str>) -> Option<usize> {
    selected
        .and_then(|id| entries.iter().position(|e| e.id == id))
        .or_else(|| (!entries.is_empty()).then_some(0))
}

/// An entry of the diff or merge tool picker (ADR-062).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOption {
    GitConfig,
    None,
    /// An entry of the list by its id.
    Entry(String),
    /// A tool Git offers ("<name> (Git)").
    Git(String),
}

impl ToolOption {
    /// The choice the option stores.
    pub fn choice(&self) -> ToolChoice {
        match self {
            Self::GitConfig => ToolChoice::GitConfig,
            Self::None => ToolChoice::None,
            Self::Entry(id) => ToolChoice::Entry(id.clone()),
            Self::Git(name) => ToolChoice::Tool(name.clone()),
        }
    }

    /// The label in the picker; `configured` is Git's `diff.tool` /
    /// `merge.tool`, if known.
    pub fn label(&self, entries: &[ToolEntry], configured: Option<&str>) -> String {
        match self {
            Self::GitConfig => format!(
                "Use Git config ({})",
                configured.unwrap_or("not configured")
            ),
            Self::None => "None".to_owned(),
            Self::Entry(id) => entries
                .iter()
                .find(|e| &e.id == id)
                .map_or_else(|| id.clone(), |e| e.name.trim().to_owned()),
            Self::Git(name) => format!("{name} (Git)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalTools {
    /// The tool list (M20, ADR-062). A missing key means the default list
    /// (VS Code); a stored list, also an empty one, is kept.
    pub entries: Vec<ToolEntry>,
    /// The entry used as external editor (M8-07, M20); `None`: none.
    pub editor: Option<String>,
    pub diff_tool: ToolChoice,
    pub merge_tool: ToolChoice,
    /// Command template with `{path}`; empty means auto-detection.
    pub terminal_command: String,
    /// "Delete .orig files after merging" (`mergetool.keepBackup=false`,
    /// M10-08); off by default like GitKraken.
    pub delete_orig: bool,
}

impl Default for ExternalTools {
    fn default() -> Self {
        Self {
            entries: vec![ToolEntry::vscode()],
            editor: None,
            diff_tool: ToolChoice::GitConfig,
            merge_tool: ToolChoice::GitConfig,
            terminal_command: String::new(),
            delete_orig: false,
        }
    }
}

impl ExternalTools {
    /// The diff or merge tool setting.
    pub fn choice_mut(&mut self, kind: git_core::ToolKind) -> &mut ToolChoice {
        match kind {
            git_core::ToolKind::Diff => &mut self.diff_tool,
            git_core::ToolKind::Merge => &mut self.merge_tool,
        }
    }

    pub fn choice(&self, kind: git_core::ToolKind) -> &ToolChoice {
        match kind {
            git_core::ToolKind::Diff => &self.diff_tool,
            git_core::ToolKind::Merge => &self.merge_tool,
        }
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }

    pub fn entry(&self, id: &str) -> Option<&ToolEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The runnable entry `id` for `purpose` (missing, empty or invalid
    /// command: `None`).
    fn runnable_entry(&self, id: &str, purpose: Purpose) -> Option<&ToolEntry> {
        let index = self.index(id)?;
        runnable(&self.entries, index, purpose).then(|| &self.entries[index])
    }

    /// The global editor command (empty: none). A selection that points to
    /// a missing entry or one without a valid open command is "None"; the
    /// id stays stored.
    pub fn editor_command(&self) -> String {
        self.editor
            .as_deref()
            .and_then(|id| self.runnable_entry(id, Purpose::Open))
            .map(|e| e.open.trim().to_owned())
            .unwrap_or_default()
    }

    /// The diff or merge choice that applies: an entry that is missing or
    /// has no valid command for `kind` is "None".
    pub fn effective_choice(&self, kind: git_core::ToolKind) -> ToolChoice {
        match self.choice(kind) {
            ToolChoice::Entry(id) if self.runnable_entry(id, kind.into()).is_none() => {
                ToolChoice::None
            }
            choice => choice.clone(),
        }
    }

    /// The command of the effective entry for `kind`, if it is one.
    pub fn entry_command(&self, kind: git_core::ToolKind) -> Option<&str> {
        match self.choice(kind) {
            ToolChoice::Entry(id) => self
                .runnable_entry(id, kind.into())
                .map(|e| e.command(kind.into()).trim()),
            _ => None,
        }
    }

    /// Name of the diff or merge tool for messages ("Waiting for …").
    pub fn tool_name(&self, kind: git_core::ToolKind) -> Option<String> {
        match self.effective_choice(kind) {
            ToolChoice::Tool(name) => Some(name),
            ToolChoice::Entry(id) => self.entry(&id).map(|e| e.name.trim().to_owned()),
            ToolChoice::GitConfig | ToolChoice::None => None,
        }
    }

    /// Ids of the entries the editor picker offers, in list order.
    pub fn editor_entries(&self) -> Vec<&ToolEntry> {
        self.offered(Purpose::Open)
    }

    /// Entries the picker of `purpose` offers, in list order.
    pub fn offered(&self, purpose: Purpose) -> Vec<&ToolEntry> {
        (0..self.entries.len())
            .filter(|&i| offered(&self.entries, i, purpose))
            .map(|i| &self.entries[i])
            .collect()
    }

    /// The diff or merge picker (ADR-062): "Use Git config", "None", the
    /// entries with a valid command for `kind`, then every tool Git offers
    /// (and a chosen Git tool it does not list any more); the internal
    /// [`git_core::CUSTOM_TOOL`] is never listed.
    pub fn tool_options(&self, kind: git_core::ToolKind, git_tools: &[String]) -> Vec<ToolOption> {
        let mut list = vec![ToolOption::GitConfig, ToolOption::None];
        list.extend(
            self.offered(kind.into())
                .into_iter()
                .map(|e| ToolOption::Entry(e.id.clone())),
        );
        let mut names: Vec<String> = git_tools.to_vec();
        if let ToolChoice::Tool(name) = self.choice(kind) {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        list.extend(
            names
                .into_iter()
                .filter(|name| name != git_core::CUSTOM_TOOL)
                .map(ToolOption::Git),
        );
        list
    }

    /// Where entry `id` is selected: "external editor", "diff tool",
    /// "merge tool" (stored selections, for the delete question).
    pub fn usage(&self, id: &str) -> Vec<&'static str> {
        let mut used = Vec::new();
        if self.editor.as_deref() == Some(id) {
            used.push("external editor");
        }
        for (kind, label) in [
            (git_core::ToolKind::Diff, "diff tool"),
            (git_core::ToolKind::Merge, "merge tool"),
        ] {
            if *self.choice(kind) == ToolChoice::Entry(id.to_owned()) {
                used.push(label);
            }
        }
        used
    }

    /// Deletes entry `id`; the selections that pointed to it become
    /// "None" (ADR-062).
    pub fn delete_entry(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
        if self.editor.as_deref() == Some(id) {
            self.editor = None;
        }
        for kind in [git_core::ToolKind::Diff, git_core::ToolKind::Merge] {
            let choice = self.choice_mut(kind);
            if *choice == ToolChoice::Entry(id.to_owned()) {
                *choice = ToolChoice::None;
            }
        }
    }

    /// The option the stored choice shows as.
    pub fn shown_option(&self, kind: git_core::ToolKind) -> ToolOption {
        match self.effective_choice(kind) {
            ToolChoice::GitConfig => ToolOption::GitConfig,
            ToolChoice::None => ToolOption::None,
            ToolChoice::Entry(id) => ToolOption::Entry(id),
            ToolChoice::Tool(name) => ToolOption::Git(name),
        }
    }
}

/// Presets of the external editor (M8-07); a preset fills the editable
/// command template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorPreset {
    None,
    VsCode,
    VsCodium,
    SublimeText,
    IntellijIdea,
    Zed,
    Custom,
}

impl EditorPreset {
    pub const ALL: [Self; 7] = [
        Self::None,
        Self::VsCode,
        Self::VsCodium,
        Self::SublimeText,
        Self::IntellijIdea,
        Self::Zed,
        Self::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::VsCode => "VS Code",
            Self::VsCodium => "VSCodium",
            Self::SublimeText => "Sublime Text",
            Self::IntellijIdea => "IntelliJ IDEA",
            Self::Zed => "Zed",
            Self::Custom => "Custom",
        }
    }

    /// Template a preset puts into the command field (`None` for Custom,
    /// which keeps the current text).
    pub fn template(self) -> Option<&'static str> {
        match self {
            Self::None => Some(""),
            Self::VsCode => Some("code {path}"),
            Self::VsCodium => Some("codium {path}"),
            Self::SublimeText => Some("subl {path}"),
            Self::IntellijIdea => Some("idea {path}"),
            Self::Zed => Some("zed {path}"),
            Self::Custom => None,
        }
    }

    /// The preset a stored template matches (Custom for anything else).
    pub fn of(template: &str) -> Self {
        let template = template.trim();
        Self::ALL
            .into_iter()
            .find(|preset| preset.template() == Some(template))
            .unwrap_or(Self::Custom)
    }
}

/// Name of the editor an editor command starts (M15-02), for labels: the
/// preset by its template, else by the program's file name (first token,
/// quotes removed, stem compared case-insensitively), else the program's
/// file name. "none" for an empty command.
pub fn editor_name(command: &str) -> String {
    let command = command.trim();
    if command.is_empty() {
        return "none".to_owned();
    }
    match EditorPreset::of(command) {
        EditorPreset::Custom | EditorPreset::None => {}
        preset => return preset.label().to_owned(),
    }
    let program = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(rest),
        None => command.split_whitespace().next().unwrap_or(command),
    };
    let file = program
        .rsplit(['/', '\\'])
        .next()
        .filter(|f| !f.is_empty())
        .unwrap_or(program);
    let stem = match file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => file,
    };
    let preset = match stem.to_lowercase().as_str() {
        "code" => Some(EditorPreset::VsCode),
        "codium" => Some(EditorPreset::VsCodium),
        "subl" | "sublime_text" => Some(EditorPreset::SublimeText),
        "idea" | "idea64" => Some(EditorPreset::IntellijIdea),
        "zed" => Some(EditorPreset::Zed),
        _ => None,
    };
    preset.map_or_else(|| file.to_owned(), |p| p.label().to_owned())
}

impl std::fmt::Display for EditorPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Category of the settings view (list on the left).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    General,
    Appearance,
    /// Saved commit identities and the default (M8-03).
    Identities,
    /// Commit and tag signing in the global Git config (M10-04).
    Signing,
    /// Commit message prompt and stash message template (M10-07, M10-10).
    Templates,
    ExternalTools,
    /// Identity of the active tab's repository (M8-04).
    Identity,
    /// SSH key of the active tab's repository (M8-06).
    SshKey,
    /// Commit template of the active tab's repository (M10-06).
    Commit,
    /// External editor of the active tab's repository (M10-11).
    RepoEditor,
    /// Font, sizes and options of the code views (M11-06, GitKraken
    /// "Editor").
    Editor,
    /// Git flow of the active tab's repository (M7-07).
    Gitflow,
    /// The theme list, its files and the editor (M17-02, ADR-059): a
    /// sub-page of Appearance ("Manage themes…", ADR-060), not in the
    /// sidebar.
    Themes,
    /// The tool list (M20, ADR-062): a sub-page of External tools
    /// ("Manage tools…"), not in the sidebar.
    Tools,
}

impl Category {
    /// Categories that apply everywhere, at the top of the list.
    pub const GLOBAL: [Self; 7] = [
        Self::General,
        Self::Appearance,
        Self::Editor,
        Self::Identities,
        Self::Signing,
        Self::Templates,
        Self::ExternalTools,
    ];
    /// Categories of the active tab's repository, below the group header
    /// "Repository · <name>" (M8-01, like GitKraken's repo-specific
    /// preferences).
    pub const REPOSITORY: [Self; 5] = [
        Self::Identity,
        Self::SshKey,
        Self::Commit,
        Self::RepoEditor,
        Self::Gitflow,
    ];

    /// The sidebar entry that is highlighted for the category: sub-pages
    /// highlight their parent (Themes → Appearance, ADR-060).
    pub fn sidebar(self) -> Self {
        match self {
            Self::Themes => Self::Appearance,
            Self::Tools => Self::ExternalTools,
            other => other,
        }
    }

    /// The category one level up (Esc and the back button), `None` for
    /// the categories of the sidebar.
    pub fn parent(self) -> Option<Self> {
        match self {
            Self::Themes => Some(Self::Appearance),
            Self::Tools => Some(Self::ExternalTools),
            _ => None,
        }
    }

    /// The category belongs to the repository group.
    pub fn is_repository(self) -> bool {
        Self::REPOSITORY.contains(&self)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Identities => "Identities",
            Self::Signing => "Signing",
            Self::Templates => "Templates",
            Self::Identity => "Identity",
            Self::SshKey => "SSH key",
            Self::Commit => "Commit",
            Self::RepoEditor => "External editor",
            Self::Editor => "Editor",
            Self::ExternalTools => "External tools",
            Self::Gitflow => "Gitflow",
            Self::Themes => "Themes",
            Self::Tools => "Tools",
        }
    }
}

/// State of the open settings view.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub category: Category,
    /// Theme selected in Preferences → Appearance → Manage themes (`None`: the active one).
    pub theme_selected: Option<String>,
    /// The theme editor, while a user theme is edited (M17-03).
    pub theme_editor: Option<crate::theme_editor::Editor>,
    /// Entry selected in External tools → Manage tools (`None`: the first).
    pub tool_selected: Option<String>,
    /// Field opened from the palette, highlighted for a moment (M12-12).
    pub highlight: Option<&'static str>,
    /// Text of the auto-fetch input (may be invalid while typing).
    pub auto_fetch_input: String,
    /// Why the typed interval is not applied.
    pub error: Option<String>,
    /// Diff and merge tools Git offers (External tools); `None` until loaded.
    pub tools: Option<Result<(git_core::ToolInfo, git_core::ToolInfo), String>>,
    /// Gitflow form (filled when the category opens with a repository).
    pub flow: Option<FlowForm>,
    /// SSH key category (M8-06): keys found in `~/.ssh` (`None` while
    /// loading), the typed key path, and whether replacing a custom
    /// `core.sshCommand` was confirmed.
    pub ssh_keys: Option<Vec<std::path::PathBuf>>,
    pub ssh_path: String,
    pub ssh_replace: bool,
    /// Repository the repository category's form was built for; the form
    /// follows the active tab (M8-01).
    pub repo: Option<(crate::app::TabId, std::path::PathBuf)>,
    /// `init.defaultBranch` of the global Git config (`None` while loading),
    /// the typed value and why saving failed (M10-03).
    pub default_branch: Option<Option<String>>,
    pub default_branch_input: String,
    pub default_branch_error: Option<String>,
    /// Signing category (M10-04): `None` while loading.
    pub signing: Option<Result<SigningPage, String>>,
    /// Typed GPG program (saved with Save).
    pub gpg_program_input: String,
    /// The repository's editor command being typed, and "Custom" picked
    /// while it matches a preset (M10-11).
    pub repo_editor_input: String,
    pub repo_editor_custom: bool,
    /// "Custom" was picked for the stash template while its text still
    /// matches a preset (M10-10).
    pub stash_custom: bool,
    /// `core.longpaths` of the global config (Windows, M10-08); `None`
    /// while loading.
    pub long_paths: Option<bool>,
    /// Commit template category (M10-06): `None` while loading; the typed
    /// summary (the description lives in `App::template_description`).
    pub template: Option<Result<git_core::commit_template::TemplateSettings, String>>,
    pub template_summary: String,
    /// Monospace font families for Preferences → Editor (M11-06), read
    /// from the font database when the category opens.
    pub fonts: Vec<String>,
}

/// What the Signing category shows (M10-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningPage {
    pub global: git_core::signing::SigningSettings,
    /// Signing keys the active repository sets itself (read-only).
    pub overrides: Vec<(String, String)>,
    /// Secret keys of the GPG program, or why they could not be listed.
    pub gpg_keys: Result<Vec<git_core::signing::GpgKey>, String>,
    /// Public SSH keys in `~/.ssh` (`.pub` next to a private key).
    pub ssh_keys: Vec<std::path::PathBuf>,
}

/// Fields of the Gitflow category, in [`FlowForm::values`] order.
pub const FLOW_FIELDS: [&str; 6] = [
    "Production branch",
    "Development branch",
    "Feature prefix",
    "Release prefix",
    "Hotfix prefix",
    "Version tag prefix",
];

/// Values typed in the Gitflow category (M7-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowForm {
    pub values: [String; 6],
}

impl FlowForm {
    pub fn new(config: &git_core::FlowConfig) -> Self {
        Self {
            values: [
                config.production.clone(),
                config.development.clone(),
                config.feature.clone(),
                config.release.clone(),
                config.hotfix.clone(),
                config.version_tag.clone(),
            ],
        }
    }

    pub fn config(&self) -> git_core::FlowConfig {
        let [production, development, feature, release, hotfix, version_tag] = self.values.clone();
        git_core::FlowConfig {
            production: production.trim().to_owned(),
            development: development.trim().to_owned(),
            feature: feature.trim().to_owned(),
            release: release.trim().to_owned(),
            hotfix: hotfix.trim().to_owned(),
            version_tag: version_tag.trim().to_owned(),
        }
    }
}

impl Page {
    pub fn new(settings: &Settings) -> Self {
        Self {
            category: Category::General,
            theme_selected: None,
            theme_editor: None,
            tool_selected: None,
            highlight: None,
            auto_fetch_input: settings.general.auto_fetch_minutes.to_string(),
            error: None,
            tools: None,
            flow: None,
            ssh_keys: None,
            ssh_path: String::new(),
            ssh_replace: false,
            repo: None,
            default_branch: None,
            default_branch_input: String::new(),
            default_branch_error: None,
            signing: None,
            gpg_program_input: String::new(),
            long_paths: None,
            stash_custom: false,
            repo_editor_input: String::new(),
            repo_editor_custom: false,
            template: None,
            template_summary: String::new(),
            fonts: Vec::new(),
        }
    }

    pub fn with_category(mut self, category: Category) -> Self {
        self.category = category;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Invalid(String),
    UnknownVersion(u64),
}

/// Parses a settings file; a newer version is rejected.
pub fn parse(text: &str) -> Result<Settings, ParseError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| ParseError::Invalid(e.to_string()))?;
    if !value.is_object() {
        return Err(ParseError::Invalid("not an object".into()));
    }
    // A file without version is read as version 1.
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::from(VERSION));
    if version != u64::from(VERSION) {
        return Err(ParseError::UnknownVersion(version));
    }
    let mut settings: Settings =
        serde_json::from_value(value).map_err(|e| ParseError::Invalid(e.to_string()))?;
    settings.version = VERSION;
    settings.general.auto_fetch_minutes = settings
        .general
        .auto_fetch_minutes
        .min(MAX_AUTO_FETCH_MINUTES);
    settings.appearance.zoom_percent = clamp_zoom(settings.appearance.zoom_percent);
    // A default that points nowhere (edited file) is dropped.
    let identities = &mut settings.identities;
    if identities
        .default
        .is_some_and(|d| d >= identities.list.len())
    {
        identities.default = None;
    }
    Ok(settings)
}

pub fn to_json(settings: &Settings) -> String {
    // Serializing plain data cannot fail; fall back to an empty object just in case.
    serde_json::to_string_pretty(settings).unwrap_or_else(|_| "{}".into())
}

/// Result of loading the settings at startup.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub settings: Settings,
    /// One-time message for the user (e.g. the file was broken and reset).
    pub notice: Option<String>,
    /// The file has to be written: it did not exist yet (values migrated
    /// from the session) or was reset.
    pub write: bool,
}

/// Reads the settings. Without a settings file, `auto_fetch_minutes` is
/// taken over from the session file `session` once (ADR-023).
pub fn load(path: &Path, session: Option<&Path>) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let mut settings = Settings::default();
            if let Some(minutes) = session.and_then(legacy_auto_fetch) {
                settings.general.auto_fetch_minutes = minutes.min(MAX_AUTO_FETCH_MINUTES);
            }
            return Loaded {
                settings,
                notice: None,
                write: true,
            };
        }
        Err(e) => {
            return Loaded {
                settings: Settings::default(),
                notice: Some(format!("Could not read the settings file: {e}")),
                write: false,
            }
        }
    };
    match parse(&text) {
        Ok(settings) => Loaded {
            settings,
            notice: None,
            write: false,
        },
        Err(error) => {
            let backup = path.with_extension("json.bak");
            let moved = std::fs::rename(path, &backup).is_ok();
            let reason = match error {
                ParseError::Invalid(_) => "was unreadable".to_string(),
                ParseError::UnknownVersion(v) => format!("has unknown version {v}"),
            };
            let backup_note = if moved {
                format!(" A backup was saved as {}.", backup.display())
            } else {
                String::new()
            };
            Loaded {
                settings: Settings::default(),
                notice: Some(format!(
                    "The settings file {reason}; defaults are used.{backup_note}"
                )),
                write: moved,
            }
        }
    }
}

/// `auto_fetch_minutes` of a session file written before M5-02.
fn legacy_auto_fetch(session: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(session).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("auto_fetch_minutes")?
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
}

/// Writes atomically (temp file, then rename).
pub fn save(path: &Path, json: &str) -> io::Result<()> {
    crate::session::save(path, json)
}

/// Zoom range and step in percent (M5-04).
pub const ZOOM_MIN: u32 = 70;
pub const ZOOM_MAX: u32 = 200;
pub const ZOOM_STEP: u32 = 10;
pub const ZOOM_DEFAULT: u32 = 100;

/// Next zoom level up or down, snapped to the 10 % grid and kept in range.
pub fn zoom_step(percent: u32, up: bool) -> u32 {
    let snapped = clamp_zoom(percent) / ZOOM_STEP * ZOOM_STEP;
    let next = if up {
        snapped + ZOOM_STEP
    } else if snapped < clamp_zoom(percent) {
        // Between two steps (edited file): down goes to the lower one.
        snapped
    } else {
        snapped.saturating_sub(ZOOM_STEP)
    };
    clamp_zoom(next)
}

/// Zoom level within [`ZOOM_MIN`]..=[`ZOOM_MAX`].
pub fn clamp_zoom(percent: u32) -> u32 {
    percent.clamp(ZOOM_MIN, ZOOM_MAX)
}

/// Parses the auto-fetch interval typed in the settings view.
pub fn parse_auto_fetch(input: &str) -> Result<u32, String> {
    let input = input.trim();
    match input.parse::<u32>() {
        Ok(minutes) if minutes <= MAX_AUTO_FETCH_MINUTES => Ok(minutes),
        Ok(_) => Err(format!("At most {MAX_AUTO_FETCH_MINUTES} minutes")),
        Err(_) if input.is_empty() => Err("Enter a number of minutes (0 = off)".into()),
        Err(_) => Err("Whole minutes only (0 = off)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn editor_presets_and_older_files() {
        // Files written before M8-07 have no editor: none.
        let old = parse(r#"{"tools": {"terminal_command": "wt"}}"#).unwrap();
        assert_eq!(old.tools.editor, None);
        assert_eq!(old.tools.terminal_command, "wt");
        assert_eq!(EditorPreset::of(""), EditorPreset::None);
        assert_eq!(EditorPreset::of(" code {path} "), EditorPreset::VsCode);
        assert_eq!(EditorPreset::of("code -n {path}"), EditorPreset::Custom);
        for preset in EditorPreset::ALL {
            if let Some(template) = preset.template() {
                assert_eq!(EditorPreset::of(template), preset, "{template}");
            }
        }
    }

    #[test]
    fn identities_load_from_older_files_and_round_trip() {
        // Files written before M8-03 have no "identities": empty, sync on.
        let old = parse(r#"{"version": 1, "general": {"auto_fetch_minutes": 7}}"#).unwrap();
        assert_eq!(old.identities, Identities::default());
        assert!(old.identities.sync_global);
        assert_eq!(old.version, VERSION, "version unchanged");

        let mut settings = Settings::default();
        settings.identities.list.push(SavedIdentity {
            name: "Ana".into(),
            email: "ana@x.org".into(),
        });
        settings.identities.default = Some(0);
        settings.identities.sync_global = false;
        assert_eq!(parse(&to_json(&settings)).unwrap(), settings);

        // A default pointing nowhere is dropped.
        let broken = parse(r#"{"identities": {"list": [], "default": 3}}"#).unwrap();
        assert_eq!(broken.identities.default, None);
    }

    #[test]
    fn classic_dark_loads_as_dusk() {
        // The theme was removed (ADR-054); saved settings open in Dusk
        // (ADR-060).
        let parsed = parse(r#"{"appearance": {"theme": "classic-dark"}}"#).unwrap();
        assert_eq!(parsed.appearance.theme, ThemeChoice::theme("dusk"));
        let json = to_json(&parsed);
        assert!(json.contains("\"dusk\""), "{json}");
        assert!(!json.contains("classic-dark"), "{json}");
    }

    /// ADR-058, ADR-060: values written before M18 still load; old ids
    /// read as their new id in all three fields and are written back as
    /// the new id.
    #[test]
    fn old_theme_values_still_load() {
        let renames = [
            ("dark", "dusk"),
            ("classic-dark", "dusk"),
            ("dark-plus", "studio-dark"),
            ("github-dark", "midnight"),
            ("light-plus", "studio-light"),
            ("solarized-light", "sepia"),
            ("github-light", "daylight"),
            ("light", "light"),
            ("warm-dark", "warm-dark"),
        ];
        for (stored, want) in renames {
            let text = format!(
                r#"{{"appearance": {{"theme": "{stored}", "system_dark": "{stored}", "system_light": "{stored}"}}}}"#
            );
            let parsed = parse(&text).unwrap();
            assert_eq!(
                parsed.appearance.theme,
                ThemeChoice::theme(want),
                "{stored}"
            );
            assert_eq!(parsed.appearance.system_dark, want, "{stored}");
            assert_eq!(parsed.appearance.system_light, want, "{stored}");
            let json = to_json(&parsed);
            assert_eq!(json.matches(&format!("\"{want}\"")).count(), 3, "{json}");
            if stored != want {
                assert!(!json.contains(&format!("\"{stored}\"")), "{json}");
            }
        }
        let parsed = parse(r#"{"appearance": {"theme": "system"}}"#).unwrap();
        assert_eq!(parsed.appearance.theme, ThemeChoice::System);
        // The System mapping starts with Dusk and Light.
        assert_eq!(parsed.appearance.system_dark, "dusk");
        assert_eq!(parsed.appearance.system_light, "light");
        // An unknown id is kept (the app falls back to Dusk with a note).
        let parsed = parse(r#"{"appearance": {"theme": "no-such-theme"}}"#).unwrap();
        assert_eq!(parsed.appearance.theme, ThemeChoice::theme("no-such-theme"));
    }

    #[test]
    fn theme_keys_round_trip() {
        let mut settings = Settings::default();
        settings.appearance.theme = ThemeChoice::System;
        settings.appearance.system_dark = "monokai".into();
        settings.appearance.system_light = "sepia".into();
        let json = to_json(&settings);
        assert!(json.contains(r#""theme": "system""#), "{json}");
        assert_eq!(parse(&json).unwrap(), settings);
        settings.appearance.theme = ThemeChoice::theme("warm-dark");
        assert_eq!(parse(&to_json(&settings)).unwrap(), settings);
    }

    #[test]
    fn small_options_load_from_older_files() {
        // Files written before M10-08: prune on, labels on, .orig kept.
        let old = parse(r#"{"version": 1, "general": {"auto_fetch_minutes": 3}}"#).unwrap();
        assert!(old.general.prune_on_fetch);
        assert!(old.appearance.toolbar_labels);
        assert!(!old.tools.delete_orig);
        let mut settings = Settings::default();
        settings.general.prune_on_fetch = false;
        settings.appearance.toolbar_labels = false;
        settings.tools.delete_orig = true;
        assert_eq!(parse(&to_json(&settings)).unwrap(), settings);
    }

    #[test]
    fn templates_load_from_older_files_and_round_trip() {
        // Files written before M10-07 get the default prompt.
        let old = parse(r#"{"version": 1}"#).unwrap();
        assert_eq!(
            old.templates.commit_prompt,
            git_core::prompt::DEFAULT_INSTRUCTION
        );
        assert!(!old.templates.stash_enabled);
        assert_eq!(
            old.templates.stash_template,
            crate::stash_template::PRESETS[0]
        );
        let mut settings = Settings::default();
        settings.templates.commit_prompt = "Be brief.".into();
        settings.templates.stash_enabled = true;
        settings.templates.stash_template = "{sha}".into();
        assert_eq!(parse(&to_json(&settings)).unwrap(), settings);
    }

    #[test]
    fn identity_list_rules() {
        let ana = git_core::Identity::new("Ana", "ana@x.org").unwrap();
        let mut identities = Identities::default();
        // First use: the global identity becomes the first one and default.
        assert!(identities.import(&ana));
        assert_eq!(
            identities.default_identity().unwrap().label(),
            "Ana <ana@x.org>"
        );
        assert!(!identities.import(&ana), "only into an empty list");

        identities.list.push(SavedIdentity {
            name: "Work".into(),
            email: "work@x.org".into(),
        });
        assert_eq!(
            identities.problem(" Ana ", "ana@x.org", None).as_deref(),
            Some("This identity exists already")
        );
        assert_eq!(
            identities.problem("Ana", "ana@x.org", Some(0)),
            None,
            "itself"
        );
        assert_eq!(
            identities.problem("Ana", "no-at", None).as_deref(),
            Some("The email address needs an @")
        );
        assert_eq!(identities.problem("Bo", "bo@x.org", None), None);

        // Removing shifts the default; removing the default clears it.
        identities.default = Some(1);
        identities.remove(0);
        assert_eq!(identities.default, Some(0));
        identities.remove(0);
        assert_eq!(identities.default, None);
        identities.remove(5);
        assert!(identities.list.is_empty());
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gitloom-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample() -> Settings {
        let mut settings = Settings::default();
        settings.general.auto_fetch_minutes = 15;
        settings.appearance.theme = ThemeChoice::theme("light");
        settings.appearance.zoom_percent = 120;
        settings.file_list.tree = true;
        settings.file_list.descending = true;
        settings.appearance.avatars = true;
        settings.tools.diff_tool = ToolChoice::Tool("meld".into());
        settings.tools.entries.push(ToolEntry {
            id: "tool-1".into(),
            name: "Meld".into(),
            open: String::new(),
            diff: "meld {local} {remote}".into(),
            merge: "meld --output={merged} {local} {base} {remote}".into(),
        });
        settings.tools.merge_tool = ToolChoice::Entry("tool-1".into());
        settings.tools.editor = Some(VSCODE_ID.into());
        settings.tools.terminal_command = "alacritty --working-directory {path}".into();
        settings.diff.view_mode = DiffMode::Split;
        settings.appearance.graph_columns.refs = 150.0;
        settings.appearance.graph_columns.graph = Some(84.0);
        settings.appearance.graph_sha = false;
        settings
    }

    #[test]
    fn flow_form_round_trip_and_trimming() {
        let config = git_core::FlowConfig::defaults(&["main"]);
        let mut form = FlowForm::new(&config);
        assert_eq!(form.config(), config);
        form.values[1] = " dev ".into();
        assert_eq!(form.config().development, "dev");
        assert_eq!(FLOW_FIELDS.len(), form.values.len());
    }

    #[test]
    fn settings_without_diff_mode_load_with_hunk_view() {
        // A settings file of 1.0 (before M7-01).
        let text = r#"{
            "version": 1,
            "general": { "auto_fetch_minutes": 10, "check_updates": false },
            "appearance": { "theme": "dark", "zoom_percent": 100, "avatars": false },
            "tools": { "diff_tool": "git-config", "merge_tool": "git-config", "terminal_command": "" }
        }"#;
        let settings = parse(text).unwrap();
        assert_eq!(settings.diff.view_mode, DiffMode::Hunk);
        assert_eq!(settings.version, VERSION);
        assert!(to_json(&sample()).contains(r#""view_mode": "split""#));
    }

    /// Settings files before the fix after M15 read unchanged; entries
    /// and selections round trip (M20).
    #[test]
    fn tool_choices_of_older_files_load_and_entries_round_trip() {
        let text = r#"{
            "version": 1,
            "tools": { "diff_tool": { "tool": "meld" }, "merge_tool": "none" }
        }"#;
        let settings = parse(text).unwrap();
        assert_eq!(settings.tools.diff_tool, ToolChoice::Tool("meld".into()));
        assert_eq!(settings.tools.merge_tool, ToolChoice::None);
        assert_eq!(settings.tools.entries, [ToolEntry::vscode()]);
        let json = to_json(&sample());
        assert!(json.contains(r#""entry": "tool-1""#), "{json}");
        assert!(json.contains(r#""editor": "vscode""#), "{json}");
        assert_eq!(parse(&json).unwrap().tools, sample().tools);
    }

    /// M20 (ADR-062, no migration): a settings file from before M20 with
    /// an editor command and command selections loads without the damaged
    /// path; the old selections read as "Use Git config", the editor as
    /// none, everything else stays.
    #[test]
    fn settings_before_the_tool_list_load_without_their_tool_choices() {
        let text = r#"{
            "version": 1,
            "general": { "auto_fetch_minutes": 12 },
            "appearance": { "theme": "light", "zoom_percent": 125 },
            "tools": {
                "diff_tool": { "command": "code --wait --diff {local} {remote}" },
                "merge_tool": { "command": "meld --output={merged} {local} {base} {remote}" },
                "terminal_command": "wt -d {path}",
                "editor_command": "zed {path}",
                "delete_orig": true
            }
        }"#;
        let settings = parse(text).unwrap();
        assert_eq!(settings.tools.diff_tool, ToolChoice::GitConfig);
        assert_eq!(settings.tools.merge_tool, ToolChoice::GitConfig);
        assert_eq!(settings.tools.editor, None);
        assert_eq!(settings.tools.entries, [ToolEntry::vscode()]);
        assert_eq!(settings.tools.terminal_command, "wt -d {path}");
        assert!(settings.tools.delete_orig);
        assert_eq!(settings.general.auto_fetch_minutes, 12);
        assert_eq!(settings.appearance.zoom_percent, 125);
        assert_eq!(settings.appearance.theme, ThemeChoice::theme("light"));
        assert!(!to_json(&settings).contains("editor_command"));
        // An unknown variant is still a damaged file.
        assert!(parse(r#"{"tools": {"diff_tool": {"magic": 1}}}"#).is_err());
    }

    #[test]
    fn stored_tool_lists_are_kept_also_empty() {
        let empty = parse(r#"{"tools": {"entries": []}}"#).unwrap();
        assert!(
            empty.tools.entries.is_empty(),
            "a deleted VS Code stays deleted"
        );
        assert!(parse(&to_json(&empty)).unwrap().tools.entries.is_empty());
        let missing = parse(r#"{"tools": {}}"#).unwrap();
        assert_eq!(missing.tools.entries, [ToolEntry::vscode()]);
    }

    fn entry(id: &str, name: &str, open: &str, diff: &str, merge: &str) -> ToolEntry {
        ToolEntry {
            id: id.into(),
            name: name.into(),
            open: open.into(),
            diff: diff.into(),
            merge: merge.into(),
        }
    }

    #[test]
    fn effective_editor_and_tool_choices() {
        use git_core::ToolKind::{Diff, Merge};
        let mut tools = ExternalTools::default();
        tools
            .entries
            .push(entry("tool-1", "Zed", "zed {path}", "", ""));
        assert_eq!(tools.editor_command(), "", "none selected");
        tools.editor = Some("tool-1".into());
        assert_eq!(tools.editor_command(), "zed {path}");
        tools.editor = Some("tool-9".into());
        assert_eq!(tools.editor_command(), "", "missing entry");
        tools.editor = Some("tool-1".into());
        tools.entries[1].open = "  ".into();
        assert_eq!(tools.editor_command(), "", "empty command");
        tools.entries[1].open = "zed 'x {path}".into();
        assert_eq!(tools.editor_command(), "", "unclosed quote");
        // The stored id stays: filling the command in again works.
        tools.entries[1].open = "zed {path}".into();
        assert_eq!(tools.editor_command(), "zed {path}");

        tools.diff_tool = ToolChoice::Entry(VSCODE_ID.into());
        tools.merge_tool = ToolChoice::Entry("tool-1".into());
        assert_eq!(
            tools.effective_choice(Diff),
            ToolChoice::Entry(VSCODE_ID.into())
        );
        assert_eq!(tools.entry_command(Diff), Some(VSCODE_DIFF));
        assert_eq!(tools.tool_name(Diff).as_deref(), Some("VS Code"));
        assert_eq!(
            tools.effective_choice(Merge),
            ToolChoice::None,
            "no merge command"
        );
        assert_eq!(tools.entry_command(Merge), None);
        assert_eq!(tools.tool_name(Merge), None);
        tools.diff_tool = ToolChoice::Entry("gone".into());
        assert_eq!(tools.effective_choice(Diff), ToolChoice::None);
        tools.diff_tool = ToolChoice::Tool("meld".into());
        assert_eq!(
            tools.effective_choice(Diff),
            ToolChoice::Tool("meld".into())
        );
        assert_eq!(tools.tool_name(Diff).as_deref(), Some("meld"));
        assert_eq!(tools.effective_choice(Merge).clone(), ToolChoice::None);
        tools.merge_tool = ToolChoice::GitConfig;
        assert_eq!(tools.effective_choice(Merge), ToolChoice::GitConfig);
    }

    #[test]
    fn entry_validation() {
        let mut entries = vec![
            ToolEntry::vscode(),
            entry("tool-1", "vs code", "", "", ""),
            entry("tool-2", " ", "", "", ""),
        ];
        assert_eq!(entry_problems(&entries, 0).name, Some(NAME_TAKEN));
        assert_eq!(entry_problems(&entries, 1).name, Some(NAME_TAKEN));
        assert_eq!(entry_problems(&entries, 2).name, Some(NAME_EMPTY));
        entries[1].name = "Meld".into();
        assert!(!entry_problems(&entries, 0).any());
        assert!(
            !entry_problems(&entries, 1).any(),
            "empty commands are fine"
        );
        let problems = |e: ToolEntry| entry_problems(&[e], 0);
        let p = problems(entry("x", "X", "x '{path}", "", ""));
        assert_eq!(p.open, Some(UNCLOSED_QUOTE));
        let p = problems(entry("x", "X", "", "x {local}", ""));
        assert_eq!(p.diff, Some(DIFF_PLACEHOLDERS));
        let p = problems(entry("x", "X", "", "x {remote}", ""));
        assert_eq!(p.diff, Some(DIFF_PLACEHOLDERS));
        let p = problems(entry("x", "X", "", "x \"{local} {remote}", ""));
        assert_eq!(p.diff, Some(UNCLOSED_QUOTE));
        let p = problems(entry("x", "X", "", "", "x {local} {remote} {base}"));
        assert_eq!(p.merge, Some(MERGE_PLACEHOLDER));
        let p = problems(entry("x", "X", "", "", "x {merged}"));
        assert_eq!(p, EntryProblems::default());
        // A problem keeps the entry out of that purpose only.
        let e = [entry("x", "X", "x {path}", "x {local}", "x {merged}")];
        assert!(offered(&e, 0, Purpose::Open));
        assert!(!offered(&e, 0, Purpose::Diff));
        assert!(offered(&e, 0, Purpose::Merge));
        // A name problem keeps it out of the pickers, not from running.
        let e = [
            ToolEntry::vscode(),
            entry("t", "VS CODE", "x {path}", "", ""),
        ];
        assert!(runnable(&e, 1, Purpose::Open) && !offered(&e, 1, Purpose::Open));
    }

    #[test]
    fn ids_names_and_restore() {
        let mut entries = vec![ToolEntry::vscode()];
        assert_eq!(next_id(&entries), "tool-1");
        entries.push(entry("tool-1", "New tool", "", "", ""));
        entries.push(entry("tool-3", "Zed", "", "", ""));
        assert_eq!(next_id(&entries), "tool-2");
        assert_eq!(
            next_id(&[]),
            "tool-1",
            "vscode is reserved for the built-in entry"
        );
        assert_eq!(unique_name(&entries, "New tool"), "New tool 2");
        assert_eq!(unique_name(&entries, "Zed copy"), "Zed copy");
        entries.push(entry("tool-2", "zed COPY", "", "", ""));
        assert_eq!(unique_name(&entries, "Zed copy"), "Zed copy 2");
        assert_eq!(unique_name(&entries, "Meld"), "Meld");

        // Missing: added at the top; changed: reset in place; others stay.
        let mut entries = vec![entry("tool-1", "Zed", "zed {path}", "", "")];
        restore_vscode(&mut entries);
        assert_eq!(entries[0], ToolEntry::vscode());
        assert_eq!(entries[1].name, "Zed");
        entries.swap(0, 1);
        entries[1].name = "Code".into();
        entries[1].open = "code -n {path}".into();
        restore_vscode(&mut entries);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1], ToolEntry::vscode(), "reset where it is");
        assert_eq!(entries[0].open, "zed {path}");
        let before = entries.clone();
        restore_vscode(&mut entries);
        assert_eq!(entries, before, "unchanged");
    }

    #[test]
    fn delete_resets_the_selections_that_used_the_entry() {
        let mut tools = ExternalTools::default();
        tools
            .entries
            .push(entry("tool-1", "Zed", "zed {path}", "", ""));
        tools.editor = Some(VSCODE_ID.into());
        tools.diff_tool = ToolChoice::Entry(VSCODE_ID.into());
        tools.merge_tool = ToolChoice::Tool("meld".into());
        assert_eq!(tools.usage(VSCODE_ID), ["external editor", "diff tool"]);
        assert!(tools.usage("tool-1").is_empty());
        tools.delete_entry(VSCODE_ID);
        assert_eq!(tools.entries.len(), 1);
        assert_eq!(tools.editor, None);
        assert_eq!(tools.diff_tool, ToolChoice::None);
        assert_eq!(tools.merge_tool, ToolChoice::Tool("meld".into()));
        assert_eq!(shown_tool(&tools.entries, Some(VSCODE_ID)), Some(0));
        assert_eq!(shown_tool(&[], None), None);
    }

    #[test]
    fn tool_pickers_list_entries_then_git_tools() {
        use git_core::ToolKind::{Diff, Merge};
        let mut tools = ExternalTools::default();
        tools
            .entries
            .push(entry("tool-1", "Zed", "zed {path}", "", ""));
        tools
            .entries
            .push(entry("tool-2", "Meld", "", "meld {local} {remote}", "meld"));
        let git = [
            "meld".to_owned(),
            "gitloom-custom".to_owned(),
            "vimdiff".to_owned(),
        ];
        assert_eq!(
            tools.tool_options(Diff, &git),
            [
                ToolOption::GitConfig,
                ToolOption::None,
                ToolOption::Entry(VSCODE_ID.into()),
                ToolOption::Entry("tool-2".into()),
                ToolOption::Git("meld".into()),
                ToolOption::Git("vimdiff".into()),
            ]
        );
        // Meld's merge command lacks {merged}.
        assert_eq!(
            tools.tool_options(Merge, &[]),
            [
                ToolOption::GitConfig,
                ToolOption::None,
                ToolOption::Entry(VSCODE_ID.into())
            ]
        );
        let editors: Vec<_> = tools
            .editor_entries()
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(editors, [VSCODE_ID, "tool-1"]);
        // A chosen Git tool that Git does not list stays selectable.
        tools.diff_tool = ToolChoice::Tool("p4merge".into());
        assert_eq!(
            tools.tool_options(Diff, &[]).last(),
            Some(&ToolOption::Git("p4merge".into()))
        );
        assert_eq!(tools.shown_option(Diff), ToolOption::Git("p4merge".into()));
        assert_eq!(
            ToolOption::Entry("tool-2".into()).label(&tools.entries, None),
            "Meld"
        );
        assert_eq!(
            ToolOption::GitConfig.label(&tools.entries, Some("meld")),
            "Use Git config (meld)"
        );
        assert_eq!(ToolOption::Git("bc".into()).label(&[], None), "bc (Git)");
        assert_eq!(
            ToolOption::Entry("tool-1".into()).choice(),
            ToolChoice::Entry("tool-1".into())
        );
    }

    #[test]
    fn defaults() {
        let settings = Settings::default();
        assert_eq!(settings.version, VERSION);
        assert_eq!(settings.general.auto_fetch_minutes, 5);
        assert_eq!(settings.appearance.theme, ThemeChoice::theme("dusk"));
        assert_eq!(settings.appearance.zoom_percent, 100);
        assert!(!settings.appearance.avatars, "avatars are opt-in");
        assert_eq!(settings.tools.diff_tool, ToolChoice::GitConfig);
        assert_eq!(settings.tools.merge_tool, ToolChoice::GitConfig);
        assert_eq!(settings.tools.editor, None);
        assert_eq!(settings.tools.entries, [ToolEntry::vscode()]);
        assert!(settings.tools.terminal_command.is_empty());
    }

    #[test]
    fn round_trip() {
        let settings = sample();
        assert_eq!(parse(&to_json(&settings)), Ok(settings));
    }

    #[test]
    fn missing_and_unknown_fields_fall_back_to_defaults() {
        let text = r#"{
            "version": 1,
            "general": { "auto_fetch_minutes": 0, "future_option": true },
            "appearance": { "theme": "light" },
            "something_new": [1, 2]
        }"#;
        let settings = parse(text).unwrap();
        assert_eq!(settings.general.auto_fetch_minutes, 0);
        assert_eq!(settings.appearance.theme, ThemeChoice::theme("light"));
        assert_eq!(settings.appearance.zoom_percent, 100);
        assert_eq!(settings.tools, ExternalTools::default());
        assert_eq!(parse("{}"), Ok(Settings::default()));
    }

    #[test]
    fn broken_values_and_versions_are_rejected() {
        assert!(matches!(parse("not json"), Err(ParseError::Invalid(_))));
        assert!(matches!(parse("[1]"), Err(ParseError::Invalid(_))));
        assert!(matches!(
            parse(r#"{"appearance": {"zoom_percent": "big"}}"#),
            Err(ParseError::Invalid(_))
        ));
        assert_eq!(
            parse(r#"{"version": 7}"#),
            Err(ParseError::UnknownVersion(7))
        );
    }

    #[test]
    fn editor_settings_default_when_missing() {
        // A file written before M11-06 has no "editor".
        let settings = parse(r#"{"version": 1, "general": {"auto_fetch_minutes": 5}}"#).unwrap();
        assert_eq!(settings.editor, EditorSettings::default());
        assert_eq!(settings.editor.font_size, 12);
        assert!(settings.editor.line_numbers && settings.editor.syntax_highlighting);
        assert!(!settings.editor.word_wrap);
        // Partial: the rest falls back.
        let settings = parse(r#"{"editor": {"font": "Fira Code", "tab_size": 2}}"#).unwrap();
        assert_eq!(settings.editor.font, "Fira Code");
        assert_eq!(settings.editor.tab_size, 2);
        assert_eq!(settings.editor.font_size, 12);
    }

    #[test]
    fn migration_takes_the_interval_from_the_session_once() {
        let dir = temp_dir("migrate");
        let session = dir.join("session.json");
        std::fs::write(&session, r#"{"version": 1, "auto_fetch_minutes": 12}"#).unwrap();
        let path = dir.join("settings.json");

        let loaded = load(&path, Some(&session));
        assert_eq!(loaded.settings.general.auto_fetch_minutes, 12);
        assert!(loaded.write, "the migrated settings are written");
        assert!(loaded.notice.is_none());

        // Once the settings file exists, the session is not read again.
        let mut changed = loaded.settings.clone();
        changed.general.auto_fetch_minutes = 30;
        save(&path, &to_json(&changed)).unwrap();
        let loaded = load(&path, Some(&session));
        assert_eq!(loaded.settings.general.auto_fetch_minutes, 30);
        assert!(!loaded.write);

        // Without a session value: defaults.
        let loaded = load(&dir.join("other.json"), Some(&dir.join("missing.json")));
        assert_eq!(loaded.settings, Settings::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupted_file_gives_defaults_and_is_kept_as_backup() {
        let dir = temp_dir("corrupt");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ broken").unwrap();

        let loaded = load(&path, None);
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded
            .notice
            .is_some_and(|n| n.contains("defaults are used")));
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("settings.json.bak")).unwrap(),
            "{ broken"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_then_load() {
        let dir = temp_dir("save");
        let path = dir.join("nested").join("settings.json");
        save(&path, &to_json(&sample())).unwrap();
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(load(&path, None).settings, sample());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn zoom_steps_and_bounds() {
        assert_eq!(zoom_step(100, true), 110);
        assert_eq!(zoom_step(100, false), 90);
        assert_eq!(zoom_step(70, false), 70, "lower bound");
        assert_eq!(zoom_step(200, true), 200, "upper bound");
        assert_eq!(zoom_step(190, true), 200);
        assert_eq!(zoom_step(80, false), 70);
        // Off-grid values from an edited file snap to the grid.
        assert_eq!(zoom_step(125, true), 130);
        assert_eq!(zoom_step(125, false), 120);
        assert_eq!(zoom_step(20, true), 80, "out of range is clamped first");
        assert_eq!(zoom_step(500, false), 190);
        let mut level = ZOOM_MIN;
        let mut steps = 0;
        while level < ZOOM_MAX {
            level = zoom_step(level, true);
            steps += 1;
        }
        assert_eq!(steps, 13, "70 % to 200 % in 10 % steps");
        assert_eq!(
            parse(r#"{"appearance": {"zoom_percent": 900}}"#).map(|s| s.appearance.zoom_percent),
            Ok(ZOOM_MAX)
        );
    }

    #[test]
    fn auto_fetch_input() {
        assert_eq!(parse_auto_fetch(" 10 "), Ok(10));
        assert_eq!(parse_auto_fetch("0"), Ok(0));
        assert!(parse_auto_fetch("").is_err());
        assert!(parse_auto_fetch("-1").is_err());
        assert!(parse_auto_fetch("2.5").is_err());
        assert!(parse_auto_fetch("5000").is_err());
    }
}
