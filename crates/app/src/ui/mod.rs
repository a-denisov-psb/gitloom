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
mod start;
mod statusbar;
mod tabs;
mod toolbar;

use iced::widget::{button, column, container, pane_grid, row, space, text};
use iced::{Element, Fill};

use crate::app::{App, Message, Tab, TabState};
use crate::panes::PaneKind;
use crate::repo_view::{CenterView, RepoView};
use crate::theme;

pub fn view(app: &App) -> Element<'_, Message> {
    let mut layout = column![tabs::view(app)].spacing(1);
    layout = match app.active_tab() {
        Some(tab) => layout.push(tab_body(app, tab)),
        None => layout.push(space::vertical()),
    };
    layout = layout.push(statusbar::view(app));

    container(layout)
        .width(Fill)
        .height(Fill)
        .style(theme::root)
        .into()
}

fn tab_body<'a>(app: &'a App, tab: &'a Tab) -> Element<'a, Message> {
    match &tab.state {
        TabState::Start => start::view(app),
        TabState::Loading { path } => centered(column![
            text("Loading repository…").size(16),
            muted(path.display().to_string()),
        ]),
        TabState::Failed { path, error } => {
            let mut content = column![banner(error, tab.id), muted(path.display().to_string())];
            if app.session.is_recent(path) {
                content = content.push(
                    button(text("Remove from recent").size(12))
                        .on_press(Message::RemoveRecent(tab.id))
                        .padding([3, 10])
                        .style(button::secondary),
                );
            }
            centered(content)
        }
        TabState::Ready(view) => {
            let mut content = column![toolbar::view(view)].spacing(1);
            if let Some(error) = &view.error {
                content = content.push(banner(error, tab.id));
            }
            content.push(panes(app, view)).into()
        }
    }
}

fn panes<'a>(app: &'a App, view: &'a RepoView) -> Element<'a, Message> {
    pane_grid(&app.panes.state, move |_pane, kind, _maximized| {
        let content = match kind {
            PaneKind::Sidebar => sidebar::view(view),
            PaneKind::Center => match &view.center {
                CenterView::Graph => graph::view(view),
                CenterView::Diff(diff) => diff::view(view, diff),
            },
            PaneKind::Panel => panel::view(view),
        };
        pane_grid::Content::new(content)
    })
    .spacing(1)
    .min_size(160)
    .on_resize(6, Message::PaneResized)
    .style(theme::panes)
    .into()
}

/// Error message with a retry button, shown inside the tab.
fn banner<'a>(error: &'a str, tab: crate::app::TabId) -> Element<'a, Message> {
    container(
        row![
            text(error).color(theme::TEXT),
            space::horizontal(),
            button(text("Retry").size(12))
                .on_press(Message::Retry(tab))
                .padding([3, 10])
                .style(button::secondary),
        ]
        .spacing(12)
        .align_y(iced::Center),
    )
    .padding([8, 12])
    .width(Fill)
    .style(theme::error_banner)
    .into()
}

fn centered<'a>(content: iced::widget::Column<'a, Message>) -> Element<'a, Message> {
    container(content.spacing(10).align_x(iced::Center).max_width(720))
        .center(Fill)
        .style(theme::graph)
        .into()
}

/// Muted secondary text.
pub(crate) fn muted<'a>(content: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(content).color(theme::TEXT_MUTED)
}

/// Single-letter badge for a file change (M, A, D, R, ?, !).
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
