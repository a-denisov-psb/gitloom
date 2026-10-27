//! File history view (M5-09): commits of one file on the left, the selected
//! commit's diff of that file on the right.

use iced::widget::{button, column, container, row, scrollable, space, text, Column};
use iced::{Element, Fill, Length};

use crate::app::Message;
use crate::file_history::{BlameView, FileHistoryView, HistoryMode};
use crate::format;
use crate::icons::{self, Icon};
use crate::repo_view::{Loadable, RepoView};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::with_tooltip_text;

const LIST_WIDTH: f32 = 420.0;
/// Blame rows have a fixed height so only the visible ones are built.
const BLAME_ROW: f32 = 18.0;
const BLAME_OVERSCAN: usize = 20;
const GUTTER_WIDTH: f32 = 300.0;

pub fn view<'a>(repo: &'a RepoView, history: &'a FileHistoryView) -> Element<'a, Message> {
    let has_selection = history.selected_entry().is_some();
    let header = container(
        row![
            icons::tinted(Icon::History, 14.0, |p| p.text_muted),
            text("History of").size(13).style(theme::text_muted),
            text(&history.path).wrapping(text::Wrapping::None),
            space::horizontal(),
            mode_toggle(history.mode),
            with_tooltip_text(
                button(text("Show in graph").size(12))
                    .on_press_maybe(has_selection.then_some(Message::FileHistoryShowInGraph))
                    .padding([3, 10])
                    .style(button::secondary)
                    .into(),
                "Close the history and select this commit (double click)".to_owned(),
            ),
            with_tooltip_text(
                button(icons::icon(Icon::Close, 15.0))
                    .on_press(Message::Shortcut(crate::shortcuts::Shortcut::Close))
                    .padding([2, 6])
                    .style(theme::flat)
                    .into(),
                "Close history (Esc)".to_owned(),
            ),
        ]
        .spacing(8)
        .padding([6, 12])
        .align_y(iced::Center),
    )
    .width(Fill)
    .clip(true)
    .style(theme::column_header);

    let list: Element<'a, Message> = match &history.entries {
        Loadable::Loading => centered("Loading history…"),
        Loadable::Failed(error) => container(text(error).style(theme::text_danger))
            .padding(12)
            .into(),
        Loadable::Ready(entries) if entries.is_empty() => centered("No commits changed this file"),
        Loadable::Ready(entries) => {
            let rows = Column::with_children(entries.iter().enumerate().map(|(index, entry)| {
                let mut line = row![
                    container(text(format::initials(&entry.author)).size(10))
                        .width(24)
                        .height(24)
                        .center(24)
                        .style(theme::avatar),
                    column![
                        text(&entry.summary).size(13).wrapping(text::Wrapping::None),
                        muted(format!(
                            "{} · {}",
                            entry.author,
                            format::date_time(entry.time)
                        ))
                        .size(11)
                        .wrapping(text::Wrapping::None),
                    ]
                    .spacing(1)
                    .width(Fill),
                ]
                .spacing(8)
                .align_y(iced::Center);
                if let Some(old) = &entry.old_path {
                    line = line.push(with_tooltip_text(
                        icons::tinted(Icon::FileRenamed, 14.0, |p| p.warning).into(),
                        format!("Renamed from {old}"),
                    ));
                }
                let item: Element<'a, Message> = button(line)
                    .on_press(Message::FileHistorySelected(index))
                    .padding([4, 10])
                    .width(Fill)
                    .clip(true)
                    .style(theme::list_item(history.selected == Some(index)))
                    .into();
                let item = crate::ui::context_area::double_click_area(
                    item,
                    Message::FileHistoryShowInGraph,
                );
                crate::ui::context_area::context_area(item, move |position| {
                    Message::FileHistoryEntryMenu { index, position }
                })
            }));
            scrollable(rows)
                .id(history.list_scroll_id.clone())
                .height(Fill)
                .into()
        }
    };

    let diff: Element<'a, Message> = match &history.diff {
        Some(diff) => crate::ui::diff::embedded(repo, diff),
        None => centered(""),
    };

    if history.mode == HistoryMode::Blame {
        let body = match &history.blame {
            Some(blame) => blame_view(blame),
            None => centered("Loading blame…"),
        };
        return column![header, body].spacing(1).into();
    }

    column![
        header,
        row![
            container(list)
                .width(Length::Fixed(LIST_WIDTH))
                .height(Fill)
                .style(theme::panel),
            container(diff).width(Fill).height(Fill),
        ]
        .spacing(1)
        .height(Fill),
    ]
    .spacing(1)
    .into()
}

fn mode_toggle<'a>(mode: HistoryMode) -> Element<'a, Message> {
    let segment = |label: &'a str, value: HistoryMode| {
        button(text(label).size(12))
            .on_press(Message::HistoryModeChosen(value))
            .padding([3, 10])
            .style(if mode == value {
                theme::mode_active
            } else {
                theme::flat
            })
    };
    row![
        segment("History", HistoryMode::History),
        segment("Blame", HistoryMode::Blame)
    ]
    .spacing(2)
    .into()
}

/// Blame of the file (M5-10): per commit group a gutter with short SHA,
/// author and age (alternating shading, full summary as tooltip; a click
/// selects the commit in the history), then line number and content.
/// Only the visible rows are built.
fn blame_view(view: &BlameView) -> Element<'_, Message> {
    let blame = match &view.blame {
        Loadable::Loading => return centered("Loading blame…"),
        Loadable::Failed(error) => {
            return container(text(error).style(theme::text_danger))
                .padding(12)
                .into()
        }
        Loadable::Ready(blame) => blame,
    };
    if blame.binary {
        return centered("Binary file: no blame");
    }
    if blame.lines.is_empty() {
        return centered("Empty file");
    }
    let target = match &view.rev {
        Some(rev) => format!("Blame at commit {}", &rev[..rev.len().min(7)]),
        None => "Blame of the working copy".to_owned(),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let total = blame.lines.len();
    let (first, last) = crate::ui::rows_in_view(
        view.scroll.offset,
        view.scroll.height,
        total,
        BLAME_ROW,
        BLAME_OVERSCAN,
    );
    let number_width = (total.to_string().len() as f32) * 8.0 + 12.0;
    let mut rows = Column::with_capacity(last - first + 2)
        .push(space::vertical().height(first as f32 * BLAME_ROW));
    for (line_index, (commit_index, content)) in
        blame.lines.iter().enumerate().take(last).skip(first)
    {
        let group = blame
            .groups
            .partition_point(|g| g.start <= line_index)
            .saturating_sub(1);
        // The first fully visible row of a group repeats its label.
        let first_visible = (view.scroll.offset.max(0.0) / BLAME_ROW).ceil() as usize;
        let starts_group = line_index == first_visible
            || blame
                .groups
                .get(group)
                .is_some_and(|g| g.start == line_index);
        let commit = &blame.commits[*commit_index];
        let gutter: Element<'_, Message> = if starts_group {
            let label = if commit.is_uncommitted() {
                row![muted("Not committed yet").size(11)]
            } else {
                row![
                    text(&commit.id[..commit.id.len().min(7)])
                        .size(11)
                        .font(iced::Font::MONOSPACE),
                    text(&commit.author).size(11).wrapping(text::Wrapping::None),
                    space::horizontal(),
                    muted(format::relative(commit.time, now)).size(11),
                ]
                .spacing(8)
            };
            let cell = button(label.align_y(iced::Center))
                .on_press_maybe(
                    (!commit.is_uncommitted())
                        .then(|| Message::BlameCommitClicked(commit.id.clone())),
                )
                .padding([0, 8])
                .height(BLAME_ROW)
                .width(Fill)
                .style(theme::flat);
            if commit.is_uncommitted() {
                cell.into()
            } else {
                with_tooltip_text(cell.into(), commit.summary.clone())
            }
        } else {
            space::horizontal().into()
        };
        let line = row![
            container(gutter).width(GUTTER_WIDTH).height(BLAME_ROW),
            text((line_index + 1).to_string())
                .size(11)
                .font(iced::Font::MONOSPACE)
                .style(theme::text_muted)
                .width(number_width)
                .align_x(iced::Right),
            text(content.replace('\t', "    "))
                .size(12)
                .font(iced::Font::MONOSPACE)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(10)
        .height(BLAME_ROW)
        .align_y(iced::Center)
        .clip(true);
        rows = rows.push(
            container(line)
                .width(Fill)
                .style(theme::blame_band(group % 2 == 1)),
        );
    }
    rows = rows.push(space::vertical().height((total - last) as f32 * BLAME_ROW));
    column![
        container(muted(target).size(11)).padding([4, 12]),
        scrollable(rows)
            .id(view.scroll_id.clone())
            .on_scroll(|viewport| Message::BlameScrolled {
                offset: viewport.absolute_offset().y,
                height: viewport.bounds().height,
            })
            .height(Fill),
    ]
    .into()
}

fn centered<'a>(label: &'a str) -> Element<'a, Message> {
    container(muted(label)).center(Fill).into()
}
