//! Rendering of the modal dialog and the context menu above the window.

use iced::widget::{
    button, column, container, mouse_area, opaque, row, scrollable, space, text, Column,
};
use iced::{Element, Fill, Font, Length, Padding};

use crate::app::Message;
use crate::overlay::{ButtonKind, ContextMenu, MenuEntry, Modal, MENU_ITEM_HEIGHT, MENU_WIDTH};
use crate::theme;
use crate::ui::muted;

const DIALOG_WIDTH: f32 = 520.0;

/// Dialog centered over a dimmed, input-blocking backdrop.
pub fn modal(modal: &Modal) -> Element<'_, Message> {
    let mut content = column![text(&modal.title).size(17), text(&modal.text)].spacing(12);
    if let Some(details) = &modal.details {
        content = content.push(
            container(
                scrollable(
                    text(details)
                        .font(Font::MONOSPACE)
                        .size(12)
                        .wrapping(text::Wrapping::WordOrGlyph),
                )
                .height(Length::Shrink),
            )
            .max_height(260)
            .padding(8)
            .width(Fill)
            .style(theme::dialog_details),
        );
    }
    let buttons = modal.buttons.iter().enumerate().map(|(index, b)| {
        let style = match b.kind {
            ButtonKind::Primary => button::primary,
            ButtonKind::Secondary => button::secondary,
            ButtonKind::Danger => button::danger,
        };
        button(text(&b.label))
            .on_press(Message::ModalAction(index))
            .padding([6, 14])
            .style(style)
            .into()
    });
    content = content.push(row![space::horizontal()].extend(buttons).spacing(8));

    let dialog = container(content)
        .width(DIALOG_WIDTH)
        .padding(20)
        .style(theme::dialog);

    // The backdrop swallows clicks so nothing below reacts while the dialog is open.
    opaque(
        mouse_area(container(dialog).center(Fill).style(theme::backdrop))
            .on_press(Message::ModalClosed)
            .interaction(iced::mouse::Interaction::Idle),
    )
}

/// Menu at its origin; a click anywhere else closes it.
pub fn context_menu(menu: &ContextMenu, window: iced::Size) -> Element<'_, Message> {
    let origin = menu.origin(window);
    let entries = Column::with_children(menu.entries.iter().enumerate().map(|(index, entry)| {
        match entry {
            MenuEntry::Separator => container(space::horizontal())
                .height(1)
                .width(Fill)
                .style(theme::menu_separator)
                .padding(Padding::from([4, 0]))
                .into(),
            MenuEntry::Item {
                label,
                shortcut,
                enabled,
                ..
            } => {
                // Items never wrap: a wrapped label would overlap the next item.
                let mut line = row![
                    text(label).size(13).wrapping(text::Wrapping::None),
                    space::horizontal()
                ];
                if let Some(shortcut) = shortcut {
                    line = line.push(muted(shortcut).size(11));
                }
                button(line.align_y(iced::Center))
                    .on_press_maybe(enabled.then_some(Message::MenuSelected(index)))
                    .height(MENU_ITEM_HEIGHT)
                    .padding([4, 10])
                    .width(Fill)
                    .style(theme::menu_item)
                    .into()
            }
        }
    }));

    let menu = container(entries)
        .width(MENU_WIDTH)
        .padding(4)
        .style(theme::menu);

    opaque(
        mouse_area(
            container(menu)
                .padding(Padding::default().left(origin.x).top(origin.y))
                .width(Fill)
                .height(Fill),
        )
        .on_press(Message::MenuClosed)
        .on_right_press(Message::MenuClosed),
    )
}
