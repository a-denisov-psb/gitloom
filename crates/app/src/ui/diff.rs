//! Center pane: read-only file diff (replaces the graph while open).

use git_core::{DiffHunk, DiffKind, DiffLine, DiffLineKind, FileDiff};
use iced::widget::{button, column, container, row, scrollable, space, text, Column};
use iced::{alignment, Element, Fill, Font};

use crate::app::Message;
use crate::repo_view::{DiffView, Loadable, RepoView, HUNK_HEADER};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::with_tooltip;

const LINE_HEIGHT: f32 = 20.0;
const NUMBER_WIDTH: f32 = 52.0;
const MARKER_WIDTH: f32 = 16.0;
const OVERSCAN: usize = 20;
const TEXT_SIZE: f32 = 12.0;

pub fn view<'a>(_repo: &'a RepoView, diff: &'a DiffView) -> Element<'a, Message> {
    let request = &diff.request;
    let source = match &request.kind {
        DiffKind::Unstaged => "Unstaged".to_string(),
        DiffKind::Untracked => "Untracked".to_string(),
        DiffKind::Staged => "Staged".to_string(),
        DiffKind::Commit(id) => format!("commit {}", &id[..id.len().min(7)]),
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

    let header = container(
        row![
            text(title).wrapping(text::Wrapping::None),
            muted(source).size(12),
            space::horizontal(),
            modes,
            button(text("×").size(15))
                .on_press(Message::DiffClosed)
                .padding([0, 8])
                .style(theme::flat),
        ]
        .spacing(12)
        .padding([6, 12])
        .align_y(iced::Center),
    )
    .width(Fill)
    .clip(true)
    .style(theme::column_header);

    let body: Element<'a, Message> = match &diff.diff {
        Loadable::Loading => centered(muted("Loading diff…")),
        Loadable::Failed(error) => centered(text(error).color(theme::DANGER)),
        Loadable::Ready(file) => content(diff, file),
    };

    container(column![header, body])
        .width(Fill)
        .height(Fill)
        .style(theme::graph)
        .into()
}

fn content<'a>(diff: &'a DiffView, file: &'a FileDiff) -> Element<'a, Message> {
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
    for &(hunk, line) in &diff.rows[first..last] {
        let Some(hunk) = file.hunks.get(hunk as usize) else {
            continue;
        };
        rows = rows.push(if line == HUNK_HEADER {
            hunk_header(hunk)
        } else {
            match hunk.lines.get(line as usize) {
                Some(line) => diff_line(line),
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

fn hunk_header(hunk: &DiffHunk) -> Element<'_, Message> {
    container(
        text(&hunk.header)
            .font(Font::MONOSPACE)
            .size(TEXT_SIZE)
            .color(theme::TEXT_MUTED)
            .wrapping(text::Wrapping::None),
    )
    .padding([0, 12])
    .height(LINE_HEIGHT)
    .width(Fill)
    .align_y(alignment::Vertical::Center)
    .style(theme::diff_hunk_header)
    .into()
}

fn diff_line(line: &DiffLine) -> Element<'_, Message> {
    let number = |n: Option<u32>| {
        container(
            text(n.map_or_else(String::new, |n| n.to_string()))
                .font(Font::MONOSPACE)
                .size(TEXT_SIZE)
                .color(theme::TEXT_MUTED),
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
    .style(theme::diff_line(line.kind))
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
