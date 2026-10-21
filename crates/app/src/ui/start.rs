//! Start screen of an empty tab.

use iced::widget::{column, container, text};
use iced::{Element, Fill};

use crate::app::Message;
use crate::theme;
use crate::ui::muted;

pub fn view<'a>() -> Element<'a, Message> {
    let content = column![
        text("No repository open").size(18),
        muted("Start Gitloom with a path: gitloom <path>"),
    ]
    .spacing(12)
    .align_x(iced::Center);

    container(content).center(Fill).style(theme::graph).into()
}
