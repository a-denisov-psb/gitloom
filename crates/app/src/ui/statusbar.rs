//! Bottom status bar.

use iced::widget::{button, container, row, space, text};
use iced::{Element, Fill};

use crate::app::{App, Message};
use crate::theme;
use crate::ui::muted;

pub fn view(app: &App) -> Element<'_, Message> {
    let repository = app.active_tab().and_then(|t| t.view()).is_some();
    let mut content = row![].spacing(16).padding([3, 12]).align_y(iced::Center);
    // Panel toggles at both ends like Zed (fix 2026-11-12), only in
    // repository tabs.
    if repository {
        content = content.push(panel_toggle(PanelToggle::Left, app.panes.sidebar_visible));
    }
    if let Some(crash) = &app.crash_notice {
        content = content.push(crash_notice(crash));
    }
    if let Some(release) = &app.update_available {
        content = content.push(update_notice(release));
    }
    content = content
        .push(text(&app.status).size(12))
        .push(space::horizontal());

    if let Some(view) = app.active_tab().and_then(|t| t.view()) {
        let snapshot = &view.snapshot;
        if let (Some(op), Some(tab)) = (&view.network, app.active_tab()) {
            content = content
                .push(
                    row![
                        crate::icons::tinted(crate::icons::Icon::Refresh, 12.0, |p| p.text_muted),
                        muted(network_label(op)).size(12)
                    ]
                    .spacing(4)
                    .align_y(iced::Center),
                )
                .push(
                    button(text("Cancel").size(11))
                        .on_press(Message::CancelNetwork(tab.id))
                        .padding([0, 8])
                        .style(button::secondary),
                );
        } else if let Some(busy) = &view.busy {
            content = content.push(
                row![
                    crate::icons::tinted(crate::icons::Icon::Refresh, 12.0, |p| p.text_muted),
                    muted(format!("{busy}…")).size(12)
                ]
                .spacing(4)
                .align_y(iced::Center),
            );
        }
        if view.refreshing {
            content = content.push(muted("Refreshing…").size(12));
        }
        content = content
            .push(muted(format!("branch: {}", snapshot.branch_label())).size(12))
            .push(muted(commit_count(snapshot.commit_count(), snapshot.truncated)).size(12));
    }

    content = content.push(muted(concat!("v", env!("CARGO_PKG_VERSION"))).size(12));
    if repository {
        content = content.push(panel_toggle(PanelToggle::Right, app.panes.panel_visible));
    }

    container(content).width(Fill).style(theme::chrome).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelToggle {
    Left,
    Right,
}

/// Tooltip of a panel toggle: what a click does, with its shortcut; the
/// state is in the text, not only in the icon.
pub fn panel_tooltip(toggle: PanelToggle, shown: bool) -> String {
    let (name, action) = match toggle {
        PanelToggle::Left => ("left panel", crate::actions::ActionId::ToggleSidebar),
        PanelToggle::Right => ("commit panel", crate::actions::ActionId::TogglePanel),
    };
    let verb = if shown { "Hide" } else { "Show" };
    match action.shortcut_text() {
        Some(keys) => format!("{verb} {name} ({keys})"),
        None => format!("{verb} {name}"),
    }
}

/// Icon of a panel toggle: filled side part while the panel is shown,
/// outline while hidden (like VS Code).
pub fn panel_icon(toggle: PanelToggle, shown: bool) -> crate::icons::Icon {
    use crate::icons::Icon;
    match (toggle, shown) {
        (PanelToggle::Left, true) => Icon::PanelLeftFilled,
        (PanelToggle::Left, false) => Icon::PanelLeft,
        (PanelToggle::Right, true) => Icon::PanelRightFilled,
        (PanelToggle::Right, false) => Icon::PanelRight,
    }
}

/// Toggle of the left or the commit panel in the status bar (fix
/// 2026-11-12, replaces the toolbar toggles of M11-14): a 14 px icon in a
/// 22 × 20 px click area, no background in any state; muted, the text
/// color on hover.
fn panel_toggle<'a>(toggle: PanelToggle, shown: bool) -> Element<'a, Message> {
    use crate::shortcuts::Shortcut;
    let shortcut = match toggle {
        PanelToggle::Left => Shortcut::ToggleSidebar,
        PanelToggle::Right => Shortcut::TogglePanel,
    };
    let icon = crate::icons::hover_tinted(
        panel_icon(toggle, shown),
        14.0,
        |p| p.text_muted,
        |p| p.text,
        4.0,
    );
    crate::ui::panel::with_tooltip_text(
        button(container(icon).center(Fill))
            // Like the shortcut, without asking which widget has the focus.
            .on_press(Message::ShortcutChecked {
                shortcut,
                text_input_focused: false,
            })
            .padding(0)
            .width(22)
            .height(20)
            .style(theme::bare)
            .into(),
        panel_tooltip(toggle, shown),
    )
}

/// "Fetch: Receiving objects 45%" (phase and percentage once Git reports them).
pub fn network_label(op: &crate::repo_view::NetworkOp) -> String {
    match &op.progress {
        Some(git_core::Progress {
            phase,
            percent: Some(percent),
        }) => format!("{}: {phase} {percent}%", op.label),
        Some(git_core::Progress { phase, .. }) => format!("{}: {phase}…", op.label),
        None => format!("{}…", op.label),
    }
}

fn commit_count(count: usize, truncated: bool) -> String {
    let count = crate::format::thousands(count);
    if truncated {
        format!("{count} commits (history truncated)")
    } else {
        format!("{count} commits")
    }
}

/// "Gitloom closed unexpectedly" with the report actions (M6-05).
fn crash_notice(crash: &crate::crash::Pending) -> Element<'_, Message> {
    let name = git_core::identity::DISPLAY_NAME;
    let label = if crash.fatal {
        format!("{name} closed unexpectedly.")
    } else {
        format!("{name} had an internal error.")
    };
    let action = |label: &'static str, message: Message| {
        button(text(label).size(11))
            .on_press(message)
            .padding([0, 8])
            .style(button::secondary)
    };
    row![
        crate::icons::tinted(crate::icons::Icon::Alert, 12.0, |p| p.warning),
        text(label).size(12).style(crate::theme::text_warning),
        action("Open report folder", Message::CrashReportOpenFolder),
        action("Copy report", Message::CrashReportCopy),
        action("Dismiss", Message::CrashReportDismissed),
    ]
    .spacing(6)
    .align_y(iced::Center)
    .into()
}

/// "Gitloom X is available" with the release page link (M6-06).
fn update_notice(release: &crate::update::Release) -> Element<'_, Message> {
    let action = |label: &'static str, message: Message| {
        button(text(label).size(11))
            .on_press(message)
            .padding([0, 8])
            .style(button::secondary)
    };
    row![
        crate::icons::tinted(crate::icons::Icon::Info, 12.0, |p| p.accent),
        text(format!(
            "{} {} is available.",
            git_core::identity::DISPLAY_NAME,
            release.version
        ))
        .size(12)
        .style(crate::theme::text_accent),
        action("Release notes", Message::OpenLink(release.url.clone())),
        action("Dismiss", Message::UpdateDismissed),
    ]
    .spacing(6)
    .align_y(iced::Center)
    .into()
}
