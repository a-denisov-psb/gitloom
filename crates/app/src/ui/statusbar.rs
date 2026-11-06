//! Bottom status bar.

use iced::widget::{button, container, row, space, text};
use iced::{Element, Fill};

use crate::app::{App, Message};
use crate::theme;
use crate::ui::muted;

pub fn view(app: &App) -> Element<'_, Message> {
    let mut content = row![].spacing(16).padding([3, 12]);
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

    container(content).width(Fill).style(theme::chrome).into()
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
