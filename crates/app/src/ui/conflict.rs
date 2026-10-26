//! Center pane: conflict resolution view (M4-05, ADR-022). "Ours" top
//! left, "theirs" top right with checkboxes per hunk and line, the editable
//! output below.

use git_core::{Chunk, ConflictContent, ConflictText, PickState, Side, WholeFileReason};
use iced::widget::{
    button, checkbox, column, container, row, scrollable, space, text, text_editor, Column,
};
use iced::{Element, Fill, FillPortion, Font};

use crate::app::Message;
use crate::conflict::ConflictView;
use crate::repo_view::{Loadable, RepoView};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::{with_tooltip_text, write_tooltip};

const TEXT_SIZE: f32 = 12.0;

pub fn view<'a>(repo: &'a RepoView, conflict: &'a ConflictView) -> Element<'a, Message> {
    let idle = repo.write_block.is_none() && repo.busy.is_none();
    let close = button(text("Close").size(12))
        .padding([2, 8])
        .style(button::secondary)
        .on_press(Message::DiffClosed);
    let title = row![text(&conflict.path).size(14), space::horizontal()]
        .spacing(8)
        .align_y(iced::Center);

    let body: Element<'a, Message> = match &conflict.loaded {
        Loadable::Loading => muted("Loading conflict…").into(),
        Loadable::Failed(error) => text(error).style(theme::text_danger).into(),
        Loadable::Ready(loaded) => match &loaded.content {
            ConflictContent::Text(conflict_text) => {
                return text_view(repo, conflict, conflict_text, title, close, idle);
            }
            ConflictContent::WholeFile(reason) => whole_file(repo, *reason, idle),
        },
    };
    column![row![title, close].align_y(iced::Center), body]
        .spacing(12)
        .padding(12)
        .into()
}

fn action<'a>(
    repo: &RepoView,
    label: &'a str,
    style: fn(&iced::Theme, button::Status) -> button::Style,
    message: Option<Message>,
    blocker: Option<String>,
) -> Element<'a, Message> {
    let button = button(text(label).size(12)).padding([3, 10]).style(style);
    if repo.write_block.is_some() {
        return write_tooltip(button.into(), repo);
    }
    match blocker {
        Some(reason) => with_tooltip_text(button.into(), reason),
        None => button.on_press_maybe(message).into(),
    }
}

fn text_view<'a>(
    repo: &'a RepoView,
    conflict: &'a ConflictView,
    conflict_text: &'a ConflictText,
    title: iced::widget::Row<'a, Message>,
    close: iced::widget::Button<'a, Message>,
    idle: bool,
) -> Element<'a, Message> {
    let save_blocker = conflict.save_blocker();
    let toolbar = row![
        title,
        button(text("Take all ours").size(12))
            .padding([3, 10])
            .style(button::secondary)
            .on_press(Message::ConflictTakeAll(Side::Ours)),
        button(text("Take all theirs").size(12))
            .padding([3, 10])
            .style(button::secondary)
            .on_press(Message::ConflictTakeAll(Side::Theirs)),
        action(
            repo,
            "Save and mark resolved",
            button::success,
            idle.then_some(Message::ConflictSave),
            save_blocker,
        ),
        close,
    ]
    .spacing(8)
    .align_y(iced::Center);

    let (ours_label, theirs_label) = &conflict.labels;
    let sides = row![
        side_panel(conflict, conflict_text, Side::Ours, ours_label),
        side_panel(conflict, conflict_text, Side::Theirs, theirs_label),
    ]
    .spacing(8)
    .height(FillPortion(3));

    let open = conflict.picks.open_count();
    let mut output_header = row![muted("Output")].spacing(10).align_y(iced::Center);
    if conflict.edited {
        output_header = output_header
            .push(
                text("Edited by hand: picks no longer change the output")
                    .size(11)
                    .style(theme::text_warning),
            )
            .push(
                button(text("Reset output").size(11))
                    .padding([2, 8])
                    .style(button::secondary)
                    .on_press(Message::ConflictResetOutput),
            );
    } else if open > 0 {
        let left = if open == 1 {
            "1 conflict left".to_string()
        } else {
            format!("{open} conflicts left")
        };
        output_header = output_header.push(muted(left).size(11));
    }
    let output = text_editor(&conflict.output)
        .on_action(Message::ConflictOutputEdited)
        .font(Font::MONOSPACE)
        .size(TEXT_SIZE)
        .height(Fill);

    column![
        toolbar,
        sides,
        column![output_header, output]
            .spacing(6)
            .height(FillPortion(2)),
    ]
    .spacing(10)
    .padding(12)
    .into()
}

/// Line text without its terminator.
fn display(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

fn side_panel<'a>(
    conflict: &'a ConflictView,
    conflict_text: &'a ConflictText,
    side: Side,
    label: &'a str,
) -> Element<'a, Message> {
    let ours = side == Side::Ours;
    let mut rows = Column::new().spacing(0);
    let mut hunk = 0;
    for chunk in &conflict_text.chunks {
        match chunk {
            Chunk::Clean(lines) => {
                for line in lines {
                    rows = rows.push(
                        container(
                            text(display(line))
                                .font(Font::MONOSPACE)
                                .size(TEXT_SIZE)
                                .style(theme::text_muted),
                        )
                        .padding([0, 26]),
                    );
                }
            }
            Chunk::Conflict(conflict_hunk) => {
                let index = hunk;
                hunk += 1;
                let lines = if ours {
                    &conflict_hunk.ours
                } else {
                    &conflict_hunk.theirs
                };
                let state = conflict.picks.hunk_state(index, side);
                let label = match state {
                    PickState::Some => format!("Conflict {} (some lines)", index + 1),
                    _ => format!("Conflict {}", index + 1),
                };
                rows = rows.push(
                    container(
                        checkbox(state == PickState::All)
                            .label(label)
                            .size(13)
                            .text_size(11)
                            .on_toggle(move |_| Message::ConflictToggleHunk { hunk: index, side }),
                    )
                    .padding([3, 4])
                    .width(Fill)
                    .style(theme::diff_hunk_header),
                );
                let picks = conflict
                    .picks
                    .hunks
                    .get(index)
                    .map(|h| h.side(side))
                    .unwrap_or_default();
                for (line_index, line) in lines.iter().enumerate() {
                    let picked = picks.get(line_index).copied().unwrap_or(false);
                    rows = rows.push(
                        container(
                            row![
                                checkbox(picked).size(13).on_toggle(move |_| {
                                    Message::ConflictToggleLine {
                                        hunk: index,
                                        side,
                                        line: line_index,
                                    }
                                }),
                                text(display(line)).font(Font::MONOSPACE).size(TEXT_SIZE),
                            ]
                            .spacing(8)
                            .align_y(iced::Center),
                        )
                        .padding([0, 4])
                        .width(Fill)
                        .style(theme::conflict_line(ours, picked)),
                    );
                }
                if lines.is_empty() {
                    rows = rows.push(
                        container(muted("(no lines)").size(11))
                            .padding([0, 26])
                            .width(Fill)
                            .style(theme::conflict_line(ours, false)),
                    );
                }
            }
        }
    }
    let heading = if ours {
        format!("A · ours · {label}")
    } else {
        format!("B · theirs · {label}")
    };
    column![
        text(heading).size(12).style(if ours {
            theme::text_accent
        } else {
            theme::text_warning
        }),
        container(scrollable(rows).height(Fill).width(Fill))
            .height(Fill)
            .width(Fill)
            .style(theme::graph),
    ]
    .spacing(4)
    .width(FillPortion(1))
    .into()
}

fn whole_file<'a>(repo: &RepoView, reason: WholeFileReason, idle: bool) -> Element<'a, Message> {
    let (explanation, ours_label, theirs_label) = match reason {
        WholeFileReason::TooLarge => (
            "This file is larger than 2 MB, so the conflict view is not shown. Take one \
             side's file:",
            "Take ours",
            "Take theirs",
        ),
        WholeFileReason::Binary => (
            "This file is binary (or not UTF-8), so lines cannot be picked. Take one side's file:",
            "Take ours",
            "Take theirs",
        ),
        WholeFileReason::DeletedOnOneSide { ours_deleted: true } => (
            "Ours deleted this file, theirs changed it.",
            "Delete the file",
            "Keep theirs",
        ),
        WholeFileReason::DeletedOnOneSide {
            ours_deleted: false,
        } => (
            "Ours changed this file, theirs deleted it.",
            "Keep ours",
            "Delete the file",
        ),
    };
    column![
        text(explanation),
        row![
            action(
                repo,
                ours_label,
                button::primary,
                idle.then_some(Message::ConflictTakeFile(Side::Ours)),
                None,
            ),
            action(
                repo,
                theirs_label,
                button::primary,
                idle.then_some(Message::ConflictTakeFile(Side::Theirs)),
                None,
            ),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}
