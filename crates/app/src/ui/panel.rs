//! Right panel: staging area (WIP selected) or commit details.

use git_core::{Commit, DiffKind, DiffRequest, FileChange};
use iced::widget::{
    button, checkbox, column, container, hover, row, scrollable, space, text, text_editor,
    text_input, tooltip, Column,
};
use iced::{Element, Fill, FillPortion};

use crate::app::{DiscardTarget, FileArea, Message};
use crate::format;
use crate::repo_view::{CenterView, Loadable, RepoView, Selection};
use crate::theme;
use crate::ui::context_area::context_area;
use crate::ui::{change_badge, muted};

const NOT_AVAILABLE: &str = "Not available yet";

pub fn view(view: &RepoView) -> Element<'_, Message> {
    let content = match &view.selection {
        Some(Selection::Wip) => staging(view),
        Some(Selection::Commit(id)) => match view.snapshot.commit(id) {
            Some(commit) => details(view, commit),
            None => muted("Nothing selected").into(),
        },
        None => container(muted("Nothing selected")).padding(12).into(),
    };

    container(content)
        .width(Fill)
        .height(Fill)
        .style(theme::panel)
        .into()
}

fn staging(view: &RepoView) -> Element<'_, Message> {
    let tree = &view.snapshot.working_tree;
    let header = format!(
        "{} file changes on {}",
        tree.len(),
        view.snapshot.branch_label()
    );

    let unstaged = file_section(
        format!("Unstaged Files ({})", tree.unstaged.len()),
        row![
            all_button(
                view,
                "Discard all",
                button::danger,
                !tree.unstaged.is_empty(),
                Message::Discard(DiscardTarget::All),
            ),
            all_button(
                view,
                "Stage all changes",
                button::success,
                !tree.unstaged.is_empty(),
                Message::StageAll,
            ),
        ]
        .spacing(6)
        .into(),
        tree.unstaged.iter().map(|file| {
            let kind = if file.kind == git_core::ChangeKind::Untracked {
                DiffKind::Untracked
            } else {
                DiffKind::Unstaged
            };
            file_row(view, file, kind)
        }),
    );

    let staged = file_section(
        format!("Staged Files ({})", tree.staged.len()),
        all_button(
            view,
            "Unstage all changes",
            button::danger,
            !tree.staged.is_empty(),
            Message::UnstageAll,
        ),
        tree.staged
            .iter()
            .map(|file| file_row(view, file, DiffKind::Staged)),
    );

    column![
        container(text(header)).padding([10, 12]),
        unstaged,
        staged,
        commit_box(view)
    ]
    .into()
}

/// Commit box, read-only until committing arrives in M2.
fn commit_box(view: &RepoView) -> Element<'_, Message> {
    let writable = view.write_block.is_none();
    let remaining = SUMMARY_LIMIT as i64 - view.summary.chars().count() as i64;
    let mut summary = text_input("Summary", &view.summary)
        .id(crate::app::COMMIT_SUMMARY_INPUT)
        .padding(6);
    if writable {
        summary = summary.on_input(Message::SummaryChanged);
    }
    let counter = muted(remaining.to_string())
        .size(11)
        .width(28)
        .align_x(iced::Right)
        .color(if remaining < 0 {
            theme::DANGER
        } else {
            theme::TEXT_MUTED
        });

    let mut description = text_editor(&view.description)
        .placeholder("Description")
        .padding(6)
        .height(90)
        // Ctrl+Enter commits instead of inserting a line break.
        .key_binding(|press| {
            let enter = matches!(
                press.key.as_ref(),
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
            );
            if enter && press.modifiers.command() {
                let shortcut = if press.modifiers.shift() {
                    crate::shortcuts::Shortcut::StageAllAndCommit
                } else {
                    crate::shortcuts::Shortcut::Commit
                };
                return Some(text_editor::Binding::Custom(Message::Shortcut(shortcut)));
            }
            text_editor::Binding::from_key_press(press)
        });
    if writable {
        description = description.on_action(Message::DescriptionEdited);
    }

    let staged = view.snapshot.working_tree.staged.len();
    let concludes = view
        .snapshot
        .state
        .operation()
        .filter(|_| view.snapshot.state.concluded_by_commit());
    let label = match (view.amend, concludes, staged) {
        (true, _, _) => "Amend previous commit".to_string(),
        (false, Some(operation), _) => format!("Commit and conclude {operation}"),
        (false, None, 1) => "Commit changes to 1 file".to_string(),
        (false, None, n) => format!("Commit changes to {n} files"),
    };
    let can_amend = writable
        && view.head_commit().is_some()
        // Unchecking stays possible when an operation started meanwhile.
        && (view.amend || view.state_block(crate::repo_view::Guarded::Amend).is_none());
    let mut amend = checkbox(view.amend)
        .label("Amend previous commit")
        .size(14)
        .text_size(12);
    if can_amend {
        amend = amend.on_toggle(Message::AmendToggled);
    }
    let blocker = crate::app::commit_blocker(view, false);
    let commit = button(text(label).width(Fill).align_x(iced::Center))
        .width(Fill)
        .padding(8)
        .style(button::primary);
    let commit: Element<'_, Message> = if writable {
        commit
            .on_press_maybe(
                (blocker.is_none() && view.busy.is_none()).then_some(Message::CommitRequested),
            )
            .into()
    } else {
        write_tooltip(commit.into(), view)
    };

    let mut content = column![
        row![muted("Commit"), space::horizontal(), amend].align_y(iced::Center),
        row![summary, counter].spacing(6).align_y(iced::Center),
        description,
        commit,
    ]
    .spacing(8)
    .padding(12);
    if view.amend && view.head_pushed() {
        content = content.push(
            text("HEAD is already pushed; amending rewrites published history.")
                .size(11)
                .color(theme::WARNING),
        );
    }
    if let Some(hint) = blocker.filter(|_| writable) {
        content = content.push(muted(hint).size(11));
    }
    content.into()
}

/// GitKraken counts down from 72 characters for the summary.
const SUMMARY_LIMIT: usize = 72;

fn details<'a>(view: &'a RepoView, commit: &'a Commit) -> Element<'a, Message> {
    let id = row![
        muted("commit:"),
        text(short(&commit.id)).font(iced::Font::MONOSPACE),
    ]
    .spacing(6);

    let body = (!commit.body.is_empty()).then(|| text(&commit.body));

    let author = column![
        text(&commit.author),
        muted(format!(
            "{} · authored {}",
            commit.email,
            format::date_time(commit.author_time)
        ))
        .size(12),
    ]
    .spacing(2);

    let parents = commit.parents.iter().map(|p| {
        muted(format!("parent: {}", short(p)))
            .font(iced::Font::MONOSPACE)
            .size(12)
            .into()
    });

    let files: Element<'a, Message> = match view.details.as_ref().map(|d| &d.changes) {
        Some(Loadable::Ready(changes)) => column![
            muted(format!("{} changed files", changes.len())),
            Column::with_children(
                changes
                    .iter()
                    .map(|file| { file_row(view, file, DiffKind::Commit(commit.id.clone())) })
            ),
        ]
        .spacing(8)
        .into(),
        Some(Loadable::Failed(error)) => text(error).color(theme::DANGER).into(),
        Some(Loadable::Loading) | None => muted("Loading changed files…").into(),
    };

    let content = column![id, text(&commit.summary).size(16)]
        .push(body)
        .push(author)
        .push(row(parents).spacing(12))
        .push(space::vertical().height(4))
        .push(files)
        .spacing(8)
        .padding(12);

    scrollable(content).height(Fill).into()
}

fn short(id: &str) -> &str {
    &id[..id.len().min(7)]
}

fn file_section<'a>(
    title: String,
    action: Element<'a, Message>,
    files: impl Iterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    // The buttons move below the title when the panel is narrow.
    let header = container(
        row![text(title).wrapping(text::Wrapping::None), action]
            .spacing(8)
            .align_y(iced::Center)
            .wrap()
            .vertical_spacing(4),
    )
    .padding([6, 12]);

    column![
        header,
        scrollable(Column::with_children(files)).height(Fill)
    ]
    .height(FillPortion(1))
    .into()
}

fn file_row<'a>(view: &'a RepoView, file: &'a FileChange, kind: DiffKind) -> Element<'a, Message> {
    let request = DiffRequest {
        kind,
        path: file.path.clone(),
        old_path: file.old_path.clone(),
    };
    let selected = matches!(&view.center, CenterView::Diff(diff) if diff.request == request);

    let label = match &file.old_path {
        Some(old) => format!("{old} → {}", file.path),
        None => file.path.clone(),
    };

    let area = match &request.kind {
        DiffKind::Staged => FileArea::Staged,
        DiffKind::Unstaged | DiffKind::Untracked => FileArea::Unstaged,
        DiffKind::Commit(_) => FileArea::Commit,
    };
    let row = button(
        row![
            change_badge(file.kind),
            text(label).wrapping(text::Wrapping::None),
        ]
        .align_y(iced::Center),
    )
    .on_press(Message::DiffOpened(request))
    .padding([3, 12])
    .width(Fill)
    .clip(true)
    .style(theme::list_item(selected));

    // Like GitKraken: the stage/unstage button appears while hovering the row.
    let quick_actions = match area {
        FileArea::Unstaged => Some(row![
            button(text("Discard").size(11))
                .on_press(Message::Discard(DiscardTarget::File(file.clone())))
                .padding([2, 8])
                .style(button::danger),
            button(text("Stage File").size(11))
                .on_press(Message::Stage(file.clone()))
                .padding([2, 8])
                .style(button::secondary),
        ]),
        FileArea::Staged => Some(row![button(text("Unstage File").size(11))
            .on_press(Message::Unstage(file.clone()))
            .padding([2, 8])
            .style(button::secondary)]),
        FileArea::Commit => None,
    };
    let content: Element<'a, Message> = match quick_actions {
        Some(actions) if view.write_block.is_none() && view.busy.is_none() => hover(
            row,
            container(actions.spacing(4))
                .align_x(iced::Right)
                .align_y(iced::Center)
                .padding([0, 8])
                .width(Fill)
                .height(Fill),
        ),
        _ => row.into(),
    };

    let file = file.clone();
    context_area(content, move |position| Message::FileContextMenu {
        file: file.clone(),
        area,
        position,
    })
}

/// "Stage all" / "Unstage all": active when writing is possible and the list is not empty.
fn all_button<'a>(
    view: &RepoView,
    label: &'a str,
    style: fn(&iced::Theme, button::Status) -> button::Style,
    enabled: bool,
    message: Message,
) -> Element<'a, Message> {
    let button = button(text(label).size(12).wrapping(text::Wrapping::None))
        .padding([3, 8])
        .style(style);
    if view.write_block.is_some() {
        return write_tooltip(button.into(), view);
    }
    button
        .on_press_maybe((enabled && view.busy.is_none()).then_some(message))
        .into()
}

/// Wraps a disabled control in the standard "Not available yet" tooltip.
pub(crate) fn with_tooltip(content: Element<'_, Message>) -> Element<'_, Message> {
    with_tooltip_text(content, NOT_AVAILABLE.to_owned())
}

/// Tooltip for a disabled write action: why writing is blocked, if it is.
pub(crate) fn write_tooltip<'a>(
    content: Element<'a, Message>,
    view: &RepoView,
) -> Element<'a, Message> {
    let reason = view
        .write_block
        .clone()
        .unwrap_or_else(|| NOT_AVAILABLE.to_owned());
    with_tooltip_text(content, reason)
}

/// Wraps a control in a tooltip with `message`.
pub(crate) fn with_tooltip_text(
    content: Element<'_, Message>,
    message: String,
) -> Element<'_, Message> {
    tooltip(
        content,
        container(text(message).size(12))
            .padding([4, 8])
            .style(theme::tooltip),
        tooltip::Position::Bottom,
    )
    .into()
}
