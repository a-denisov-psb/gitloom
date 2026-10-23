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

pub mod context_area;
mod diff;
mod graph;
mod overlays;
pub(crate) mod panel;
mod sidebar;
mod start;
pub(crate) mod statusbar;
mod tabs;
mod toolbar;

pub use graph::reveal_offset;

use iced::widget::{button, column, container, pane_grid, row, space, text};
use iced::{Element, Fill};

use crate::app::{App, Message, Tab, TabState};
use crate::panes::PaneKind;
use crate::repo_view::{CenterView, RepoView};
use crate::theme;

pub fn view(app: &App) -> Element<'_, Message> {
    let mut layout = column![tabs::view(app)].spacing(1);
    if let Some(notice) = &app.notice {
        layout = layout.push(notice_banner(notice));
    }
    layout = match app.active_tab() {
        Some(tab) => layout.push(tab_body(app, tab)),
        None => layout.push(space::vertical()),
    };
    layout = layout.push(statusbar::view(app));

    let base: Element<'_, Message> = container(layout)
        .width(Fill)
        .height(Fill)
        .style(theme::root)
        .into();
    let mut layers = vec![base];
    if let Some(menu) = &app.context_menu {
        layers.push(overlays::context_menu(menu, app.window_size));
    }
    if let Some(modal) = &app.modal {
        layers.push(overlays::modal(modal));
    }
    if layers.len() == 1 {
        return layers.remove(0);
    }
    iced::widget::Stack::with_children(layers)
        .width(Fill)
        .height(Fill)
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
            let mut content = column![toolbar::view(view, app.pull_mode)].spacing(1);
            if let Some(state) = view.snapshot.state_banner() {
                content = content.push(state_banner(view, state));
            }
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

/// App-wide one-time notice with a dismiss button.
fn notice_banner(notice: &str) -> Element<'_, Message> {
    container(
        row![
            text(notice).color(theme::TEXT),
            space::horizontal(),
            button(text("Dismiss").size(12))
                .on_press(Message::DismissNotice)
                .padding([3, 10])
                .style(button::secondary),
        ]
        .spacing(12)
        .align_y(iced::Center),
    )
    .padding([8, 12])
    .width(Fill)
    .style(theme::notice_banner)
    .into()
}

/// Operation in progress, like GitKraken's "Merging X into Y" bar. Abort
/// and continue arrive with M4 and are shown disabled.
fn state_banner(view: &RepoView, state: String) -> Element<'_, Message> {
    let git_state = &view.snapshot.state;
    let hint = if git_state.concluded_by_commit() {
        "Resolve conflicts, stage them and commit to conclude."
    } else {
        "Resolve conflicts and stage them, then continue in a terminal."
    };
    let disabled = |label: &'static str| {
        panel::with_tooltip(
            button(text(label).size(12))
                .padding([3, 10])
                .style(button::secondary)
                .into(),
        )
    };
    let mut actions = row![].spacing(6);
    if !git_state.concluded_by_commit() {
        actions = actions.push(disabled("Continue"));
    }
    actions = actions.push(disabled("Abort"));
    container(
        row![
            text(state).color(theme::TEXT),
            muted(hint).size(12),
            space::horizontal(),
            actions,
        ]
        .spacing(12)
        .align_y(iced::Center),
    )
    .padding([6, 12])
    .width(Fill)
    .style(theme::state_banner)
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

/// Rows `first..last` of a fixed-height list intersecting the viewport,
/// extended by `overscan` rows on both sides.
pub fn rows_in_view(
    offset: f32,
    height: f32,
    total: usize,
    row_height: f32,
    overscan: usize,
) -> (usize, usize) {
    let top = (offset.max(0.0) / row_height).floor() as usize;
    let bottom = ((offset.max(0.0) + height.max(0.0)) / row_height).ceil() as usize;
    let first = top.saturating_sub(overscan).min(total);
    let last = (bottom + overscan).min(total);
    (first, last.max(first))
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
