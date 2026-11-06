//! Settings view (M5-02): replaces the tab content like GitKraken's
//! preferences; categories on the left, the form of the selected one on the
//! right. Changes apply immediately.

use iced::widget::{button, column, container, row, scrollable, space, text, text_input, Column};
use iced::{Element, Fill, Length};

use crate::app::{App, Message};
use crate::settings::{Category, Page, Settings, ThemeChoice, ToolChoice};
use crate::theme;
use crate::ui::muted;

/// Id of the auto-fetch input, focused when the view opens.
pub const AUTO_FETCH_INPUT: &str = "settings-auto-fetch";

pub fn view<'a>(app: &'a App, page: &'a Page) -> Element<'a, Message> {
    let categories = Column::with_children(Category::ALL.into_iter().map(|category| {
        button(text(category.title()))
            .on_press(Message::SettingsCategory(category))
            .padding([6, 12])
            .width(Fill)
            .style(theme::list_item(category == page.category))
            .into()
    }))
    .spacing(2)
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

    let form = match page.category {
        Category::General => general(&app.settings, page),
        Category::Appearance => appearance(app),
        Category::ExternalTools => external_tools(&app.settings, page),
    };

    let header = row![
        text("Preferences").size(18),
        space::horizontal(),
        button(text("Close"))
            .on_press(Message::SettingsClosed)
            .padding([4, 14])
            .style(button::secondary),
    ]
    .align_y(iced::Center)
    .padding([10, 16]);

    let body = row![
        container(categories).height(Fill).style(theme::chrome),
        scrollable(
            column![text(page.category.title()).size(16), form]
                .spacing(16)
                .padding(20)
                .max_width(640)
        )
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
    content.into()
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
    column![
        field(
            "Auto-fetch interval",
            input.into(),
            "Fetches all remotes of open repositories in the background. 0 turns auto-fetch off.",
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

fn appearance(app: &App) -> Element<'_, Message> {
    let appearance = &app.settings.appearance;
    let system_note = match app.system_theme {
        iced::theme::Mode::Light => "System: follows the operating system (now light).",
        iced::theme::Mode::Dark => "System: follows the operating system (now dark).",
        iced::theme::Mode::None => {
            "System: the operating system reports no preference here, so Dark is used."
        }
    };
    column![
        field(
            "Theme",
            segmented(
                &[
                    (ThemeChoice::Dark, "Dark"),
                    (ThemeChoice::Light, "Light"),
                    (ThemeChoice::System, "System"),
                ],
                appearance.theme,
                Message::ThemeChosen,
            ),
            system_note,
        ),
        field("Zoom", zoom(appearance.zoom_percent), zoom_note()),
        field(
            "Load avatars from Gravatar",
            iced::widget::toggler(appearance.avatars)
                .on_toggle(Message::AvatarsToggled)
                .label(if appearance.avatars { "On" } else { "Off" })
                .into(),
            "Sends a hash of each author's email address to gravatar.com",
        ),
    ]
    .spacing(18)
    .into()
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

/// Entry of the diff/merge tool lists.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolOption {
    choice: ToolChoice,
    label: String,
}

impl std::fmt::Display for ToolOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// "Use Git config (meld)", the installed tools, "None"; the current
/// choice stays listed even if Git does not offer it (any more).
fn tool_options(info: Option<&git_core::ToolInfo>, current: &ToolChoice) -> Vec<ToolOption> {
    let configured = info
        .and_then(|i| i.configured.clone())
        .unwrap_or_else(|| "not configured".to_owned());
    let mut options = vec![ToolOption {
        choice: ToolChoice::GitConfig,
        label: format!("Use Git config ({configured})"),
    }];
    let mut names: Vec<String> = info.map(|i| i.available.clone()).unwrap_or_default();
    if let ToolChoice::Tool(name) = current {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    options.extend(names.into_iter().map(|name| ToolOption {
        label: name.clone(),
        choice: ToolChoice::Tool(name),
    }));
    options.push(ToolOption {
        choice: ToolChoice::None,
        label: "None".to_owned(),
    });
    options
}

fn tool_picker<'a>(
    info: Option<&git_core::ToolInfo>,
    current: &ToolChoice,
    on_choose: fn(ToolChoice) -> Message,
) -> Element<'a, Message> {
    let options = tool_options(info, current);
    let selected = options.iter().find(|o| &o.choice == current).cloned();
    iced::widget::pick_list(options, selected, move |option: ToolOption| {
        on_choose(option.choice)
    })
    .width(Length::Fixed(320.0))
    .into()
}

fn external_tools<'a>(settings: &'a Settings, page: &'a Page) -> Element<'a, Message> {
    let tools = &settings.tools;
    let (diff_info, merge_info, note) = match &page.tools {
        Some(Ok((diff, merge))) => (Some(diff), Some(merge), None),
        Some(Err(error)) => (
            None,
            None,
            Some(format!("Could not ask Git for its tools: {error}")),
        ),
        None => (
            None,
            None,
            Some("Open a repository to list the tools Git offers.".to_owned()),
        ),
    };
    let mut content = column![].spacing(18);
    if let Some(note) = note {
        content = content.push(muted(note).size(12));
    }
    content
        .push(field(
            "Diff tool",
            tool_picker(diff_info, &tools.diff_tool, Message::DiffToolChosen),
            "Used by Ctrl+D and \"Open in external diff tool\" (git difftool).",
        ))
        .push(field(
            "Merge tool",
            tool_picker(merge_info, &tools.merge_tool, Message::MergeToolChosen),
            "Used by \"Open in merge tool\" for conflicted files (git mergetool).",
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
    fn tool_options_list_git_config_tools_and_none() {
        let info = git_core::ToolInfo {
            configured: Some("meld".into()),
            available: vec!["meld".into(), "kdiff3".into()],
        };
        let labels: Vec<String> = tool_options(Some(&info), &ToolChoice::GitConfig)
            .into_iter()
            .map(|o| o.label)
            .collect();
        assert_eq!(labels, ["Use Git config (meld)", "meld", "kdiff3", "None"]);
        // A chosen tool Git does not list (any more) stays selectable.
        let options = tool_options(None, &ToolChoice::Tool("custom".into()));
        assert_eq!(options[0].label, "Use Git config (not configured)");
        assert!(options
            .iter()
            .any(|o| o.choice == ToolChoice::Tool("custom".into())));
    }
}
