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

mod column_handle;
mod conflict;
pub mod context_area;
pub(crate) mod diff;
mod file_history;
mod graph;
#[cfg(test)]
pub use graph::lanes_width;
pub use graph::{optimal_column_width, MIN_GRAPH_WIDTH};
pub(crate) mod overlays;
pub(crate) mod panel;
mod rebase;
pub(crate) mod settings;
mod sidebar;
pub(crate) mod start;
pub(crate) mod statusbar;
mod tabs;
mod toolbar;

pub use graph::reveal_offset;
pub use graph::visible_range as graph_visible_range;

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
    // Icons snap to the physical pixel grid of this frame (M9-05).
    crate::icons::set_scale(app.physical_scale());
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
        layers.push(overlays::context_menu(menu, app.window_size()));
    }
    if let Some(form) = &app.form {
        layers.push(overlays::form(form));
    }
    if let Some(dialog) = &app.clone_dialog {
        layers.push(overlays::clone_dialog(dialog));
    }
    if let Some(dialog) = &app.init_dialog {
        layers.push(overlays::init_dialog(dialog));
    }
    if let Some(prompt) = &app.prompt_dialog {
        layers.push(overlays::prompt_dialog(prompt));
    }
    if let Some(dialog) = &app.message_dialog {
        layers.push(overlays::message_dialog(dialog));
    }
    if let Some(palette) = &app.palette {
        layers.push(overlays::palette(palette));
    }
    if let Some(filter) = &app.shortcut_list {
        layers.push(overlays::shortcut_list(filter));
    }
    if app.about {
        layers.push(overlays::about());
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
            // Labelled toolbar buttons need about 1 100 logical pixels.
            // Icons only when the window is narrow or labels are off (M10-08).
            let compact =
                app.window_size().width < 1100.0 || !app.settings.appearance.toolbar_labels;
            let pickers = toolbar::Pickers {
                repository: app.action_block(crate::shortcuts::Shortcut::OpenRepoPalette),
                branch: app.action_block(crate::shortcuts::Shortcut::CheckoutBranch),
                editor: !app.editor_command().is_empty(),
            };
            let mut content =
                column![toolbar::view(view, app.pull_mode, compact, pickers)].spacing(1);
            let finish = view.snapshot.flow_finish.as_ref();
            match (view.snapshot.state_banner(), finish) {
                // A Git flow finish stopped in a merge: "Finishing … · Merging …".
                (Some(state), Some(plan)) => {
                    content = content.push(state_banner(
                        view,
                        format!("{} · {state}", plan.title()),
                        true,
                    ));
                }
                (Some(state), None) => content = content.push(state_banner(view, state, false)),
                // A finish waits (after a restart or a failed step).
                (None, Some(plan)) => content = content.push(finish_banner(view, plan)),
                (None, None) => {}
            }
            if let Some(banner) = view.conflict_banner() {
                content = content.push(conflict_banner(view, banner));
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
                CenterView::Graph => panel_depth(graph::view(
                    view,
                    &app.avatars,
                    &app.settings.appearance,
                    app.theme_kind().palette(),
                )),
                CenterView::Diff(diff) => diff::view(view, diff, app.settings.diff.view_mode),
                CenterView::Conflict(conflict) => {
                    with_background(conflict::view(view, conflict, &app.editor_style()))
                }
                CenterView::Rebase(panel) => with_background(rebase::view(view, panel)),
                CenterView::FileHistory(history) => with_background(file_history::view(
                    view,
                    history,
                    &app.avatars,
                    app.settings.diff.view_mode,
                    &app.editor_style(),
                )),
            },
            PaneKind::Panel => panel::view(view, &app.avatars),
        };
        pane_grid::Content::new(content)
    })
    .spacing(1)
    .min_size(160)
    .on_resize(6, Message::PaneResized)
    .style(theme::panes)
    .into()
}

/// Shadows of the side panels falling onto the graph (`panel_shadow`,
/// Classic Dark only, M11-04): narrow gradients at both edges.
fn panel_depth(content: Element<'_, Message>) -> Element<'_, Message> {
    const WIDTH: f32 = 6.0;
    let edge = |left: bool| -> Element<'_, Message> {
        container(
            container(space::horizontal())
                .width(WIDTH)
                .height(Fill)
                .style(theme::panel_shadow(left)),
        )
        .width(Fill)
        .height(Fill)
        .align_x(if left {
            iced::alignment::Horizontal::Left
        } else {
            iced::alignment::Horizontal::Right
        })
        .into()
    };
    iced::widget::Stack::with_children([content, edge(true), edge(false)]).into()
}

/// Center views without a background of their own get the graph's.
fn with_background(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .width(Fill)
        .height(Fill)
        .style(theme::graph)
        .into()
}

/// App-wide one-time notice with a dismiss button.
fn notice_banner(notice: &str) -> Element<'_, Message> {
    container(
        row![
            crate::icons::tinted(crate::icons::Icon::Info, 15.0, |p| p.warning),
            text(notice).style(theme::text_normal),
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
/// A Git flow finish that is not running (after a restart or a failed
/// step): "Continue finishing" / "Abort" (M7-10).
fn finish_banner<'a>(
    view: &'a RepoView,
    plan: &git_core::flow_finish::FinishPlan,
) -> Element<'a, Message> {
    let idle = view.write_block.is_none() && view.busy.is_none();
    container(
        row![
            crate::icons::tinted(crate::icons::Icon::Merge, 15.0, |p| p.accent),
            text(plan.title()).style(theme::text_normal),
            muted(format!(
                "{} of {} steps done.",
                plan.next.min(plan.steps.len()),
                plan.steps.len()
            ))
            .size(12),
            space::horizontal(),
            button(text("Continue finishing").size(12))
                .padding([3, 10])
                .style(button::primary)
                .on_press_maybe(idle.then_some(Message::FlowFinishContinue)),
            button(text("Abort").size(12))
                .padding([3, 10])
                .style(button::danger)
                .on_press_maybe(idle.then_some(Message::FlowFinishAbort)),
        ]
        .spacing(12)
        .align_y(iced::Center),
    )
    .padding([6, 12])
    .width(Fill)
    .style(theme::state_banner)
    .into()
}

/// `finishing`: the operation belongs to a Git flow finish; Abort aborts
/// the finish (M7-10).
fn state_banner(view: &RepoView, state: String, finishing: bool) -> Element<'_, Message> {
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
    let abort = if finishing {
        Message::FlowFinishAbort
    } else {
        Message::AbortOperation
    };
    actions = actions.push(action("Abort", button::danger).on_press_maybe(idle.then_some(abort)));
    container(
        row![
            crate::icons::tinted(crate::icons::Icon::Merge, 15.0, |p| p.accent),
            text(state).style(theme::text_normal),
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

/// Conflicted files that no Git operation explains: a stash pop/apply that
/// stopped in conflicts, or leftovers (M9-01, ADR-044).
fn conflict_banner(
    view: &RepoView,
    banner: crate::repo_view::ConflictBanner,
) -> Element<'_, Message> {
    use crate::repo_view::ConflictBanner;
    let idle = view.write_block.is_none() && view.busy.is_none();
    let action = |label: &'static str, style: fn(&iced::Theme, button::Status) -> button::Style| {
        button(text(label).size(12)).padding([3, 10]).style(style)
    };
    let mut actions = row![].spacing(6);
    let hint = match &banner {
        ConflictBanner::StashConflicts { undo, .. } => {
            if *undo {
                actions = actions.push(
                    action("Undo", button::secondary).on_press_maybe(idle.then_some(Message::Undo)),
                );
            }
            let drop = action("Drop stash", button::secondary);
            actions = actions.push(panel::with_tooltip_text(
                drop.into(),
                "Resolve the conflicts first".into(),
            ));
            "Resolve the conflicts and stage them."
        }
        ConflictBanner::StashResolved { id, .. } => {
            actions = actions
                .push(
                    action("Drop stash", button::primary)
                        .on_press_maybe(idle.then(|| Message::StashDropConfirmed(id.clone()))),
                )
                .push(action("Keep stash", button::secondary).on_press(Message::KeepStash));
            "Drop the stash if you no longer need it."
        }
        ConflictBanner::Orphaned => {
            actions = actions.push(
                action("Discard conflicted files…", button::danger).on_press_maybe(
                    idle.then_some(Message::Discard(crate::app::DiscardTarget::Conflicted)),
                ),
            );
            "Resolve them in the conflict view (click a conflicted file), or discard them."
        }
    };
    container(
        row![
            crate::icons::tinted(crate::icons::Icon::Alert, 15.0, |p| p.warning),
            text(banner.text()).style(theme::text_normal),
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
            crate::icons::tinted(crate::icons::Icon::Alert, 15.0, |p| p.danger),
            text(error).style(theme::text_normal),
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

/// Room between the content of a [`vscroll`] and its scrollbar.
pub const SCROLLBAR_GAP: f32 = 2.0;

/// Vertical scrollable whose scrollbar gets its own room (M6-22): while the
/// bar is shown, the content is narrower by its width, so counts and
/// buttons on the right are never drawn below it. Without overflow the
/// content keeps the full width.
pub fn vscroll<'a>(
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Scrollable<'a, Message> {
    iced::widget::scrollable(content).spacing(SCROLLBAR_GAP)
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

/// Round avatar of size `size`, or the author's initials in a circle
/// while there is none (avatars off, loading, or none on Gravatar).
pub(crate) fn avatar<'a>(
    handle: Option<&iced::widget::image::Handle>,
    author: &str,
    size: f32,
) -> Element<'a, Message> {
    match handle {
        Some(handle) => iced::widget::image(handle.clone())
            .width(size)
            .height(size)
            .border_radius(size / 2.0)
            .into(),
        None => container(text(crate::format::initials(author)).size(size * 0.4))
            .width(size)
            .height(size)
            .center(size)
            .style(theme::avatar)
            .into(),
    }
}

/// Muted secondary text.
pub(crate) fn muted<'a>(content: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(content).style(theme::text_muted)
}

/// Icon for a file change (added, modified, deleted, renamed, untracked,
/// conflicted) in the change's color.
pub(crate) fn change_badge<'a>(kind: git_core::ChangeKind) -> Element<'a, Message> {
    container(crate::icons::change_icon(kind, 14.0))
        .width(20)
        .into()
}
