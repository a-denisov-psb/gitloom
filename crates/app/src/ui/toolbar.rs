//! Main toolbar: repo/branch pickers, actions, terminal.

use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Fill};

use crate::app::Message;
use crate::icons::{self, Icon};
use crate::repo_view::{Guarded, RepoView};
use crate::shortcuts::Shortcut;
use crate::theme;
use crate::ui::muted;
use crate::ui::panel::{with_tooltip_text, write_tooltip};

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
    compact: bool,
) -> iced::widget::Button<'a, Message> {
    let enabled = on_press.is_some();
    // Compact (narrow window, high zoom): icons only, the label is in the
    // tooltip.
    let content: Element<'a, Message> = if compact {
        icons::button_icon(icon, 18.0, enabled).into()
    } else {
        column![
            icons::button_icon(icon, 18.0, enabled),
            text(label).size(11)
        ]
        .spacing(2)
        .align_x(iced::Center)
        .into()
    };
    // Icon and label centered in the fixed button width (the column alone
    // shrinks to its content and would sit at the left edge).
    let content = container(content).width(Fill).align_x(iced::Center);
    button(content)
        .on_press_maybe(on_press)
        .padding(if compact { [8, 8] } else { [3, 10] })
        .width(if compact { 36 } else { 64 })
        .style(theme::flat)
}

fn toolbar_button<'a>(
    action: ToolbarAction,
    on_press: Option<Message>,
    compact: bool,
) -> iced::widget::Button<'a, Message> {
    tool_button(action.icon(), action.label(), on_press, compact)
}

/// Why the repository and branch pickers are disabled, if they are (from
/// the action registry, see `App::action_block`).
#[derive(Debug, Clone, Default)]
pub struct Pickers {
    pub repository: Option<String>,
    pub branch: Option<String>,
}

/// `compact`: the window is too narrow for labelled buttons (icons only).
pub fn view(
    view: &RepoView,
    pull_mode: git_core::PullMode,
    compact: bool,
    blocks: Pickers,
) -> Element<'_, Message> {
    let pickers = row![
        picker(
            "repository",
            view.name().to_owned(),
            Shortcut::OpenRepoPalette,
            blocks.repository,
        ),
        picker(
            "branch",
            view.snapshot.branch_label(),
            Shortcut::CheckoutBranch,
            blocks.branch,
        ),
    ]
    .spacing(20);

    let actions = row![
        action(view, compact, ToolbarAction::Undo),
        action(view, compact, ToolbarAction::Redo),
        divider(),
        row![pull_button(view, pull_mode, compact), pull_dropdown()].align_y(iced::Center),
        push_button(view, compact),
        divider(),
        branch_button(view, compact),
        action(view, compact, ToolbarAction::Stash),
        action(view, compact, ToolbarAction::Pop),
    ]
    .spacing(2)
    .align_y(iced::Center);

    let content = row![
        pickers,
        space::horizontal(),
        actions,
        space::horizontal(),
        action(view, compact, ToolbarAction::Terminal),
        settings_button(compact),
    ]
    .padding([4, 12])
    .align_y(iced::Center);

    container(content).width(Fill).style(theme::chrome).into()
}

/// Gear button: opens the settings view (Ctrl+,).
fn settings_button<'a>(compact: bool) -> Element<'a, Message> {
    let tip = match crate::actions::ActionId::Settings.shortcut_text() {
        Some(keys) => format!("Preferences ({keys})"),
        None => "Preferences".to_owned(),
    };
    with_tooltip_text(
        tool_button(
            Icon::Settings,
            "Settings",
            Some(Message::SettingsOpened),
            compact,
        )
        .into(),
        tip,
    )
}

/// Repository / branch picker: opens the palette list of `action`
/// (M6-17; GitKraken opens a dropdown, see UX-PARITY). `block` is why the
/// action cannot run, shown as the tooltip of the disabled picker.
fn picker<'a>(
    caption: &'a str,
    value: String,
    action: Shortcut,
    block: Option<String>,
) -> Element<'a, Message> {
    let tip = block.clone().unwrap_or_else(|| picker_tooltip(action));
    with_tooltip_text(
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
        .on_press_maybe(block.is_none().then_some(Message::PickerClicked(action)))
        .padding([3, 8])
        .style(theme::picker)
        .into(),
        tip,
    )
}

/// Tooltip of an enabled picker: what it opens, with the shortcut if any.
fn picker_tooltip(action: Shortcut) -> String {
    let label = match action {
        Shortcut::CheckoutBranch => "Checkout branch",
        _ => "Open recent repository",
    };
    match action.shortcut_text() {
        Some(keys) => format!("{label} ({keys})"),
        None => label.to_owned(),
    }
}

fn action<'a>(view: &RepoView, compact: bool, action: ToolbarAction) -> Element<'a, Message> {
    let disabled = || -> Element<'a, Message> { toolbar_button(action, None, compact).into() };
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
                toolbar_button(action, view.busy.is_none().then_some(message), compact).into(),
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
            // Labelled in the tooltip (needed when compact).
            with_tooltip_text(
                toolbar_button(action, Some(message), compact).into(),
                action.label().to_owned(),
            )
        }
        // Nothing to stash or pop: disabled, the tooltip says why (and
        // names the button when compact).
        (ToolbarAction::Stash, _) if view.write_block.is_none() => {
            with_tooltip_text(disabled(), "Stash: no changes to stash".to_owned())
        }
        (ToolbarAction::Pop, _) if view.write_block.is_none() => {
            with_tooltip_text(disabled(), "Pop: no stash to pop".to_owned())
        }
        (ToolbarAction::Terminal, _) => with_tooltip_text(
            toolbar_button(action, Some(Message::OpenTerminal), compact).into(),
            match crate::actions::ActionId::OpenTerminal.shortcut_text() {
                Some(keys) => format!("Open a terminal in this repository ({keys})"),
                None => "Open a terminal in this repository".to_owned(),
            },
        ),
        _ => write_tooltip(disabled(), view),
    }
}

/// Pull in the current mode; the tooltip names the mode.
fn pull_button<'a>(
    view: &RepoView,
    mode: git_core::PullMode,
    compact: bool,
) -> Element<'a, Message> {
    if view.write_block.is_some() {
        return write_tooltip(
            toolbar_button(ToolbarAction::Pull, None, compact).into(),
            view,
        );
    }
    with_tooltip_text(
        toolbar_button(
            ToolbarAction::Pull,
            view.busy.is_none().then_some(Message::Pull(None)),
            compact,
        )
        .into(),
        mode.label().to_owned(),
    )
}

/// New branch at the selected commit (inline input, Ctrl+B).
fn branch_button<'a>(view: &RepoView, compact: bool) -> Element<'a, Message> {
    if view.write_block.is_some() {
        return write_tooltip(
            toolbar_button(ToolbarAction::Branch, None, compact).into(),
            view,
        );
    }
    with_tooltip_text(
        toolbar_button(
            ToolbarAction::Branch,
            view.busy.is_none().then_some(Message::NewBranchStart(None)),
            compact,
        )
        .into(),
        match crate::actions::ActionId::CreateBranch.shortcut_text() {
            Some(keys) => format!("Create a branch at the selected commit ({keys})"),
            None => "Create a branch at the selected commit".to_owned(),
        },
    )
}

/// Push the current branch; the tooltip names the target.
fn push_button<'a>(view: &RepoView, compact: bool) -> Element<'a, Message> {
    if view.write_block.is_some() {
        return write_tooltip(
            toolbar_button(ToolbarAction::Push, None, compact).into(),
            view,
        );
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
            compact,
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
