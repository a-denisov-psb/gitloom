//! Right panel: staging area (WIP selected) or commit details.

use git_core::{Commit, DiffKind, DiffRequest, FileChange};
use iced::widget::{
    button, checkbox, column, container, hover, row, space, text, text_editor, text_input, tooltip,
    Column,
};
use iced::{Element, Fill, FillPortion};

use crate::app::{DiscardTarget, FileArea, Message};
use crate::format;
use crate::repo_view::{CenterView, DetailChanges, Loadable, RepoView, Selection};
use crate::theme;
use crate::ui::context_area::context_area;
use crate::ui::{change_badge, muted};

const NOT_AVAILABLE: &str = "Not available yet";

pub fn view<'a>(view: &'a RepoView, avatars: &'a crate::avatars::Avatars) -> Element<'a, Message> {
    let content = match &view.selection {
        _ if view.is_multi() => multi_selection(view),
        Some(Selection::Wip) => staging(view),
        Some(Selection::Commit(id)) => match view.snapshot.commit(id) {
            Some(commit) => details(view, commit, avatars),
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

/// Several commits selected: their summaries and the actions for them.
/// Cherry-pick follows in M4-09.
fn multi_selection(view: &RepoView) -> Element<'_, Message> {
    let count = view.multi.len();
    let commits = Column::with_children(view.multi.ids().iter().filter_map(|id| {
        let commit = view.snapshot.commit(id)?;
        Some(
            row![
                text(short(&commit.id)).font(iced::Font::MONOSPACE).size(12),
                text(&commit.summary)
                    .size(13)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(8)
            .into(),
        )
    }))
    .spacing(4);
    let pick = button(text(format!("Cherry-pick {count} commits")).size(12))
        .padding([4, 10])
        .style(button::secondary);
    // Applied oldest first; the selection is in graph order (newest first).
    let mut oldest_first = view.multi.ids().to_vec();
    oldest_first.reverse();
    let can_pick = view.snapshot.state.is_clean() && view.busy.is_none();
    let pick: Element<'_, Message> = if view.write_block.is_some() {
        write_tooltip(pick.into(), view)
    } else {
        pick.on_press_maybe(can_pick.then_some(Message::CherryPick(oldest_first)))
            .into()
    };
    let mut actions = row![pick].spacing(8);
    if crate::repo_view::squashable(&view.snapshot, view.multi.ids()) {
        let squash = button(text(format!("Squash {count} commits")).size(12))
            .padding([4, 10])
            .style(button::secondary);
        actions = actions.push(if view.write_block.is_some() {
            write_tooltip(squash.into(), view)
        } else {
            squash
                .on_press_maybe(view.busy.is_none().then_some(Message::SquashSelected))
                .into()
        });
    }
    let content = column![
        text(format!("{count} commits selected")).size(16),
        actions,
        crate::ui::vscroll(commits).height(Fill),
    ]
    .spacing(12)
    .padding(12);
    content.into()
}

fn staging(view: &RepoView) -> Element<'_, Message> {
    let tree = &view.snapshot.working_tree;
    let header = format!(
        "{} file changes on {}",
        tree.len(),
        view.snapshot.branch_label()
    );

    // Conflicts get their own section above the unstaged files (M4-03).
    let (conflicted, unstaged_files): (Vec<_>, Vec<_>) = tree
        .unstaged
        .iter()
        .partition(|f| f.kind == git_core::ChangeKind::Conflicted);
    let conflicts = (!conflicted.is_empty()).then(|| {
        file_section(
            format!("Conflicted Files ({})", conflicted.len()),
            // Staging a file marks it resolved; the conflict view comes in M4-05.
            space::horizontal().into(),
            conflicted
                .iter()
                .map(|file| file_row(view, file, DiffKind::Unstaged)),
        )
    });

    let has_unstaged = !unstaged_files.is_empty();
    let unstaged = file_section(
        format!("Unstaged Files ({})", unstaged_files.len()),
        row![
            all_button(
                view,
                "Discard all",
                button::danger,
                has_unstaged,
                Message::Discard(DiscardTarget::All),
            ),
            all_button(
                view,
                "Stage all changes",
                button::success,
                has_unstaged,
                Message::StageAll,
            ),
        ]
        .spacing(6)
        .into(),
        unstaged_files.into_iter().map(|file| {
            let kind = if file.kind == git_core::ChangeKind::Untracked {
                DiffKind::Untracked
            } else {
                DiffKind::Unstaged
            };
            file_row(view, file, kind)
        }),
    );

    let staged = file_section_with_menu(
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
        Some(Message::StagedHeaderMenu),
    );

    column![container(text(header)).padding([10, 12])]
        .push(conflicts)
        .push(unstaged)
        .push(staged)
        .push(commit_box(view))
        .into()
}

/// Commit box, read-only until committing arrives in M2.
fn commit_box(view: &RepoView) -> Element<'_, Message> {
    let writable = view.write_block.is_none();
    let remaining = SUMMARY_LIMIT as i64 - view.summary.chars().count() as i64;
    // The counter sits inside the field at the right edge like GitKraken
    // (M9-07); the right padding keeps the text clear of it.
    let mut summary = text_input("Summary", &view.summary)
        .id(crate::app::COMMIT_SUMMARY_INPUT)
        .padding(iced::Padding::new(6.0).right(COUNTER_ROOM))
        .style(theme::input);
    if writable {
        summary = summary.on_input(Message::SummaryChanged);
    }
    let counter = text(remaining.to_string())
        .size(11)
        .style(match counter_level(remaining) {
            CounterLevel::Normal => theme::text_muted,
            CounterLevel::Warning => theme::text_warning,
            CounterLevel::Over => theme::text_danger,
        });
    let summary = iced::widget::stack![
        summary,
        container(counter)
            .width(Fill)
            .height(Fill)
            .align_x(iced::Right)
            .align_y(iced::Center)
            .padding(iced::Padding::ZERO.right(8)),
    ];

    let mut description = text_editor(&view.description)
        .placeholder("Description")
        .padding(6)
        .style(theme::editor)
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
    let continues = view.snapshot.state.operation();
    let label = match (view.amend, continues, staged) {
        (true, _, _) => "Amend previous commit".to_string(),
        (false, Some(operation), _) => format!("Continue {operation}"),
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
        identity_line(view),
        summary,
        description,
        commit,
    ]
    .spacing(8)
    .padding(12);
    // A rebase always runs on a detached HEAD; the warning would be noise.
    let rebasing = matches!(view.snapshot.state, git_core::RepoState::Rebasing { .. });
    if matches!(view.snapshot.head, git_core::Head::Detached { .. }) && !rebasing {
        content = content.push(
            text("You are in detached HEAD; create a branch to keep your commits.")
                .size(11)
                .style(theme::text_warning),
        );
    }
    if view.amend && view.head_pushed() {
        content = content.push(
            text("HEAD is already pushed; amending rewrites published history.")
                .size(11)
                .style(theme::text_warning),
        );
    }
    if let Some(hint) = blocker.filter(|_| writable) {
        content = content.push(muted(hint).size(11));
    }
    content.into()
}

/// "Committing as <name> <email>" above the summary (M8-04): the identity
/// the next commit uses, its origin as tooltip; a click opens the identity
/// menu. Without one: "No Git identity set" in the warning color.
fn identity_line(view: &RepoView) -> Element<'_, Message> {
    let resolved = &view.snapshot.identity;
    let (label, tip): (Element<'_, Message>, String) = match resolved.identity() {
        Some(identity) => (
            muted(format!("Committing as {}", identity.label()))
                .size(11)
                .wrapping(text::Wrapping::None)
                .into(),
            match resolved.origin() {
                Some(origin) => format!("{}; click to choose another", origin.description()),
                None => "Click to choose another identity".to_owned(),
            },
        ),
        None => (
            text("No Git identity set")
                .size(11)
                .style(theme::text_warning)
                .into(),
            "Commits need a name and email: click to choose an identity".to_owned(),
        ),
    };
    let line = container(
        row![
            label,
            crate::icons::tinted(crate::icons::Icon::ChevronDown, 11.0, |p| p.text_muted)
        ]
        .spacing(4)
        .align_y(iced::Center),
    )
    .clip(true);
    let area = crate::ui::context_area::click_area(line, Message::IdentityMenu);
    with_tooltip_text(
        iced::widget::mouse_area(area)
            .interaction(iced::mouse::Interaction::Pointer)
            .into(),
        tip,
    )
}

/// GitKraken counts down from 72 characters for the summary.
const SUMMARY_LIMIT: usize = 72;

/// Right padding of the summary field that holds the counter (M9-07).
const COUNTER_ROOM: f32 = 40.0;

/// Color of the summary counter: muted, warning at 10 or fewer characters
/// left, error below 0 (the commit is not blocked).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CounterLevel {
    Normal,
    Warning,
    Over,
}

fn counter_level(remaining: i64) -> CounterLevel {
    match remaining {
        r if r < 0 => CounterLevel::Over,
        r if r <= 10 => CounterLevel::Warning,
        _ => CounterLevel::Normal,
    }
}

fn details<'a>(
    view: &'a RepoView,
    commit: &'a Commit,
    avatars: &crate::avatars::Avatars,
) -> Element<'a, Message> {
    let id = row![
        muted("commit:"),
        text(short(&commit.id)).font(iced::Font::MONOSPACE),
    ]
    .spacing(6);

    let body = (!commit.body.is_empty()).then(|| text(&commit.body));

    let author = row![
        crate::ui::avatar(avatars.get(&commit.email), &commit.author, 32.0),
        {
            let mut lines = column![
                text(&commit.author),
                muted(author_line(&commit.email, commit.author_time)).size(12),
            ]
            .spacing(2);
            if let Some(badge) = signature_badge(view, &commit.id) {
                lines = lines.push(badge);
            }
            lines
        },
    ]
    .spacing(10)
    .align_y(iced::Center);

    let parents = commit.parents.iter().map(|p| {
        muted(format!("parent: {}", short(p)))
            .font(iced::Font::MONOSPACE)
            .size(12)
            .into()
    });

    let files: Element<'a, Message> = match view.details.as_ref().map(|d| &d.changes) {
        Some(Loadable::Ready(DetailChanges::Commit(changes))) => column![
            muted(changed_files(changes.len())),
            Column::with_children(
                changes
                    .iter()
                    .map(|file| { file_row(view, file, DiffKind::Commit(commit.id.clone())) })
            ),
        ]
        .spacing(8)
        .into(),
        Some(Loadable::Ready(DetailChanges::Stash(changes))) => {
            stash_files(view, &commit.id, changes)
        }
        Some(Loadable::Failed(error)) => text(error).style(theme::text_danger).into(),
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

    crate::ui::vscroll(content).height(Fill).into()
}

/// Signature badge after the author line (M10-05): green for a good
/// signature, red for a bad one, muted otherwise; the key id as tooltip.
fn signature_badge<'a>(view: &RepoView, id: &str) -> Option<Element<'a, Message>> {
    use git_core::signature::SignatureState;
    let Some(Loadable::Ready(Some(signature))) = view.signatures.get(id) else {
        return None;
    };
    let style = match signature.state {
        SignatureState::Good => theme::text_success,
        SignatureState::Bad => theme::text_danger,
        _ => theme::text_muted,
    };
    let icon = match signature.state {
        SignatureState::Bad => crate::icons::Icon::Alert,
        _ => crate::icons::Icon::Check,
    };
    let badge = row![
        crate::icons::colored(icon, 12.0, move |t| {
            style(t).color.unwrap_or(iced::Color::BLACK)
        }),
        text(signature.text()).size(12).style(style),
    ]
    .spacing(4)
    .align_y(iced::Center);
    Some(if signature.key.is_empty() {
        badge.into()
    } else {
        with_tooltip_text(badge.into(), format!("Key {}", signature.key))
    })
}

/// "1 changed file", "0 changed files", "2 changed files".
pub(crate) fn changed_files(count: usize) -> String {
    if count == 1 {
        "1 changed file".to_owned()
    } else {
        format!("{count} changed files")
    }
}

/// "email · authored date", without the separator when the email is empty.
pub(crate) fn author_line(email: &str, time: i64) -> String {
    let date = format::date_time(time);
    if email.is_empty() {
        format!("authored {date}")
    } else {
        format!("{email} · authored {date}")
    }
}

/// Files of a stash entry in sections like the working copy view.
fn stash_files<'a>(
    view: &'a RepoView,
    id: &str,
    changes: &'a git_core::StashChanges,
) -> Element<'a, Message> {
    use git_core::StashPart;
    let mut content = column![muted(changed_files(changes.file_count()))].spacing(8);
    for (title, part, files) in [
        ("Staged", StashPart::Staged, &changes.staged),
        ("Unstaged", StashPart::Unstaged, &changes.unstaged),
        ("Untracked", StashPart::Untracked, &changes.untracked),
    ] {
        if files.is_empty() {
            continue;
        }
        let kind = DiffKind::Stash {
            id: id.to_owned(),
            part,
        };
        content = content.push(
            column![
                text(format!("{title} ({})", files.len())).size(12),
                Column::with_children(files.iter().map(|file| file_row(view, file, kind.clone()))),
            ]
            .spacing(2),
        );
    }
    content.into()
}

fn short(id: &str) -> &str {
    &id[..id.len().min(7)]
}

fn file_section<'a>(
    title: String,
    action: Element<'a, Message>,
    files: impl Iterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    file_section_with_menu(title, action, files, None)
}

/// [`file_section`] whose header opens a context menu (Staged Files, M10-07).
fn file_section_with_menu<'a>(
    title: String,
    action: Element<'a, Message>,
    files: impl Iterator<Item = Element<'a, Message>>,
    menu: Option<fn(iced::Point) -> Message>,
) -> Element<'a, Message> {
    // The buttons move below the title when the panel is narrow.
    let header: Element<'a, Message> = container(
        row![text(title).wrapping(text::Wrapping::None), action]
            .spacing(8)
            .align_y(iced::Center)
            .wrap()
            .vertical_spacing(4),
    )
    .padding([6, 12])
    .width(Fill)
    .into();
    let header = match menu {
        Some(menu) => crate::ui::context_area::context_area(header, menu),
        None => header,
    };

    column![
        header,
        crate::ui::vscroll(Column::with_children(files)).height(Fill)
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
    let selected = match &view.center {
        CenterView::Diff(diff) => diff.request == request,
        CenterView::Conflict(conflict) => {
            file.kind == git_core::ChangeKind::Conflicted && conflict.path == file.path
        }
        CenterView::Graph | CenterView::Rebase(_) | CenterView::FileHistory(_) => false,
    };

    let label = match &file.old_path {
        Some(old) => format!("{old} → {}", file.path),
        None => file.path.clone(),
    };

    let area = match &request.kind {
        DiffKind::Staged => FileArea::Staged,
        DiffKind::Unstaged | DiffKind::Untracked => FileArea::Unstaged,
        DiffKind::Commit(_) | DiffKind::Stash { .. } => FileArea::Commit,
    };
    let row = button(
        row![
            change_badge(file.kind),
            text(label).wrapping(text::Wrapping::None),
        ]
        .align_y(iced::Center),
    )
    .on_press(if file.kind == git_core::ChangeKind::Conflicted {
        Message::ConflictOpened(file.path.clone())
    } else {
        Message::DiffOpened(request)
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_files_plural() {
        assert_eq!(changed_files(0), "0 changed files");
        assert_eq!(changed_files(1), "1 changed file");
        assert_eq!(changed_files(2), "2 changed files");
    }

    #[test]
    fn counter_warns_at_ten_and_turns_red_below_zero() {
        assert_eq!(counter_level(72), CounterLevel::Normal);
        assert_eq!(counter_level(11), CounterLevel::Normal);
        assert_eq!(counter_level(10), CounterLevel::Warning);
        assert_eq!(counter_level(0), CounterLevel::Warning);
        assert_eq!(counter_level(-1), CounterLevel::Over);
    }
}
