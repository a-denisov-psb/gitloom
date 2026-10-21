//! Main toolbar: repo/branch pickers, actions, terminal.

use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Fill};

use crate::app::Message;
use crate::repo_view::RepoView;
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::{with_tooltip, write_tooltip};

/// Toolbar actions; all write operations arrive in later milestones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolbarAction {
    Undo,
    Redo,
    Pull,
    Push,
    Branch,
    Stash,
    Pop,
    Terminal,
}

impl ToolbarAction {
    fn label(self) -> &'static str {
        match self {
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Pull => "Pull",
            Self::Push => "Push",
            Self::Branch => "Branch",
            Self::Stash => "Stash",
            Self::Pop => "Pop",
            Self::Terminal => "Terminal",
        }
    }

    /// Placeholder glyphs until a proper icon set is added.
    fn glyph(self) -> &'static str {
        match self {
            Self::Undo => "↶",
            Self::Redo => "↷",
            Self::Pull => "↓",
            Self::Push => "↑",
            Self::Branch => "⑂",
            Self::Stash => "⊕",
            Self::Pop => "⊖",
            Self::Terminal => "›_",
        }
    }
}

pub fn view(view: &RepoView) -> Element<'_, Message> {
    let pickers = row![
        picker("repository", view.name().to_owned()),
        picker("branch", view.snapshot.branch_label()),
    ]
    .spacing(20);

    let actions = row![
        action(view, ToolbarAction::Undo),
        action(view, ToolbarAction::Redo),
        divider(),
        action(view, ToolbarAction::Pull),
        action(view, ToolbarAction::Push),
        divider(),
        action(view, ToolbarAction::Branch),
        action(view, ToolbarAction::Stash),
        action(view, ToolbarAction::Pop),
    ]
    .spacing(2)
    .align_y(iced::Center);

    let content = row![
        pickers,
        space::horizontal(),
        actions,
        space::horizontal(),
        action(view, ToolbarAction::Terminal),
    ]
    .padding([4, 12])
    .align_y(iced::Center);

    container(content).width(Fill).style(theme::chrome).into()
}

/// Repository / branch picker; the dropdowns follow with checkout (M3).
fn picker<'a>(caption: &'a str, value: String) -> Element<'a, Message> {
    with_tooltip(
        button(column![muted(caption).size(10), text(format!("{value} ▾"))].spacing(1))
            .padding([3, 8])
            .style(theme::picker)
            .into(),
    )
}

fn action<'a>(view: &RepoView, action: ToolbarAction) -> Element<'a, Message> {
    let content = column![text(action.glyph()).size(16), text(action.label()).size(11)]
        .spacing(1)
        .align_x(iced::Center);

    let button = button(content)
        .padding([3, 10])
        .width(64)
        .style(theme::flat)
        .into();
    if action == ToolbarAction::Terminal {
        with_tooltip(button)
    } else {
        write_tooltip(button, view)
    }
}

fn divider<'a>() -> Element<'a, Message> {
    container(space::vertical().width(1).height(28))
        .style(|_| iced::widget::container::Style {
            background: Some(theme::BORDER.into()),
            ..Default::default()
        })
        .into()
}
