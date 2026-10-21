//! Start screen of an empty tab: open a repository or pick a recent one.

use iced::widget::{button, column, container, row, text, text_input};
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

    let content = column![
        text("Gitloom").size(22),
        open,
        muted("Ctrl+O").size(11),
        path,
        container(muted("Recent repositories")).padding([12, 0]),
        muted("No recent repositories yet").size(12),
    ]
    .spacing(10)
    .align_x(iced::Center);

    container(content).center(Fill).style(theme::graph).into()
}
