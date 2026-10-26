//! Rendering of the modal dialog and the context menu above the window.

use iced::widget::{
    button, column, container, mouse_area, opaque, pick_list, row, scrollable, space, text,
    text_input, Column,
};
use iced::{Element, Fill, Font, Length, Padding};

use crate::app::Message;
use crate::overlay::{
    ButtonKind, ContextMenu, FieldKind, Form, MenuEntry, Modal, MENU_ITEM_HEIGHT, MENU_WIDTH,
};
use crate::theme;
use crate::ui::muted;

const DIALOG_WIDTH: f32 = 520.0;

/// Dialog centered over a dimmed, input-blocking backdrop.
pub fn modal(modal: &Modal) -> Element<'_, Message> {
    let mut content = column![text(&modal.title).size(17), text(&modal.text)].spacing(12);
    if let Some(details) = &modal.details {
        // Git indents file lists with tabs, which the font has no glyph for.
        let details = details.replace('\t', "    ");
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

/// Id of the first input of a form, focused when the form opens.
pub const FORM_FIRST_INPUT: &str = "form-first-input";

/// Form dialog: labelled inputs, validation message, Cancel and submit.
pub fn form(form: &Form) -> Element<'_, Message> {
    let mut content = column![text(&form.title).size(17)].spacing(12);
    if !form.text.is_empty() {
        content = content.push(text(&form.text));
    }
    let mut first_text = true;
    for (index, field) in form.fields.iter().enumerate() {
        let input: Element<'_, Message> = match &field.kind {
            FieldKind::Text => {
                let mut input = text_input("", &field.value)
                    .on_input(move |value| Message::FormInput {
                        field: index,
                        value,
                    })
                    .on_submit(Message::FormSubmit)
                    .padding(6);
                if first_text {
                    input = input.id(FORM_FIRST_INPUT);
                    first_text = false;
                }
                input.into()
            }
            FieldKind::Choice(options) => {
                pick_list(options.clone(), Some(field.value.clone()), move |value| {
                    Message::FormInput {
                        field: index,
                        value,
                    }
                })
                .width(Fill)
                .into()
            }
        };
        content = content.push(column![muted(&field.label).size(12), input].spacing(4));
    }
    if let Some(error) = &form.error {
        content = content.push(text(error).size(12).color(theme::DANGER));
    }
    content = content.push(
        row![
            space::horizontal(),
            button(text("Cancel"))
                .on_press(Message::FormClosed)
                .padding([6, 14])
                .style(button::secondary),
            button(text(&form.submit))
                .on_press(Message::FormSubmit)
                .padding([6, 14])
                .style(button::primary),
        ]
        .spacing(8),
    );
    let dialog = container(content)
        .width(DIALOG_WIDTH)
        .padding(20)
        .style(theme::dialog);
    opaque(
        mouse_area(container(dialog).center(Fill).style(theme::backdrop))
            .interaction(iced::mouse::Interaction::Idle),
    )
}

/// Menu at its origin; a click anywhere else closes it.
/// Commit message dialog (reword, squash).
pub fn message_dialog(dialog: &crate::overlay::MessageDialog) -> Element<'_, Message> {
    let mut content = column![
        text(&dialog.title).size(17),
        iced::widget::text_editor(&dialog.editor)
            .on_action(Message::CommitMessageEdited)
            .padding(6)
            .height(180),
    ]
    .spacing(12);
    if let Some(error) = &dialog.error {
        content = content.push(text(error).size(12).color(theme::DANGER));
    }
    content = content.push(
        row![
            space::horizontal(),
            button(text("Cancel"))
                .on_press(Message::CommitMessageClosed)
                .padding([6, 14])
                .style(button::secondary),
            button(text(&dialog.submit))
                .on_press(Message::CommitMessageSubmit)
                .padding([6, 14])
                .style(button::primary),
        ]
        .spacing(8),
    );
    let dialog = container(content)
        .width(DIALOG_WIDTH)
        .padding(20)
        .style(theme::dialog);
    opaque(
        mouse_area(container(dialog).center(Fill).style(theme::backdrop))
            .interaction(iced::mouse::Interaction::Idle),
    )
}

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
                hint,
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
                let item = button(line.align_y(iced::Center))
                    .on_press_maybe(enabled.then_some(Message::MenuSelected(index)))
                    .height(MENU_ITEM_HEIGHT)
                    .padding([4, 10])
                    .width(Fill)
                    .style(theme::menu_item);
                match hint {
                    Some(hint) => crate::ui::panel::with_tooltip_text(item.into(), hint.clone()),
                    None => item.into(),
                }
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
