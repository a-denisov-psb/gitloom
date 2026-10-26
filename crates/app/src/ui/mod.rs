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

mod conflict;
pub mod context_area;
mod diff;
mod graph;
pub(crate) mod overlays;
pub(crate) mod panel;
mod rebase;
pub(crate) mod settings;
mod sidebar;
mod start;
pub(crate) mod statusbar;
mod tabs;
mod toolbar;

pub use graph::reveal_offset;

/// Id of the inline new-branch input in the graph.
pub const NEW_BRANCH_INPUT: &str = "new-branch-input";

/// Id of the inline rename input in the sidebar.
pub const RENAME_INPUT: &str = "rename-input";

use iced::widget::{button, column, container, pane_grid, row, space, text};
use iced::{Element, Fill};

use crate::app::{App, Message, Tab, TabState};
use crate::panes::PaneKind;
use crate::repo_view::{CenterView, RepoView};
use crate::theme;

/// Makes a branch entry (graph pill or sidebar row) draggable onto other
/// branches and a drop target itself; `highlight` outlines it as the
/// current drop target.
pub(crate) fn draggable_branch<'a>(
    content: Element<'a, Message>,
    name: &str,
    kind: git_core::RefKind,
    highlight: bool,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = if highlight {
        container(content).style(theme::drop_target).into()
    } else {
        content
    };
    let event = |make: fn(String, git_core::RefKind) -> Message| make(name.to_owned(), kind);
    let drop_name = name.to_owned();
    context_area::drag_area(
        content,
        context_area::DragEvents {
            start: event(|name, kind| Message::BranchDragStart { name, kind }),
            enter: event(|name, kind| Message::BranchDragEnter { name, kind }),
            leave: event(|name, kind| Message::BranchDragLeave { name, kind }),
        },
        move |position| Message::BranchDropped {
            name: drop_name.clone(),
            kind,
            position,
        },
    )
}

pub fn view(app: &App) -> Element<'_, Message> {
    let mut layout = column![tabs::view(app)].spacing(1);
    if let Some(notice) = &app.notice {
        layout = layout.push(notice_banner(notice));
    }
    layout = match (&app.settings_page, app.active_tab()) {
        (Some(page), _) => layout.push(settings::view(app, page)),
        (None, Some(tab)) => layout.push(tab_body(app, tab)),
        (None, None) => layout.push(space::vertical()),
    };
    layout = layout.push(statusbar::view(app));

    let base: Element<'_, Message> = container(layout)
        .width(Fill)
        .height(Fill)
        .style(theme::root)
        .into();
    // Ends a branch drag released outside a drop target (M4-11).
    let base = context_area::release_area(base, Message::BranchDragEnd);
    let mut layers = vec![base];
    if let Some(menu) = &app.context_menu {
        layers.push(overlays::context_menu(menu, app.window_size));
    }
    if let Some(form) = &app.form {
        layers.push(overlays::form(form));
    }
    if let Some(dialog) = &app.message_dialog {
        layers.push(overlays::message_dialog(dialog));
    }
    if let Some(filter) = &app.shortcut_list {
        layers.push(overlays::shortcut_list(filter));
    }
    if let Some(modal) = &app.modal {
        layers.push(overlays::modal(modal));
    }
    // Always a stack, even without overlays: switching between a bare base
    // and a stack would change the widget tree, and iced would drop the
    // widget state below (graph scroll position, focus) when a menu opens.
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
                CenterView::Conflict(conflict) => conflict::view(view, conflict),
                CenterView::Rebase(panel) => rebase::view(view, panel),
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

/// Operation in progress, like GitKraken's "Merging X into Y" bar, with
/// Continue (uses the commit panel's message), Skip and Abort.
fn state_banner(view: &RepoView, state: String) -> Element<'_, Message> {
    let Some(operation) = view.snapshot.state.kind() else {
        return space::horizontal().into();
    };
    let conflicts = !view.conflicted_files().is_empty();
    let hint = if conflicts {
        "Resolve conflicts and stage them, then continue."
    } else {
        "Check the message in the commit panel, then continue."
    };
    let idle = view.write_block.is_none() && view.busy.is_none();
    let action = |label: &'static str, style: fn(&iced::Theme, button::Status) -> button::Style| {
        button(text(label).size(12)).padding([3, 10]).style(style)
    };
    let blocker = crate::app::commit_blocker(view, false);
    let continue_button = action("Continue", button::primary)
        .on_press_maybe((idle && blocker.is_none()).then_some(Message::ContinueOperation));
    let continue_button: Element<'_, Message> = match (&view.write_block, blocker) {
        (Some(_), _) => panel::write_tooltip(continue_button.into(), view),
        (None, Some(reason)) => panel::with_tooltip_text(continue_button.into(), reason),
        (None, None) => continue_button.into(),
    };
    let mut actions = row![continue_button].spacing(6);
    if operation.can_skip() {
        actions = actions.push(
            action("Skip", button::secondary)
                .on_press_maybe(idle.then_some(Message::SkipOperation)),
        );
    }
    actions = actions.push(
        action("Abort", button::danger).on_press_maybe(idle.then_some(Message::AbortOperation)),
    );
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
