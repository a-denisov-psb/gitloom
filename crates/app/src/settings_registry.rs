//! Settings registry (M12-12): one list of the settings with their page,
//! kind and keywords. It feeds the command palette ("Toggle: …", "Set: …
//! ›", "Preferences: …") and the field anchors of the Preferences pages
//! (scroll and highlight). Pure.

use crate::app::Message;
use crate::settings::{Category, DiffMode, EditorPreset, Settings, ThemeChoice};

/// A boolean setting of `settings.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    PruneOnFetch,
    CheckUpdates,
    Avatars,
    ToolbarLabels,
    GraphAuthor,
    GraphDate,
    GraphSha,
    LineNumbers,
    WordWrap,
    SyntaxHighlighting,
    IdentitySync,
    StashTemplate,
    DeleteOrig,
}

impl Toggle {
    pub fn value(self, settings: &Settings) -> bool {
        match self {
            Self::PruneOnFetch => settings.general.prune_on_fetch,
            Self::CheckUpdates => settings.general.check_updates,
            Self::Avatars => settings.appearance.avatars,
            Self::ToolbarLabels => settings.appearance.toolbar_labels,
            Self::GraphAuthor => settings.appearance.graph_author,
            Self::GraphDate => settings.appearance.graph_date,
            Self::GraphSha => settings.appearance.graph_sha,
            Self::LineNumbers => settings.editor.line_numbers,
            Self::WordWrap => settings.editor.word_wrap,
            Self::SyntaxHighlighting => settings.editor.syntax_highlighting,
            Self::IdentitySync => settings.identities.sync_global,
            Self::StashTemplate => settings.templates.stash_enabled,
            Self::DeleteOrig => settings.tools.delete_orig,
        }
    }

    /// The message the Preferences control sends for `on`.
    pub fn message(self, on: bool) -> Message {
        use crate::graph_columns::ColumnId;
        match self {
            Self::PruneOnFetch => Message::PruneToggled(on),
            Self::CheckUpdates => Message::UpdatesToggled(on),
            Self::Avatars => Message::AvatarsToggled(on),
            Self::ToolbarLabels => Message::ToolbarLabelsToggled(on),
            Self::GraphAuthor => Message::GraphColumnShown(ColumnId::Author, on),
            Self::GraphDate => Message::GraphColumnShown(ColumnId::Date, on),
            Self::GraphSha => Message::GraphColumnShown(ColumnId::Sha, on),
            Self::LineNumbers => Message::EditorLineNumbersToggled(on),
            Self::WordWrap => Message::EditorWordWrapToggled(on),
            Self::SyntaxHighlighting => Message::EditorHighlightingToggled(on),
            Self::IdentitySync => Message::IdentitySyncToggled(on),
            Self::StashTemplate => Message::StashTemplateToggled(on),
            Self::DeleteOrig => Message::DeleteOrigToggled(on),
        }
    }
}

/// A setting with a short list of values, changed in daily work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Theme,
    DiffView,
    StashTemplate,
    ExternalEditor,
    /// The repository's own editor (with a repository tab).
    RepoEditor,
}

impl Choice {
    #[cfg(test)]
    pub const ALL: [Self; 5] = [
        Self::Theme,
        Self::DiffView,
        Self::StashTemplate,
        Self::ExternalEditor,
        Self::RepoEditor,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Theme => "Set: Theme ›",
            Self::DiffView => "Set: Diff view ›",
            Self::StashTemplate => "Set: Stash message template ›",
            Self::ExternalEditor => "Set: External editor ›",
            Self::RepoEditor => "Repository: External editor ›",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Theme => "Theme: type a name",
            Self::DiffView => "Diff view: type a mode",
            Self::StashTemplate => "Stash message template: type a template",
            Self::ExternalEditor | Self::RepoEditor => "External editor: type a name",
        }
    }

    pub fn repository(self) -> bool {
        self == Self::RepoEditor
    }

    /// The values (labels) to choose from; `global_editor` is the label of
    /// "use the global editor" for the repository choice.
    pub fn options(self, global_editor: &str) -> Vec<String> {
        let editors = || {
            EditorPreset::ALL
                .into_iter()
                .filter(|p| *p != EditorPreset::Custom)
                .map(|p| p.label().to_owned())
        };
        match self {
            Self::Theme => ["Dark", "Light", "System"].map(str::to_owned).to_vec(),
            Self::DiffView => DiffMode::ALL.iter().map(|m| m.label().to_owned()).collect(),
            Self::StashTemplate => crate::stash_template::PRESETS
                .iter()
                .map(|p| (*p).to_owned())
                .collect(),
            Self::ExternalEditor => editors().collect(),
            Self::RepoEditor => std::iter::once(global_editor.to_owned())
                .chain(editors().filter(|l| l != "None"))
                .collect(),
        }
    }

    /// The messages that set `value` (a label from [`Self::options`]).
    pub fn messages(self, value: &str) -> Vec<Message> {
        match self {
            Self::Theme => {
                let theme = match value {
                    "Light" => ThemeChoice::Light,
                    "System" => ThemeChoice::System,
                    _ => ThemeChoice::Dark,
                };
                vec![Message::ThemeChosen(theme)]
            }
            Self::DiffView => DiffMode::ALL
                .iter()
                .find(|m| m.label() == value)
                .map(|m| vec![Message::DiffModeChanged(*m)])
                .unwrap_or_default(),
            // Choosing a template switches the template on.
            Self::StashTemplate => vec![
                Message::StashTemplateToggled(true),
                Message::StashTemplateChosen(value.to_owned()),
            ],
            Self::ExternalEditor => EditorPreset::ALL
                .into_iter()
                .find(|p| p.label() == value)
                .map(|p| vec![Message::EditorPresetChosen(p)])
                .unwrap_or_default(),
            Self::RepoEditor => vec![Message::RepoEditorChosen(value.to_owned())],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Toggle(Toggle),
    Choice(Choice),
    /// Changed on its page only (text, list, Git config).
    Field,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting {
    /// Label in the palette.
    pub label: &'static str,
    /// Page; `None` for settings without one (diff view).
    pub category: Option<Category>,
    /// Label of the field on the page that is highlighted (`None`: the
    /// page has no labelled field for it).
    pub field: Option<&'static str>,
    pub kind: Kind,
    pub keywords: &'static [&'static str],
}

impl Setting {
    /// Shown on this platform ("Support long paths" is Windows only).
    pub fn available(&self) -> bool {
        self.label != "Support long paths" || cfg!(windows)
    }
}

const fn setting(
    label: &'static str,
    category: Category,
    field: &'static str,
    kind: Kind,
    keywords: &'static [&'static str],
) -> Setting {
    Setting {
        label,
        category: Some(category),
        field: Some(field),
        kind,
        keywords,
    }
}

use Category as C;
use Kind::{Choice as Ch, Field as F, Toggle as T};

/// Every setting, in page order (the order of the fields on the page).
pub const SETTINGS: &[Setting] = &[
    setting(
        "Auto-fetch interval",
        C::General,
        "Auto-fetch interval",
        F,
        &["fetch", "minutes"],
    ),
    setting(
        "Prune remote branches on fetch",
        C::General,
        "Prune remote branches on fetch",
        T(Toggle::PruneOnFetch),
        &["fetch", "prune"],
    ),
    setting(
        "Default branch name",
        C::General,
        "Default branch name",
        F,
        &["init.defaultBranch", "main"],
    ),
    setting(
        "Check for updates",
        C::General,
        "Check for updates",
        T(Toggle::CheckUpdates),
        &["update", "release"],
    ),
    setting(
        "Support long paths",
        C::General,
        "Support long paths",
        F,
        &["windows", "core.longpaths"],
    ),
    setting(
        "Theme",
        C::Appearance,
        "Theme",
        Ch(Choice::Theme),
        &["dark", "light", "color"],
    ),
    setting("Zoom", C::Appearance, "Zoom", F, &["scale", "size"]),
    setting(
        "Load avatars from Gravatar",
        C::Appearance,
        "Load avatars from Gravatar",
        T(Toggle::Avatars),
        &["avatar", "gravatar"],
    ),
    setting(
        "Show toolbar labels",
        C::Appearance,
        "Show toolbar labels",
        T(Toggle::ToolbarLabels),
        &["toolbar", "icons"],
    ),
    setting(
        "Show commit author in graph",
        C::Appearance,
        "Show commit author in graph",
        T(Toggle::GraphAuthor),
        &["column", "author"],
    ),
    setting(
        "Show commit date and time in graph",
        C::Appearance,
        "Show commit date and time in graph",
        T(Toggle::GraphDate),
        &["column", "date"],
    ),
    setting(
        "Show commit SHA in graph",
        C::Appearance,
        "Show commit SHA in graph",
        T(Toggle::GraphSha),
        &["column", "sha", "hash"],
    ),
    setting("Font", C::Editor, "Font", F, &["monospace", "code"]),
    setting("Font size", C::Editor, "Font size", F, &["code"]),
    setting("Tab size", C::Editor, "Tab size", F, &["indent"]),
    setting(
        "Show line numbers",
        C::Editor,
        "Show line numbers",
        T(Toggle::LineNumbers),
        &["diff", "gutter"],
    ),
    setting(
        "Word wrap",
        C::Editor,
        "Word wrap",
        T(Toggle::WordWrap),
        &["diff", "wrap"],
    ),
    setting(
        "Syntax highlighting",
        C::Editor,
        "Syntax highlighting",
        T(Toggle::SyntaxHighlighting),
        &["diff", "colors"],
    ),
    setting(
        "Keep the global Git config updated with the default identity",
        C::Identities,
        "Default identity",
        T(Toggle::IdentitySync),
        &["identity", "profile", "user.name"],
    ),
    setting(
        "Signing format",
        C::Signing,
        "Format",
        F,
        &["gpg", "ssh", "sign"],
    ),
    setting(
        "Signing key",
        C::Signing,
        "Signing key",
        F,
        &["gpg", "ssh", "sign"],
    ),
    setting(
        "GPG program",
        C::Signing,
        "GPG program",
        F,
        &["gpg", "sign"],
    ),
    setting(
        "Sign commits by default",
        C::Signing,
        "Sign commits by default",
        F,
        &["commit.gpgSign"],
    ),
    setting(
        "Sign tags by default",
        C::Signing,
        "Sign tags by default",
        F,
        &["tag.gpgSign"],
    ),
    setting(
        "Commit message prompt",
        C::Templates,
        "Commit message prompt",
        F,
        &["ai", "prompt"],
    ),
    setting(
        "Use stash message template",
        C::Templates,
        "Stash message",
        T(Toggle::StashTemplate),
        &["stash"],
    ),
    setting(
        "Stash message template",
        C::Templates,
        "Stash message",
        Ch(Choice::StashTemplate),
        &["stash"],
    ),
    setting("Diff tool", C::ExternalTools, "Diff tool", F, &["difftool"]),
    setting(
        "Merge tool",
        C::ExternalTools,
        "Merge tool",
        F,
        &["mergetool"],
    ),
    setting(
        "Terminal",
        C::ExternalTools,
        "Terminal",
        F,
        &["shell", "console"],
    ),
    setting(
        "External editor",
        C::ExternalTools,
        "External editor",
        Ch(Choice::ExternalEditor),
        &["editor", "vs code", "ide"],
    ),
    setting(
        "Delete .orig files after merging",
        C::ExternalTools,
        "Delete .orig files after merging",
        T(Toggle::DeleteOrig),
        &["merge", "backup"],
    ),
    setting(
        "Repository identity",
        C::Identity,
        "Use identity",
        F,
        &["profile", "user.name"],
    ),
    setting(
        "Repository SSH key",
        C::SshKey,
        "SSH key",
        F,
        &["core.sshCommand"],
    ),
    setting(
        "Repository commit template",
        C::Commit,
        "Summary",
        F,
        &["commit.template"],
    ),
    setting(
        "Repository external editor",
        C::RepoEditor,
        "External editor",
        Ch(Choice::RepoEditor),
        &["editor"],
    ),
    setting(
        "Gitflow",
        C::Gitflow,
        "Gitflow",
        F,
        &["git flow", "feature", "release", "hotfix"],
    ),
    Setting {
        label: "Diff view",
        category: None,
        field: None,
        kind: Ch(Choice::DiffView),
        keywords: &["hunk", "inline", "split"],
    },
];

/// Every page of the Preferences.
pub fn pages() -> impl Iterator<Item = Category> {
    Category::GLOBAL.into_iter().chain(Category::REPOSITORY)
}

/// Position of `field` among the registry fields of its page, 0.0 (top) to
/// 1.0 (bottom), to scroll the page there.
pub fn scroll_position(category: Category, field: &str) -> f32 {
    let fields: Vec<&str> = SETTINGS
        .iter()
        .filter(|s| s.category == Some(category))
        .filter_map(|s| s.field)
        .collect();
    let index = fields.iter().position(|f| *f == field).unwrap_or(0);
    if fields.len() <= 1 {
        0.0
    } else {
        index as f32 / (fields.len() - 1) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_boolean_setting_has_a_toggle() {
        let toggles: Vec<Toggle> = SETTINGS
            .iter()
            .filter_map(|s| match s.kind {
                Kind::Toggle(t) => Some(t),
                _ => None,
            })
            .collect();
        for toggle in [
            Toggle::PruneOnFetch,
            Toggle::CheckUpdates,
            Toggle::Avatars,
            Toggle::ToolbarLabels,
            Toggle::GraphAuthor,
            Toggle::GraphDate,
            Toggle::GraphSha,
            Toggle::LineNumbers,
            Toggle::WordWrap,
            Toggle::SyntaxHighlighting,
            Toggle::IdentitySync,
            Toggle::StashTemplate,
            Toggle::DeleteOrig,
        ] {
            assert!(toggles.contains(&toggle), "{toggle:?}");
        }
        let settings = Settings::default();
        for toggle in toggles {
            // The message is the one of the Preferences control.
            let _ = toggle.message(!toggle.value(&settings));
        }
    }

    #[test]
    fn every_page_has_settings_and_choices_have_values() {
        for page in pages() {
            assert!(
                SETTINGS.iter().any(|s| s.category == Some(page)),
                "{page:?}"
            );
        }
        for choice in Choice::ALL {
            let options = choice.options("Use global setting");
            assert!(options.len() >= 2, "{choice:?}");
            for option in &options {
                assert!(!choice.messages(option).is_empty(), "{choice:?} {option}");
            }
        }
        assert_eq!(
            scroll_position(Category::General, "Auto-fetch interval"),
            0.0
        );
        assert_eq!(
            scroll_position(Category::General, "Support long paths"),
            1.0
        );
    }
}
