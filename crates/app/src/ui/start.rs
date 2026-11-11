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
        big_button(Icon::Pull, "Clone a repo", String::new(), None),
        big_button(Icon::Plus, "Start a local repo", String::new(), None),
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

/// A large action button: icon, title and a muted second line (shortcut).
/// Without a message it is disabled with "Not available yet".
fn big_button<'a>(
    icon: Icon,
    title: &'a str,
    hint: String,
    message: Option<Message>,
) -> Element<'a, Message> {
    let enabled = message.is_some();
    let title = text(title).size(15).style(if enabled {
        theme::text_normal
    } else {
        theme::text_faint
    });
    let mut lines = column![title].spacing(2);
    if !hint.is_empty() {
        lines = lines.push(muted(hint).size(11));
    }
    let content = row![icons::button_icon(icon, 20.0, enabled), lines]
        .spacing(12)
        .align_y(iced::Center);
    let button = button(content)
        .on_press_maybe(message)
        .padding([12, 16])
        .width(Fill)
        .style(button::secondary);
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
        button(
            column![
                text(name).style(link_text),
                muted(repo.path.display().to_string()).size(11)
            ]
            .spacing(1),
        )
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
