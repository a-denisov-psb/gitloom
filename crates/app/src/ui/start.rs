//! Start screen of an empty tab, laid out like GitKraken's New Tab
//! (M10-01): large buttons to open, clone or start a repository on the
//! left; recent repositories, Customize and Resources on the right.

use git_core::identity::{ISSUES_URL, REPOSITORY_URL};
use iced::widget::{button, column, container, row, space, text, text_input, Column};
use iced::{Element, Fill};

use crate::app::{App, Message};
use crate::icons::{self, Icon};
use crate::theme;
use crate::ui::muted;

/// Where the Resources links lead (opened in the browser).
pub fn user_guide_url() -> String {
    format!("{REPOSITORY_URL}/blob/master/docs/user/README.md")
}

pub fn new_issue_url() -> String {
    format!("{ISSUES_URL}/new")
}

pub fn release_notes_url() -> String {
    format!("{REPOSITORY_URL}/blob/master/CHANGELOG.md")
}

pub fn view(app: &App) -> Element<'_, Message> {
    let left = column![
        big_button(
            Icon::FolderOpen,
            "Open a repo",
            crate::actions::ActionId::OpenRepository
                .shortcut_text()
                .unwrap_or_default(),
            Some(Message::OpenRepository),
        ),
        path_input(app),
        big_button(
            Icon::Pull,
            "Clone a repo",
            String::new(),
            Some(Message::CloneStart),
        ),
        big_button(
            Icon::Plus,
            "Start a local repo",
            String::new(),
            Some(Message::InitStart),
        ),
    ]
    .spacing(10)
    .width(280);

    let right = column![
        heading("Recent repositories"),
        recent(app),
        heading("Customize"),
        link("Preferences", Message::SettingsOpened),
        link(
            "Manage identities",
            Message::SettingsOpenedAt(crate::settings::Category::Identities),
        ),
        heading("Resources"),
        link("User guide", Message::OpenLink(user_guide_url())),
        link("Report an issue", Message::OpenLink(new_issue_url())),
        link("Release notes", Message::OpenLink(release_notes_url())),
    ]
    .spacing(4)
    .width(440);

    let content = column![
        text(git_core::identity::DISPLAY_NAME).size(22),
        row![left, right].spacing(40),
    ]
    .spacing(20)
    .align_x(iced::Center);

    container(crate::ui::vscroll(
        container(content).center_x(Fill).padding(24),
    ))
    .center(Fill)
    .style(theme::graph)
    .into()
}

/// Height of the start screen buttons (M11-12; three quarters since M15-07).
const START_BUTTON_HEIGHT: f32 = 45.0;

/// A large action button: icon, title and a second line (shortcut).
/// Without a message it is disabled with "Not available yet".
fn big_button<'a>(
    icon: Icon,
    title: &'a str,
    hint: String,
    message: Option<Message>,
) -> Element<'a, Message> {
    let enabled = message.is_some();
    // Label, shortcut and icon in the normal text color on the light
    // accent fill (M15-07).
    let mut lines = column![text(title).size(14)].spacing(1);
    if !hint.is_empty() {
        lines = lines.push(text(hint).size(11));
    }
    let icon: Element<'a, Message> = if enabled {
        icons::colored(icon, 18.0, theme::start_button_icon).into()
    } else {
        icons::button_icon(icon, 18.0, false).into()
    };
    let content = row![icon, lines].spacing(12).align_y(iced::Center);
    // All three have one size, with or without a shortcut line.
    let content = container(content)
        .height(Fill)
        .align_y(iced::alignment::Vertical::Center);
    let button = button(content)
        .on_press_maybe(message)
        .padding([0, 16])
        .width(Fill)
        .height(START_BUTTON_HEIGHT)
        .style(if enabled {
            theme::start_button
        } else {
            button::secondary
        });
    if enabled {
        button.into()
    } else {
        crate::ui::panel::with_tooltip_text(button.into(), "Not available yet".to_owned())
    }
}

/// Fallback for systems without a folder dialog (e.g. no XDG portal on
/// Linux, ADR-010).
fn path_input(app: &App) -> Element<'_, Message> {
    row![
        text_input("Or enter a path", &app.start_path)
            .id(crate::app::START_PATH_INPUT)
            .on_input(Message::StartPathChanged)
            .on_submit(Message::StartPathSubmitted)
            .padding(6)
            .style(theme::input)
            .width(Fill),
        button(text("Open"))
            .on_press_maybe(
                (!app.start_path.trim().is_empty()).then_some(Message::StartPathSubmitted)
            )
            .padding([6, 12])
            .style(button::secondary),
    ]
    .spacing(6)
    .into()
}

fn heading<'a>(label: &'a str) -> Element<'a, Message> {
    container(muted(label).size(12))
        .padding(iced::Padding::from([0, 10]).top(14).bottom(4))
        .into()
}

/// Recent repositories: name as a link, path muted (up to 20).
fn recent(app: &App) -> Element<'_, Message> {
    if app.session.recent.is_empty() {
        return container(muted("No recent repositories yet").size(12))
            .padding([4, 10])
            .into();
    }
    Column::with_children(app.session.recent.iter().map(|repo| {
        let name = repo.path.file_name().map_or_else(
            || repo.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        // Repositories inside WSL carry a badge (M12-14).
        let mut title = row![text(name).style(link_text)]
            .spacing(8)
            .align_y(iced::Center);
        if let Some(badge) = crate::app::wsl_badge(&repo.path) {
            title = title.push(muted(badge).size(11));
        }
        button(column![title, muted(repo.path.display().to_string()).size(11)].spacing(1))
            .on_press(Message::OpenRecent(repo.path.clone()))
            .padding([4, 10])
            .width(Fill)
            .style(theme::list_item(false))
            .into()
    }))
    .spacing(2)
    .into()
}

fn link<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    button(row![text(label).style(link_text), space::horizontal()])
        .on_press(message)
        .padding([3, 10])
        .width(Fill)
        .style(theme::list_item(false))
        .into()
}

fn link_text(theme: &iced::Theme) -> text::Style {
    text::Style {
        color: Some(theme::palette(theme).accent),
    }
}
