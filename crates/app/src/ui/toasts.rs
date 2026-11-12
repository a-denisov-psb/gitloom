//! Toast notifications at the bottom left of the window (M12-02).

use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text};
use iced::{Element, Fill, Font};

use crate::app::Message;
use crate::icons::{self, Icon};
use crate::theme;
use crate::toast::{Toast, ToastKind, Toasts};

const WIDTH: f32 = 380.0;

/// The toast layer; `None` without toasts.
pub fn view(toasts: &Toasts) -> Option<Element<'_, Message>> {
    if toasts.list.is_empty() {
        return None;
    }
    let stack = column(toasts.list.iter().map(toast)).spacing(8);
    Some(
        container(stack)
            .padding(iced::Padding::default().left(12).bottom(36))
            .width(Fill)
            .height(Fill)
            .align_x(iced::alignment::Horizontal::Left)
            .align_y(iced::alignment::Vertical::Bottom)
            .into(),
    )
}

fn toast(toast: &Toast) -> Element<'_, Message> {
    let kind = toast.kind;
    let icon = match kind {
        ToastKind::Error | ToastKind::Warning => Icon::Alert,
        ToastKind::Info | ToastKind::Success => Icon::Info,
    };
    let id = toast.id;
    let close = button(icons::icon(Icon::Close, 13.0))
        .on_press(Message::ToastDismissed(id))
        .padding([2, 4])
        .style(theme::flat);
    let mut body = column![
        row![
            icons::colored(icon, 16.0, move |t| theme::toast_color(
                theme::palette(t),
                kind
            )),
            text(&toast.title).size(14),
            space::horizontal(),
            close,
        ]
        .spacing(8)
        .align_y(iced::Center),
        text(&toast.text).size(13),
    ]
    .spacing(6);
    let mut buttons = row![].spacing(6);
    if let Some((label, _)) = &toast.action {
        buttons = buttons.push(
            button(text(label).size(12))
                .on_press(Message::ToastAction(id))
                .padding([3, 10])
                .style(button::primary),
        );
    }
    if let Some(details) = &toast.details {
        let label = if toast.expanded {
            "Hide details"
        } else {
            "Details"
        };
        buttons = buttons
            .push(
                button(text(label).size(12))
                    .on_press(Message::ToastDetails(id))
                    .padding([3, 10])
                    .style(button::secondary),
            )
            .push(
                button(text("Copy").size(12))
                    .on_press(Message::CopyToClipboard(details.clone()))
                    .padding([3, 10])
                    .style(button::secondary),
            );
    }
    body = body.push(buttons);
    if let (true, Some(details)) = (toast.expanded, &toast.details) {
        body = body.push(
            container(
                scrollable(
                    text(details.replace('\t', "    "))
                        .font(Font::MONOSPACE)
                        .size(11)
                        .wrapping(text::Wrapping::WordOrGlyph),
                )
                .height(iced::Length::Shrink),
            )
            .max_height(180)
            .padding(6)
            .width(Fill)
            .style(theme::dialog_details),
        );
    }
    let card = container(body)
        .padding([10, 12])
        .width(WIDTH)
        .style(theme::toast(kind));
    // Resting on a toast pauses its timeout.
    mouse_area(iced::widget::opaque(card))
        .on_enter(Message::ToastHovered { id, hovered: true })
        .on_exit(Message::ToastHovered { id, hovered: false })
        .into()
}
