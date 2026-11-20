//! Settings view (M5-02): replaces the tab content like GitKraken's
//! preferences; categories on the left, the form of the selected one on the
//! right. Changes apply immediately.

use iced::widget::{button, column, container, row, space, text, text_input, Column};

use crate::graph_columns::ColumnId;
use iced::{Element, Fill, Length};

use crate::app::{App, Message};
use crate::settings::{Category, Page, Settings, ThemeChoice, ToolChoice, ToolPick};
use crate::theme;
use crate::ui::muted;

/// Id of the auto-fetch input, focused when the view opens.
pub const AUTO_FETCH_INPUT: &str = "settings-auto-fetch";

pub fn view<'a>(app: &'a App, page: &'a Page) -> Element<'a, Message> {
    let category_button = |category: Category, enabled: bool| -> Element<'a, Message> {
        let button = button(text(category.title()))
            .on_press_maybe(enabled.then_some(Message::SettingsCategory(category)))
            .padding([6, 12])
            .width(Fill)
            .style(theme::list_item(category == page.category.sidebar()));
        if enabled {
            button.into()
        } else {
            crate::ui::panel::with_tooltip_text(button.into(), NO_REPOSITORY.to_owned())
        }
    };
    // Global categories, then the group of the active tab's repository
    // (M8-01, GitKraken's "Repo-Specific Preferences").
    let repo = app.active_repo_name();
    let categories = Column::with_children(
        Category::GLOBAL
            .into_iter()
            .map(|category| category_button(category, true)),
    )
    .spacing(2)
    .push(repository_header(repo))
    .extend(
        Category::REPOSITORY
            .into_iter()
            .map(|category| category_button(category, repo.is_some())),
    )
    .push(space::vertical())
    .push(
        button(text(format!("About {}", git_core::identity::DISPLAY_NAME)))
            .on_press(Message::AboutOpened)
            .padding([6, 12])
            .width(Fill)
            .style(theme::list_item(false)),
    )
    .padding(8)
    .width(220)
    .height(Fill);

    HIGHLIGHT.with(|h| h.set(page.highlight));
    let form = match page.category {
        Category::General => general(&app.settings, page),
        Category::Appearance => appearance(app),
        Category::Editor => editor(app, page),
        Category::Identities => identities(app),
        Category::Signing => signing(page),
        Category::Templates => templates(app),
        Category::ExternalTools => external_tools(&app.settings, page),
        Category::Identity => repository_identity(app),
        Category::SshKey => ssh_key(app, page),
        Category::Commit => commit_template(app, page),
        Category::RepoEditor => repository_editor(app, page),
        Category::Gitflow => gitflow(app, page),
        Category::Themes => match &page.theme_editor {
            Some(editor) => theme_editor(editor),
            None => themes_page(app),
        },
    };
    HIGHLIGHT.with(|h| h.set(None));

    let header = row![
        text("Preferences").size(18),
        space::horizontal(),
        crate::ui::panel::with_tooltip_text(
            button(crate::icons::icon(crate::icons::Icon::Close, 16.0))
                .on_press(Message::SettingsClosed)
                .padding([4, 8])
                .style(theme::flat)
                .into(),
            "Close (Esc)".to_owned(),
        ),
    ]
    .align_y(iced::Center)
    .padding([10, 16]);

    let body = row![
        container(categories).height(Fill).style(theme::chrome),
        crate::ui::vscroll(
            column![category_heading(app, page.category), form]
                .spacing(16)
                .padding(20)
                .max_width(640)
        )
        .id(SETTINGS_SCROLL)
        .width(Fill)
        .height(Fill),
    ]
    .spacing(1);

    container(column![container(header).width(Fill).style(theme::chrome), body].spacing(1))
        .width(Fill)
        .height(Fill)
        .style(theme::graph)
        .into()
}

/// Button in Appearance → Theme that opens the theme list (ADR-060).
pub const MANAGE_THEMES: &str = "Manage themes…";

/// Why the repository categories are disabled on the start screen.
pub const NO_REPOSITORY: &str = "Open a repository first";

/// Header of the repository group: "Repository · <name>" (cut with "…"
/// and the full name as tooltip when long), "Repository" without one.
fn repository_header<'a>(repo: Option<&str>) -> Element<'a, Message> {
    let label = repository_label(repo);
    let header = container(muted(label.clone()).size(11).wrapping(text::Wrapping::None))
        .padding(iced::Padding::default().top(14).bottom(4).left(12).right(8))
        .width(Fill)
        .clip(true);
    match repo {
        Some(name) if label != format!("Repository · {name}") => {
            crate::ui::panel::with_tooltip_text(header.into(), name.to_owned())
        }
        _ => header.into(),
    }
}

/// Longest repository name shown in the group header.
const HEADER_NAME_CHARS: usize = 22;

/// Text of the repository group header.
pub fn repository_label(repo: Option<&str>) -> String {
    match repo {
        None => "Repository".to_owned(),
        Some(name) if name.chars().count() > HEADER_NAME_CHARS => {
            let cut: String = name.chars().take(HEADER_NAME_CHARS - 1).collect();
            format!("Repository · {cut}…")
        }
        Some(name) => format!("Repository · {name}"),
    }
}

/// Title of the form; Gitflow names the repository it applies to.
/// Text of the "back" link of a sub-page (ADR-060).
pub fn back_label(parent: Category) -> String {
    format!("← {}", parent.title())
}

/// Title of the form; a sub-page shows "← <parent>" above its title
/// (ADR-060: Appearance → Manage themes).
fn category_heading<'a>(app: &App, category: Category) -> Element<'a, Message> {
    let title = text(category_title(app, category)).size(16);
    match category.parent() {
        Some(parent) => column![
            crate::ui::panel::with_tooltip_text(
                button(text(back_label(parent)).size(13))
                    .on_press(Message::SettingsCategory(parent))
                    .padding([2, 0])
                    .style(button::text)
                    .into(),
                "Back (Esc)".to_owned(),
            ),
            title,
        ]
        .spacing(6)
        .into(),
        None => title.into(),
    }
}

fn category_title(app: &App, category: Category) -> String {
    match (category, app.active_repo_name()) {
        (Category::Gitflow, Some(name)) => format!("Gitflow · {name}"),
        (Category::Identity, Some(name)) => format!("Identity · {name}"),
        (Category::SshKey, Some(name)) => format!("SSH key · {name}"),
        (Category::Commit, Some(name)) => format!("Commit · {name}"),
        (Category::RepoEditor, Some(name)) => format!("External editor · {name}"),
        _ => category.title().to_owned(),
    }
}

/// Preferences → Repository → Identity (M8-04): what the next commit uses,
/// with its origin, and the identity choices of the commit panel.
fn repository_identity(app: &App) -> Element<'_, Message> {
    let Some(view) = app.active_tab().and_then(crate::app::Tab::view) else {
        return muted(NO_REPOSITORY).into();
    };
    let resolved = &view.snapshot.identity;
    let value = |label: &'static str, value: &Option<git_core::ConfigValue>| {
        let shown: Element<'_, Message> = match value {
            Some(v) => row![
                text(v.value.clone()),
                muted(v.origin.description()).size(12)
            ]
            .spacing(8)
            .align_y(iced::Center)
            .into(),
            None => text("not set").style(theme::text_warning).into(),
        };
        row![muted(label).width(70), shown]
            .spacing(8)
            .align_y(iced::Center)
    };
    let writable = app.write_block().is_none() && view.busy.is_none();
    let options = crate::app::identity_options(resolved, &app.settings.identities.list);
    let choices = Column::with_children(options.into_iter().map(|option| {
        let writes = matches!(
            option.choice,
            crate::app::IdentityChoice::Saved(_)
                | crate::app::IdentityChoice::UseGlobal
                | crate::app::IdentityChoice::Add
        );
        let label = if option.current {
            format!("{} · current", option.label)
        } else {
            option.label
        };
        button(text(label))
            .on_press_maybe(
                ((writable || !writes) && !option.current)
                    .then_some(Message::IdentityChosen(option.choice)),
            )
            .padding([5, 12])
            .width(Fill)
            .style(theme::list_item(option.current))
            .into()
    }))
    .spacing(2);
    column![
        muted(
            "The identity this repository's commits use: user.name and user.email from its \
               own Git config, otherwise from the global one."
        )
        .size(12),
        column![
            value("Name", &resolved.name),
            value("Email", &resolved.email)
        ]
        .spacing(6),
        field("Use identity", choices.into(), None),
    ]
    .spacing(16)
    .into()
}

/// Preferences → Repository → SSH key (M8-06): the system default or a key
/// for this repository's network operations (`core.sshCommand`).
fn ssh_key<'a>(app: &'a App, page: &'a Page) -> Element<'a, Message> {
    use git_core::{ConfigOrigin, SshSetting};
    let Some(view) = app.active_tab().and_then(crate::app::Tab::view) else {
        return muted(NO_REPOSITORY).into();
    };
    let ssh = &view.snapshot.ssh;
    let local = ssh.origin == Some(ConfigOrigin::Local);
    let custom_local = local && matches!(ssh.setting, SshSetting::Custom(_));
    let writable = app.write_block().is_none() && view.busy.is_none();
    let enabled = writable && (!custom_local || page.ssh_replace);
    let describe = |setting: &SshSetting| match setting {
        SshSetting::SystemDefault => "the system default".to_owned(),
        SshSetting::Key(path) => path.display().to_string(),
        SshSetting::Custom(command) => format!("custom command {command}"),
    };

    let mut content = column![muted(
        "The key Git uses for fetch, pull and push of this repository (core.sshCommand in \
         its Git config). Passphrases are asked for when needed and never stored."
    )
    .size(12)]
    .spacing(16);
    let urls = view
        .snapshot
        .remote_urls
        .iter()
        .map(|(_, url)| url.as_str());
    if !git_core::ssh::has_ssh_remotes(urls) {
        content = content.push(
            text("This repository has no SSH remotes")
                .size(12)
                .style(theme::text_warning),
        );
    }
    // What applies now.
    let current: Element<'a, Message> = match (&ssh.setting, ssh.origin) {
        (SshSetting::Custom(command), Some(ConfigOrigin::Local)) => {
            let mut line = row![text(format!("Custom SSH command: {command}"))]
                .spacing(10)
                .align_y(iced::Center);
            if !page.ssh_replace {
                line = line.push(
                    button(text("Replace…").size(12))
                        .on_press_maybe(
                            writable.then_some(Message::SshReplace { confirmed: false }),
                        )
                        .padding([2, 10])
                        .style(button::secondary),
                );
            }
            line.into()
        }
        (setting, Some(origin)) if origin != ConfigOrigin::Local => text(format!(
            "Inherited {}: {}. Choosing a key overrides it for this repository.",
            origin.description(),
            describe(setting)
        ))
        .into(),
        (setting, _) => text(format!("Uses {}", describe(setting))).into(),
    };
    content = content.push(current);

    // Choices: system default (or the inherited value), the keys, a file.
    let current_key = match (&ssh.setting, local) {
        (SshSetting::Key(path), true) => Some(git_core::ssh::normalized_key_path(path)),
        _ => None,
    };
    let default_label = match ssh.origin {
        Some(origin) if origin != ConfigOrigin::Local => {
            format!("Inherited ({})", origin.description())
        }
        _ => "System default (ssh-agent, ~/.ssh/config)".to_owned(),
    };
    let choice = |label: String, current: bool, message: Message| -> Element<'a, Message> {
        let label = if current {
            format!("{label} · current")
        } else {
            label
        };
        button(text(label))
            .on_press_maybe((enabled && !current).then_some(message))
            .padding([5, 12])
            .width(Fill)
            .style(theme::list_item(current))
            .into()
    };
    let mut choices =
        Column::new()
            .spacing(2)
            .push(choice(default_label, !local, Message::SshKeyChosen(None)));
    match &page.ssh_keys {
        None => choices = choices.push(muted("Looking for keys in ~/.ssh…").size(12)),
        Some(keys) if keys.is_empty() => {
            choices = choices.push(muted("No keys found in ~/.ssh").size(12));
        }
        Some(keys) => {
            for key in keys {
                let current = current_key.as_deref()
                    == Some(git_core::ssh::normalized_key_path(key).as_str());
                choices = choices.push(choice(
                    key.display().to_string(),
                    current,
                    Message::SshKeyChosen(Some(key.clone())),
                ));
            }
        }
    }
    // A key elsewhere: file dialog, or a typed path (like the start screen).
    let mut path = text_input("Path of a key file", &page.ssh_path)
        .padding(6)
        .style(theme::input);
    if enabled {
        path = path
            .on_input(Message::SshPathInput)
            .on_submit(Message::SshPathSubmit);
    }
    let other = row![
        button(text("Browse…"))
            .on_press_maybe(enabled.then_some(Message::SshBrowse))
            .padding([5, 14])
            .style(button::secondary),
        path,
        button(text("Use"))
            .on_press_maybe(
                (enabled && !page.ssh_path.trim().is_empty()).then_some(Message::SshPathSubmit)
            )
            .padding([5, 14])
            .style(button::secondary),
    ]
    .spacing(8)
    .align_y(iced::Center);
    content
        .push(field("Use", choices.into(), None))
        .push(field("Other key", other.into(), None))
        .into()
}

/// Preferences → Gitflow (M7-07): initialize, edit or remove Git flow for
/// the active tab's repository.
fn gitflow<'a>(app: &'a App, page: &'a Page) -> Element<'a, Message> {
    let (Some(form), Some(initialized)) = (&page.flow, app.active_flow_initialized()) else {
        return muted(
            "Open a repository first. Gitflow settings apply to the repository of the \
                      active tab.",
        )
        .into();
    };
    let fields = Column::with_children(crate::settings::FLOW_FIELDS.iter().enumerate().map(
        |(index, label)| {
            let note = match index {
                0 => Some(
                    "Releases and hotfixes are merged and tagged here. Usually main or master \
                     of this repository.",
                ),
                1 => Some("Features start here; releases start from it."),
                5 => Some("Put before the version in tag names, e.g. \"v\" for v1.2.0."),
                _ => None,
            };
            field(
                label,
                text_input("", &form.values[index])
                    .on_input(move |value| Message::FlowFieldChanged(index, value))
                    .on_submit(Message::FlowSave)
                    .padding(6)
                    .style(theme::input)
                    .width(260)
                    .into(),
                note,
            )
        },
    ))
    .spacing(14);
    let problem = form.config().problem();
    let busy = app.active_writable_reason();
    let save_label = if initialized {
        "Save"
    } else {
        "Initialize Gitflow"
    };
    let enabled = problem.is_none() && busy.is_none();
    let mut actions = row![button(text(save_label))
        .on_press_maybe(enabled.then_some(Message::FlowSave))
        .padding([5, 14])
        .style(button::primary)]
    .spacing(8);
    if initialized {
        actions = actions.push(
            button(text("Remove Gitflow"))
                .on_press_maybe(busy.is_none().then_some(Message::FlowRemove))
                .padding([5, 14])
                .style(button::danger),
        );
    }
    let intro = if initialized {
        "Git flow is initialized for this repository (git-flow compatible settings)."
    } else {
        "Initializing writes git-flow compatible settings and creates missing branches: \
         development from the production branch, production from the current commit."
    };
    let mut content = column![muted(intro).size(12), fields].spacing(16);
    if let Some(reason) = problem.or(busy) {
        content = content.push(text(reason).size(12).style(theme::text_danger));
    }
    let mut content = content.push(actions);
    // The graph layout (M13-03), once Git flow is initialized.
    if initialized {
        let on = app
            .active_tab()
            .and_then(crate::app::Tab::view)
            .is_some_and(|v| v.snapshot.flow_layout);
        // Marked experimental (M15-04): a muted pill after the toggle.
        let pill = container(text("Experimental").size(11))
            .padding([1, 6])
            .style(theme::time_marker);
        content = content.push(field(
            "Show the graph in the Gitflow layout",
            row![
                iced::widget::toggler(on)
                    .on_toggle(Message::FlowLayoutSet)
                    .label(if on { "On" } else { "Off" }),
                pill,
            ]
            .spacing(10)
            .align_y(iced::Center)
            .into(),
            "Production left, then hotfix, release, development and features, one color per \
             branch type, also for deleted branches. Experimental: can look messy when the \
             history did not follow Git flow.",
        ));
    }
    content.into()
}

/// Labelled setting with an explanation below.
fn field<'a>(
    label: &'a str,
    control: Element<'a, Message>,
    note: impl Into<Option<&'a str>>,
) -> Element<'a, Message> {
    let mut content = column![text(label), control].spacing(6);
    if let Some(note) = note.into() {
        content = content.push(muted(note).size(12));
    }
    // A field opened from the palette is highlighted for a moment (M12-12).
    if HIGHLIGHT.with(|h| h.get() == Some(label)) {
        return container(content)
            .padding(8)
            .width(Fill)
            .style(theme::setting_highlight)
            .into();
    }
    content.into()
}

thread_local! {
    /// Label of the highlighted field while a page is built (view only).
    static HIGHLIGHT: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// Scrollable of a Preferences page (palette deep links scroll it).
pub const SETTINGS_SCROLL: &str = "settings-page";

/// Label of the default font in the font list.
const DEFAULT_FONT: &str = "Default";

/// Preferences → Editor (M11-06, GitKraken "Editor"): the code views.
fn editor<'a>(app: &'a App, page: &'a Page) -> Element<'a, Message> {
    use crate::editor_style::{MAX_FONT_SIZE, MAX_TAB_SIZE, MIN_FONT_SIZE, MIN_TAB_SIZE};
    let editor = &app.settings.editor;
    let mut fonts = vec![DEFAULT_FONT.to_owned()];
    fonts.extend(page.fonts.iter().cloned());
    if !editor.font.is_empty() && !fonts.contains(&editor.font) {
        // A font from the settings that is not installed (any more).
        fonts.push(editor.font.clone());
    }
    let selected = if editor.font.is_empty() {
        DEFAULT_FONT.to_owned()
    } else {
        editor.font.clone()
    };
    let font = iced::widget::pick_list(fonts, Some(selected), |font: String| {
        Message::EditorFontChosen(if font == DEFAULT_FONT {
            String::new()
        } else {
            font
        })
    })
    .width(280);
    let stepper = |value: u32, min: u32, max: u32, unit: &str, message: fn(u32) -> Message| {
        let step = |icon, target: u32, enabled: bool| -> Element<'a, Message> {
            button(crate::icons::icon(icon, 14.0))
                .on_press_maybe(enabled.then_some(message(target)))
                .padding([5, 12])
                .style(button::secondary)
                .into()
        };
        row![
            step(
                crate::icons::Icon::Minus,
                value.saturating_sub(1),
                value > min
            ),
            text(format!("{value}{unit}"))
                .width(60)
                .align_x(iced::Center),
            step(crate::icons::Icon::Plus, value + 1, value < max),
        ]
        .spacing(6)
        .align_y(iced::Center)
        .into()
    };
    column![
        field(
            "Font",
            font.into(),
            "Monospace fonts installed on this computer; Default is the system's monospace font.",
        ),
        field(
            "Font size",
            stepper(
                editor.font_size,
                MIN_FONT_SIZE,
                MAX_FONT_SIZE,
                " px",
                Message::EditorFontSize
            ),
            None,
        ),
        field(
            "Tab size",
            stepper(
                editor.tab_size,
                MIN_TAB_SIZE,
                MAX_TAB_SIZE,
                "",
                Message::EditorTabSize
            ),
            "Tabs are shown as this many columns; files are not changed.",
        ),
        field(
            "Show line numbers",
            iced::widget::toggler(editor.line_numbers)
                .on_toggle(Message::EditorLineNumbersToggled)
                .label(if editor.line_numbers { "On" } else { "Off" })
                .into(),
            None,
        ),
        field(
            "Word wrap",
            iced::widget::toggler(editor.word_wrap)
                .on_toggle(Message::EditorWordWrapToggled)
                .label(if editor.word_wrap { "On" } else { "Off" })
                .into(),
            "Long lines continue on the next row in diffs; no horizontal scrolling then.",
        ),
        field(
            "Syntax highlighting",
            iced::widget::toggler(editor.syntax_highlighting)
                .on_toggle(Message::EditorHighlightingToggled)
                .label(if editor.syntax_highlighting {
                    "On"
                } else {
                    "Off"
                })
                .into(),
            "Files over 1 MB or 20,000 lines stay plain.",
        ),
        muted("Applies to diffs, blame, file history and the conflict view; the commit message fields keep their font.")
            .size(12),
    ]
    .spacing(18)
    .into()
}

/// Preferences → Repository → External editor (M10-11): the global setting
/// or the same presets and a custom command, stored in the repository config.
fn repository_editor<'a>(app: &'a App, page: &'a Page) -> Element<'a, Message> {
    use crate::settings::EditorPreset;
    let global = &app.settings.tools.editor_command;
    let global_label = crate::app::use_global_editor_label(global);
    let current = app
        .active_tab()
        .and_then(crate::app::Tab::view)
        .and_then(|v| v.snapshot.repo_editor.clone());
    let selected = match (&current, page.repo_editor_custom) {
        (_, true) => EditorPreset::Custom.label().to_owned(),
        (None, false) => global_label.clone(),
        (Some(command), false) => EditorPreset::of(command).label().to_owned(),
    };
    let options: Vec<String> = std::iter::once(global_label)
        .chain(
            EditorPreset::ALL
                .into_iter()
                .filter(|p| *p != EditorPreset::None)
                .map(|p| p.label().to_owned()),
        )
        .collect();
    let picker = iced::widget::pick_list(options, Some(selected), Message::RepoEditorChosen)
        .width(Length::Fixed(320.0));
    // The full global command below the picker (M15-02); wraps.
    let global_line = if global.trim().is_empty() {
        "Global: none".to_owned()
    } else {
        format!("Global: {}", global.trim())
    };
    let mut content = column![column![
        picker,
        muted(global_line).size(12).width(Length::Fixed(320.0))
    ]
    .spacing(4)]
    .spacing(10);
    if current.is_some() || page.repo_editor_custom {
        let changed = page.repo_editor_input.trim() != current.as_deref().unwrap_or("");
        content = content.push(
            row![
                text_input("code {path}", &page.repo_editor_input)
                    .on_input(Message::RepoEditorInput)
                    .on_submit(Message::RepoEditorSave)
                    .padding(6)
                    .style(theme::input)
                    .width(Length::Fixed(320.0)),
                button(text("Save"))
                    .on_press_maybe(changed.then_some(Message::RepoEditorSave))
                    .padding([5, 12])
                    .style(button::secondary),
            ]
            .spacing(8)
            .align_y(iced::Center),
        );
    }
    column![
        text("External editor"),
        content,
        muted(format!(
            "Used for this repository by Alt+E, the toolbar, the palette and the file menus \
             ({} in its Git config); {{path}} is the folder or file.",
            git_core::identity::config_key("externalEditor")
        ))
        .size(12),
    ]
    .spacing(6)
    .into()
}

/// Preferences → Repository → Commit (M10-06): like GitKraken's "Commit
/// Template": apply and remove-comments options, summary with counter,
/// description, save.
fn commit_template<'a>(app: &'a App, page: &'a Page) -> Element<'a, Message> {
    let settings = match &page.template {
        None => return muted("Loading…").into(),
        Some(Err(error)) => {
            return text(format!("Could not read the commit template: {error}"))
                .style(theme::text_danger)
                .into()
        }
        Some(Ok(settings)) => settings,
    };
    let options = &settings.options;
    let check = |label: &'static str, on: bool, apply: bool| {
        iced::widget::checkbox(on)
            .label(label)
            .on_toggle(move |on| Message::TemplateOptionToggled { apply, on })
    };
    let remaining = 72_i64 - page.template_summary.chars().count() as i64;
    let summary = row![
        text_input("Summary", &page.template_summary)
            .on_input(Message::TemplateSummaryInput)
            .padding(6)
            .style(theme::input)
            .width(Length::Fixed(480.0)),
        muted(remaining.to_string()).size(11),
    ]
    .spacing(8)
    .align_y(iced::Center);
    let description = iced::widget::text_editor(&app.template_description)
        .placeholder("Description")
        .on_action(Message::TemplateDescriptionEdited)
        .padding(6)
        .style(theme::editor)
        .height(140)
        .width(480.0);
    let location = match &settings.file {
        Some(file) if file.shared() => format!(
            "Stored in {} (commit.template, {} config: used by all repositories).",
            file.path.display(),
            file.scope
        ),
        Some(file) => format!("Stored in {} (commit.template).", file.path.display()),
        None => format!(
            "Saving creates .git/{}/commit-template.txt and sets commit.template for this \
             repository.",
            git_core::identity::BINARY_NAME
        ),
    };
    column![
        check(
            "Apply this template to commit messages",
            options.apply,
            true
        ),
        check(
            "Remove comments from commit messages",
            options.remove_comments,
            false
        ),
        field("Summary", summary.into(), None),
        description,
        button(text("Save commit template"))
            .on_press(Message::TemplateSave)
            .padding([6, 14])
            .style(button::primary),
        muted(location).size(12),
    ]
    .spacing(14)
    .into()
}

/// Preferences → Templates (M10-07): the commit message prompt for all
/// repositories; the commit template itself is per repository.
fn templates(app: &App) -> Element<'_, Message> {
    let editor = iced::widget::text_editor(&app.prompt_instruction)
        .on_action(Message::PromptInstructionEdited)
        .padding(6)
        .style(theme::editor)
        .height(140)
        .width(560.0);
    let changed = app.settings.templates.commit_prompt != git_core::prompt::DEFAULT_INSTRUCTION;
    column![
        text("Commit message prompt (all repositories)").size(15),
        muted(
            "The instruction at the top of \"Copy commit message prompt…\" (staged files menu, \
             command palette). Paste the prompt into a tool of your choice."
        )
        .size(12),
        editor,
        button(text("Reset to default"))
            .on_press_maybe(changed.then_some(Message::PromptInstructionReset))
            .padding([5, 12])
            .style(button::secondary),
        muted("Commit template: set per repository in Repository · <name> → Commit.").size(12),
        space::vertical().height(12),
        stash_template(app),
    ]
    .spacing(10)
    .into()
}

/// "Stash message" (M10-10): like the external editor setting, a switch,
/// a preset list with "Custom" and a text field for a custom template.
fn stash_template(app: &App) -> Element<'_, Message> {
    use crate::stash_template::{preset_hint, Rendered, PRESETS};
    let templates = &app.settings.templates;
    let enabled = templates.stash_enabled;
    let custom = app.settings_page.as_ref().is_some_and(|p| p.stash_custom)
        || !PRESETS.contains(&templates.stash_template.as_str());
    let selected = if custom {
        crate::app::STASH_CUSTOM.to_owned()
    } else {
        templates.stash_template.clone()
    };
    let options: Vec<String> = PRESETS
        .iter()
        .map(|p| (*p).to_owned())
        .chain(std::iter::once(crate::app::STASH_CUSTOM.to_owned()))
        .collect();
    let choice: Element<'_, Message> = if enabled {
        iced::widget::pick_list(options, Some(selected), Message::StashTemplateChosen)
            .width(Length::Fixed(420.0))
            .into()
    } else {
        // Disabled while the switch is off.
        text_input("", &selected)
            .padding(6)
            .style(theme::input)
            .width(Length::Fixed(420.0))
            .into()
    };
    let mut section = column![
        text("Stash message").size(15),
        iced::widget::checkbox(enabled)
            .label("Use stash message template")
            .on_toggle(Message::StashTemplateToggled),
        choice,
    ]
    .spacing(10);
    if enabled && custom {
        section = section.push(
            text_input("{branch} · {datetime}", &templates.stash_template)
                .on_input(Message::StashTemplateInput)
                .padding(6)
                .style(theme::input)
                .width(Length::Fixed(420.0)),
        );
    }
    if let Some(hint) = preset_hint(&templates.stash_template).filter(|_| enabled && !custom) {
        section = section.push(muted(hint).size(12));
    }
    let preview = app.stash_template_preview();
    let line = match &preview {
        None => muted("Open a repository to see a preview").size(12),
        Some(rendered @ (Rendered::Empty | Rendered::Unknown(_))) => {
            text(rendered.hint().unwrap_or_default()).size(12).style(
                if matches!(rendered, Rendered::Unknown(_)) {
                    theme::text_danger
                } else {
                    theme::text_muted
                },
            )
        }
        Some(Rendered::Message(message)) => muted(format!("Preview: {message}")).size(12),
    };
    if enabled {
        section = section.push(line);
    }
    section
        .push(
            muted(
                "Placeholders: {branch} (\"detached\" without a branch), {date}, {time}, \
                 {datetime}, {sha}, {summary}. A message you enter for a stash always wins.",
            )
            .size(12),
        )
        .into()
}

/// Preferences → Signing (M10-04): format, key, GPG program and the
/// defaults, all in the global Git config; repository values are shown as
/// read-only overrides.
fn signing(page: &Page) -> Element<'_, Message> {
    use git_core::signing::SigningFormat;
    let signing = match &page.signing {
        None => return muted("Loading…").into(),
        Some(Err(error)) => {
            return column![
                text(format!("Could not read the Git config: {error}")).style(theme::text_danger),
                button(text("Retry"))
                    .on_press(Message::SigningRefresh)
                    .padding([5, 12])
                    .style(button::secondary),
            ]
            .spacing(8)
            .into()
        }
        Some(Ok(signing)) => signing,
    };
    let global = &signing.global;
    let format = segmented(
        &[
            (SigningFormat::Gpg, SigningFormat::Gpg.label()),
            (SigningFormat::Ssh, SigningFormat::Ssh.label()),
        ],
        global.format,
        |format| Message::SigningSet {
            key: "gpg.format",
            value: Some(format.config_value().to_owned()),
        },
    );
    let refresh = button(text("Refresh"))
        .on_press(Message::SigningRefresh)
        .padding([5, 12])
        .style(button::secondary);
    let key: Element<'_, Message> = match global.format {
        SigningFormat::Gpg => match &signing.gpg_keys {
            Ok(keys) => {
                let labels: Vec<String> = keys.iter().map(|k| k.label()).collect();
                let selected = global
                    .key
                    .as_ref()
                    .and_then(|id| keys.iter().find(|k| k.id.ends_with(id.as_str())))
                    .map(|k| k.label());
                let mut line =
                    row![
                        iced::widget::pick_list(labels, selected, Message::SigningGpgKeyChosen)
                            .placeholder(if keys.is_empty() {
                                "No secret keys found"
                            } else {
                                "Choose a key"
                            })
                            .width(Length::Fixed(420.0))
                    ]
                    .spacing(8)
                    .align_y(iced::Center);
                line = line.push(refresh);
                line.into()
            }
            Err(error) => column![
                text(error.clone()).size(12).style(theme::text_danger),
                row![refresh].spacing(8),
            ]
            .spacing(6)
            .into(),
        },
        SigningFormat::Ssh => {
            let labels: Vec<String> = signing
                .ssh_keys
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            let selected = global.key.clone();
            row![
                iced::widget::pick_list(labels, selected, |path| Message::SigningSet {
                    key: "user.signingkey",
                    value: Some(path),
                })
                .placeholder("Choose a public key")
                .width(Length::Fixed(420.0)),
                button(text("Browse…"))
                    .on_press(Message::SigningBrowseKey)
                    .padding([5, 12])
                    .style(button::secondary),
            ]
            .spacing(8)
            .align_y(iced::Center)
            .into()
        }
    };
    let program_changed = page.gpg_program_input.trim() != global.program.as_deref().unwrap_or("");
    let program = row![
        text_input("gpg", &page.gpg_program_input)
            .on_input(Message::SigningProgramInput)
            .on_submit(Message::SigningProgramSave)
            .padding(6)
            .style(theme::input)
            .width(Length::Fixed(420.0)),
        button(text("Save"))
            .on_press_maybe(program_changed.then_some(Message::SigningProgramSave))
            .padding([5, 12])
            .style(button::secondary),
    ]
    .spacing(8)
    .align_y(iced::Center);
    let toggle = |label: &'static str, on: bool, key: &'static str| {
        iced::widget::toggler(on)
            .label(label)
            .on_toggle(move |on| Message::SigningSet {
                key,
                value: Some(on.to_string()),
            })
    };
    let mut content = column![
        field("Format", format, None),
        field(
            "Signing key",
            key,
            "Written to user.signingkey: a GPG key id, or the .pub file of an SSH key. \
             Passphrases are asked by GPG's pinentry or the SSH agent.",
        ),
        field(
            "GPG program",
            program.into(),
            "gpg.program; empty uses Git's default (gpg).",
        ),
        toggle(
            "Sign commits by default",
            global.sign_commits,
            "commit.gpgSign"
        ),
        toggle("Sign tags by default", global.sign_tags, "tag.gpgSign"),
    ]
    .spacing(18);
    if !signing.overrides.is_empty() {
        let mut lines = column![text("Overridden in this repository").size(13)].spacing(4);
        for (key, value) in &signing.overrides {
            lines = lines.push(muted(format!("{key} = {value}")).size(12));
        }
        content = content.push(lines);
    }
    content.into()
}

/// Preferences → Identities (M8-03): saved identities, the default and the
/// global sync option.
fn identities(app: &App) -> Element<'_, Message> {
    let identities = &app.settings.identities;
    // Setting or editing the default writes the global config while sync
    // is on: blocked while Git is missing or a write runs.
    let global_block = identities
        .sync_global
        .then(|| app.global_identity_block())
        .flatten();
    let action = |label: &'static str, message: Message, block: Option<&String>| {
        let button = button(text(label).size(12))
            .padding([2, 10])
            .style(button::secondary);
        match block {
            Some(reason) => crate::ui::panel::with_tooltip_text(button.into(), reason.clone()),
            None => button.on_press(message).into(),
        }
    };
    let mut list = Column::new().spacing(6);
    if identities.list.is_empty() {
        list = list.push(muted("No identities yet."));
    }
    for (index, saved) in identities.list.iter().enumerate() {
        let default = identities.default == Some(index);
        let mut line = row![column![text(&saved.name), muted(&saved.email).size(12)].spacing(2)]
            .spacing(8)
            .align_y(iced::Center);
        if default {
            line = line.push(
                container(text("Default").size(11))
                    .padding([1, 6])
                    .style(theme::ref_pill(git_core::RefKind::LocalBranch, true)),
            );
        }
        line = line.push(space::horizontal());
        if !default {
            line = line.push(action(
                "Set as default",
                Message::IdentitySetDefault(index),
                global_block.as_ref(),
            ));
        }
        line = line
            .push(action(
                "Edit",
                Message::IdentityEditStart(index),
                if default { global_block.as_ref() } else { None },
            ))
            .push(action(
                "Remove",
                Message::IdentityRemove {
                    index,
                    confirmed: false,
                },
                None,
            ));
        list = list.push(
            container(line)
                .padding([6, 10])
                .width(Fill)
                .style(theme::dialog_details),
        );
    }
    let add = button(text("Add identity"))
        .on_press(Message::IdentityAddStart)
        .padding([5, 14])
        .style(button::primary);
    let sync = iced::widget::checkbox(identities.sync_global)
        .label("Keep the global Git config updated with the default identity")
        .on_toggle(Message::IdentitySyncToggled);
    column![
        muted(
            "Name and email for commits. A repository uses the identity chosen under \
             \"Committing as\" in the details panel, otherwise Git's global identity."
        )
        .size(12),
        list,
        add,
        field(
            "Default identity",
            sync.into(),
            "While on, setting or editing the default writes user.name and user.email to the \
             global Git config. Off: the global config is never changed.",
        ),
    ]
    .spacing(16)
    .into()
}

fn general<'a>(settings: &'a Settings, page: &'a Page) -> Element<'a, Message> {
    let mut input = column![row![
        text_input("5", &page.auto_fetch_input)
            .id(AUTO_FETCH_INPUT)
            .on_input(Message::AutoFetchInput)
            .padding(6)
            .style(theme::input)
            .width(90),
        muted("minutes"),
    ]
    .spacing(8)
    .align_y(iced::Center)]
    .spacing(4);
    if let Some(error) = &page.error {
        input = input.push(text(error).size(12).style(theme::text_danger));
    }
    let updates = settings.general.check_updates;
    // Default branch name of new repositories (`init.defaultBranch`, M10-03).
    let loaded = page.default_branch.is_some();
    let changed = page
        .default_branch
        .as_ref()
        .is_some_and(|value| value.as_deref().unwrap_or("") != page.default_branch_input.trim());
    let mut branch = text_input("main", &page.default_branch_input)
        .padding(6)
        .style(theme::input)
        .width(200);
    if loaded {
        branch = branch
            .on_input(Message::DefaultBranchInput)
            .on_submit(Message::DefaultBranchSave);
    }
    let mut default_branch = column![row![
        branch,
        button(text("Save"))
            .on_press_maybe(changed.then_some(Message::DefaultBranchSave))
            .padding([6, 12])
            .style(button::secondary),
    ]
    .spacing(8)
    .align_y(iced::Center)]
    .spacing(4);
    if let Some(error) = &page.default_branch_error {
        default_branch = default_branch.push(text(error).size(12).style(theme::text_danger));
    }
    column![
        field(
            "Auto-fetch interval",
            input.into(),
            "Fetches all remotes of open repositories in the background. 0 turns auto-fetch off.",
        ),
        field(
            "Prune remote branches on fetch",
            iced::widget::toggler(settings.general.prune_on_fetch)
                .on_toggle(Message::PruneToggled)
                .label(if settings.general.prune_on_fetch {
                    "On"
                } else {
                    "Off"
                })
                .into(),
            "Fetch, auto-fetch and pull remove remote branches that were deleted on the remote. \
             Off: Git's fetch.prune setting applies.",
        ),
        long_paths(page),
        field(
            "Default branch name",
            default_branch.into(),
            "Used for new repositories (init.defaultBranch in the global Git config). Empty: \
             main is suggested. Existing repositories keep their branches.",
        ),
        field(
            "Check for updates",
            iced::widget::toggler(updates)
                .on_toggle(Message::UpdatesToggled)
                .label(if updates { "On" } else { "Off" })
                .into(),
            "Asks api.github.com for the latest release once a day and shows a notice in the \
             status bar. Nothing is downloaded or installed.",
        ),
    ]
    .spacing(18)
    .into()
}

/// "Support long paths" (`core.longpaths`, global; Windows only, M10-08).
fn long_paths(page: &Page) -> Element<'_, Message> {
    if !cfg!(windows) {
        return space::vertical().height(0).into();
    }
    let on = page.long_paths.unwrap_or(false);
    let mut toggle = iced::widget::toggler(on).label(if on { "On" } else { "Off" });
    if page.long_paths.is_some() {
        toggle = toggle.on_toggle(Message::LongPathsToggled);
    }
    field(
        "Support long paths",
        toggle.into(),
        "Lets Git use paths longer than 260 characters (core.longpaths in the global Git config).",
    )
}

/// Segmented control: one button per option, the chosen one highlighted.
fn segmented<'a, T: Copy + PartialEq + 'a>(
    options: &[(T, &'a str)],
    current: T,
    on_choose: fn(T) -> Message,
) -> Element<'a, Message> {
    row(options.iter().map(|&(value, label)| {
        let chosen = value == current;
        button(text(label))
            .on_press(on_choose(value))
            .padding([5, 14])
            .style(if chosen {
                button::primary
            } else {
                button::secondary
            })
            .into()
    }))
    .spacing(4)
    .into()
}

/// The swatch strip of a theme: background, surface, accent, text and
/// three lane colors, framed in the theme's border color.
fn theme_swatches<'a>(def: &crate::themes::ThemeDef) -> Element<'a, Message> {
    let p = &def.palette;
    let colors = [
        p.bg_graph, p.bg_panel, p.accent, p.text, p.lanes[0], p.lanes[1], p.lanes[2],
    ];
    let border = p.border;
    let strip = row(colors.into_iter().map(|color| {
        container(space::horizontal())
            .width(10)
            .height(16)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(color.into()),
                ..container::Style::default()
            })
            .into()
    }));
    // The strip's frame is the theme's own border, so dark swatches stay
    // apart from a dark background and light ones from a light one.
    container(strip)
        .padding(1)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(border.into()),
            border: iced::Border::default().rounded(2),
            ..container::Style::default()
        })
        .into()
}

/// One theme of the Appearance lists (ADR-058): a swatch strip
/// (background, surface, accent, text, three lane colors), the name and a
/// check mark when it is the current one.
fn theme_entry<'a>(
    def: &crate::themes::ThemeDef,
    current: bool,
    message: Message,
) -> Element<'a, Message> {
    let strip = theme_swatches(def);
    let mark: Element<'a, Message> = if current {
        crate::icons::tinted(crate::icons::Icon::Check, 14.0, |p| p.accent).into()
    } else {
        space::horizontal().width(14).into()
    };
    button(
        row![strip, text(def.name.clone()), space::horizontal(), mark]
            .spacing(10)
            .align_y(iced::Alignment::Center),
    )
    .on_press(message)
    .padding([4, 8])
    .width(Fill)
    .style(theme::list_item(current))
    .into()
}

/// A column of theme entries under a small heading.
fn theme_group<'a>(
    heading: &'a str,
    themes: &[std::sync::Arc<crate::themes::ThemeDef>],
    current: &str,
    message: impl Fn(&str) -> Message,
) -> Element<'a, Message> {
    let mut list = column![muted(heading).size(12)].spacing(2).width(Fill);
    for def in themes {
        list = list.push(theme_entry(def, def.id == current, message(&def.id)));
    }
    list.into()
}

/// The themes listed under a base: in the Theme list's column and in
/// the System list of that mode (ADR-058: only themes of the base).
fn slot_themes(base: crate::theme::ThemeKind) -> Vec<std::sync::Arc<crate::themes::ThemeDef>> {
    crate::themes::of_base(base)
}

/// Appearance → Theme (ADR-058): the themes grouped Dark and Light side by
/// side, then System; with System, the themes used for a dark and a light
/// operating system.
fn theme_choice(app: &App) -> Element<'_, Message> {
    use crate::theme::ThemeKind;
    let appearance = &app.settings.appearance;
    let chosen = match &appearance.theme {
        ThemeChoice::Theme(id) => id.as_str(),
        ThemeChoice::System => "",
    };
    let choose = |id: &str| Message::ThemeChosen(ThemeChoice::theme(id));
    let dark = slot_themes(ThemeKind::Dark);
    let light = slot_themes(ThemeKind::Light);
    let system = appearance.theme == ThemeChoice::System;
    let system_entry = button(
        row![
            text("System"),
            space::horizontal(),
            if system {
                Element::from(crate::icons::tinted(crate::icons::Icon::Check, 14.0, |p| {
                    p.accent
                }))
            } else {
                space::horizontal().width(14).into()
            }
        ]
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::ThemeChosen(ThemeChoice::System))
    .padding([4, 8])
    .width(Fill)
    .style(theme::list_item(system));
    let mut content = column![
        row![
            theme_group("Dark themes", &dark, chosen, choose),
            theme_group("Light themes", &light, chosen, choose),
        ]
        .spacing(16),
        container(system_entry).width(Length::FillPortion(1)),
    ]
    .spacing(8)
    .max_width(640);
    if system {
        content = content.push(
            row![
                theme_group(
                    "When the system is dark",
                    &dark,
                    &appearance.system_dark,
                    |id| Message::SystemThemeMapped(ThemeKind::Dark, id.to_owned()),
                ),
                theme_group(
                    "When the system is light",
                    &light,
                    &appearance.system_light,
                    |id| Message::SystemThemeMapped(ThemeKind::Light, id.to_owned()),
                ),
            ]
            .spacing(16),
        );
    }
    content.into()
}

/// A swatch filled with `color` over the panel (alpha shows as lighter).
fn swatch<'a>(color: iced::Color, width: f32, height: f32) -> Element<'a, Message> {
    container(space::horizontal())
        .width(width)
        .height(height)
        .style(move |t: &iced::Theme| container::Style {
            background: Some(color.into()),
            border: iced::Border::default()
                .rounded(3)
                .width(1)
                .color(theme::palette(t).border),
            ..container::Style::default()
        })
        .into()
}

/// One color of the theme editor (M17-03): label, swatch (opens the RGB
/// sliders), hex field (marked while the value is not a color).
fn color_row<'a>(
    editor: &crate::theme_editor::Editor,
    field: crate::theme_editor::Field,
    extra: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    use crate::theme_editor::Edit;
    let color = editor.color(field);
    let invalid = editor.invalid(field);
    let mut line = row![
        text(field.label()).width(150),
        button(swatch(color, 36.0, 20.0))
            .on_press(Message::ThemeEdit(Edit::Picker(field)))
            .padding(0)
            .style(button::text),
        text_input(
            if field.has_alpha() {
                "#RRGGBBAA"
            } else {
                "#RRGGBB"
            },
            &editor.text(field)
        )
        .on_input(move |value| Message::ThemeEdit(Edit::Hex(field, value)))
        .width(110)
        .padding([3, 6])
        .style(theme::inline_input(invalid)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);
    if invalid {
        line = line.push(
            text(if field.has_alpha() {
                "#RRGGBB or #RRGGBBAA"
            } else {
                "#RRGGBB"
            })
            .size(12)
            .style(theme::text_danger),
        );
    }
    if let Some(extra) = extra {
        line = line.push(extra);
    }
    if editor.picker != Some(field) {
        return line.into();
    }
    // Picker: RGB (and alpha) sliders (M17-03: no extra dependency).
    let rgba = color.into_rgba8();
    let channels: &[(&str, usize)] = if field.has_alpha() {
        &[("R", 0), ("G", 1), ("B", 2), ("A", 3)]
    } else {
        &[("R", 0), ("G", 1), ("B", 2)]
    };
    let sliders = Column::with_children(channels.iter().map(|&(name, channel)| {
        row![
            muted(name).width(14),
            iced::widget::slider(0..=255u8, rgba[channel], move |value| {
                Message::ThemeEdit(Edit::Channel(field, channel, value))
            })
            .width(240),
            text(rgba[channel].to_string()).size(12).width(30),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    }))
    .spacing(4)
    .padding(iced::Padding::default().left(160.0));
    column![line, sliders].spacing(6).into()
}

/// The theme editor (M17-03, ADR-059): name and base, the color groups,
/// Derive, the feature color list and the syntax theme; Save, Save and use,
/// Revert, Close.
fn theme_editor(editor: &crate::theme_editor::Editor) -> Element<'_, Message> {
    use crate::theme::ThemeKind;
    use crate::theme_editor::{Edit, Field};
    let section = |title: &'static str| text(title).size(15);
    let header = row![
        text_input("Name", &editor.data.name)
            .on_input(|name| Message::ThemeEdit(Edit::Name(name)))
            .width(240)
            .padding([4, 8])
            .style(theme::input),
        segmented(
            &[(ThemeKind::Dark, "Dark"), (ThemeKind::Light, "Light")],
            editor.data.base,
            |base| Message::ThemeEdit(Edit::Base(base)),
        ),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);
    let dirty = editor.dirty();
    let buttons = row![
        button(text("Save"))
            .on_press_maybe(dirty.then_some(Message::ThemeEditorSave { use_it: false }))
            .padding([5, 12]),
        button(text("Save and use"))
            .on_press(Message::ThemeEditorSave { use_it: true })
            .padding([5, 12])
            .style(button::secondary),
        button(text("Revert"))
            .on_press_maybe(dirty.then_some(Message::ThemeEditorRevert))
            .padding([5, 12])
            .style(button::secondary),
        space::horizontal(),
        button(text("Close"))
            .on_press(Message::ThemeEditorClose)
            .padding([5, 12])
            .style(button::secondary),
    ]
    .spacing(6);
    let rows = |fields: &[Field]| -> Element<'_, Message> {
        Column::with_children(fields.iter().map(|f| color_row(editor, *f, None)))
            .spacing(6)
            .into()
    };
    let derive = button(text("Derive the other colors from the main colors").size(12))
        .on_press(Message::ThemeEdit(Edit::Derive))
        .padding([4, 10])
        .style(button::secondary);
    let mut main = column![section("Main colors"), rows(&Field::MAIN)].spacing(8);
    if editor.derive_hint {
        main = main.push(
            text("A main color changed: derive the other colors to match?")
                .size(12)
                .style(theme::text_accent),
        );
    }
    main = main.push(derive);
    let lanes: Vec<Field> = (0..8).map(Field::Lane).collect();
    let features = editor.data.palette.flow.features.as_slice().len();
    let mut feature_rows = column![].spacing(6);
    for i in 0..features {
        let remove = (features > 1).then(|| {
            Element::from(
                button(text("Remove").size(12))
                    .on_press(Message::ThemeEdit(Edit::RemoveFeature(i)))
                    .padding([2, 8])
                    .style(button::text),
            )
        });
        feature_rows = feature_rows.push(color_row(editor, Field::Feature(i), remove));
    }
    let mut flow = column![section("Git flow"), rows(&Field::FLOW), feature_rows].spacing(8);
    if features == 1 {
        flow = flow.push(muted("All feature branches use this color").size(12));
    }
    if features < crate::theme::FeatureColors::MAX {
        flow = flow.push(
            button(text("Add feature color").size(12))
                .on_press(Message::ThemeEdit(Edit::AddFeature))
                .padding([4, 10])
                .style(button::secondary),
        );
    }
    let syntax = column![
        section("Syntax"),
        iced::widget::pick_list(
            crate::theme_editor::syntax_names(),
            Some(editor.data.syntax.name().to_owned()),
            |name| Message::ThemeEdit(Edit::Syntax(name)),
        ),
        muted("Colors of code in diffs, blame, file history and the conflict view.").size(12),
    ]
    .spacing(8);
    let controls = column![
        header,
        buttons,
        main,
        column![
            section("More colors"),
            muted("Node text on the graph is black or white, whichever reads better on the lanes.")
                .size(12),
            rows(&Field::MORE)
        ]
        .spacing(8),
        column![section("Graph"), rows(&lanes)].spacing(8),
        flow,
        syntax,
    ]
    .spacing(20)
    .width(Length::FillPortion(3));
    // The preview is drawn with the edited theme; everything else keeps
    // the active one (ADR-044, ADR-059).
    row![
        controls,
        container(crate::ui::theme_preview::view(editor)).width(Length::FillPortion(2)),
    ]
    .spacing(20)
    .into()
}

/// Preferences → Appearance → Manage themes (M17-02, ADR-059): built-in themes (read only,
/// lock icon) and "My themes" with the invalid files and their reason;
/// the file actions below act on the selected theme.
fn themes_page(app: &App) -> Element<'_, Message> {
    use crate::icons::{self, Icon};
    use crate::theme_file::Entry;
    let selected = app.selected_theme();
    let active = app.active_theme().id.clone();
    let entry = |def: &crate::themes::ThemeDef| -> Element<'_, Message> {
        let lock: Element<'_, Message> = if crate::theme_file::stem_of(&def.id).is_none() {
            crate::ui::panel::with_tooltip_text(
                icons::tinted(Icon::Lock, 12.0, |p| p.text_muted).into(),
                "Built-in theme: read only".to_owned(),
            )
        } else {
            space::horizontal().width(12).into()
        };
        let in_use: Element<'_, Message> = if def.id == active {
            muted("In use").size(12).into()
        } else {
            space::horizontal().width(0).into()
        };
        button(
            row![
                lock,
                theme_swatches(def),
                text(def.name.clone()),
                space::horizontal(),
                in_use
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::ThemeSelected(def.id.clone()))
        .padding([4, 8])
        .width(Fill)
        .style(theme::list_item(def.id == selected.id))
        .into()
    };
    let all = crate::themes::all();
    let mut list = column![muted("Built-in themes").size(12)].spacing(2);
    for def in all
        .iter()
        .filter(|d| crate::theme_file::stem_of(&d.id).is_none())
    {
        list = list.push(entry(def));
    }
    list = list.push(space::vertical().height(8));
    list = list.push(muted("My themes").size(12));
    let mut mine = 0;
    for file in &app.theme_files {
        match file {
            Entry::Theme(def) => {
                if let Some(def) = all.iter().find(|d| d.id == def.id) {
                    list = list.push(entry(def));
                    mine += 1;
                }
            }
            Entry::Invalid { file, reason } => {
                list = list.push(
                    container(
                        text(format!("{file} (invalid: {reason})"))
                            .size(12)
                            .style(theme::text_warning),
                    )
                    .padding([4, 8]),
                );
                mine += 1;
            }
        }
    }
    if mine == 0 {
        list = list.push(
            container(muted("None yet. \"New theme\" copies the selected theme.").size(12))
                .padding([4, 8]),
        );
    }
    let user = crate::theme_file::stem_of(&selected.id).is_some();
    let action = |label: &'static str, message: Message, enabled: bool, reason: &'static str| {
        let button = button(text(label))
            .on_press_maybe(enabled.then_some(message))
            .padding([5, 12])
            .style(button::secondary);
        if enabled {
            Element::from(button)
        } else {
            crate::ui::panel::with_tooltip_text(button.into(), reason.to_owned())
        }
    };
    const BUILT_IN: &str = "Built-in themes cannot be changed; \"New theme\" makes a copy";
    let actions = row![
        action("New theme", Message::ThemeNew, true, ""),
        action("Edit…", Message::ThemeEditOpened, user, BUILT_IN),
        action("Rename…", Message::ThemeRename, user, BUILT_IN),
        action(
            "Delete…",
            Message::ThemeDelete { confirmed: false },
            user,
            BUILT_IN
        ),
        action("Import…", Message::ThemeImport, true, ""),
        action("Export…", Message::ThemeExport, true, ""),
        action("Open folder", Message::ThemeOpenFolder, true, ""),
    ]
    .spacing(6)
    .wrap();
    column![
        container(list).max_width(520),
        actions,
        muted(format!(
            "\"New theme\" copies \"{}\" into a file of its own in the themes folder.",
            selected.name
        ))
        .size(12),
    ]
    .spacing(14)
    .into()
}

fn appearance(app: &App) -> Element<'_, Message> {
    let appearance = &app.settings.appearance;
    let system_note = match app.system_theme {
        iced::theme::Mode::Light => "System: follows the operating system (now light).",
        iced::theme::Mode::Dark => "System: follows the operating system (now dark).",
        iced::theme::Mode::None => {
            "System: the operating system reports no mode here, so the dark choice is used."
        }
    };
    let theme_field = field("Theme", theme_choice(app), system_note);
    // Own themes: the theme list and editor open as a sub-page (M17-02,
    // ADR-060).
    let theme_field: Element<'_, Message> = column![
        theme_field,
        button(text(MANAGE_THEMES))
            .on_press(Message::SettingsCategory(Category::Themes))
            .padding([5, 12])
            .style(button::secondary),
    ]
    .spacing(8)
    .into();
    // An unknown chosen or mapped id is named (ADR-044, ADR-058).
    let theme_field: Element<'_, Message> = match app.theme_note() {
        Some(note) => column![
            theme_field,
            text(format!("{note}.")).size(12).style(theme::text_warning)
        ]
        .spacing(4)
        .into(),
        None => theme_field,
    };
    column![
        theme_field,
        field("Zoom", zoom(appearance.zoom_percent), zoom_note()),
        field(
            "Load avatars from Gravatar",
            iced::widget::toggler(appearance.avatars)
                .on_toggle(Message::AvatarsToggled)
                .label(if appearance.avatars { "On" } else { "Off" })
                .into(),
            "Sends a hash of each author's email address to gravatar.com",
        ),
        field(
            "Show toolbar labels",
            iced::widget::toggler(appearance.toolbar_labels)
                .on_toggle(Message::ToolbarLabelsToggled)
                .label(if appearance.toolbar_labels {
                    "On"
                } else {
                    "Off"
                })
                .into(),
            "Off: icons only, the names are in the tooltips.",
        ),
        graph_column_toggle(
            "Show commit author in graph",
            appearance.graph_author,
            ColumnId::Author
        ),
        graph_column_toggle(
            "Show commit date and time in graph",
            appearance.graph_date,
            ColumnId::Date
        ),
        graph_column_toggle(
            "Show commit SHA in graph",
            appearance.graph_sha,
            ColumnId::Sha
        ),
    ]
    .spacing(18)
    .into()
}

/// One of the column options (M11-03); the gear at the right end of the
/// graph header has the same.
fn graph_column_toggle<'a>(label: &'a str, shown: bool, column: ColumnId) -> Element<'a, Message> {
    field(
        label,
        iced::widget::toggler(shown)
            .on_toggle(move |on| Message::GraphColumnShown(column, on))
            .label(if shown { "On" } else { "Off" })
            .into(),
        "Also in the gear menu at the right end of the graph header.",
    )
}

fn zoom<'a>(percent: u32) -> Element<'a, Message> {
    use crate::settings::{zoom_step, ZOOM_DEFAULT, ZOOM_MAX, ZOOM_MIN};
    let step = |label: Element<'a, Message>, target: u32, enabled: bool| {
        let tip = if target > percent {
            zoom_tip("Zoom in", crate::actions::ActionId::ZoomIn)
        } else {
            zoom_tip("Zoom out", crate::actions::ActionId::ZoomOut)
        };
        crate::ui::panel::with_tooltip_text(
            button(label)
                .on_press_maybe(enabled.then_some(Message::ZoomTo(target)))
                .padding([5, 12])
                .style(button::secondary)
                .into(),
            tip,
        )
    };
    row![
        step(
            crate::icons::icon(crate::icons::Icon::Minus, 14.0).into(),
            zoom_step(percent, false),
            percent > ZOOM_MIN
        ),
        text(format!("{percent} %")).width(60).align_x(iced::Center),
        step(
            crate::icons::icon(crate::icons::Icon::Plus, 14.0).into(),
            zoom_step(percent, true),
            percent < ZOOM_MAX
        ),
        step(text("Reset").into(), ZOOM_DEFAULT, percent != ZOOM_DEFAULT),
    ]
    .spacing(6)
    .align_y(iced::Center)
    .into()
}

/// "Ctrl+= / Ctrl+- / Ctrl+0" for this platform.
fn zoom_note() -> &'static str {
    if crate::actions::MAC {
        "⌘= / ⌘- / ⌘0; F11 toggles fullscreen."
    } else {
        "Ctrl+= / Ctrl+- / Ctrl+0; F11 toggles fullscreen."
    }
}

/// Id of the command field of the diff or merge tool.
pub fn tool_input(kind: git_core::ToolKind) -> &'static str {
    match kind {
        git_core::ToolKind::Diff => "settings-diff-tool-command",
        git_core::ToolKind::Merge => "settings-merge-tool-command",
    }
}

/// Entry of the diff/merge tool lists.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolOption {
    pick: ToolPick,
    label: String,
}

impl std::fmt::Display for ToolOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// "Use Git config (meld)", "None", the presets, "Custom", then the tools
/// Git offers that no preset covers (fix after M15).
fn tool_options(
    kind: git_core::ToolKind,
    info: Option<&git_core::ToolInfo>,
    current: &ToolChoice,
) -> Vec<ToolOption> {
    let configured = info.and_then(|i| i.configured.as_deref());
    let available = info.map(|i| i.available.as_slice()).unwrap_or_default();
    ToolPick::list(kind, available, current)
        .into_iter()
        .map(|pick| ToolOption {
            label: pick.label(configured),
            pick,
        })
        .collect()
}

/// The tool list and, for a preset or "Custom", the editable command.
fn tool_control<'a>(
    kind: git_core::ToolKind,
    info: Option<&git_core::ToolInfo>,
    current: &'a ToolChoice,
    custom: bool,
) -> Element<'a, Message> {
    let (on_choose, on_input) = match kind {
        git_core::ToolKind::Diff => (
            Message::DiffToolChosen as fn(ToolPick) -> Message,
            Message::DiffToolCommandInput as fn(String) -> Message,
        ),
        git_core::ToolKind::Merge => (
            Message::MergeToolChosen as fn(ToolPick) -> Message,
            Message::MergeToolCommandInput as fn(String) -> Message,
        ),
    };
    let options = tool_options(kind, info, current);
    let shown = ToolPick::of(current, kind, custom);
    let selected = options.iter().find(|o| o.pick == shown).cloned();
    let picker = iced::widget::pick_list(options, selected, move |option: ToolOption| {
        on_choose(option.pick)
    })
    .width(Length::Fixed(220.0));
    let command = match current {
        ToolChoice::Command(command) => Some(command.as_str()),
        _ if custom => Some(""),
        _ => None,
    };
    let mut control = row![picker].spacing(8).align_y(iced::Center);
    if let Some(command) = command {
        control = control.push(
            text_input(placeholder_hint(kind), command)
                .id(tool_input(kind))
                .on_input(on_input)
                .padding(6)
                .style(theme::input)
                .width(Length::Fixed(420.0)),
        );
    }
    control.into()
}

/// Empty command field: what to type.
fn placeholder_hint(kind: git_core::ToolKind) -> &'static str {
    match kind {
        git_core::ToolKind::Diff => "program {local} {remote}",
        git_core::ToolKind::Merge => "program {local} {remote} {base} {merged}",
    }
}

const DIFF_TOOL_NOTE: &str = "Used by Ctrl+D and \"Open in external diff tool\" (git difftool). \
     In a command, {local} and {remote} are the old and the new version.";

const MERGE_TOOL_NOTE: &str = "Used by \"Open in merge tool\" for conflicted files (git \
     mergetool). In a command, {local} is ours, {remote} theirs, {base} the common version and \
     {merged} the file to save the result to.";

fn external_tools<'a>(settings: &'a Settings, page: &'a Page) -> Element<'a, Message> {
    let tools = &settings.tools;
    let (diff_info, merge_info, note) = match &page.tools {
        Some(Ok((diff, merge))) => (Some(diff), Some(merge), None),
        Some(Err(error)) => (
            None,
            None,
            Some(format!("Could not ask Git for its tools: {error}")),
        ),
        // Loading: the presets are there already.
        None => (None, None, None),
    };
    let mut content = column![].spacing(18);
    if let Some(note) = note {
        content = content.push(muted(note).size(12));
    }
    content
        .push(field(
            "Diff tool",
            tool_control(
                git_core::ToolKind::Diff,
                diff_info,
                &tools.diff_tool,
                page.diff_tool_custom,
            ),
            DIFF_TOOL_NOTE,
        ))
        .push(field(
            "Merge tool",
            tool_control(
                git_core::ToolKind::Merge,
                merge_info,
                &tools.merge_tool,
                page.merge_tool_custom,
            ),
            MERGE_TOOL_NOTE,
        ))
        .push(field(
            "Terminal",
            text_input("Detect automatically", &tools.terminal_command)
                .on_input(Message::TerminalCommandInput)
                .padding(6)
                .style(theme::input)
                .width(Length::Fixed(420.0))
                .into(),
            "Command for Alt+T and the toolbar's Terminal button; {path} is the repository folder. Empty: the first terminal found.",
        ))
        .push(field("External editor", editor_control(settings, page), EDITOR_NOTE))
        .push(field(
            "Delete .orig files after merging",
            iced::widget::toggler(tools.delete_orig)
                .on_toggle(Message::DeleteOrigToggled)
                .label(if tools.delete_orig { "On" } else { "Off" })
                .into(),
            "The merge tool leaves no .orig backup next to the resolved file \
             (mergetool.keepBackup=false).",
        ))
        .into()
}

const EDITOR_NOTE: &str = "Opens the repository (Alt+E, toolbar, palette) and files (file \
     context menus); {path} is the folder or file, without {path} it is added at the end.";

/// Preset list and command template of the external editor (M8-07).
fn editor_control<'a>(settings: &'a Settings, page: &'a Page) -> Element<'a, Message> {
    let template = &settings.tools.editor_command;
    let preset = if page.editor_custom {
        crate::settings::EditorPreset::Custom
    } else {
        crate::settings::EditorPreset::of(template)
    };
    let picker = iced::widget::pick_list(
        crate::settings::EditorPreset::ALL,
        Some(preset),
        Message::EditorPresetChosen,
    )
    .width(Length::Fixed(180.0));
    let mut command = text_input("No editor", template)
        .padding(6)
        .style(theme::input)
        .width(Length::Fixed(320.0));
    if preset != crate::settings::EditorPreset::None {
        command = command.on_input(Message::EditorCommandInput);
    }
    row![picker, command]
        .spacing(8)
        .align_y(iced::Center)
        .into()
}

/// Tooltip of an icon-only zoom button, with its shortcut.
fn zoom_tip(label: &str, action: crate::actions::ActionId) -> String {
    match action.shortcut_text() {
        Some(keys) => format!("{label} ({keys})"),
        None => label.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_options_list_git_config_none_presets_custom_and_git_tools() {
        use git_core::ToolKind;
        let info = git_core::ToolInfo {
            configured: Some("meld".into()),
            available: vec!["meld".into(), "kdiff3".into(), "bc4".into()],
        };
        let labels = |kind, current: &ToolChoice| -> Vec<String> {
            tool_options(kind, Some(&info), current)
                .into_iter()
                .map(|o| o.label)
                .collect()
        };
        assert_eq!(
            labels(ToolKind::Diff, &ToolChoice::GitConfig),
            [
                "Use Git config (meld)",
                "None",
                "VS Code",
                "VSCodium",
                "Zed",
                "Meld",
                "WinMerge",
                "Beyond Compare",
                "Custom",
                "kdiff3 (Git)"
            ]
        );
        // Merge: IntelliJ IDEA, no Zed.
        let merge = labels(ToolKind::Merge, &ToolChoice::GitConfig);
        assert!(merge.contains(&"IntelliJ IDEA".to_owned()));
        assert!(!merge.contains(&"Zed".to_owned()));
        // A chosen tool Git does not list (any more) stays selectable.
        let options = tool_options(ToolKind::Diff, None, &ToolChoice::Tool("custom".into()));
        assert_eq!(options[0].label, "Use Git config (not configured)");
        assert!(options
            .iter()
            .any(|o| o.pick == ToolPick::Git("custom".into())));
    }

    /// The System lists offer only themes of their base (M16-06): five
    /// dark and five light built-ins.
    #[test]
    fn system_lists_hold_their_base_only() {
        use crate::theme::ThemeKind;
        for base in [ThemeKind::Dark, ThemeKind::Light] {
            let themes = slot_themes(base);
            assert!(themes.iter().all(|t| t.base == base), "{base:?}");
            assert!(themes.len() >= 5, "{base:?}");
        }
        let count = slot_themes(ThemeKind::Dark).len() + slot_themes(ThemeKind::Light).len();
        assert_eq!(count, crate::themes::all().len());
    }
}
