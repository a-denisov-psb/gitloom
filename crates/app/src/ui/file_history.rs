//! File history view (M5-09): commits of one file on the left, the selected
//! commit's diff of that file on the right.

use iced::widget::{button, column, container, row, scrollable, space, text, Column};
use iced::{Element, Fill, Length};

use crate::app::Message;
use crate::file_history::FileHistoryView;
use crate::format;
use crate::icons::{self, Icon};
use crate::repo_view::{Loadable, RepoView};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::with_tooltip_text;

const LIST_WIDTH: f32 = 420.0;

pub fn view<'a>(repo: &'a RepoView, history: &'a FileHistoryView) -> Element<'a, Message> {
    let has_selection = history.selected_entry().is_some();
    let header = container(
        row![
            icons::tinted(Icon::History, 14.0, |p| p.text_muted),
            text("History of").size(13).style(theme::text_muted),
            text(&history.path).wrapping(text::Wrapping::None),
            space::horizontal(),
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
                crate::ui::context_area::double_click_area(item, Message::FileHistoryShowInGraph)
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

fn centered<'a>(label: &'a str) -> Element<'a, Message> {
    container(muted(label)).center(Fill).into()
}
