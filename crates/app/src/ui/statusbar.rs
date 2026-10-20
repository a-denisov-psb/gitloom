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

    if let Some(repo) = app.repo() {
        let branch = repo.head().map_or("(detached)", |r| r.name.as_str());
        content = content
            .push(muted(format!("branch: {branch}")).size(12))
            .push(muted(format!("{} commits", repo.commits.len())).size(12));
    }

    content = content.push(muted(concat!("v", env!("CARGO_PKG_VERSION"))).size(12));

    container(content).width(Fill).style(theme::chrome).into()
}
