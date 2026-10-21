//! Center pane: file diff (replaces the graph while open).

use git_core::DiffKind;
use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Fill};

use crate::app::Message;
use crate::repo_view::{DiffView, RepoView};
use crate::theme;
use crate::ui::muted;

pub fn view<'a>(_repo: &'a RepoView, diff: &'a DiffView) -> Element<'a, Message> {
    let request = &diff.request;
    let source = match &request.kind {
        DiffKind::Unstaged => "Unstaged".to_string(),
        DiffKind::Untracked => "Untracked".to_string(),
        DiffKind::Staged => "Staged".to_string(),
        DiffKind::Commit(id) => format!("commit {}", &id[..id.len().min(7)]),
    };

    let header = container(
        row![
            text(&request.path),
            muted(source).size(12),
            space::horizontal(),
            button(text("×").size(15))
                .on_press(Message::DiffClosed)
                .padding([0, 8])
                .style(theme::flat),
        ]
        .spacing(12)
        .padding([6, 12])
        .align_y(iced::Center),
    )
    .width(Fill)
    .style(theme::column_header);

    let body = container(muted("Diff rendering follows.")).center(Fill);

    container(column![header, body])
        .width(Fill)
        .height(Fill)
        .style(theme::graph)
        .into()
}
