//! Repository tabs (top row).

use iced::widget::{button, container, row, text, Row};
use iced::{Element, Fill};

use crate::app::{App, Message};
use crate::theme;

pub fn view(app: &App) -> Element<'_, Message> {
    let tabs = app.repos.iter().enumerate().map(|(index, repo)| {
        let close = button(text("×").size(14))
            .on_press(Message::TabClosed(index))
            .padding([0, 5])
            .style(theme::flat);

        let label = row![text(&repo.name), close]
            .spacing(10)
            .align_y(iced::Center);

        button(label)
            .on_press(Message::TabSelected(index))
            .padding([5, 12])
            .style(theme::tab(index == app.active))
            .into()
    });

    let add = button(text("+").size(15))
        .on_press(Message::TabOpened)
        .padding([4, 12])
        .style(theme::flat);

    let bar: Row<'_, Message> = Row::with_children(tabs).push(add).spacing(1);

    container(bar).width(Fill).style(theme::chrome).into()
}
