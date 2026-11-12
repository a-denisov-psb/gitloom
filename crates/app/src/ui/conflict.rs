//! Center pane: conflict resolution view (M4-05, ADR-022). "Ours" top
//! left, "theirs" top right with checkboxes per hunk and line, the editable
//! output below.

use git_core::{Chunk, ConflictContent, ConflictText, PickState, Side, WholeFileReason};
use iced::widget::{
    button, checkbox, column, container, row, scrollable, space, text, text_editor, Column,
};
use iced::{Element, Fill, FillPortion};

use crate::app::Message;
use crate::conflict::ConflictView;
use crate::editor_style::EditorStyle;
use crate::repo_view::{Loadable, RepoView};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::{with_tooltip_text, write_tooltip};

pub fn view<'a>(
    repo: &'a RepoView,
    conflict: &'a ConflictView,
    style: &EditorStyle,
) -> Element<'a, Message> {
    let style = *style;
    let idle = repo.write_block.is_none() && repo.busy.is_none();
    let idle_tool = repo.write_block.is_none() && repo.busy.is_none();
    let merge_tool = crate::ui::panel::with_tooltip_text(
        button(text("Open in merge tool").size(12))
            .padding([2, 8])
            .style(button::secondary)
            .on_press_maybe(idle_tool.then(|| Message::OpenMergeTool(conflict.path.clone())))
            .into(),
        "Resolve this file in the external merge tool (git mergetool)".to_owned(),
    );
    let close = row![
        merge_tool,
        button(text("Close").size(12))
            .padding([2, 8])
            .style(button::secondary)
            .on_press(Message::DiffClosed)
    ]
    .spacing(6);
    let title = row![text(&conflict.path).size(14), space::horizontal()]
        .spacing(8)
        .align_y(iced::Center);

    let body: Element<'a, Message> = match &conflict.loaded {
        Loadable::Loading => muted("Loading conflict…").into(),
        Loadable::Failed(error) => text(error).style(theme::text_danger).into(),
        Loadable::Ready(loaded) => match &loaded.content {
            ConflictContent::Text(conflict_text) => {
                return text_view(repo, conflict, conflict_text, title, close, idle, style);
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
    close: iced::widget::Row<'a, Message>,
    idle: bool,
    style: EditorStyle,
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

    let (ours_label, theirs_label) = &conflict.headers;
    let sides = row![
        side_panel(conflict, conflict_text, Side::Ours, ours_label, style),
        side_panel(conflict, conflict_text, Side::Theirs, theirs_label, style),
    ]
    .spacing(8)
    .height(FillPortion(3));

    let mut output_header = row![muted("Output")].spacing(10).align_y(iced::Center);
    // "conflict n of N" with arrows like GitKraken (M12-04); also counts
    // the markers of a hand-edited output.
    if let Some((current, count)) = conflict.position() {
        let arrow = |icon: &'static str, next: bool, tip: &'static str| {
            with_tooltip_text(
                button(text(icon).size(11))
                    .padding([0, 6])
                    .style(button::text)
                    .on_press(Message::ConflictStep { next })
                    .into(),
                tip.to_owned(),
            )
        };
        output_header = output_header.push(
            row![
                muted(format!("conflict {current} of {count}")).size(11),
                arrow("↑", false, "Previous conflict (Alt+↑)"),
                arrow("↓", true, "Next conflict (Alt+↓)"),
            ]
            .spacing(2)
            .align_y(iced::Center),
        );
    }
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
    }
    let output = text_editor(&conflict.output)
        .on_action(Message::ConflictOutputEdited)
        // Alt+↑/↓ step through the conflicts also while typing.
        .key_binding(|press| {
            use iced::keyboard::{key::Named, Key};
            let step = match press.key.as_ref() {
                Key::Named(Named::ArrowUp) => Some(false),
                Key::Named(Named::ArrowDown) => Some(true),
                _ => None,
            };
            match step {
                Some(next) if press.modifiers.alt() && !press.modifiers.command() => {
                    Some(text_editor::Binding::Custom(Message::ConflictStep { next }))
                }
                _ => text_editor::Binding::from_key_press(press),
            }
        })
        .font(style.font)
        .size(style.size)
        .height(Fill)
        .style(theme::editor)
        .highlight_with::<crate::ui::code_highlighter::SyntaxHighlighter>(
            crate::ui::code_highlighter::Settings {
                path: conflict.path.clone(),
                theme: style.code_theme,
                enabled: style.highlight,
            },
            crate::ui::code_highlighter::format,
        );

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
    style: EditorStyle,
) -> Element<'a, Message> {
    let ours = side == Side::Ours;
    let mut rows = Column::new().spacing(0);
    let mut hunk = 0;
    // Line of this side's file (clean lines and its own conflict lines),
    // for the highlighting spans (M11-09).
    let mut file_line = 0;
    let highlight = conflict
        .highlight
        .as_ref()
        .filter(|h| style.highlight && h.theme == style.code_theme);
    let code = |line: &str, index: usize, muted_text: bool| -> Element<'a, Message> {
        let line = display(line);
        match highlight.and_then(|h| h.spans(ours, index)) {
            Some(spans) => iced::widget::rich_text(
                crate::highlight::segments(line, spans, style.tab)
                    .into_iter()
                    .map(|(piece, span)| crate::ui::diff::code_span(piece, span, &style))
                    .collect::<Vec<_>>(),
            )
            .font(style.font)
            .size(style.size)
            .into(),
            None => {
                let text = text(style.expand(line).into_owned())
                    .font(style.font)
                    .size(style.size);
                if muted_text {
                    text.style(theme::text_muted).into()
                } else {
                    text.into()
                }
            }
        }
    };
    for chunk in &conflict_text.chunks {
        match chunk {
            Chunk::Clean(lines) => {
                for line in lines {
                    rows = rows.push(container(code(line, file_line, true)).padding([0, 26]));
                    file_line += 1;
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
                                code(line, file_line + line_index, false),
                            ]
                            .spacing(8)
                            .align_y(iced::Center),
                        )
                        .padding([0, 4])
                        .width(Fill)
                        .style(theme::conflict_line(ours, picked)),
                    );
                }
                file_line += lines.len();
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
        format!("A · {label}")
    } else {
        format!("B · {label}")
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
