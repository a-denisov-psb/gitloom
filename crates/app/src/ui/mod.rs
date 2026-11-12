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

mod code_highlighter;
mod column_handle;
mod conflict;
pub mod context_area;
pub(crate) mod diff;
mod file_history;
pub(crate) mod graph;
mod inert;
mod shift;
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
mod toasts;
pub(crate) mod toolbar;

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
    // Ends a branch drag released outside a drop target (M4-11).
    let base = context_area::release_area(base, Message::BranchDragEnd);
    // Behind the Preferences modal the app is visible but out of reach:
    // no clicks, and Tab keeps the focus inside the modal (M11-13).
    let base = inert::inert(base, app.settings_page.is_some());
    let mut layers = vec![base];
    if let Some(page) = &app.settings_page {
        layers.push(preferences(app, page));
    }
    // Toasts sit above the app and the Preferences, below menus and dialogs.
    if let Some(toasts) = toasts::view(&app.toasts) {
        layers.push(toasts);
    }
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
            // Git refuses a repository owned by another user: say so in
            // plain words and offer to trust it (M12-14, ADR-044).
            let mut content = match git_core::dubious_ownership(error) {
                Some(git_path) => column![
                    text("Git does not trust this repository").size(16),
                    muted(
                        "The folder belongs to another user, so Git refuses to work in it. \
                         Trust it if you know where it comes from."
                    ),
                    button(text("Trust this repository…").size(12))
                        .on_press(Message::TrustRepository {
                            tab: tab.id,
                            path: git_path.to_owned(),
                        })
                        .padding([3, 10])
                        .style(button::primary),
                    muted(path.display().to_string()),
                ]
                .spacing(10),
                None => column![banner(error, tab.id), muted(path.display().to_string())],
            };
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
                column![toolbar::view(view, app.pull_mode, compact, pickers,)].spacing(1);
            // A running operation shows in the graph and the right panel
            // (M12-05); a Git flow finish that waits (after a restart or a
            // failed step) keeps its bar.
            if let (true, Some(plan)) = (
                view.snapshot.state.is_clean(),
                view.snapshot.flow_finish.as_ref(),
            ) {
                content = content.push(finish_banner(view, plan));
            }
            if let Some(notice) = app.wsl_notice(view) {
                content = content.push(wsl_banner(notice, view.backend.path()));
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
                CenterView::Graph => {
                    graph::view(view, &app.avatars, &app.settings.appearance, app.now)
                }
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
            PaneKind::Panel => panel::view(view, &app.avatars, app.settings.file_list),
        };
        pane_grid::Content::new(content)
    })
    .spacing(1)
    .min_size(160)
    .on_resize(6, Message::PaneResized)
    .style(theme::panes)
    .into()
}

/// Size of the Preferences modal in a window `window` large (logical
/// pixels): min(80 % of the width, 960) × min(85 % of the height, 680);
/// below 720 × 520 it fills the window (M11-13).
pub fn preferences_size(window: iced::Size) -> iced::Size {
    if window.width < 720.0 || window.height < 520.0 {
        return window;
    }
    iced::Size::new(
        (window.width * 0.8).min(960.0),
        (window.height * 0.85).min(680.0),
    )
}

/// Preferences as a centered modal over the dimmed app (M11-13). A click
/// on the backdrop does not close it; "×" and Esc do.
fn preferences<'a>(app: &'a App, page: &'a crate::settings::Page) -> Element<'a, Message> {
    let size = preferences_size(app.window_size());
    let dialog = container(settings::view(app, page))
        .width(size.width)
        .height(size.height)
        .clip(true)
        .style(theme::dialog);
    iced::widget::opaque(container(dialog).center(Fill).style(theme::backdrop))
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

/// Repository inside WSL (M12-14): why it is slow, "Don't show again".
fn wsl_banner(notice: String, path: &std::path::Path) -> Element<'_, Message> {
    container(
        row![
            crate::icons::tinted(crate::icons::Icon::Info, 15.0, |p| p.warning),
            text(notice).style(theme::text_normal),
            space::horizontal(),
            button(text("Don't show again").size(12))
                .on_press(Message::WslNoticeDismissed(path.to_path_buf()))
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
