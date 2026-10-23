//! Bottom status bar.

use iced::widget::{container, row, space, text};
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
        if let Some(busy) = &view.busy {
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

fn commit_count(count: usize, truncated: bool) -> String {
    let count = crate::format::thousands(count);
    if truncated {
        format!("{count} commits (history truncated)")
    } else {
        format!("{count} commits")
    }
}
