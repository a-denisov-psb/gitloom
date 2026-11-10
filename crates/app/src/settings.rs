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
}

impl Default for General {
    fn default() -> Self {
        Self {
            auto_fetch_minutes: crate::fetch::DEFAULT_INTERVAL_MINUTES,
            check_updates: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
    /// Follow the operating system.
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub theme: ThemeChoice,
    /// Window scale in percent (70–200).
    pub zoom_percent: u32,
    /// Load avatars from Gravatar (ADR-026); off by default.
    pub avatars: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::Dark,
            zoom_percent: ZOOM_DEFAULT,
            avatars: false,
        }
    }
}

/// External diff or merge tool.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolChoice {
    /// Whatever `diff.tool` / `merge.tool` say.
    #[default]
    GitConfig,
    /// A tool by name, passed as `--tool=<name>`.
    Tool(String),
    None,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalTools {
    pub diff_tool: ToolChoice,
    pub merge_tool: ToolChoice,
    /// Command template with `{path}`; empty means auto-detection.
    pub terminal_command: String,
}

/// Category of the settings view (list on the left).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    General,
    Appearance,
    /// Saved commit identities and the default (M8-03).
    Identities,
    ExternalTools,
    /// Identity of the active tab's repository (M8-04).
    Identity,
    /// SSH key of the active tab's repository (M8-06).
    SshKey,
    /// Git flow of the active tab's repository (M7-07).
    Gitflow,
}

impl Category {
    /// Categories that apply everywhere, at the top of the list.
    pub const GLOBAL: [Self; 4] = [
        Self::General,
        Self::Appearance,
        Self::Identities,
        Self::ExternalTools,
    ];
    /// Categories of the active tab's repository, below the group header
    /// "Repository · <name>" (M8-01, like GitKraken's repo-specific
    /// preferences).
    pub const REPOSITORY: [Self; 3] = [Self::Identity, Self::SshKey, Self::Gitflow];

    /// The category belongs to the repository group.
    pub fn is_repository(self) -> bool {
        Self::REPOSITORY.contains(&self)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Identities => "Identities",
            Self::Identity => "Identity",
            Self::SshKey => "SSH key",
            Self::ExternalTools => "External tools",
            Self::Gitflow => "Gitflow",
        }
    }
}

/// State of the open settings view.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub category: Category,
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
            auto_fetch_input: settings.general.auto_fetch_minutes.to_string(),
            error: None,
            tools: None,
            flow: None,
            ssh_keys: None,
            ssh_path: String::new(),
            ssh_replace: false,
            repo: None,
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
        settings.appearance.theme = ThemeChoice::Light;
        settings.appearance.zoom_percent = 120;
        settings.appearance.avatars = true;
        settings.tools.diff_tool = ToolChoice::Tool("meld".into());
        settings.tools.merge_tool = ToolChoice::None;
        settings.tools.terminal_command = "alacritty --working-directory {path}".into();
        settings.diff.view_mode = DiffMode::Split;
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

    #[test]
    fn defaults() {
        let settings = Settings::default();
        assert_eq!(settings.version, VERSION);
        assert_eq!(settings.general.auto_fetch_minutes, 5);
        assert_eq!(settings.appearance.theme, ThemeChoice::Dark);
        assert_eq!(settings.appearance.zoom_percent, 100);
        assert!(!settings.appearance.avatars, "avatars are opt-in");
        assert_eq!(settings.tools.diff_tool, ToolChoice::GitConfig);
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
        assert_eq!(settings.appearance.theme, ThemeChoice::Light);
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
