//! View of the "Commit message" modal (finding #43): the commit panel's
//! parts (M15-11) in edit mode, selectable read-only fields otherwise.

use iced::widget::{
    button, column, container, mouse_area, opaque, row, space, text, text_editor, text_input,
};
use iced::{Element, Fill, Size};

use crate::app::Message;
use crate::message_modal::MessageModal;
use crate::repo_view::Loadable;
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::{
    picker_line, sparkle_button, with_counter, with_tooltip_text, COUNTER_ROOM, SUMMARY_LIMIT,
};

/// Width of the modal (at most the window width minus 32).
pub const WIDTH: f32 = 640.0;
/// Margin kept around the modal.
const MARGIN: f32 = 16.0;
/// One line of the description (13 px text).
const LINE: f32 = 13.0 * 1.3;
/// The description shows at least six lines.
const DESCRIPTION_MIN: f32 = 6.0 * LINE + 12.0;
/// Room of everything besides the description: paddings, title, author
/// row, summary, buttons and the spacing between them.
const FIXED: f32 = 2.0 * 20.0 + 24.0 + 22.0 + 31.0 + 30.0 + 5.0 * 12.0 + SLACK;
/// Rounding of the text and button heights.
const SLACK: f32 = 8.0;
/// A note line (11 px text, spacing 2).
const NOTE_LINE: f32 = 11.0 * 1.3 + 2.0;

/// Width and the highest height of the modal in a `window` large:
/// 640 × 70 % of the window, inside the window with a margin.
pub fn bounds(window: Size) -> Size {
    Size::new(
        WIDTH.min(window.width - 2.0 * MARGIN).max(0.0),
        (window.height * 0.7)
            .min(window.height - 2.0 * MARGIN)
            .max(0.0),
    )
}

/// Highest description in a modal at most `max_height` high with `extra`
/// room taken by notes and hints; never below six lines.
/// The five gaps are the title's and those between the author row,
/// summary, description, notes and buttons.
fn description_max(max_height: f32, extra: f32) -> f32 {
    (max_height - FIXED - extra).max(DESCRIPTION_MIN)
}

pub fn view<'a>(modal: &'a MessageModal, window: Size, busy: bool) -> Element<'a, Message> {
    let size = bounds(window);
    let close = button(
        container(crate::icons::tinted(crate::icons::Icon::Close, 14.0, |p| {
            p.text_muted
        }))
        .center(Fill),
    )
    .on_press(Message::CommitModalClose)
    .padding(0)
    .width(24)
    .height(24)
    .style(theme::flat);
    let title =
        row![text("Commit message").size(17), space::horizontal(), close].align_y(iced::Center);
    let content = match &modal.read_only {
        None => edit(modal, size.height, busy),
        Some(reason) => read_only(modal, reason, size.height),
    };
    let dialog = container(column![title, content].spacing(12))
        .width(size.width)
        .max_height(size.height)
        .padding(20)
        .clip(true)
        .style(theme::dialog);
    // A click beside the modal is Esc; clicks on it stay there.
    let layer: Element<'a, Message> = opaque(
        mouse_area(
            container(opaque(dialog))
                .center(Fill)
                .style(theme::backdrop),
        )
        .on_press(Message::CommitModalClose)
        .interaction(iced::mouse::Interaction::Idle),
    );
    if !modal.confirm_discard {
        return layer;
    }
    iced::widget::stack![layer, confirm_discard()].into()
}

/// Edit mode: "Authored by …" with the sparkle, summary with its counter,
/// description, notes, Cancel and Save.
fn edit(modal: &MessageModal, max_height: f32, busy: bool) -> Element<'_, Message> {
    let remaining = SUMMARY_LIMIT as i64 - modal.summary.chars().count() as i64;
    let summary = text_input("Summary", &modal.summary)
        .id(crate::message_modal::SUMMARY_INPUT)
        .padding(iced::Padding::new(6.0).right(COUNTER_ROOM))
        .style(theme::input)
        .on_input(Message::CommitModalSummary)
        .on_submit(Message::CommitModalSave);
    let summary = with_counter(summary, remaining);
    let (author, tip) = match &modal.author {
        Some(author) => (
            format!("Authored by {}", author.label()),
            "New author after saving (the author date is kept); click to choose another".to_owned(),
        ),
        None => (
            format!("Authored by {} <{}>", modal.author_name, modal.author_email),
            "Click to change the author".to_owned(),
        ),
    };
    let author = picker_line(
        muted(author).size(11).wrapping(text::Wrapping::None).into(),
        tip,
        Message::EditAuthorMenu,
    );
    let sparkle = sparkle_button(
        Some(Message::CommitPromptRequested(modal.id.clone())),
        "Copy commit message prompt…".to_owned(),
    );
    let mut notes = column![].spacing(2);
    let mut extra = 0.0;
    if let Loadable::Ready(facts) = &modal.facts {
        if facts.drops_signature {
            notes = notes.push(
                muted("Signatures of the rewritten commits are removed (signing is off)").size(11),
            );
            extra += NOTE_LINE;
        }
    }
    let error = modal.error.as_ref().map(|e| {
        extra += NOTE_LINE + 12.0;
        text(e.clone()).size(11).style(theme::text_danger)
    });
    let body = text_editor(&modal.body)
        .placeholder("Description")
        .padding(6)
        .style(theme::editor)
        .wrapping(text::Wrapping::WordOrGlyph)
        .min_height(DESCRIPTION_MIN)
        .max_height(description_max(max_height, extra))
        .on_action(Message::CommitModalBody)
        // Ctrl+Enter saves instead of inserting a line break.
        .key_binding(|press| {
            let enter = matches!(
                press.key.as_ref(),
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
            );
            if enter && press.modifiers.command() {
                return Some(text_editor::Binding::Custom(Message::CommitModalSave));
            }
            text_editor::Binding::from_key_press(press)
        });
    let save = button(text("Save"))
        .padding([6, 14])
        .style(button::primary)
        .on_press_maybe((!busy).then_some(Message::CommitModalSave));
    let buttons = row![
        space::horizontal(),
        button(text("Cancel"))
            .padding([6, 14])
            .style(button::secondary)
            .on_press(Message::CommitModalClose),
        save,
    ]
    .spacing(8);
    column![
        row![container(author).width(Fill).clip(true), sparkle]
            .spacing(6)
            .align_y(iced::Center),
        summary,
        body,
    ]
    .push(error)
    .push(notes)
    .push(buttons)
    .spacing(12)
    .into()
}

/// Read-only mode: the reason, the message as selectable fields, "Copy
/// message" and "Close".
fn read_only<'a>(
    modal: &'a MessageModal,
    reason: &'a str,
    max_height: f32,
) -> Element<'a, Message> {
    let mut content = column![].spacing(12);
    let mut extra = 0.0;
    if !reason.is_empty() {
        // Two lines at most, padding 6, the gap below.
        extra += 2.0 * 12.0 * 1.3 + 12.0 + 12.0;
        content = content.push(
            container(
                row![
                    crate::icons::tinted(crate::icons::Icon::Info, 14.0, |p| p.accent),
                    text(reason)
                        .size(12)
                        .width(Fill)
                        .wrapping(text::Wrapping::WordOrGlyph),
                ]
                .spacing(8)
                .align_y(iced::Center),
            )
            .padding([6, 10])
            .width(Fill)
            .style(theme::state_banner),
        );
    }
    let summary = text_editor(&modal.summary_view)
        .size(16)
        .padding(6)
        .style(theme::editor)
        .wrapping(text::Wrapping::WordOrGlyph)
        .on_action(|action| Message::CommitModalText {
            body: false,
            action,
        });
    let body = text_editor(&modal.body)
        .placeholder("No description")
        .padding(6)
        .style(theme::editor)
        .wrapping(text::Wrapping::WordOrGlyph)
        .min_height(DESCRIPTION_MIN)
        .max_height(description_max(max_height, extra))
        .on_action(|action| Message::CommitModalText { body: true, action });
    let copy = with_tooltip_text(
        button(text("Copy message"))
            .padding([6, 14])
            .style(button::secondary)
            .on_press(Message::CommitModalCopy)
            .into(),
        "Copies the summary and the description".to_owned(),
    );
    let buttons = row![
        space::horizontal(),
        copy,
        button(text("Close"))
            .padding([6, 14])
            .style(button::primary)
            .on_press(Message::CommitModalClose),
    ]
    .spacing(8);
    content.push(summary).push(body).push(buttons).into()
}

/// "Discard changes?" over the modal: "Keep editing" and "Discard".
fn confirm_discard<'a>() -> Element<'a, Message> {
    let dialog = container(
        column![
            text("Discard changes?").size(17),
            text("The changed message and author are lost."),
            row![
                space::horizontal(),
                button(text("Keep editing"))
                    .padding([6, 14])
                    .style(button::secondary)
                    .on_press(Message::CommitModalKeepEditing),
                button(text("Discard"))
                    .padding([6, 14])
                    .style(button::danger)
                    .on_press(Message::CommitModalDiscard),
            ]
            .spacing(8),
        ]
        .spacing(12),
    )
    .width(380)
    .padding(20)
    .style(theme::dialog);
    opaque(
        mouse_area(
            container(opaque(dialog))
                .center(Fill)
                .style(theme::backdrop),
        )
        .on_press(Message::CommitModalKeepEditing)
        .interaction(iced::mouse::Interaction::Idle),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_stays_inside_the_window() {
        assert_eq!(bounds(Size::new(1400.0, 860.0)), Size::new(640.0, 602.0));
        let small = bounds(Size::new(600.0, 400.0));
        assert_eq!(small.width, 568.0);
        assert_eq!(small.height, 280.0);
        assert_eq!(description_max(200.0, 0.0), DESCRIPTION_MIN);
    }
}
