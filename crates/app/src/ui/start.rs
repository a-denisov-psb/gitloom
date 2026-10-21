//! Start screen of an empty tab: open a repository or pick a recent one.

use iced::widget::{button, column, container, row, text, text_input, Column};
use iced::{Element, Fill};

use crate::app::{App, Message};
use crate::theme;
use crate::ui::muted;

pub fn view(app: &App) -> Element<'_, Message> {
    let open = button(text("Open repository"))
        .on_press(Message::OpenRepository)
        .padding([6, 14])
        .style(button::primary);

    // Fallback for systems without a folder dialog (e.g. no XDG portal on Linux).
    let path = row![
        text_input("Or enter a path", &app.start_path)
            .id(crate::app::START_PATH_INPUT)
            .on_input(Message::StartPathChanged)
            .on_submit(Message::StartPathSubmitted)
            .padding(6)
            .width(320),
        button(text("Open"))
            .on_press_maybe(
                (!app.start_path.trim().is_empty()).then_some(Message::StartPathSubmitted)
            )
            .padding([6, 12])
            .style(button::secondary),
    ]
    .spacing(6);

    let recent: Element<'_, Message> = if app.session.recent.is_empty() {
        muted("No recent repositories yet").size(12).into()
    } else {
        Column::with_children(app.session.recent.iter().map(|repo| {
            let name = repo.path.file_name().map_or_else(
                || repo.path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            button(column![text(name), muted(repo.path.display().to_string()).size(11)].spacing(1))
                .on_press(Message::OpenRecent(repo.path.clone()))
                .padding([4, 10])
                .width(420)
                .style(theme::list_item(false))
                .into()
        }))
        .spacing(2)
        .into()
    };

    let content = column![
        text("Gitloom").size(22),
        open,
        muted("Ctrl+O").size(11),
        path,
        container(muted("Recent repositories")).padding([12, 0]),
        recent,
    ]
    .spacing(10)
    .align_x(iced::Center);

    container(content).center(Fill).style(theme::graph).into()
}
