//! Settings view (M5-02): replaces the tab content like GitKraken's
//! preferences; categories on the left, the form of the selected one on the
//! right. Changes apply immediately.

use iced::widget::{button, column, container, row, scrollable, space, text, text_input, Column};
use iced::{Element, Fill, Length};

use crate::app::{App, Message};
use crate::settings::{Category, Page, Settings, ThemeChoice, ToolChoice};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::with_tooltip;

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
    .padding(8)
    .width(220);

    let form = match page.category {
        Category::General => general(page),
        Category::Appearance => appearance(app),
        Category::ExternalTools => external_tools(&app.settings),
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

/// Control of a later task: its current value, disabled with a tooltip.
fn not_yet(value: String) -> Element<'static, Message> {
    with_tooltip(
        button(text(value))
            .padding([5, 12])
            .width(Length::Fixed(260.0))
            .style(button::secondary)
            .into(),
    )
}

fn general(page: &Page) -> Element<'_, Message> {
    let mut input = column![row![
        text_input("5", &page.auto_fetch_input)
            .id(AUTO_FETCH_INPUT)
            .on_input(Message::AutoFetchInput)
            .padding(6)
            .width(90),
        muted("minutes"),
    ]
    .spacing(8)
    .align_y(iced::Center)]
    .spacing(4);
    if let Some(error) = &page.error {
        input = input.push(text(error).size(12).style(theme::text_danger));
    }
    field(
        "Auto-fetch interval",
        input.into(),
        "Fetches all remotes of open repositories in the background. 0 turns auto-fetch off.",
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
            not_yet(if appearance.avatars { "On" } else { "Off" }.to_owned()),
            "Sends a hash of each author's email address to gravatar.com",
        ),
    ]
    .spacing(18)
    .into()
}

fn zoom<'a>(percent: u32) -> Element<'a, Message> {
    use crate::settings::{zoom_step, ZOOM_DEFAULT, ZOOM_MAX, ZOOM_MIN};
    let step = |label: Element<'a, Message>, target: u32, enabled: bool| {
        button(label)
            .on_press_maybe(enabled.then_some(Message::ZoomTo(target)))
            .padding([5, 12])
            .style(button::secondary)
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

fn tool_label(choice: &ToolChoice) -> String {
    match choice {
        ToolChoice::GitConfig => "Use Git config".to_owned(),
        ToolChoice::Tool(name) => name.clone(),
        ToolChoice::None => "None".to_owned(),
    }
}

fn external_tools(settings: &Settings) -> Element<'_, Message> {
    let tools = &settings.tools;
    let terminal = if tools.terminal_command.is_empty() {
        "Detect automatically".to_owned()
    } else {
        tools.terminal_command.clone()
    };
    column![
        field("Diff tool", not_yet(tool_label(&tools.diff_tool)), None),
        field("Merge tool", not_yet(tool_label(&tools.merge_tool)), None),
        field(
            "Terminal",
            not_yet(terminal),
            "Command used by Alt+T; {path} is replaced by the repository folder.",
        ),
    ]
    .spacing(18)
    .into()
}
