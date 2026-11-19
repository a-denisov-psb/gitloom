//! Preview of the theme editor (M17-04, ADR-059): static sample content
//! drawn with the edited theme only (`themer` with the registry's preview
//! entry), while the editor controls keep the active theme. Below it the
//! contrast warnings.

use iced::widget::{button, column, container, row, space, text, themer, Column};
use iced::{Element, Fill, Theme};

use crate::app::Message;
use crate::theme::{self, palette};
use crate::theme_editor::Editor;

/// A node of the sample graph in lane color id `lane`.
fn node<'a>(lane: u32) -> Element<'a, Message> {
    container(text("GL").size(9).style(|t: &Theme| text::Style {
        color: Some(palette(t).node_text),
    }))
    .width(18)
    .height(18)
    .center_x(18)
    .center_y(18)
    .style(move |t: &Theme| container::Style {
        background: Some(theme::lane_color(&palette(t), lane).into()),
        border: iced::Border::default().rounded(9),
        ..container::Style::default()
    })
    .into()
}

/// A row background: none, hover or selected.
fn row_style(state: u8) -> impl Fn(&Theme) -> container::Style {
    move |t: &Theme| {
        let p = palette(t);
        let background = match state {
            1 => Some(p.bg_hover.into()),
            2 => Some(p.bg_selected.into()),
            _ => None,
        };
        container::Style {
            background,
            text_color: Some(p.text),
            ..container::Style::default()
        }
    }
}

fn graph_rows<'a>() -> Element<'a, Message> {
    use git_core::RefKind;
    let flow = git_graph::flow::FLOW_COLOR_BASE;
    let rows: [(u32, &str, RefKind, bool, &str, u8); 4] = [
        (
            0,
            "main",
            RefKind::LocalBranch,
            true,
            "Merge branch 'feature/login'",
            0,
        ),
        (
            1,
            "origin/main",
            RefKind::RemoteBranch,
            false,
            "Fix the login form",
            1,
        ),
        (2, "v1.2.0", RefKind::Tag, false, "Release 1.2.0", 2),
        (
            3,
            "develop",
            RefKind::LocalBranch,
            false,
            "Add the settings page",
            0,
        ),
    ];
    let lines = Column::with_children(rows.into_iter().map(
        |(lane, name, kind, head, message, state)| {
            container(
                row![
                    container(text(name).size(11))
                        .padding([1, 6])
                        .style(theme::ref_pill(kind, head)),
                    node(lane),
                    text(message).size(12),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 6])
            .width(Fill)
            .style(row_style(state))
            .into()
        },
    ));
    // Git flow colors: production, hotfix, release, development, the
    // feature colors and unknown.
    let chips = row([0_u32, 1, 2, 3, 4, 5, 6, 7, 8]
        .into_iter()
        .map(|id| node(flow + id)))
    .spacing(4);
    column![
        lines,
        row![text("Git flow").size(11).style(theme::text_muted), chips]
            .spacing(8)
            .align_y(iced::Alignment::Center)
    ]
    .spacing(6)
    .into()
}

/// The changed byte range of `line` against `other` (common prefix and
/// suffix removed), for the word diff emphasis of the sample.
fn changed(line: &str, other: &str) -> Option<std::ops::Range<usize>> {
    let prefix = line
        .char_indices()
        .zip(other.chars())
        .take_while(|((_, a), b)| a == b)
        .last()
        .map_or(0, |((i, c), _)| i + c.len_utf8());
    let suffix = line[prefix..]
        .chars()
        .rev()
        .zip(other[prefix.min(other.len())..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum::<usize>();
    let end = line.len() - suffix;
    (prefix < end).then_some(prefix..end)
}

fn diff_sample<'a>(editor: &Editor) -> Element<'a, Message> {
    use crate::theme_editor::{preview_code, PREVIEW_NEW, PREVIEW_OLD};
    use git_core::DiffLineKind;
    let (old, new) = preview_code(editor.data.syntax);
    let style = crate::editor_style::EditorStyle::default();
    let old_lines: Vec<&str> = PREVIEW_OLD.lines().collect();
    let new_lines: Vec<&str> = PREVIEW_NEW.lines().collect();
    let p = &editor.data.palette;
    let mut lines = Vec::new();
    // Two removed lines, two added lines, then the unchanged rest.
    for (kind, outcome, own, other) in [
        (DiffLineKind::Removed, &old, &old_lines, &new_lines),
        (DiffLineKind::Added, &new, &new_lines, &old_lines),
    ] {
        for index in 0..2 {
            let line = own.get(index).copied().unwrap_or_default();
            let other = other.get(index).copied().unwrap_or_default();
            lines.push((kind, outcome.clone(), index, line, changed(line, other)));
        }
    }
    for (index, line) in new_lines.iter().enumerate().skip(2) {
        lines.push((DiffLineKind::Context, new.clone(), index, line, None));
    }
    let emphasis = |kind| match kind {
        DiffLineKind::Added => Some(p.diff_added_emphasis),
        DiffLineKind::Removed => Some(p.diff_removed_emphasis),
        DiffLineKind::Context => None,
    };
    Column::with_children(
        lines
            .into_iter()
            .map(|(kind, outcome, index, line, ranges)| {
                let ranges: Vec<std::ops::Range<usize>> = ranges.into_iter().collect();
                let pieces = crate::highlight::pieces(
                    line,
                    outcome.line(index).unwrap_or(&[]),
                    ranges.as_slice(),
                    style.tab,
                );
                let background = emphasis(kind);
                let spans: Vec<iced::widget::text::Span<'a>> = pieces
                    .into_iter()
                    .map(|piece| {
                        crate::ui::diff::code_span(piece.text, piece.style, &style)
                            .background_maybe(background.filter(|_| piece.emphasis))
                    })
                    .collect();
                let marker = match kind {
                    DiffLineKind::Added => "+",
                    DiffLineKind::Removed => "-",
                    DiffLineKind::Context => " ",
                };
                container(
                    row![
                        text(marker)
                            .font(style.font)
                            .size(12)
                            .style(theme::text_muted),
                        iced::widget::rich_text(spans)
                            .font(style.font)
                            .size(12)
                            .wrapping(text::Wrapping::None),
                    ]
                    .spacing(6),
                )
                .padding([1, 6])
                .width(Fill)
                .clip(true)
                .style(theme::diff_line(kind, false))
                .into()
            }),
    )
    .into()
}

fn panel_sample<'a>() -> Element<'a, Message> {
    let status = row![
        text("Success").style(theme::text_success),
        text("Warning").style(theme::text_warning),
        text("Danger").style(theme::text_danger),
        text("Accent").style(theme::text_accent),
    ]
    .spacing(12);
    container(
        column![
            text("Panel text"),
            text("Muted text").style(theme::text_muted),
            container(text("Hovered row"))
                .padding([3, 6])
                .width(Fill)
                .style(row_style(1)),
            container(text("Selected row"))
                .padding([3, 6])
                .width(Fill)
                .style(row_style(2)),
            row![
                button(text("Primary")).on_press(Message::Noop),
                button(text("Secondary"))
                    .on_press(Message::Noop)
                    .style(button::secondary),
                button(text("Danger"))
                    .on_press(Message::Noop)
                    .style(button::danger),
            ]
            .spacing(6),
            status,
        ]
        .spacing(6),
    )
    .padding(8)
    .width(Fill)
    .style(|t: &Theme| {
        let p = palette(t);
        container::Style {
            background: Some(p.bg_panel.into()),
            border: iced::Border::default().rounded(4).width(1).color(p.border),
            text_color: Some(p.text),
            ..container::Style::default()
        }
    })
    .into()
}

/// The preview panel and the warnings.
pub fn view<'a>(editor: &Editor) -> Element<'a, Message> {
    let def = editor
        .data
        .clone()
        .into_def(crate::themes::PREVIEW_ID.to_owned());
    let sample = container(
        column![
            text("Preview").size(12).style(theme::text_muted),
            graph_rows(),
            diff_sample(editor),
            panel_sample(),
        ]
        .spacing(10),
    )
    .padding(10)
    .width(Fill)
    .style(|t: &Theme| {
        let p = palette(t);
        container::Style {
            background: Some(p.bg_graph.into()),
            border: iced::Border::default().rounded(4).width(1).color(p.border),
            text_color: Some(p.text),
            ..container::Style::default()
        }
    });
    let preview = themer(Some(theme::iced_theme(&def)), sample);
    let warnings = crate::theme_editor::warnings(&editor.data);
    let notes: Element<'a, Message> = if warnings.is_empty() {
        text("No contrast warnings.")
            .size(12)
            .style(theme::text_muted)
            .into()
    } else {
        Column::with_children(
            std::iter::once(
                text("Contrast warnings (saving still works):")
                    .size(12)
                    .style(theme::text_warning)
                    .into(),
            )
            .chain(warnings.into_iter().map(|w| text(w).size(12).into())),
        )
        .spacing(2)
        .into()
    };
    column![preview, notes, space::vertical().height(0)]
        .spacing(10)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_ranges() {
        assert_eq!(changed("a -> u32 {", "a -> u64 {"), Some(6..8));
        assert_eq!(changed("same", "same"), None);
        assert_eq!(changed("abc", ""), Some(0..3));
    }
}
