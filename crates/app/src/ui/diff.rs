//! Center pane: file diff (replaces the graph while open).

use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Fill};

use crate::app::{DiffSource, DiffTarget, Message, Repo};
use crate::theme;
use crate::ui::muted;

pub fn view<'a>(repo: &'a Repo, target: &'a DiffTarget) -> Element<'a, Message> {
    let source = match target.source {
        DiffSource::Unstaged => "Unstaged".to_string(),
        DiffSource::Staged => "Staged".to_string(),
        DiffSource::Commit(index) => repo
            .commits
            .get(index)
            .map_or_else(String::new, |c| format!("commit {}", c.short_id())),
    };

    // TODO: Hunk / Inline / Split modes once diffs are rendered.
    let modes = row![mode("Hunk"), mode("Inline"), mode("Split")].spacing(2);

    let header = container(
        row![
            text(&target.path),
            muted(source).size(12),
            space::horizontal(),
            modes,
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

    let body = container(muted("Diff rendering arrives with the git backend.")).center(Fill);

    container(column![header, body])
        .width(Fill)
        .height(Fill)
        .style(theme::graph)
        .into()
}

fn mode(label: &str) -> Element<'_, Message> {
    button(text(label).size(12))
        .padding([2, 8])
        .style(theme::flat)
        .into()
}
