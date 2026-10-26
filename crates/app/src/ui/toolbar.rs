//! Main toolbar: repo/branch pickers, actions, terminal.

use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Fill};

use crate::app::Message;
use crate::icons::{self, Icon};
use crate::repo_view::{Guarded, RepoView};
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::{with_tooltip, with_tooltip_text, write_tooltip};

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

    fn icon(self) -> Icon {
        match self {
            Self::Undo => Icon::Undo,
            Self::Redo => Icon::Redo,
            Self::Pull => Icon::Pull,
            Self::Push => Icon::Push,
            Self::Branch => Icon::Branch,
            Self::Stash => Icon::Stash,
            Self::Pop => Icon::Pop,
            Self::Terminal => Icon::Terminal,
        }
    }
}

/// Toolbar button: icon above its label; without `on_press` it is disabled
/// and the icon is dimmed like the label.
fn tool_button<'a>(
    icon: Icon,
    label: &'a str,
    on_press: Option<Message>,
) -> iced::widget::Button<'a, Message> {
    let enabled = on_press.is_some();
    button(
        column![
            icons::button_icon(icon, 18.0, enabled),
            text(label).size(11)
        ]
        .spacing(2)
        .align_x(iced::Center),
    )
    .on_press_maybe(on_press)
    .padding([3, 10])
    .width(64)
    .style(theme::flat)
}

fn toolbar_button<'a>(
    action: ToolbarAction,
    on_press: Option<Message>,
) -> iced::widget::Button<'a, Message> {
    tool_button(action.icon(), action.label(), on_press)
}

pub fn view(view: &RepoView, pull_mode: git_core::PullMode) -> Element<'_, Message> {
    let pickers = row![
        picker("repository", view.name().to_owned()),
        picker("branch", view.snapshot.branch_label()),
    ]
    .spacing(20);

    let actions = row![
        action(view, ToolbarAction::Undo),
        action(view, ToolbarAction::Redo),
        divider(),
        row![pull_button(view, pull_mode), pull_dropdown()].align_y(iced::Center),
        push_button(view),
        divider(),
        branch_button(view),
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
        settings_button(),
    ]
    .padding([4, 12])
    .align_y(iced::Center);

    container(content).width(Fill).style(theme::chrome).into()
}

/// Gear button: opens the settings view (Ctrl+,).
fn settings_button<'a>() -> Element<'a, Message> {
    let tip = match crate::actions::ActionId::Settings.shortcut_text() {
        Some(keys) => format!("Preferences ({keys})"),
        None => "Preferences".to_owned(),
    };
    with_tooltip_text(
        tool_button(Icon::Settings, "Settings", Some(Message::SettingsOpened)).into(),
        tip,
    )
}

/// Repository / branch picker; the dropdowns follow with checkout (M3).
fn picker<'a>(caption: &'a str, value: String) -> Element<'a, Message> {
    with_tooltip(
        button(
            column![
                muted(caption).size(10),
                row![
                    text(value),
                    icons::tinted(Icon::ChevronDown, 12.0, |p| p.text_muted)
                ]
                .spacing(4)
                .align_y(iced::Center)
            ]
            .spacing(1),
        )
        .padding([3, 8])
        .style(theme::picker)
        .into(),
    )
}

fn action<'a>(view: &RepoView, action: ToolbarAction) -> Element<'a, Message> {
    let disabled = || -> Element<'a, Message> { toolbar_button(action, None).into() };
    let writable = view.write_block.is_none() && view.busy.is_none();
    // Undo/Redo name the action in their tooltip.
    if matches!(action, ToolbarAction::Undo | ToolbarAction::Redo) {
        let (stack, message, verb) = if action == ToolbarAction::Undo {
            (&view.undo, Message::Undo, "Undo")
        } else {
            (&view.redo, Message::Redo, "Redo")
        };
        if view.write_block.is_some() {
            return write_tooltip(disabled(), view);
        }
        if let Some(reason) = stack
            .last()
            .filter(|a| matches!(a, crate::undo::UndoAction::Commit { .. }))
            .and_then(|_| view.state_block(Guarded::MoveHead))
        {
            return with_tooltip_text(disabled(), reason);
        }
        return match stack.last() {
            Some(last) => with_tooltip_text(
                toolbar_button(action, view.busy.is_none().then_some(message)).into(),
                format!("{verb}: {}", last.label()),
            ),
            None => with_tooltip_text(disabled(), format!("Nothing to {}", verb.to_lowercase())),
        };
    }
    let message = match action {
        ToolbarAction::Stash if !view.snapshot.working_tree.is_empty() => Some(Message::StashPush),
        ToolbarAction::Pop => view
            .snapshot
            .stashes
            .first()
            .map(|s| Message::StashPop(s.id.clone())),
        _ => None,
    };
    if let Some(reason) = view
        .state_block(Guarded::Stash)
        .filter(|_| matches!(action, ToolbarAction::Stash | ToolbarAction::Pop))
        .filter(|_| view.write_block.is_none())
    {
        return with_tooltip_text(disabled(), reason);
    }
    match (action, message) {
        (ToolbarAction::Stash | ToolbarAction::Pop, Some(message)) if writable => {
            toolbar_button(action, Some(message)).into()
        }
        // Nothing to stash or pop: plain disabled button.
        (ToolbarAction::Stash | ToolbarAction::Pop, _) if view.write_block.is_none() => disabled(),
        (ToolbarAction::Terminal, _) => with_tooltip(disabled()),
        _ => write_tooltip(disabled(), view),
    }
}

/// Pull in the current mode; the tooltip names the mode.
fn pull_button<'a>(view: &RepoView, mode: git_core::PullMode) -> Element<'a, Message> {
    if view.write_block.is_some() {
        return write_tooltip(toolbar_button(ToolbarAction::Pull, None).into(), view);
    }
    with_tooltip_text(
        toolbar_button(
            ToolbarAction::Pull,
            view.busy.is_none().then_some(Message::Pull(None)),
        )
        .into(),
        mode.label().to_owned(),
    )
}

/// New branch at the selected commit (inline input, Ctrl+B).
fn branch_button<'a>(view: &RepoView) -> Element<'a, Message> {
    if view.write_block.is_some() {
        return write_tooltip(toolbar_button(ToolbarAction::Branch, None).into(), view);
    }
    with_tooltip_text(
        toolbar_button(
            ToolbarAction::Branch,
            view.busy.is_none().then_some(Message::NewBranchStart(None)),
        )
        .into(),
        match crate::actions::ActionId::CreateBranch.shortcut_text() {
            Some(keys) => format!("Create a branch at the selected commit ({keys})"),
            None => "Create a branch at the selected commit".to_owned(),
        },
    )
}

/// Push the current branch; the tooltip names the target.
fn push_button<'a>(view: &RepoView) -> Element<'a, Message> {
    if view.write_block.is_some() {
        return write_tooltip(toolbar_button(ToolbarAction::Push, None).into(), view);
    }
    let tooltip = match view.snapshot.head_upstream() {
        Some((local, Some((remote, branch)))) => format!("Push {local} to {remote}/{branch}"),
        Some((local, None)) => format!("Push {local} and set its upstream"),
        None => "Check out a branch to push".to_owned(),
    };
    with_tooltip_text(
        toolbar_button(
            ToolbarAction::Push,
            view.busy
                .is_none()
                .then_some(Message::Push { force: false }),
        )
        .into(),
        tooltip,
    )
}

/// Arrow next to Pull that opens the Pull menu (Fetch all, pull modes).
fn pull_dropdown<'a>() -> Element<'a, Message> {
    let arrow = container(icons::tinted(Icon::ChevronDown, 12.0, |p| p.text_muted))
        .padding([14, 3])
        .style(theme::dropdown_arrow);
    crate::ui::context_area::click_area(
        iced::widget::mouse_area(arrow).interaction(iced::mouse::Interaction::Pointer),
        Message::PullMenu,
    )
}

fn divider<'a>() -> Element<'a, Message> {
    container(space::vertical().width(1).height(28))
        .style(theme::divider)
        .into()
}
