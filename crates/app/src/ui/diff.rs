//! Center pane: read-only file diff (replaces the graph while open).

use git_core::{DiffHunk, DiffKind, DiffLine, DiffLineKind, DiffRequest, FileDiff};
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text, Column};
use iced::{alignment, Element, Fill};

use crate::app::{DiscardTarget, Message};
use crate::repo_view::{DiffRow, DiffView, Loadable, RepoView};
use crate::settings::DiffMode;
use crate::theme;
use crate::ui::muted;

use crate::diff_hscroll;
use crate::editor_style::EditorStyle;

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
    // Hunk / Inline / Split as an icon segmented toggle (M15-08).
    let modes = modes.push(
        container(row(DiffMode::ALL.map(|m| mode_button(m, m == mode))).spacing(0))
            .style(theme::segmented),
    );

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
        .push(
            (diff.style.highlight
                && diff
                    .highlight
                    .as_ref()
                    .is_some_and(|sides| sides.too_large()))
            .then(|| muted("Too large for syntax highlighting").size(12)),
        )
        .push(space::horizontal())
        .push(selection_action)
        .push(if standalone {
            // Like GitKraken (M15-08): Blame, History, change arrows, the
            // view toggle, External, Close.
            row![
                blame_button(request, diff),
                history_button(request),
                modes,
                external_button(request),
                icon_button(
                    crate::icons::Icon::Close,
                    Some(Message::DiffClosed),
                    "Close diff (Esc)".to_owned(),
                ),
            ]
            .spacing(6)
            .align_y(iced::Center)
        } else {
            row![modes, external_button(request)]
                .spacing(6)
                .align_y(iced::Center)
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

/// Icon of the diff header (M15-08): 15 px in a 22×22 hit area; disabled
/// without a message.
fn icon_button<'a>(
    icon: crate::icons::Icon,
    message: Option<Message>,
    tip: String,
) -> Element<'a, Message> {
    let enabled = message.is_some();
    crate::ui::panel::with_tooltip_text(
        button(container(crate::icons::button_icon(icon, 15.0, enabled)).center(Fill))
            .on_press_maybe(message)
            .padding(0)
            .width(22)
            .height(22)
            .style(theme::flat)
            .into(),
        tip,
    )
}

/// `name` with the shortcut of `action`, if it has one.
fn with_shortcut(name: &str, action: crate::actions::ActionId) -> String {
    match action.shortcut_text() {
        Some(keys) => format!("{name} ({keys})"),
        None => name.to_owned(),
    }
}

/// "Open in external diff tool" (M5-12; Ctrl+D).
fn external_button<'a>(request: &DiffRequest) -> Element<'a, Message> {
    let enabled = !matches!(request.kind, DiffKind::Untracked | DiffKind::Stash { .. });
    let tip = if enabled {
        with_shortcut(
            "Open in external diff tool",
            crate::actions::ActionId::ExternalDiff,
        )
    } else {
        "Not available for untracked files and stashes".to_owned()
    };
    icon_button(
        crate::icons::Icon::ExternalTool,
        enabled.then_some(Message::ExternalDiff),
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
        with_shortcut("File history", crate::actions::ActionId::FileHistory)
    } else {
        "No history for this file here".to_owned()
    };
    icon_button(
        crate::icons::Icon::History,
        enabled.then(|| Message::FileHistoryOpened {
            path: request.path.clone(),
            start,
        }),
        tip,
    )
}

/// What "Blame" in the diff header opens (M15-08), like "File blame" of
/// the file menus (M13-07): the file at the commit of a commit diff,
/// otherwise the working file; or why it is disabled. `file` is the
/// loaded diff (added and deleted files).
pub(crate) fn blame_target(
    request: &DiffRequest,
    file: Option<&FileDiff>,
) -> Result<Message, &'static str> {
    let added = file.is_some_and(|f| f.old_path.is_none() && f.new_path.is_some());
    let deleted = file.is_some_and(|f| f.new_path.is_none() && f.old_path.is_some());
    let path = request.path.clone();
    match &request.kind {
        DiffKind::Untracked => Err("Untracked files have no history"),
        DiffKind::Stash { .. } => Err("Not available for stashes"),
        DiffKind::Commit(_) if deleted => Err("Deleted in this commit"),
        DiffKind::Commit(_) if added => Err("Added in this commit"),
        DiffKind::Commit(id) => Ok(Message::FileBlameOpened {
            path,
            start: Some(id.clone()),
            rev: Some(id.clone()),
        }),
        DiffKind::Unstaged | DiffKind::Staged if deleted => Err("Deleted in the working copy"),
        DiffKind::Staged if added => Err("Not committed yet"),
        DiffKind::Unstaged | DiffKind::Staged => Ok(Message::FileBlameOpened {
            path,
            start: None,
            rev: None,
        }),
    }
}

/// "Blame" in the diff header (M15-08).
fn blame_button<'a>(request: &DiffRequest, diff: &DiffView) -> Element<'a, Message> {
    let file = match &diff.diff {
        Loadable::Ready(file) => Some(file),
        _ => None,
    };
    match blame_target(request, file) {
        Ok(message) => icon_button(
            crate::icons::Icon::Blame,
            Some(message),
            "File blame".to_owned(),
        ),
        Err(reason) => icon_button(crate::icons::Icon::Blame, None, reason.to_owned()),
    }
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
        // Word wrap (M11-07): no horizontal scrolling.
        let wrap = diff.wrap_layout(size.width);
        let max = if wrap.is_some() {
            0.0
        } else {
            diff_hscroll::max_offset(diff.layout, diff.text_width, size.width, &diff.style)
        };
        let offset = diff.h_offset.min(max);
        let list = crate::ui::context_area::wheel_observer(
            rows(repo, diff, file, offset, wrap, size.width),
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
    let gutter = diff_hscroll::gutter(diff.layout, &diff.style);
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

/// How a wrapped row is drawn (M11-07): its visual lines and the width
/// its code text wraps at.
#[derive(Debug, Clone, Copy)]
struct Wrapped {
    lines: u32,
    text_width: f32,
}

/// The visible rows of `file` with the code text moved left by `offset`,
/// or wrapped by `wrap` in a diff `width` wide.
fn rows<'a>(
    repo: &'a RepoView,
    diff: &'a DiffView,
    file: &'a FileDiff,
    offset: f32,
    wrap: Option<crate::diff_wrap::WrapLayout>,
    width: f32,
) -> Element<'a, Message> {
    // Only visible lines are built; spacers keep the scrollbar exact.
    let style = &diff.style;
    let line_height = style.line_height();
    let total = diff.rows.len();
    let (first, last) = match &wrap {
        Some(layout) => layout.visible(
            diff.scroll.offset,
            diff.scroll.height,
            line_height,
            OVERSCAN,
        ),
        None => crate::ui::rows_in_view(
            diff.scroll.offset,
            diff.scroll.height,
            total,
            line_height,
            OVERSCAN,
        ),
    };
    // Visual lines above and below the built rows.
    let (above, below) = match &wrap {
        Some(layout) => (
            layout.start(first),
            layout.total().saturating_sub(layout.start(last)),
        ),
        None => (first as u32, (total - last) as u32),
    };
    // Code text wraps at whole characters of the monospace font.
    let wrap_width = wrap.as_ref().map(|_| {
        let area = diff_hscroll::text_area(diff.layout, width, style);
        let columns = crate::diff_wrap::columns(area, diff.char_width);
        columns as f32 * diff.char_width + 0.5
    });
    let wrapped = |row: usize| -> Option<Wrapped> {
        Some(Wrapped {
            lines: wrap.as_ref()?.lines(row),
            text_width: wrap_width?,
        })
    };
    let mut rows = Column::with_capacity(last - first + 2)
        .push(space::vertical().height(above as f32 * line_height));
    let staged = diff.staged();
    let blocked = diff.line_staging_block();
    let selectable = staged.is_some() && blocked.is_none();
    for (index, &row) in diff.rows[first..last].iter().enumerate() {
        let row_index = first + index;
        let (h, l) = match row {
            DiffRow::Header(h) => {
                if let Some(hunk) = file.hunks.get(h as usize) {
                    rows = rows.push(hunk_header(repo, hunk, h as usize, staged, blocked, style));
                }
                continue;
            }
            DiffRow::Block(b) => {
                rows = rows.push(block_bar(repo, b as usize, staged, blocked, line_height));
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
                        diff,
                        hunk,
                        h,
                        side,
                        right,
                        row_index,
                        selectable,
                        note,
                        offset,
                        wrapped(row_index),
                    )
                };
                let height = wrapped(row_index).map_or(1, |w| w.lines) as f32 * line_height;
                rows = rows.push(row![
                    cell(left, false),
                    container(space::vertical().height(height))
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
                    let element = diff_line(diff, line, selected, offset, wrapped(row_index));
                    if selectable && line.kind != DiffLineKind::Context {
                        mouse_area(element)
                            .on_press(Message::DiffLineClicked(row_index))
                            .interaction(iced::mouse::Interaction::Pointer)
                            .into()
                    } else {
                        element
                    }
                }
                None => space::vertical().height(line_height).into(),
            }
        });
    }
    rows = rows.push(space::vertical().height(below as f32 * line_height));

    scrollable(rows)
        .id(diff.scroll_id.clone())
        .on_scroll(|viewport| Message::DiffScrolled {
            offset: viewport.absolute_offset().y,
            height: viewport.bounds().height,
            width: viewport.bounds().width,
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
    style: &EditorStyle,
) -> Element<'a, Message> {
    let mut content = row![text(&hunk.header)
        .font(style.font)
        .size(style.size)
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
        .height(style.line_height())
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
    line_height: f32,
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
        .height(line_height)
        .width(Fill)
        .align_y(alignment::Vertical::Center)
        .style(theme::diff_block_bar)
        .into()
}

/// "Previous change" / "Next change" (Inline and Split, like GitKraken's
/// arrows); disabled at the ends.
fn change_navigation<'a>(diff: &DiffView) -> Element<'a, Message> {
    let height = diff.style.line_height();
    let top = (diff.scroll.offset / height).round() as usize;
    let visible = (diff.scroll.height / height).floor() as usize;
    let arrow = |next: bool| {
        let enabled = diff.change_target(top, visible, next).is_some();
        let (icon, tip) = if next {
            (crate::icons::Icon::ChevronDown, "Next change")
        } else {
            (crate::icons::Icon::ChevronUp, "Previous change")
        };
        icon_button(
            icon,
            enabled.then_some(Message::DiffJumpChange { next }),
            tip.to_owned(),
        )
    };
    row![arrow(false), arrow(true)].spacing(2).into()
}

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
    wrapped: Option<Wrapped>,
) -> Element<'a, Message> {
    let style = &diff.style;
    let height = wrapped.map_or(1, |w| w.lines) as f32 * style.line_height();
    let Some((l, line)) = line.and_then(|l| hunk.lines.get(l as usize).map(|line| (l, line)))
    else {
        let content: Element<'a, Message> = match note {
            Some(note) => container(muted(note).size(style.size))
                .padding([0, 12])
                .into(),
            None => space::horizontal().into(),
        };
        return container(content)
            .height(height)
            .width(Fill)
            .align_y(if wrapped.is_some() {
                alignment::Vertical::Top
            } else {
                alignment::Vertical::Center
            })
            .style(theme::diff_empty_side)
            .into();
    };
    let selected = diff.selection.contains(&(h, l));
    let number = if right { line.new_line } else { line.old_line };
    let mut cells = row![].align_y(cells_align(wrapped));
    if style.numbers {
        cells = cells.push(line_number(number, style));
    }
    let cell: Element<'a, Message> = container(
        cells
            .push(marker(line.kind, style))
            .push(line_text(diff, line, offset, wrapped)),
    )
    .height(height)
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

/// Wrapped rows keep numbers and markers on their first visual line.
fn cells_align(wrapped: Option<Wrapped>) -> alignment::Vertical {
    if wrapped.is_some() {
        alignment::Vertical::Top
    } else {
        alignment::Vertical::Center
    }
}

/// A line number cell (right-aligned in its column).
fn line_number<'a>(number: Option<u32>, style: &EditorStyle) -> Element<'a, Message> {
    container(
        text(number.map_or_else(String::new, |n| n.to_string()))
            .font(style.font)
            .size(style.size)
            .style(theme::text_muted),
    )
    .width(style.number_width())
    .height(style.line_height())
    .padding([0, 6])
    .align_x(alignment::Horizontal::Right)
    .align_y(alignment::Vertical::Center)
    .into()
}

/// The `+`/`-` marker of a line.
fn marker<'a>(kind: DiffLineKind, style: &EditorStyle) -> Element<'a, Message> {
    let marker = match kind {
        DiffLineKind::Context => " ",
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "-",
    };
    text(marker)
        .font(style.font)
        .size(style.size)
        .width(style.marker_width())
        .height(style.line_height())
        .align_y(alignment::Vertical::Center)
        .into()
}

/// The text of a line as shown: tabs expanded, with the no-newline note.
fn shown_content(line: &DiffLine, style: &EditorStyle) -> String {
    let mut content = style.expand(&line.content).into_owned();
    if line.missing_newline {
        content.push_str(diff_hscroll::NO_NEWLINE);
    }
    content
}

fn diff_line<'a>(
    diff: &DiffView,
    line: &DiffLine,
    selected: bool,
    offset: f32,
    wrapped: Option<Wrapped>,
) -> Element<'a, Message> {
    let style = &diff.style;
    let mut cells = row![].align_y(cells_align(wrapped));
    if style.numbers {
        cells = cells
            .push(line_number(line.old_line, style))
            .push(line_number(line.new_line, style));
    }
    container(
        cells
            .push(marker(line.kind, style))
            .push(line_text(diff, line, offset, wrapped)),
    )
    .height(wrapped.map_or(1, |w| w.lines) as f32 * style.line_height())
    .width(Fill)
    .clip(true)
    .align_y(alignment::Vertical::Center)
    .style(theme::diff_line(line.kind, selected))
    .into()
}

/// The code of a diff line: highlighted when the result is there
/// (M11-09), else plain.
fn line_text<'a>(
    diff: &DiffView,
    line: &DiffLine,
    offset: f32,
    wrapped: Option<Wrapped>,
) -> Element<'a, Message> {
    let style = &diff.style;
    match diff.spans_for(line) {
        Some(spans) => {
            let mut pieces = crate::highlight::segments(&line.content, spans, style.tab);
            if line.missing_newline {
                pieces.push((diff_hscroll::NO_NEWLINE.to_owned(), None));
            }
            rich_code(pieces, offset, style, wrapped)
        }
        None => code_text(shown_content(line, style), offset, style, wrapped),
    }
}

/// An iced text span of a highlighted piece.
pub fn code_span<'a>(
    text: String,
    piece: Option<crate::highlight::SpanStyle>,
    style: &EditorStyle,
) -> iced::widget::text::Span<'a> {
    let span = iced::widget::span(text);
    let Some(piece) = piece else {
        return span;
    };
    let [r, g, b, a] = piece.color;
    span.color(iced::Color::from_rgba8(r, g, b, f32::from(a) / 255.0))
        .font(iced::Font {
            weight: if piece.bold {
                iced::font::Weight::Bold
            } else {
                style.font.weight
            },
            style: if piece.italic {
                iced::font::Style::Italic
            } else {
                style.font.style
            },
            ..style.font
        })
}

/// Highlighted code text, placed like [`code_text`].
fn rich_code<'a>(
    pieces: Vec<(String, Option<crate::highlight::SpanStyle>)>,
    offset: f32,
    style: &EditorStyle,
    wrapped: Option<Wrapped>,
) -> Element<'a, Message> {
    let spans: Vec<iced::widget::text::Span<'a>> = pieces
        .into_iter()
        .map(|(text, piece)| code_span(text, piece, style))
        .collect();
    let rich = iced::widget::rich_text(spans)
        .font(style.font)
        .size(style.size);
    if let Some(wrapped) = wrapped {
        return rich
            .line_height(text::LineHeight::Absolute(style.line_height().into()))
            .wrapping(text::Wrapping::Glyph)
            .width(wrapped.text_width)
            .into();
    }
    let rich = rich.wrapping(text::Wrapping::None);
    if offset <= 0.0 {
        return rich.into();
    }
    iced::widget::pin(
        container(rich)
            .height(style.line_height())
            .align_y(alignment::Vertical::Center),
    )
    .x(-offset)
    .width(Fill)
    .height(style.line_height())
    .into()
}

/// Code text of a line, moved left by `offset` and clipped to its room;
/// line numbers and markers stay in place (M7-16).
fn code_text<'a>(
    content: String,
    offset: f32,
    style: &EditorStyle,
    wrapped: Option<Wrapped>,
) -> Element<'a, Message> {
    if let Some(wrapped) = wrapped {
        // Breaks at any character so the line count matches the model
        // (whole characters per visual line, M11-07).
        return text(content)
            .font(style.font)
            .size(style.size)
            .line_height(text::LineHeight::Absolute(style.line_height().into()))
            .wrapping(text::Wrapping::Glyph)
            .width(wrapped.text_width)
            .into();
    }
    let text = text(content)
        .font(style.font)
        .size(style.size)
        .wrapping(text::Wrapping::None);
    if offset <= 0.0 {
        return text.into();
    }
    iced::widget::pin(
        container(text)
            .height(style.line_height())
            .align_y(alignment::Vertical::Center),
    )
    .x(-offset)
    .width(Fill)
    .height(style.line_height())
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
    let icon = match mode {
        DiffMode::Hunk => crate::icons::Icon::ListCollapse,
        DiffMode::Inline => crate::icons::Icon::AlignJustify,
        DiffMode::Split => crate::icons::Icon::Columns2,
    };
    let button = button(container(crate::icons::icon(icon, 15.0)).center(Fill))
        .padding(0)
        .width(22)
        .height(22)
        .on_press(Message::DiffModeChanged(mode));
    let button = if active {
        button.style(theme::mode_active)
    } else {
        button.style(theme::flat)
    };
    crate::ui::panel::with_tooltip_text(
        button.into(),
        with_shortcut(
            &format!("{} view", mode.label()),
            crate::actions::ActionId::DiffView(mode),
        ),
    )
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Fill).into()
}

#[cfg(test)]
mod header_tests {
    use super::*;

    fn request(kind: DiffKind) -> DiffRequest {
        DiffRequest {
            kind,
            path: "src/a.rs".into(),
            old_path: None,
        }
    }

    fn file(old: Option<&str>, new: Option<&str>) -> FileDiff {
        FileDiff {
            old_path: old.map(str::to_owned),
            new_path: new.map(str::to_owned),
            binary: false,
            too_large: false,
            encoding: None,
            hunks: Vec::new(),
        }
    }

    /// M15-08: Blame like "File blame" of the file menus; disabled with
    /// the reason for untracked, stash and deleted files.
    #[test]
    fn blame_targets_and_reasons() {
        let modified = file(Some("src/a.rs"), Some("src/a.rs"));
        let added = file(None, Some("src/a.rs"));
        let deleted = file(Some("src/a.rs"), None);
        let reason = |kind: DiffKind, f: &FileDiff| blame_target(&request(kind), Some(f)).err();
        assert_eq!(
            reason(DiffKind::Untracked, &added),
            Some("Untracked files have no history")
        );
        let stash = DiffKind::Stash {
            id: "s".into(),
            part: git_core::StashPart::Unstaged,
        };
        assert_eq!(reason(stash, &modified), Some("Not available for stashes"));
        assert_eq!(
            reason(DiffKind::Commit("c".into()), &deleted),
            Some("Deleted in this commit")
        );
        assert_eq!(
            reason(DiffKind::Unstaged, &deleted),
            Some("Deleted in the working copy")
        );
        assert_eq!(
            reason(DiffKind::Commit("c".into()), &added),
            Some("Added in this commit")
        );
        assert_eq!(reason(DiffKind::Staged, &added), Some("Not committed yet"));
        assert!(matches!(
            blame_target(&request(DiffKind::Commit("c".into())), Some(&modified)),
            Ok(Message::FileBlameOpened { start: Some(s), rev: Some(r), .. }) if s == "c" && r == "c"
        ));
        for kind in [DiffKind::Unstaged, DiffKind::Staged] {
            assert!(matches!(
                blame_target(&request(kind), Some(&modified)),
                Ok(Message::FileBlameOpened {
                    start: None,
                    rev: None,
                    ..
                })
            ));
        }
    }
}
