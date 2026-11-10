//! Center pane: read-only file diff (replaces the graph while open).

use git_core::{DiffHunk, DiffKind, DiffLine, DiffLineKind, DiffRequest, FileDiff};
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text, Column};
use iced::{alignment, Element, Fill, Font};

use crate::app::{DiscardTarget, Message};
use crate::repo_view::{DiffRow, DiffView, Loadable, RepoView};
use crate::settings::DiffMode;
use crate::theme;
use crate::ui::muted;

use crate::diff_hscroll::{self, MARKER_WIDTH, NUMBER_WIDTH, TEXT_SIZE};

const LINE_HEIGHT: f32 = 20.0;
const OVERSCAN: usize = 20;
/// Height of the horizontal scrollbar strip below the diff (M7-16).
const HBAR_HEIGHT: f32 = 10.0;

pub fn view<'a>(repo: &'a RepoView, diff: &'a DiffView, mode: DiffMode) -> Element<'a, Message> {
    view_with(repo, diff, mode, true)
}

/// Diff inside another view (file history): no close or history button.
pub fn embedded<'a>(
    repo: &'a RepoView,
    diff: &'a DiffView,
    mode: DiffMode,
) -> Element<'a, Message> {
    view_with(repo, diff, mode, false)
}

fn view_with<'a>(
    repo: &'a RepoView,
    diff: &'a DiffView,
    mode: DiffMode,
    standalone: bool,
) -> Element<'a, Message> {
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

    // Until the Inline and Split renderers land (M7-03/M7-04), both show
    // the full-context diff with the hunk renderer.
    let mut modes = row![].spacing(2).align_y(iced::Center);
    if diff.layout.has_blocks() && matches!(diff.diff, Loadable::Ready(_)) {
        modes = modes
            .push(change_navigation(diff))
            .push(space::horizontal().width(8));
    }
    let modes = modes.extend(DiffMode::ALL.map(|m| mode_button(m, m == mode)));

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
        ]
        .push(encoding_label(diff))
        .push(space::horizontal())
        .push(selection_action)
        .push(if standalone {
            row![
                external_button(request),
                history_button(request),
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
            .spacing(12)
        } else {
            row![external_button(request), modes].spacing(12)
        })
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

/// "UTF-16 LE" / "UTF-16 BE" for a decoded UTF-16 file (M7-17).
fn encoding_label<'a>(diff: &DiffView) -> Option<Element<'a, Message>> {
    let Loadable::Ready(file) = &diff.diff else {
        return None;
    };
    file.encoding
        .map(|encoding| muted(encoding.label()).size(12).into())
}

/// "Open in external diff tool" (M5-12; Ctrl+D).
fn external_button<'a>(request: &DiffRequest) -> Element<'a, Message> {
    let enabled = !matches!(request.kind, DiffKind::Untracked | DiffKind::Stash { .. });
    let tip = match (
        enabled,
        crate::actions::ActionId::ExternalDiff.shortcut_text(),
    ) {
        (false, _) => "Not available for untracked files and stashes".to_owned(),
        (true, Some(keys)) => format!("Open in external diff tool ({keys})"),
        (true, None) => "Open in external diff tool".to_owned(),
    };
    crate::ui::panel::with_tooltip_text(
        button(text("External").size(12))
            .on_press_maybe(enabled.then_some(Message::ExternalDiff))
            .padding([2, 8])
            .style(button::secondary)
            .into(),
        tip,
    )
}

/// "History" in the diff header (M5-09): the file's history, starting at
/// the commit of a commit diff.
fn history_button<'a>(request: &DiffRequest) -> Element<'a, Message> {
    let (start, enabled) = match &request.kind {
        DiffKind::Commit(id) => (Some(id.clone()), true),
        DiffKind::Untracked | DiffKind::Stash { .. } => (None, false),
        DiffKind::Unstaged | DiffKind::Staged => (None, true),
    };
    let tip = if enabled {
        "History of this file"
    } else {
        "No history for this file here"
    };
    crate::ui::panel::with_tooltip_text(
        button(text("History").size(12))
            .on_press_maybe(enabled.then(|| Message::FileHistoryOpened {
                path: request.path.clone(),
                start,
            }))
            .padding([2, 8])
            .style(button::secondary)
            .into(),
        tip.to_owned(),
    )
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

    // The horizontal range depends on the width: decided per layout pass.
    iced::widget::responsive(move |size| {
        let max = diff_hscroll::max_offset(diff.layout, diff.text_width, size.width);
        let offset = diff.h_offset.min(max);
        let list = crate::ui::context_area::wheel_observer(
            rows(repo, diff, file, offset),
            |delta, shift| {
                diff_hscroll::wheel_to_horizontal(delta, shift, cfg!(target_os = "macos"))
                    .map(Message::DiffWheel)
            },
        );
        if max <= 0.0 {
            return list;
        }
        column![list, horizontal_bar(diff, size.width, max)].into()
    })
    .into()
}

/// Horizontal scrollbar under the code text (M7-16): its range is the
/// largest offset; it reports moves and follows wheel scrolling.
fn horizontal_bar<'a>(diff: &'a DiffView, width: f32, max: f32) -> Element<'a, Message> {
    let gutter = diff_hscroll::gutter(diff.layout);
    let bar_width = (width - gutter - diff_hscroll::SCROLLBAR_ROOM).max(1.0);
    let bar = scrollable(
        space::horizontal()
            .width(bar_width + max)
            .height(HBAR_HEIGHT),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::new(),
    ))
    .id(diff.hscroll_id.clone())
    .on_scroll(|viewport| Message::DiffHScrolled {
        offset: viewport.absolute_offset().x,
        max: viewport.content_bounds().width - viewport.bounds().width,
    })
    .width(bar_width)
    .height(HBAR_HEIGHT);
    container(bar)
        .padding(iced::Padding::default().left(gutter))
        .width(Fill)
        .into()
}

/// The visible rows of `file` with the code text moved left by `offset`.
fn rows<'a>(
    repo: &'a RepoView,
    diff: &'a DiffView,
    file: &'a FileDiff,
    offset: f32,
) -> Element<'a, Message> {
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
    let blocked = diff.line_staging_block();
    let selectable = staged.is_some() && blocked.is_none();
    for (index, &row) in diff.rows[first..last].iter().enumerate() {
        let row_index = first + index;
        let (h, l) = match row {
            DiffRow::Header(h) => {
                if let Some(hunk) = file.hunks.get(h as usize) {
                    rows = rows.push(hunk_header(repo, hunk, h as usize, staged, blocked));
                }
                continue;
            }
            DiffRow::Block(b) => {
                rows = rows.push(block_bar(repo, b as usize, staged, blocked));
                continue;
            }
            DiffRow::Line(h, l) => (h, l),
            DiffRow::Split(h, left, right) => {
                let Some(hunk) = file.hunks.get(h as usize) else {
                    continue;
                };
                // The empty side of an added/deleted file says so once.
                let first_row = diff.block_rows.first().map_or(0, |b| b + 1);
                let note = match (&file.old_path, &file.new_path) {
                    _ if row_index != first_row => None,
                    (None, Some(_)) => Some((false, "File added")),
                    (Some(_), None) => Some((true, "File deleted")),
                    _ => None,
                };
                let cell = |side: Option<u32>, right: bool| {
                    let note = note.filter(|(r, _)| *r == right).map(|(_, t)| t);
                    split_cell(
                        diff, hunk, h, side, right, row_index, selectable, note, offset,
                    )
                };
                rows = rows.push(row![
                    cell(left, false),
                    container(space::vertical().height(LINE_HEIGHT))
                        .width(1)
                        .style(theme::divider),
                    cell(right, true),
                ]);
                continue;
            }
        };
        let Some(hunk) = file.hunks.get(h as usize) else {
            continue;
        };
        rows = rows.push({
            match hunk.lines.get(l as usize) {
                Some(line) => {
                    let selected = diff.selection.contains(&(h, l));
                    let element = diff_line(line, selected, offset);
                    if selectable && line.kind != DiffLineKind::Context {
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
    blocked: Option<&'static str>,
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
                .push(line_action(
                    repo,
                    blocked,
                    discard,
                    Message::Discard(DiscardTarget::Hunk(index)),
                ))
                .push(space::horizontal().width(6));
        }
        let label = if staged { "Unstage hunk" } else { "Stage hunk" };
        let button = button(text(label).size(11))
            .padding([0, 8])
            .style(button::secondary);
        content = content.push(line_action(
            repo,
            blocked,
            button,
            Message::ApplyHunk(index),
        ));
    }
    container(content)
        .padding([0, 12])
        .height(LINE_HEIGHT)
        .width(Fill)
        .align_y(alignment::Vertical::Center)
        .style(theme::diff_hunk_header)
        .into()
}

/// Slim bar above a change block (Inline view, M7-03): the block's
/// stage/unstage/discard actions; read-only diffs show just the bar.
fn block_bar<'a>(
    repo: &'a RepoView,
    index: usize,
    staged: Option<bool>,
    blocked: Option<&'static str>,
) -> Element<'a, Message> {
    let mut content = row![space::horizontal()].align_y(iced::Center);
    if let Some(staged) = staged {
        if !staged {
            let discard = button(text("Discard block").size(11))
                .padding([0, 8])
                .style(button::danger);
            content = content
                .push(line_action(
                    repo,
                    blocked,
                    discard,
                    Message::Discard(DiscardTarget::Block(index)),
                ))
                .push(space::horizontal().width(6));
        }
        let label = if staged {
            "Unstage block"
        } else {
            "Stage block"
        };
        let button = button(text(label).size(11))
            .padding([0, 8])
            .style(button::secondary);
        content = content.push(line_action(
            repo,
            blocked,
            button,
            Message::ApplyBlock(index),
        ));
    }
    container(content)
        .padding([0, 12])
        .height(LINE_HEIGHT)
        .width(Fill)
        .align_y(alignment::Vertical::Center)
        .style(theme::diff_block_bar)
        .into()
}

/// "Previous change" / "Next change" (Inline and Split, like GitKraken's
/// arrows); disabled at the ends.
fn change_navigation<'a>(diff: &DiffView) -> Element<'a, Message> {
    let top = (diff.scroll.offset / LINE_HEIGHT).round() as usize;
    let visible = (diff.scroll.height / LINE_HEIGHT).floor() as usize;
    let arrow = |next: bool| {
        let enabled = diff.change_target(top, visible, next).is_some();
        let (icon, tip) = if next {
            (crate::icons::Icon::ChevronDown, "Next change")
        } else {
            (crate::icons::Icon::ChevronUp, "Previous change")
        };
        crate::ui::panel::with_tooltip_text(
            button(crate::icons::button_icon(icon, 14.0, enabled))
                .on_press_maybe(enabled.then_some(Message::DiffJumpChange { next }))
                .padding([2, 6])
                .style(theme::flat)
                .into(),
            tip.to_owned(),
        )
    };
    row![arrow(false), arrow(true)].spacing(2).into()
}

/// Height of one diff row (for scrolling to rows).
pub const ROW_HEIGHT: f32 = LINE_HEIGHT;

/// One half of a Split row: number, marker and content of the old (left)
/// or new (right) side; an empty cell where the other side has more lines
/// (`note` names an added or deleted file there).
#[allow(clippy::too_many_arguments)]
fn split_cell<'a>(
    diff: &DiffView,
    hunk: &'a DiffHunk,
    h: u32,
    line: Option<u32>,
    right: bool,
    row: usize,
    selectable: bool,
    note: Option<&'static str>,
    offset: f32,
) -> Element<'a, Message> {
    let Some((l, line)) = line.and_then(|l| hunk.lines.get(l as usize).map(|line| (l, line)))
    else {
        let content: Element<'a, Message> = match note {
            Some(note) => container(muted(note).size(TEXT_SIZE))
                .padding([0, 12])
                .into(),
            None => space::horizontal().into(),
        };
        return container(content)
            .height(LINE_HEIGHT)
            .width(Fill)
            .align_y(alignment::Vertical::Center)
            .style(theme::diff_empty_side)
            .into();
    };
    let selected = diff.selection.contains(&(h, l));
    let number = if right { line.new_line } else { line.old_line };
    let marker = match line.kind {
        DiffLineKind::Context => " ",
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "-",
    };
    let mut content = line.content.clone();
    if line.missing_newline {
        content.push_str(diff_hscroll::NO_NEWLINE);
    }
    let cell: Element<'a, Message> = container(
        row![
            container(
                text(number.map_or_else(String::new, |n| n.to_string()))
                    .font(Font::MONOSPACE)
                    .size(TEXT_SIZE)
                    .style(theme::text_muted),
            )
            .width(NUMBER_WIDTH)
            .padding([0, 6])
            .align_x(alignment::Horizontal::Right),
            text(marker)
                .font(Font::MONOSPACE)
                .size(TEXT_SIZE)
                .width(MARKER_WIDTH),
            code_text(content, offset),
        ]
        .align_y(iced::Center),
    )
    .height(LINE_HEIGHT)
    .width(Fill)
    .clip(true)
    .align_y(alignment::Vertical::Center)
    .style(theme::diff_line(line.kind, selected))
    .into();
    if selectable && line.kind != DiffLineKind::Context {
        mouse_area(cell)
            .on_press(Message::DiffSideClicked { row, right })
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    } else {
        cell
    }
}

fn diff_line(line: &DiffLine, selected: bool, offset: f32) -> Element<'_, Message> {
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
        content.push_str(diff_hscroll::NO_NEWLINE);
    }

    container(
        row![
            number(line.old_line),
            number(line.new_line),
            text(marker)
                .font(Font::MONOSPACE)
                .size(TEXT_SIZE)
                .width(MARKER_WIDTH),
            code_text(content, offset),
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

/// Code text of a line, moved left by `offset` and clipped to its room;
/// line numbers and markers stay in place (M7-16).
fn code_text<'a>(content: String, offset: f32) -> Element<'a, Message> {
    let text = text(content)
        .font(Font::MONOSPACE)
        .size(TEXT_SIZE)
        .wrapping(text::Wrapping::None);
    if offset <= 0.0 {
        return text.into();
    }
    iced::widget::pin(
        container(text)
            .height(LINE_HEIGHT)
            .align_y(alignment::Vertical::Center),
    )
    .x(-offset)
    .width(Fill)
    .height(LINE_HEIGHT)
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

/// Hunk, block or line action: disabled with `blocked` as tooltip for
/// UTF-16 files (M7-17), else like [`write_action`].
fn line_action<'a>(
    repo: &RepoView,
    blocked: Option<&'static str>,
    button: iced::widget::Button<'a, Message>,
    message: Message,
) -> Element<'a, Message> {
    match blocked {
        Some(reason) => crate::ui::panel::with_tooltip_text(button.into(), reason.to_owned()),
        None => write_action(repo, button, message),
    }
}

/// Hunk / Inline / Split; the active one is highlighted.
fn mode_button<'a>(mode: DiffMode, active: bool) -> Element<'a, Message> {
    let button = button(text(mode.label()).size(12))
        .padding([2, 8])
        .on_press(Message::DiffModeChanged(mode));
    let button = if active {
        button.style(theme::mode_active)
    } else {
        button.style(theme::flat)
    };
    crate::ui::panel::with_tooltip_text(button.into(), format!("{} view", mode.label()))
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Fill).into()
}
