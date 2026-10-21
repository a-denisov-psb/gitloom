//! View layer. Layout mirrors GitKraken:
//!
//! ```text
//! +--------------------------------------------------------------+
//! | repo tabs                                                    |
//! | toolbar: repo/branch pickers | actions            | terminal |
//! +-----------+--------------------------------+-----------------+
//! | sidebar   | graph (or diff)                | details/staging |
//! +-----------+--------------------------------+-----------------+
//! | status bar                                                   |
//! +--------------------------------------------------------------+
//! ```

mod diff;
mod graph;
mod panel;
mod sidebar;
mod statusbar;
mod tabs;
mod toolbar;

use iced::widget::{button, column, container, pane_grid, text};
use iced::{Element, Fill};

use crate::app::{App, CenterView, Message, PaneKind};
use crate::theme;

pub fn view(app: &App) -> Element<'_, Message> {
    let body: Element<'_, Message> = match app.repo() {
        Some(repo) => pane_grid(&app.panes, move |_pane, kind, _maximized| {
            let content = match kind {
                PaneKind::Sidebar => sidebar::view(repo),
                PaneKind::Center => match &repo.center {
                    CenterView::Graph => graph::view(repo),
                    CenterView::Diff(target) => diff::view(repo, target),
                },
                PaneKind::Panel => panel::view(repo),
            };
            pane_grid::Content::new(content)
        })
        .spacing(1)
        .min_size(160)
        .on_resize(6, Message::PaneResized)
        .style(theme::panes)
        .into(),
        None => empty_state(),
    };

    let mut layout = column![tabs::view(app)].spacing(1);
    if let Some(repo) = app.repo() {
        layout = layout.push(toolbar::view(repo));
    }
    layout = layout.push(body).push(statusbar::view(app));

    container(layout)
        .width(Fill)
        .height(Fill)
        .style(theme::root)
        .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
    let content = column![
        text("No repository open").size(18),
        button(text("Open sample repository"))
            .on_press(Message::TabOpened)
            .padding([6, 14])
            .style(button::primary),
    ]
    .spacing(12)
    .align_x(iced::Center);

    container(content).center(Fill).style(theme::graph).into()
}

/// Muted secondary text.
pub(crate) fn muted<'a>(content: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(content).color(theme::TEXT_MUTED)
}

/// Single-letter badge for a file change (M, A, D, R, ?).
pub(crate) fn change_badge<'a>(kind: git_core::ChangeKind) -> text::Text<'a> {
    use git_core::ChangeKind;
    let letter = match kind {
        ChangeKind::Added => "A",
        ChangeKind::Modified => "M",
        ChangeKind::Deleted => "D",
        ChangeKind::Renamed => "R",
        ChangeKind::Untracked => "?",
        ChangeKind::Conflicted => "!",
    };
    text(letter)
        .color(theme::change_color(kind))
        .width(16)
        .font(iced::Font::MONOSPACE)
}
