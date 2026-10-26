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
        Category::Appearance => appearance(&app.settings),
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
        input = input.push(text(error).size(12).color(theme::DANGER));
    }
    field(
        "Auto-fetch interval",
        input.into(),
        "Fetches all remotes of open repositories in the background. 0 turns auto-fetch off.",
    )
}

fn appearance(settings: &Settings) -> Element<'_, Message> {
    let appearance = &settings.appearance;
    let theme = match appearance.theme {
        ThemeChoice::Dark => "Dark",
        ThemeChoice::Light => "Light",
        ThemeChoice::System => "System",
    };
    column![
        field("Theme", not_yet(theme.to_owned()), None),
        field(
            "Zoom",
            not_yet(format!("{} %", appearance.zoom_percent)),
            "Ctrl+= / Ctrl+- / Ctrl+0",
        ),
        field(
            "Load avatars from Gravatar",
            not_yet(if appearance.avatars { "On" } else { "Off" }.to_owned()),
            "Sends a hash of each author's email address to gravatar.com",
        ),
    ]
    .spacing(18)
    .into()
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
