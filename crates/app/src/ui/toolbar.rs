//! Main toolbar: repo/branch pickers, actions, terminal.

use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Fill};

use crate::app::{Message, Picker, Repo, ToolbarAction};
use crate::theme;
use crate::ui::muted;

pub fn view(repo: &Repo) -> Element<'_, Message> {
    let branch = repo.head().map_or("(detached)", |r| r.name.as_str());

    let pickers = row![
        picker(Picker::Repository, "repository", &repo.name),
        picker(Picker::Branch, "branch", branch),
    ]
    .spacing(20);

    let actions = row![
        action(ToolbarAction::Undo),
        action(ToolbarAction::Redo),
        divider(),
        action(ToolbarAction::Pull),
        action(ToolbarAction::Push),
        divider(),
        action(ToolbarAction::Branch),
        action(ToolbarAction::Stash),
        action(ToolbarAction::Pop),
    ]
    .spacing(2)
    .align_y(iced::Center);

    let content = row![
        pickers,
        space::horizontal(),
        actions,
        space::horizontal(),
        action(ToolbarAction::Terminal),
    ]
    .padding([4, 12])
    .align_y(iced::Center);

    container(content).width(Fill).style(theme::chrome).into()
}

// TODO: open a dropdown (recent repos / branch list) on press.
fn picker<'a>(picker: Picker, caption: &'a str, value: &'a str) -> Element<'a, Message> {
    button(column![muted(caption).size(10), text(format!("{value} ▾"))].spacing(1))
        .on_press(Message::PickerOpened(picker))
        .padding([3, 8])
        .style(theme::flat)
        .into()
}

fn action<'a>(action: ToolbarAction) -> Element<'a, Message> {
    let content = column![text(action.glyph()).size(16), text(action.label()).size(11)]
        .spacing(1)
        .align_x(iced::Center);

    button(content)
        .on_press(Message::Toolbar(action))
        .padding([3, 10])
        .width(64)
        .style(theme::flat)
        .into()
}

fn divider<'a>() -> Element<'a, Message> {
    container(space::vertical().width(1).height(28))
        .style(|_| iced::widget::container::Style {
            background: Some(theme::BORDER.into()),
            ..Default::default()
        })
        .into()
}
