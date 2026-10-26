//! Repository tabs (top row).

use iced::widget::{button, container, row, text, Row};
use iced::{Element, Fill};

use crate::app::{App, Message};
use crate::icons::{self, Icon};
use crate::theme;
use crate::ui::panel::with_tooltip_text;

pub fn view(app: &App) -> Element<'_, Message> {
    let tabs = app.tabs.iter().enumerate().map(|(index, tab)| {
        let close = with_tooltip_text(
            button(icons::tinted(Icon::Close, 13.0, |p| p.text_muted))
                .on_press(Message::TabClosed(tab.id))
                .padding([2, 3])
                .style(theme::flat)
                .into(),
            "Close tab".to_owned(),
        );

        let label = row![text(tab.title()), close]
            .spacing(10)
            .align_y(iced::Center);

        button(label)
            .on_press(Message::TabSelected(tab.id))
            .padding([5, 12])
            .style(theme::tab(index == app.active))
            .into()
    });

    let add = with_tooltip_text(
        button(icons::icon(Icon::Plus, 15.0))
            .on_press(Message::TabOpened)
            .padding([5, 12])
            .style(theme::flat)
            .into(),
        "New tab".to_owned(),
    );

    let bar: Row<'_, Message> = Row::with_children(tabs).push(add).spacing(1);

    container(bar).width(Fill).style(theme::chrome).into()
}
