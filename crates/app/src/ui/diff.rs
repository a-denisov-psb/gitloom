//! Center pane: read-only file diff (replaces the graph while open).

use git_core::{DiffHunk, DiffKind, DiffLine, DiffLineKind, FileDiff};
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text, Column};
use iced::{alignment, Element, Fill, Font};

use crate::app::{DiscardTarget, Message};
use crate::repo_view::{DiffView, Loadable, RepoView, HUNK_HEADER};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::with_tooltip;

const LINE_HEIGHT: f32 = 20.0;
const NUMBER_WIDTH: f32 = 52.0;
const MARKER_WIDTH: f32 = 16.0;
const OVERSCAN: usize = 20;
const TEXT_SIZE: f32 = 12.0;

pub fn view<'a>(repo: &'a RepoView, diff: &'a DiffView) -> Element<'a, Message> {
    let request = &diff.request;
    let source = match &request.kind {
        DiffKind::Unstaged => "Unstaged".to_string(),
        DiffKind::Untracked => "Untracked".to_string(),
        DiffKind::Staged => "Staged".to_string(),
        DiffKind::Commit(id) => format!("commit {}", &id[..id.len().min(7)]),
        DiffKind::Stash { part, .. } => match part {
            git_core::StashPart::Staged => "Stash · staged".to_string(),
            git_core::StashPart::Unstaged => "Stash · unstaged".to_string(),
            git_core::StashPart::Untracked => "Stash · untracked".to_string(),
        },
    };
    let title = match &request.old_path {
        Some(old) => format!("{old} → {}", request.path),
        None => request.path.clone(),
    };

    let modes = row![
        mode_button("Hunk", true),
        with_tooltip(mode_button("Inline", false)),
        with_tooltip(mode_button("Split", false)),
    ]
    .spacing(2);

    // Staging actions for the selected lines (working tree diffs only).
    let selection_action = match diff.staged() {
        Some(staged) if !diff.selection.is_empty() => {
            let label = if staged {
                "Unstage selected lines"
            } else {
                "Stage selected lines"
            };
            let apply = button(text(label).size(12))
                .padding([2, 8])
                .style(if staged {
                    button::danger
                } else {
                    button::success
                });
            let mut actions = row![].spacing(6);
            if !staged {
                let discard = button(text("Discard selected lines").size(12))
                    .padding([2, 8])
                    .style(button::danger);
                actions = actions.push(write_action(
                    repo,
                    discard,
                    Message::Discard(DiscardTarget::Lines),
                ));
            }
            Some(actions.push(write_action(repo, apply, Message::ApplySelectedLines)))
        }
        _ => None,
    };

    let header = container(
        row![
            text(title).wrapping(text::Wrapping::None),
            muted(source).size(12),
            space::horizontal(),
        ]
        .push(selection_action)
        .push(
            row![
                modes,
                crate::ui::panel::with_tooltip_text(
                    button(crate::icons::icon(crate::icons::Icon::Close, 15.0))
                        .on_press(Message::DiffClosed)
                        .padding([2, 6])
                        .style(theme::flat)
                        .into(),
                    "Close diff (Esc)".to_owned(),
                ),
            ]
            .spacing(12),
        )
        .spacing(12)
        .padding([6, 12])
        .align_y(iced::Center),
    )
    .width(Fill)
    .clip(true)
    .style(theme::column_header);

    let body: Element<'a, Message> = match &diff.diff {
        Loadable::Loading if diff.fetching => centered(muted("Fetching missing content…")),
        Loadable::Loading => centered(muted("Loading diff…")),
        Loadable::Failed(error) => centered(text(error).style(theme::text_danger)),
        Loadable::Ready(file) => content(repo, diff, file),
    };

    container(column![header, body])
        .width(Fill)
        .height(Fill)
        .style(theme::graph)
        .into()
}

fn content<'a>(repo: &'a RepoView, diff: &'a DiffView, file: &'a FileDiff) -> Element<'a, Message> {
    if file.binary {
        return centered(muted("Binary file not shown"));
    }
    if file.too_large {
        return centered(muted("File too large to display (more than 2 MB)"));
    }
    if file.hunks.is_empty() {
        return centered(muted(match (&file.old_path, &file.new_path) {
            (None, Some(_)) => "Empty file added",
            (Some(_), None) => "Empty file deleted",
            _ => "No changes",
        }));
    }

    // Only visible lines are built; spacers keep the scrollbar exact.
    let total = diff.rows.len();
    let (first, last) = crate::ui::rows_in_view(
        diff.scroll.offset,
        diff.scroll.height,
        total,
        LINE_HEIGHT,
        OVERSCAN,
    );
    let mut rows = Column::with_capacity(last - first + 2)
        .push(space::vertical().height(first as f32 * LINE_HEIGHT));
    let staged = diff.staged();
    for (offset, &(h, l)) in diff.rows[first..last].iter().enumerate() {
        let row_index = first + offset;
        let Some(hunk) = file.hunks.get(h as usize) else {
            continue;
        };
        rows = rows.push(if l == HUNK_HEADER {
            hunk_header(repo, hunk, h as usize, staged)
        } else {
            match hunk.lines.get(l as usize) {
                Some(line) => {
                    let selected = diff.selection.contains(&(h, l));
                    let element = diff_line(line, selected);
                    if staged.is_some() && line.kind != DiffLineKind::Context {
                        mouse_area(element)
                            .on_press(Message::DiffLineClicked(row_index))
                            .interaction(iced::mouse::Interaction::Pointer)
                            .into()
                    } else {
                        element
                    }
                }
                None => space::vertical().height(LINE_HEIGHT).into(),
            }
        });
    }
    rows = rows.push(space::vertical().height((total - last) as f32 * LINE_HEIGHT));

    scrollable(rows)
        .id(diff.scroll_id.clone())
        .on_scroll(|viewport| Message::DiffScrolled {
            offset: viewport.absolute_offset().y,
            height: viewport.bounds().height,
        })
        .height(Fill)
        .into()
}

fn hunk_header<'a>(
    repo: &'a RepoView,
    hunk: &'a DiffHunk,
    index: usize,
    staged: Option<bool>,
) -> Element<'a, Message> {
    let mut content = row![text(&hunk.header)
        .font(Font::MONOSPACE)
        .size(TEXT_SIZE)
        .style(theme::text_muted)
        .wrapping(text::Wrapping::None)]
    .align_y(iced::Center);
    if let Some(staged) = staged {
        content = content.push(space::horizontal());
        if !staged {
            let discard = button(text("Discard hunk").size(11))
                .padding([0, 8])
                .style(button::danger);
            content = content
                .push(write_action(
                    repo,
                    discard,
                    Message::Discard(DiscardTarget::Hunk(index)),
                ))
                .push(space::horizontal().width(6));
        }
        let label = if staged { "Unstage hunk" } else { "Stage hunk" };
        let button = button(text(label).size(11))
            .padding([0, 8])
            .style(button::secondary);
        content = content.push(write_action(repo, button, Message::ApplyHunk(index)));
    }
    container(content)
        .padding([0, 12])
        .height(LINE_HEIGHT)
        .width(Fill)
        .align_y(alignment::Vertical::Center)
        .style(theme::diff_hunk_header)
        .into()
}

fn diff_line(line: &DiffLine, selected: bool) -> Element<'_, Message> {
    let number = |n: Option<u32>| {
        container(
            text(n.map_or_else(String::new, |n| n.to_string()))
                .font(Font::MONOSPACE)
                .size(TEXT_SIZE)
                .style(theme::text_muted),
        )
        .width(NUMBER_WIDTH)
        .padding([0, 6])
        .align_x(alignment::Horizontal::Right)
    };
    let marker = match line.kind {
        DiffLineKind::Context => " ",
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "-",
    };
    let mut content = line.content.clone();
    if line.missing_newline {
        content.push_str("  (no newline at end of file)");
    }

    container(
        row![
            number(line.old_line),
            number(line.new_line),
            text(marker)
                .font(Font::MONOSPACE)
                .size(TEXT_SIZE)
                .width(MARKER_WIDTH),
            text(content)
                .font(Font::MONOSPACE)
                .size(TEXT_SIZE)
                .wrapping(text::Wrapping::None),
        ]
        .align_y(iced::Center),
    )
    .height(LINE_HEIGHT)
    .width(Fill)
    .clip(true)
    .align_y(alignment::Vertical::Center)
    .style(theme::diff_line(line.kind, selected))
    .into()
}

/// Write button: enabled unless writing is blocked or busy (then with the reason).
fn write_action<'a>(
    repo: &RepoView,
    button: iced::widget::Button<'a, Message>,
    message: Message,
) -> Element<'a, Message> {
    if repo.write_block.is_some() {
        return crate::ui::panel::write_tooltip(button.into(), repo);
    }
    button
        .on_press_maybe(repo.busy.is_none().then_some(message))
        .into()
}

fn mode_button(label: &str, active: bool) -> Element<'_, Message> {
    let button = button(text(label).size(12)).padding([2, 8]);
    if active {
        button.style(theme::mode_active).into()
    } else {
        // Disabled: no `on_press`.
        button.style(theme::flat).into()
    }
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Fill).into()
}
