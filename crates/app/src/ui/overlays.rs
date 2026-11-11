//! Rendering of the modal dialog and the context menu above the window.

use iced::widget::{
    button, column, container, mouse_area, opaque, pick_list, row, scrollable, space, text,
    text_input, Column,
};
use iced::{Element, Fill, Font, Length, Padding};

use crate::app::Message;
use crate::overlay::{
    ButtonKind, ContextMenu, FieldKind, Form, MenuEntry, Modal, MENU_ITEM_HEIGHT, MENU_LABEL_SIZE,
    MENU_SHORTCUT_SIZE,
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
        let button = button(text(&b.label))
            .on_press_maybe(b.disabled.is_none().then_some(Message::ModalAction(index)))
            .padding([6, 14])
            .style(style);
        match &b.disabled {
            Some(reason) => crate::ui::panel::with_tooltip_text(button.into(), reason.clone()),
            None => button.into(),
        }
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
                    .padding(6)
                    .style(theme::input);
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
            FieldKind::Check => {
                content = content.push(
                    iced::widget::checkbox(field.value == "true")
                        .label(field.label.clone())
                        .on_toggle(move |checked| Message::FormInput {
                            field: index,
                            value: checked.to_string(),
                        }),
                );
                continue;
            }
        };
        content = content.push(column![muted(&field.label).size(12), input].spacing(4));
    }
    if let Some(error) = &form.error {
        content = content.push(text(error).size(12).style(theme::text_danger));
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

/// Keyboard shortcut list (Ctrl+/): all registered shortcuts grouped by
/// category, with a filter; actions of later tasks are marked disabled.
pub fn shortcut_list(filter: &str) -> Element<'_, Message> {
    use crate::actions::{self, MAC};

    let mut rows = Column::new().spacing(2);
    let groups = actions::shortcut_rows(filter, MAC);
    if groups.is_empty() {
        rows = rows.push(muted("No matching shortcut"));
    }
    for (category, entries) in groups {
        rows = rows.push(
            container(text(category.title()).size(13)).padding(Padding::default().top(8).bottom(2)),
        );
        for action in entries {
            let label = row![text(&action.label).size(13)].spacing(8);
            let keys = text(action.shortcut_labels(MAC))
                .size(12)
                .font(Font::MONOSPACE);
            rows = rows.push(
                row![label, space::horizontal(), keys]
                    .align_y(iced::Center)
                    .padding([2, 4]),
            );
        }
    }
    let content = column![
        row![
            text("Keyboard shortcuts").size(17),
            space::horizontal(),
            button(text("Close"))
                .on_press(Message::ShortcutListClosed)
                .padding([4, 12])
                .style(button::secondary),
        ]
        .align_y(iced::Center),
        text_input("Filter shortcuts", filter)
            .id(crate::app::SHORTCUT_FILTER_INPUT)
            .on_input(Message::ShortcutFilterChanged)
            .padding(6)
            .style(theme::input),
        crate::ui::vscroll(rows).height(Fill),
    ]
    .spacing(12);
    let dialog = container(content)
        .width(DIALOG_WIDTH + 80.0)
        .max_height(640)
        .padding(20)
        .style(theme::dialog);
    // Clicks inside the list must not reach the backdrop, which closes it.
    opaque(
        mouse_area(
            container(opaque(dialog))
                .center(Fill)
                .style(theme::backdrop),
        )
        .on_press(Message::ShortcutListClosed)
        .interaction(iced::mouse::Interaction::Idle),
    )
}

/// About dialog (M6-03): name, version, commit, license, links and the
/// third-party components.
pub fn about() -> Element<'static, Message> {
    use crate::about;
    use git_core::identity;

    let link = |label: &'static str, target: String| {
        button(text(label).size(13))
            .on_press(Message::OpenLink(target))
            .padding([4, 10])
            .style(button::secondary)
    };
    let components = about::components();
    let mut list = Column::new().spacing(1);
    for component in components {
        list = list.push(
            row![
                text(component.name).size(12).width(Fill),
                text(component.version).size(12).width(90),
                muted(component.license).size(12).width(170),
            ]
            .spacing(8),
        );
    }
    let content = column![
        row![
            column![
                text(identity::DISPLAY_NAME).size(22),
                muted(env!("CARGO_PKG_DESCRIPTION")),
            ]
            .spacing(4),
            space::horizontal(),
            button(text("Close"))
                .on_press(Message::AboutClosed)
                .padding([4, 12])
                .style(button::secondary),
        ]
        .align_y(iced::alignment::Vertical::Top),
        column![
            text(format!("Version {}", about::version())),
            muted(format!("Commit {}", about::commit())).size(12),
        ]
        .spacing(2),
        text(format!(
            "Copyright © 2026 the {} contributors. Licensed under {} – free software \
             without warranty.",
            identity::DISPLAY_NAME,
            about::LICENSE_ID
        ))
        .size(12),
        row![
            link("User guide", about::user_guide_url(about::version())),
            link("License", about::LICENSE_URL.to_owned()),
            link(
                "Third-party licenses",
                about::notices_target(std::env::current_exe().ok().as_deref())
            ),
            link("Repository", identity::REPOSITORY_URL.to_owned()),
            link("Report an issue", identity::ISSUES_URL.to_owned()),
        ]
        .spacing(8)
        .wrap(),
        text(format!("Third-party components ({})", components.len())).size(13),
        container(crate::ui::vscroll(list).height(220))
            .padding(8)
            .width(Fill)
            .style(theme::dialog_details),
    ]
    .spacing(12);
    let dialog = container(content)
        .width(DIALOG_WIDTH + 80.0)
        .padding(20)
        .style(theme::dialog);
    // Clicks inside the dialog must not reach the backdrop, which closes it.
    opaque(
        mouse_area(
            container(opaque(dialog))
                .center(Fill)
                .style(theme::backdrop),
        )
        .on_press(Message::AboutClosed)
        .interaction(iced::mouse::Interaction::Idle),
    )
}

/// Command palette (Ctrl+P): query field at the top center, results with
/// the shortcut or path on the right, disabled entries with their reason.
pub fn palette(palette: &crate::palette::Palette) -> Element<'_, Message> {
    let mut rows = Column::new().spacing(1);
    if palette.loading {
        rows = rows.push(container(muted("Loading…")).padding([6, 10]));
    } else if palette.shown_count() == 0 {
        rows = rows.push(container(muted("No matches")).padding([6, 10]));
    }
    for (index, entry) in palette.shown().enumerate() {
        let mut line = row![text(&entry.label).size(13).wrapping(text::Wrapping::None)]
            .spacing(8)
            .align_y(iced::Center);
        if let Some(reason) = &entry.disabled {
            line = line.push(muted(reason).size(11));
        }
        line = line.push(space::horizontal());
        if let Some(detail) = &entry.detail {
            line = line.push(
                muted(detail)
                    .size(11)
                    .wrapping(text::Wrapping::None)
                    .font(Font::MONOSPACE),
            );
        }
        let style = if entry.disabled.is_some() {
            theme::menu_item_disabled
        } else {
            theme::menu_item
        };
        rows = rows.push(
            button(line)
                .on_press(Message::PaletteRun(index))
                .padding([5, 10])
                .width(Fill)
                .style(move |theme, status| {
                    if index == palette.selected {
                        theme::menu_item(theme, iced::widget::button::Status::Hovered)
                    } else {
                        style(theme, status)
                    }
                }),
        );
    }
    let list = crate::ui::vscroll(rows).height(Length::Shrink);
    let content = column![
        text_input(palette.mode.placeholder(), &palette.query)
            .id(crate::app::PALETTE_INPUT)
            .on_input(Message::PaletteQuery)
            .on_submit(Message::PaletteSubmit)
            .padding(8)
            .style(theme::input)
            .size(14),
        container(list).max_height(420),
    ]
    .spacing(8);
    let dialog = container(content).width(620).padding(10).style(theme::menu);
    opaque(
        mouse_area(
            container(opaque(dialog))
                .width(Fill)
                .height(Fill)
                .align_x(iced::Center)
                .padding(Padding::default().top(60)),
        )
        .on_press(Message::PaletteClosed)
        .interaction(iced::mouse::Interaction::Idle),
    )
}

/// Menu at its origin; a click anywhere else closes it.
/// Commit message dialog (reword, squash).
/// "Clone a repository" (M10-02): URL, where to clone to, name, full path,
/// submodules; progress and Cancel while it runs.
pub fn clone_dialog(dialog: &crate::clone_dialog::CloneDialog) -> Element<'_, Message> {
    let idle = dialog.running.is_none();
    let field = |label: &'static str, input: Element<'static, Message>| {
        column![text(label).size(12), input].spacing(4)
    };
    let input = |placeholder: &str, value: &str, on_input: fn(String) -> Message| {
        let mut input = iced::widget::text_input(placeholder, value)
            .padding(6)
            .style(theme::input);
        if idle {
            input = input.on_input(on_input).on_submit(Message::CloneSubmit);
        }
        input
    };
    let url = input(
        "https://github.com/owner/repo.git",
        &dialog.url,
        Message::CloneUrlChanged,
    )
    .id(CLONE_URL_INPUT);
    let parent = row![
        input("Folder", &dialog.parent, Message::CloneParentChanged),
        button(text("Browse…"))
            .on_press_maybe(idle.then_some(Message::CloneBrowse))
            .padding([6, 12])
            .style(button::secondary),
    ]
    .spacing(6);
    let name = input("Folder name", &dialog.name, Message::CloneNameChanged);
    let full_path = dialog
        .full_path()
        .map_or_else(String::new, |p| p.display().to_string());
    let mut submodules = iced::widget::checkbox(dialog.submodules)
        .label("Clone submodules")
        .size(14)
        .text_size(12);
    if idle {
        submodules = submodules.on_toggle(Message::CloneSubmodulesToggled);
    }
    let mut content = column![
        text("Clone a repository").size(17),
        column![text("URL").size(12), url].spacing(4),
        column![text("Where to clone to").size(12), parent].spacing(4),
        column![text("Name").size(12), name].spacing(4),
        field("Full path", muted(full_path).size(12).into()),
        submodules,
    ]
    .spacing(12);
    if let Some(progress) = dialog.progress_text() {
        content = content.push(text(progress).size(12));
    }
    if let Some(error) = &dialog.error {
        content = content.push(
            text(error)
                .size(12)
                .style(theme::text_danger)
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    }
    let ready = idle && dialog.problem().is_none();
    content = content.push(
        row![
            space::horizontal(),
            button(text("Cancel"))
                .on_press(Message::CloneCancel)
                .padding([6, 14])
                .style(button::secondary),
            button(text("Clone"))
                .on_press_maybe(ready.then_some(Message::CloneSubmit))
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

/// Id of the URL field (focused when the clone dialog opens).
pub const CLONE_URL_INPUT: &str = "clone-url";

pub fn message_dialog(dialog: &crate::overlay::MessageDialog) -> Element<'_, Message> {
    let mut content = column![
        text(&dialog.title).size(17),
        iced::widget::text_editor(&dialog.editor)
            .on_action(Message::CommitMessageEdited)
            .padding(6)
            .style(theme::editor)
            .height(180),
    ]
    .spacing(12);
    if let Some(error) = &dialog.error {
        content = content.push(text(error).size(12).style(theme::text_danger));
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
                    text(label)
                        .size(MENU_LABEL_SIZE)
                        .wrapping(text::Wrapping::None),
                    space::horizontal()
                ];
                if let Some(shortcut) = shortcut {
                    line = line.push(muted(shortcut).size(MENU_SHORTCUT_SIZE));
                }
                let highlighted = menu.highlighted == Some(index);
                let item = button(line.align_y(iced::Center))
                    .on_press_maybe(enabled.then_some(Message::MenuSelected(index)))
                    .height(MENU_ITEM_HEIGHT)
                    .padding([4, 10])
                    .width(Fill)
                    .style(move |theme, status| {
                        // The keyboard highlight looks like hover.
                        let status = if highlighted && status == button::Status::Active {
                            button::Status::Hovered
                        } else {
                            status
                        };
                        theme::menu_item(theme, status)
                    });
                match hint {
                    Some(hint) => crate::ui::panel::with_tooltip_text(item.into(), hint.clone()),
                    None => item.into(),
                }
            }
        }
    }));

    let menu = container(entries)
        .width(menu.width.min(window.width))
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
