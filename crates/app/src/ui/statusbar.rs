//! Bottom status bar.

use iced::widget::{button, container, row, space, text};
use iced::{Element, Fill};

use crate::app::{App, Message};
use crate::theme;
use crate::ui::muted;

pub fn view(app: &App) -> Element<'_, Message> {
    let mut content = row![text(&app.status).size(12), space::horizontal()]
        .spacing(16)
        .padding([3, 12]);

    if let Some(view) = app.active_tab().and_then(|t| t.view()) {
        let snapshot = &view.snapshot;
        if let (Some(op), Some(tab)) = (&view.network, app.active_tab()) {
            content = content.push(muted(network_label(op)).size(12)).push(
                button(text("Cancel").size(11))
                    .on_press(Message::CancelNetwork(tab.id))
                    .padding([0, 8])
                    .style(button::secondary),
            );
        } else if let Some(busy) = &view.busy {
            content = content.push(muted(format!("⟳ {busy}…")).size(12));
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
        }) => format!("⟳ {}: {phase} {percent}%", op.label),
        Some(git_core::Progress { phase, .. }) => format!("⟳ {}: {phase}…", op.label),
        None => format!("⟳ {}…", op.label),
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
