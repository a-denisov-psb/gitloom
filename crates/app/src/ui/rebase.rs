//! Center pane: interactive rebase panel (M4-07), replacing the graph. One
//! row per commit, newest on top, with its action, inline message editors
//! for rewords and squash chains, drag or Alt+↑/↓ to reorder.

use git_core::TodoAction;
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, space, text, text_editor, Column,
};
use iced::{Element, Fill, Font};

use crate::app::Message;
use crate::rebase_panel::RebasePanel;
use crate::repo_view::{Loadable, RepoView};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::{with_tooltip_text, write_tooltip};

pub fn view<'a>(repo: &'a RepoView, panel: &'a RebasePanel) -> Element<'a, Message> {
    let header = row![
        column![
            text(format!("Interactive rebase onto {}", panel.base_label)).size(14),
            muted(
                "Newest on top · P pick · R reword · S squash · F fixup · D drop · \
                 drag or Alt+↑/↓ to reorder"
            )
            .size(11),
        ]
        .spacing(2),
        space::horizontal(),
        button(text("Cancel").size(12))
            .padding([3, 10])
            .style(button::secondary)
            .on_press(Message::DiffClosed),
        start_button(repo, panel),
    ]
    .spacing(8)
    .align_y(iced::Center);

    let body: Element<'a, Message> = match &panel.loaded {
        Loadable::Loading => muted("Loading commits…").into(),
        Loadable::Failed(error) => text(error).color(theme::DANGER).into(),
        Loadable::Ready(()) => {
            let mut rows = Column::new().spacing(2);
            for index in panel.display_order() {
                rows = rows.push(entry_row(panel, index));
            }
            // Releasing the mouse anywhere over the list ends a drag.
            mouse_area(scrollable(rows).height(Fill).width(Fill))
                .on_release(Message::RebaseDragEnd)
                .into()
        }
    };

    let mut content = column![header].spacing(10).padding(12);
    if let (Loadable::Ready(()), Some(problem)) = (&panel.loaded, panel.problem()) {
        content = content.push(text(problem).size(12).color(theme::WARNING));
    }
    content.push(body).into()
}

fn start_button<'a>(repo: &RepoView, panel: &RebasePanel) -> Element<'a, Message> {
    let start = button(text("Start rebase").size(12))
        .padding([3, 10])
        .style(button::primary);
    if repo.write_block.is_some() {
        return write_tooltip(start.into(), repo);
    }
    match panel.problem() {
        Some(problem) => with_tooltip_text(start.into(), problem),
        None => start
            .on_press_maybe(repo.busy.is_none().then_some(Message::RebaseStart))
            .into(),
    }
}

fn entry_row(panel: &RebasePanel, index: usize) -> Element<'_, Message> {
    let entry = &panel.entries[index];
    let selected = panel.selected == index;
    let summary = entry.message.lines().next().unwrap_or_default();

    let mut actions = row![].spacing(2);
    for action in TodoAction::ALL {
        let active = entry.action == action;
        actions = actions.push(
            button(text(action.label()).size(11))
                .padding([2, 6])
                // Primary stays visible on the selected row's background.
                .style(if active {
                    button::primary
                } else {
                    button::text
                })
                .on_press(Message::RebaseSetAction { index, action }),
        );
    }

    let dropped = entry.action == TodoAction::Drop;
    let melded = matches!(entry.action, TodoAction::Squash | TodoAction::Fixup);
    let mut label = row![
        text("⋮⋮").color(theme::TEXT_MUTED),
        text(&entry.id[..entry.id.len().min(7)])
            .font(Font::MONOSPACE)
            .size(12)
            .color(theme::TEXT_MUTED),
        text(summary).size(13).color(if dropped {
            theme::TEXT_MUTED
        } else {
            theme::TEXT
        }),
    ]
    .spacing(10)
    .align_y(iced::Center);
    if melded {
        label = label.push(muted("↓ into the commit below").size(11));
    }
    // The label part starts a drag; the action buttons stay clickable.
    let handle = mouse_area(container(label).width(Fill).padding([4, 0]))
        .on_press(Message::RebaseDragStart(index))
        .on_enter(Message::RebaseDragOver(index))
        .interaction(iced::mouse::Interaction::Grab);

    let mut cell = column![row![actions, handle].spacing(12).align_y(iced::Center)].spacing(4);
    if let Some(editor) = panel.message_editor(index) {
        let id = entry.id.clone();
        cell = cell.push(
            text_editor(editor)
                .on_action(move |action| Message::RebaseMessageEdited {
                    id: id.clone(),
                    action,
                })
                .size(12)
                .height(80),
        );
    }
    container(cell)
        .padding([2, 8])
        .width(Fill)
        .style(move |theme| {
            if selected {
                container::Style::default().background(theme::BG_SELECTED)
            } else {
                theme::panel(theme)
            }
        })
        .into()
}
