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

pub fn view<'a>(
    view: &'a RepoView,
    avatars: &'a crate::avatars::Avatars,
    files: crate::file_list::FileListOptions,
) -> Element<'a, Message> {
    let content = match &view.selection {
        _ if view.is_multi() => multi_selection(view),
        Some(Selection::Wip) => staging(view),
        Some(Selection::Commit(id)) => match view.snapshot.commit(id) {
            Some(commit) => details(view, commit, avatars, files),
            None => muted("Nothing selected").into(),
        },
        None => container(muted("Nothing selected")).padding(12).into(),
    };
    // Another commit is selected while an operation runs: a header leads
    // back to the conflicts (M12-05).
    let content = match view.operation() {
        Some(status) if view.selection != Some(Selection::Wip) => {
            column![operation_header(&status), content].into()
        }
        _ => content,
    };

    container(content)
        .width(Fill)
        .height(Fill)
        .style(theme::panel)
        .into()
}

/// "1 file conflict in working directory · View conflicts" (or the
/// operation's title without conflicts) above the commit details.
fn operation_header<'a>(status: &crate::operation::OperationStatus) -> Element<'a, Message> {
    let (label, action) = if status.conflicts > 0 {
        let action = if status.conflicts == 1 {
            "View conflict"
        } else {
            "View conflicts"
        };
        (status.conflicts_text(), action)
    } else {
        (status.title.clone(), "View changes")
    };
    container(
        row![
            crate::icons::tinted(crate::icons::Icon::Alert, 14.0, |p| p.warning),
            text(label)
                .size(12)
                .width(Fill)
                .wrapping(text::Wrapping::WordOrGlyph),
            button(text(action).size(12).wrapping(text::Wrapping::None))
                .padding([3, 8])
                .style(button::secondary)
                .on_press(Message::WipSelected),
        ]
        .spacing(8)
        .align_y(iced::Center),
    )
    .padding([6, 12])
    .width(Fill)
    .style(theme::operation_row)
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
    let operation = view.operation();
    // A running operation names itself like GitKraken: "Merge conflicts
    // detected" over "Merging topic into main" (M12-05).
    let header: Element<'_, Message> = match &operation {
        Some(status) => {
            let mut lines = Column::new().spacing(2);
            if let Some(heading) = status.conflicts_heading() {
                lines = lines.push(text(heading).size(12).style(theme::text_warning));
            }
            lines.push(text(status.title.clone())).into()
        }
        None => text(format!(
            "{} file changes on {}",
            tree.len(),
            view.snapshot.branch_label()
        ))
        .into(),
    };

    // Conflicts get their own section above the unstaged files (M4-03).
    let (conflicted, unstaged_files): (Vec<_>, Vec<_>) = tree
        .unstaged
        .iter()
        .partition(|f| f.kind == git_core::ChangeKind::Conflicted);
    let conflicts = (!conflicted.is_empty()).then(|| {
        file_section(
            format!("Conflicted Files ({})", conflicted.len()),
            // Stages every conflicted file; files with markers are named
            // in a confirmation first (M12-05).
            all_button(
                view,
                "Mark all resolved",
                button::success,
                true,
                Message::MarkAllResolved,
            ),
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

    // Files resolved during the operation get their own list (M12-05).
    let resolved_files = view.resolved_files();
    let resolved = operation.as_ref().map(|_| {
        file_section(
            format!("Resolved Files ({})", resolved_files.len()),
            space::horizontal().into(),
            resolved_files
                .iter()
                .map(|file| file_row(view, file, DiffKind::Staged)),
        )
    });
    let staged_files: Vec<&FileChange> = tree
        .staged
        .iter()
        .filter(|f| !resolved_files.iter().any(|r| r.path == f.path))
        .collect();
    let staged = file_section_with_menu(
        format!("Staged Files ({})", staged_files.len()),
        all_button(
            view,
            "Unstage all changes",
            button::danger,
            !tree.staged.is_empty(),
            Message::UnstageAll,
        ),
        staged_files
            .into_iter()
            .map(|file| file_row(view, file, DiffKind::Staged)),
        Some(Message::StagedHeaderMenu),
    );

    column![container(header).padding([10, 12])]
        .push(conflicts)
        .push(resolved)
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
    let operation = view.operation();
    let continues = operation.as_ref().and_then(|s| s.commit_label());
    let label = match (view.amend, continues, staged) {
        (true, _, _) => "Amend previous commit".to_string(),
        (false, Some(label), _) => label,
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
        // The identity takes the room left of the fixed-size button.
        row![
            container(identity_line(view)).width(Fill).clip(true),
            prompt_button(view)
        ]
        .spacing(6)
        .align_y(iced::Center),
        summary,
        description,
        commit,
    ]
    .spacing(8)
    .padding(12);
    if let Some(status) = &operation {
        content = content.push(operation_buttons(view, status));
    }
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

/// Buttons below the commit button while an operation runs (M12-05):
/// "Abort merge" (Skip for rebase and cherry-pick); for a stash "Undo",
/// "Drop stash" and "Keep stash" (M9); for orphaned conflicts "Discard
/// conflicted files…" (ADR-044).
fn operation_buttons<'a>(
    view: &'a RepoView,
    status: &crate::operation::OperationStatus,
) -> Element<'a, Message> {
    use crate::operation::OpKind;
    use crate::repo_view::ConflictBanner;
    let idle = view.write_block.is_none() && view.busy.is_none();
    let action = |label: String,
                  style: fn(&iced::Theme, button::Status) -> button::Style,
                  message: Option<Message>|
     -> Element<'a, Message> {
        let button = button(text(label).size(12).width(Fill).align_x(iced::Center))
            .width(Fill)
            .padding([5, 8])
            .style(style);
        if view.write_block.is_some() {
            return write_tooltip(button.into(), view);
        }
        button.on_press_maybe(message.filter(|_| idle)).into()
    };
    let mut buttons = row![].spacing(6);
    match &status.kind {
        OpKind::Git(operation) => {
            if operation.can_skip() && !status.finishing {
                buttons = buttons.push(action(
                    "Skip".into(),
                    button::secondary,
                    Some(Message::SkipOperation),
                ));
            }
            let abort = if status.finishing {
                Message::FlowFinishAbort
            } else {
                Message::AbortOperation
            };
            buttons = buttons.push(action(
                status.abort_label().unwrap_or_default(),
                button::danger,
                Some(abort),
            ));
        }
        OpKind::Stash(ConflictBanner::StashConflicts { undo, .. }) => {
            if *undo {
                buttons = buttons.push(action(
                    "Undo".into(),
                    button::secondary,
                    Some(Message::Undo),
                ));
            }
            buttons = buttons.push(with_tooltip_text(
                action("Drop stash".into(), button::secondary, None),
                "Resolve the conflicts first".into(),
            ));
        }
        OpKind::Stash(ConflictBanner::StashResolved { id, .. }) => {
            buttons = buttons
                .push(action(
                    "Drop stash".into(),
                    button::primary,
                    Some(Message::StashDropConfirmed(id.clone())),
                ))
                .push(action(
                    "Keep stash".into(),
                    button::secondary,
                    Some(Message::KeepStash),
                ));
        }
        OpKind::Stash(ConflictBanner::Orphaned) | OpKind::Orphaned => {
            buttons = buttons.push(action(
                "Discard conflicted files…".into(),
                button::danger,
                Some(Message::Discard(DiscardTarget::Conflicted)),
            ));
        }
    }
    buttons.into()
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

/// Small sparkle button at the right end of "Committing as …" (M12-13):
/// opens "Commit message prompt" (M10-07); disabled without staged
/// changes. Gitloom only copies the prompt. A muted 13 px icon like the
/// branch icons in the left panel, with the same look enabled and
/// disabled; the tooltip tells why it is disabled (maintainer request
/// 2026-11-12).
fn prompt_button(view: &RepoView) -> Element<'_, Message> {
    let (enabled, tip) = prompt_button_state(view);
    let icon = crate::icons::tinted(crate::icons::Icon::Sparkles, 13.0, |p| p.text_muted);
    with_tooltip_text(
        button(container(icon).center(Fill))
            .on_press_maybe(enabled.then_some(Message::CopyPromptRequested))
            .padding(0)
            .width(22)
            .height(22)
            .style(theme::flat)
            .into(),
        tip.to_owned(),
    )
}

/// Whether the prompt button is enabled, and its tooltip.
pub(crate) fn prompt_button_state(view: &RepoView) -> (bool, &'static str) {
    if view.snapshot.working_tree.staged.is_empty() {
        (false, "Stage changes first")
    } else {
        (true, "Copy commit message prompt…")
    }
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
    avatars: &'a crate::avatars::Avatars,
    files: crate::file_list::FileListOptions,
) -> Element<'a, Message> {
    // The message box takes at most 40 % of the panel (M14-01).
    iced::widget::responsive(move |size| details_content(view, commit, avatars, files, size.height))
        .into()
}

/// "commit: <short sha>" centered like GitKraken; a click copies the full
/// sha (M14-01).
fn details_header(id: &str) -> Element<'_, Message> {
    let label = row![
        muted("commit:"),
        text(short(id)).font(iced::Font::MONOSPACE),
    ]
    .spacing(6);
    let copy = button(label)
        .on_press(Message::CopyToClipboard(id.to_owned()))
        .padding([2, 6])
        .style(theme::flat);
    container(with_tooltip_text(copy.into(), "Copy the full sha".into()))
        .center_x(Fill)
        .into()
}

/// Summary (larger) and description in a framed, scrollable box whose text
/// can be selected and copied (M14-01).
/// Focus target of the message editor's summary (M14-05).
pub const MESSAGE_EDIT_SUMMARY: &str = "message-edit-summary";

fn message_box<'a>(
    view: &'a RepoView,
    commit: &'a Commit,
    max_height: f32,
) -> Element<'a, Message> {
    if let Some(edit) = view.message_edit.as_ref().filter(|e| e.id == commit.id) {
        return message_editor(edit);
    }
    let plain = || -> Element<'a, Message> {
        column![text(&commit.summary).size(16)]
            .push((!commit.body.is_empty()).then(|| text(&commit.body)))
            .spacing(8)
            .into()
    };
    // Always the selectable text (M15-06): a click on it no longer opens
    // the editor, the pencil does.
    let inner: Element<'a, Message> = match view.message_box.as_ref().filter(|m| m.id == commit.id)
    {
        Some(message) => {
            let summary = text_editor(&message.summary)
                .size(16)
                .padding(0)
                .style(theme::message_text)
                .on_action(|action| Message::DetailsTextInput {
                    body: false,
                    action,
                });
            let body = (!commit.body.is_empty()).then(|| {
                text_editor(&message.body)
                    .padding(0)
                    .style(theme::message_text)
                    .on_action(|action| Message::DetailsTextInput { body: true, action })
            });
            column![summary].push(body).spacing(8).into()
        }
        None => plain(),
    };
    let framed: Element<'a, Message> =
        container(crate::ui::vscroll(container(inner).padding(10).width(Fill)))
            .max_height(max_height.max(60.0))
            .width(Fill)
            .style(theme::message_box)
            .into();
    match edit_icon(view, &commit.id) {
        Some(icon) => iced::widget::hover(
            framed,
            container(icon)
                .width(Fill)
                .align_x(iced::Right)
                .padding(iced::Padding::ZERO.top(4).right(6)),
        ),
        None => framed,
    }
}

/// The edit state of the details' commit `id` as the pencil shows it
/// (M15-06): `None` without a pencil (stashes, still checking),
/// `Some(Ok(()))` enabled, `Some(Err(reason))` muted with the reason.
pub(crate) fn edit_icon_state(view: &RepoView, id: &str) -> Option<Result<(), String>> {
    if view.snapshot.is_stash(id) {
        return None;
    }
    match view.message_edit_block(id)? {
        Some(reason) => Some(Err(reason)),
        None => match &view.busy {
            Some(busy) => Some(Err(format!("Busy: {busy}"))),
            None => Some(Ok(())),
        },
    }
}

/// Pencil at the top right of the message box, shown while the box is
/// hovered (M15-06); the tooltip is on the icon only.
fn edit_icon<'a>(view: &RepoView, id: &str) -> Option<Element<'a, Message>> {
    let state = edit_icon_state(view, id)?;
    let tint: fn(&theme::ThemePalette) -> iced::Color = if state.is_ok() {
        |p| p.text
    } else {
        |p| p.text_muted
    };
    let icon = crate::icons::tinted(crate::icons::Icon::Pencil, 14.0, tint);
    let button = button(container(icon).center(Fill))
        .on_press_maybe(state.is_ok().then_some(Message::CommitEditStart))
        .padding(0)
        .width(22)
        .height(22)
        .style(theme::flat);
    let tip = state
        .err()
        .unwrap_or_else(|| "Edit the commit message".to_owned());
    Some(with_tooltip_text(button.into(), tip))
}

/// Inline message editor (M14-05): summary with the commit panel's
/// counter, description, the note about other refs, Save and Cancel.
fn message_editor(edit: &crate::repo_view::MessageEdit) -> Element<'_, Message> {
    let remaining = SUMMARY_LIMIT as i64 - edit.summary.chars().count() as i64;
    let summary = text_input("Summary", &edit.summary)
        .id(MESSAGE_EDIT_SUMMARY)
        .padding(iced::Padding::new(6.0).right(COUNTER_ROOM))
        .style(theme::input)
        .on_input(Message::CommitEditSummary)
        .on_submit(Message::CommitEditSave);
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
    // Prompt for this commit at the right of the summary row (M15-06), like
    // the sparkle of "Committing as …".
    let sparkle = with_tooltip_text(
        button(
            container(crate::icons::tinted(
                crate::icons::Icon::Sparkles,
                13.0,
                |p| p.text_muted,
            ))
            .center(Fill),
        )
        .on_press(Message::CommitPromptRequested(edit.id.clone()))
        .padding(0)
        .width(22)
        .height(22)
        .style(theme::flat)
        .into(),
        "Copy commit message prompt…".to_owned(),
    );
    let summary = row![summary, sparkle].spacing(4).align_y(iced::Center);
    let body = text_editor(&edit.body)
        .placeholder("Description")
        .padding(6)
        .style(theme::editor)
        .height(120)
        .on_action(Message::CommitEditBody)
        // Ctrl+Enter saves instead of inserting a line break.
        .key_binding(|press| {
            let enter = matches!(
                press.key.as_ref(),
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
            );
            if enter && press.modifiers.command() {
                return Some(text_editor::Binding::Custom(Message::CommitEditSave));
            }
            text_editor::Binding::from_key_press(press)
        });
    let mut notes = column![].spacing(2);
    if let Loadable::Ready(facts) = &edit.facts {
        for name in &facts.keep_old {
            notes = notes.push(muted(format!("{name} keeps the old commit")).size(12));
        }
        if facts.drops_signature {
            notes = notes.push(
                muted("Signatures of the rewritten commits are removed (signing is off)").size(12),
            );
        }
    }
    let error = edit
        .error
        .as_ref()
        .map(|e| text(e.clone()).size(12).style(theme::text_danger));
    let buttons = row![
        space::horizontal(),
        button(text("Cancel").size(12))
            .padding([4, 10])
            .style(button::secondary)
            .on_press(Message::CommitEditCancel),
        button(text("Save").size(12))
            .padding([4, 10])
            .style(button::primary)
            .on_press(Message::CommitEditSave),
    ]
    .spacing(8);
    container(
        column![summary, body]
            .push(error)
            .push(notes)
            .push(buttons)
            .spacing(8),
    )
    .padding(10)
    .width(Fill)
    .style(theme::message_box)
    .into()
}

fn details_content<'a>(
    view: &'a RepoView,
    commit: &'a Commit,
    avatars: &crate::avatars::Avatars,
    options: crate::file_list::FileListOptions,
    height: f32,
) -> Element<'a, Message> {
    let author = author_block(view, commit, avatars);

    let files: Element<'a, Message> = match view.details.as_ref().map(|d| &d.changes) {
        Some(Loadable::Ready(DetailChanges::Commit(changes))) => {
            commit_files(view, &commit.id, changes, options)
        }
        Some(Loadable::Ready(DetailChanges::Stash(changes))) => {
            stash_files(view, &commit.id, changes)
        }
        Some(Loadable::Failed(error)) => text(error).style(theme::text_danger).into(),
        Some(Loadable::Loading) | None => muted("Loading changed files…").into(),
    };

    let content = column![
        details_header(&commit.id),
        message_box(view, commit, height * 0.4)
    ]
    .push(author)
    .push(space::vertical().height(4))
    .push(files)
    .spacing(8)
    .padding(12);

    crate::ui::vscroll(content).height(Fill).into()
}

/// Author block like GitKraken's (M14-02): 40 px avatar, name, "authored
/// <date>", "committed by" when the committer differs, the signature, the
/// parents as links at the right and the co-authors below.
fn author_block<'a>(
    view: &'a RepoView,
    commit: &'a Commit,
    avatars: &crate::avatars::Avatars,
) -> Element<'a, Message> {
    let name: Element<'a, Message> = if commit.email.is_empty() {
        text(&commit.author).into()
    } else {
        with_tooltip_text(text(&commit.author).into(), commit.email.clone())
    };
    let mut lines = column![
        name,
        muted(format!(
            "authored {}",
            format::date_time(commit.author_time)
        ))
        .size(12),
    ]
    .spacing(2);
    if !commit.committer.is_empty() {
        lines = lines.push(
            muted(format!(
                "committed by {} {}",
                commit.committer,
                format::date_time(commit.commit_time)
            ))
            .size(12),
        );
    }
    if let Some(badge) = signature_badge(view, &commit.id) {
        lines = lines.push(badge);
    }
    let parents = Column::with_children(
        commit
            .parents
            .iter()
            .map(|parent| parent_link(view, parent)),
    )
    .spacing(2)
    .align_x(iced::Right);
    let top = row![
        crate::ui::avatar(avatars.get(&commit.email), &commit.author, 40.0),
        lines.width(Fill),
        parents,
    ]
    .spacing(10)
    .align_y(iced::Center);

    let co_authors = crate::trailers::co_authors(&commit.body);
    if co_authors.is_empty() {
        return top.into();
    }
    let faces = co_authors.into_iter().map(|author| {
        with_tooltip_text(
            crate::ui::avatar(avatars.get(&author.email), &author.name, 20.0),
            format!("{} <{}>", author.name, author.email),
        )
    });
    column![
        top,
        row![muted("Co-authors:").size(12)]
            .extend(faces)
            .spacing(6)
            .align_y(iced::Center),
    ]
    .spacing(8)
    .into()
}

/// "parent: <short sha>": selects the parent and scrolls to it (M14-02);
/// disabled when the parent is beyond the loaded history.
fn parent_link<'a>(view: &RepoView, parent: &'a str) -> Element<'a, Message> {
    let label = row![
        muted("parent:").size(12),
        text(short(parent)).font(iced::Font::MONOSPACE).size(12),
    ]
    .spacing(4);
    let loaded = view.snapshot.commit(parent).is_some();
    let link = button(label)
        .on_press_maybe(loaded.then(|| Message::ParentClicked(parent.to_owned())))
        .padding([1, 4])
        .style(theme::flat);
    if loaded {
        link.into()
    } else {
        with_tooltip_text(link.into(), "Not in the loaded history".into())
    }
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
    let label = match &file.old_path {
        Some(old) => format!("{old} → {}", file.path),
        None => file.path.clone(),
    };
    file_row_labeled(view, file, kind, label, 0)
}

/// [`file_row`] with its own label (tree mode: the last segment) and
/// indentation level (M14-03).
fn file_row_labeled<'a>(
    view: &'a RepoView,
    file: &'a FileChange,
    kind: DiffKind,
    label: String,
    depth: usize,
) -> Element<'a, Message> {
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

    let area = match &request.kind {
        DiffKind::Staged => FileArea::Staged,
        DiffKind::Unstaged | DiffKind::Untracked => FileArea::Unstaged,
        DiffKind::Commit(_) | DiffKind::Stash { .. } => FileArea::Commit,
    };
    let row = button(
        row![
            space::horizontal().width(tree_indent(depth)),
            change_badge(file.kind),
            text(label).wrapping(text::Wrapping::None),
        ]
        .align_y(iced::Center),
    )
    // A click on the selected file deselects it and closes its diff or
    // conflict view, like Esc (M12-01).
    .on_press(if selected {
        Message::DiffClosed
    } else if file.kind == git_core::ChangeKind::Conflicted {
        Message::ConflictOpened(file.path.clone())
    } else {
        Message::DiffOpened(request)
    })
    .padding([3, 12])
    .width(Fill)
    .clip(true)
    .style(theme::list_item(selected));

    // The stage/unstage button appears while hovering the row; Discard is
    // in the context menu only (M12-01, maintainer decision).
    let quick_actions = match area {
        FileArea::Unstaged => Some(row![button(text("Stage File").size(11))
            .on_press(Message::Stage(file.clone()))
            .padding([2, 8])
            .style(button::secondary)]),
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

/// Indentation of a tree row at `depth` (M14-03).
fn tree_indent(depth: usize) -> f32 {
    depth as f32 * 14.0
}

fn small_button(label: &str, message: Message) -> button::Button<'_, Message> {
    button(text(label).size(12))
        .padding([2, 8])
        .style(theme::flat)
        .on_press(message)
}

/// File list of a commit (M14-03): summary per kind, toolbar (sort,
/// Path/Tree, "View all files", expand/collapse) and the rows.
fn commit_files<'a>(
    view: &'a RepoView,
    id: &'a str,
    changes: &'a [FileChange],
    options: crate::file_list::FileListOptions,
) -> Element<'a, Message> {
    use crate::branch_tree::Row;
    use crate::file_list::Entry;
    let summary: Element<'a, Message> = if changes.is_empty() {
        muted(crate::file_list::summary(changes)).size(12).into()
    } else {
        row(crate::file_list::counts(changes)
            .into_iter()
            .map(|(kind, count)| {
                row![
                    crate::icons::change_icon(kind, 14.0),
                    text(crate::file_list::count_label(kind, count)).size(12),
                ]
                .spacing(4)
                .align_y(iced::Center)
                .into()
            }))
        .spacing(12)
        .into()
    };

    let all = match view.details.as_ref().and_then(|d| d.all_files.as_ref()) {
        Some(Loadable::Ready(files)) if options.all_files => Some(files.as_slice()),
        _ => None,
    };
    let entries = crate::file_list::entries(changes, all, options.descending);
    let nodes = options
        .tree
        .then(|| crate::file_list::tree(&entries, options.descending));

    let segment = |label: &'a str, active: bool, tree: bool| {
        button(text(label).size(12))
            .padding([2, 8])
            .style(if active {
                theme::mode_active
            } else {
                theme::flat
            })
            .on_press(Message::FileListTreeChosen(tree))
    };
    let sort = with_tooltip_text(
        small_button(
            if options.descending { "Z→A" } else { "A→Z" },
            Message::FileListSortToggled,
        )
        .into(),
        "Sort by path".into(),
    );
    let mut toolbar = row![
        sort,
        row![
            segment("Path", !options.tree, false),
            segment("Tree", options.tree, true)
        ],
        checkbox(options.all_files)
            .label("View all files")
            .size(14)
            .text_size(12)
            .on_toggle(Message::FileListAllFilesToggled),
    ]
    .spacing(8)
    .align_y(iced::Center);
    if let Some(nodes) = &nodes {
        let folders = crate::file_list::folder_paths(nodes);
        toolbar = toolbar.extend([
            small_button(
                "Expand all",
                Message::FileListExpandAll {
                    expand: true,
                    folders: Vec::new(),
                },
            )
            .into(),
            small_button(
                "Collapse all",
                Message::FileListExpandAll {
                    expand: false,
                    folders,
                },
            )
            .into(),
        ]);
    }

    let entry_row = |entry: Entry<'a>, label: String, depth: usize| -> Element<'a, Message> {
        match entry {
            Entry::Changed(file) => {
                file_row_labeled(view, file, DiffKind::Commit(id.to_owned()), label, depth)
            }
            // Unchanged: muted; a click shows the file's content at the
            // commit (blame view).
            Entry::Unchanged(path) => button(
                row![
                    space::horizontal().width(tree_indent(depth) + 20.0),
                    muted(label).wrapping(text::Wrapping::None),
                ]
                .align_y(iced::Center),
            )
            .on_press(Message::FileBlameOpened {
                path: path.to_owned(),
                start: Some(id.to_owned()),
                rev: Some(id.to_owned()),
            })
            .padding([3, 12])
            .width(Fill)
            .clip(true)
            .style(theme::list_item(false))
            .into(),
        }
    };
    let rows: Vec<Element<'a, Message>> = match &nodes {
        None => entries
            .iter()
            .map(|entry| {
                let label = match entry {
                    Entry::Changed(file) => match &file.old_path {
                        Some(old) => format!("{old} → {}", file.path),
                        None => file.path.clone(),
                    },
                    Entry::Unchanged(path) => (*path).to_owned(),
                };
                entry_row(*entry, label, 0)
            })
            .collect(),
        Some(nodes) => crate::branch_tree::rows(
            nodes,
            &|path| view.file_tree_collapsed.contains(path),
            false,
        )
        .into_iter()
        .map(|item| match item {
            Row::Folder {
                depth,
                name,
                path,
                expanded,
            } => {
                let (chevron, folder) = if expanded {
                    (
                        crate::icons::Icon::ChevronDown,
                        crate::icons::Icon::FolderOpen,
                    )
                } else {
                    (crate::icons::Icon::ChevronRight, crate::icons::Icon::Folder)
                };
                button(
                    row![
                        space::horizontal().width(tree_indent(depth)),
                        crate::icons::icon(chevron, 12.0),
                        crate::icons::icon(folder, 14.0),
                        text(name.to_owned()).wrapping(text::Wrapping::None),
                    ]
                    .spacing(4)
                    .align_y(iced::Center),
                )
                .on_press(Message::FileListFolderToggled(path.to_owned()))
                .padding([3, 12])
                .width(Fill)
                .clip(true)
                .style(theme::list_item(false))
                .into()
            }
            Row::Branch {
                depth,
                index,
                label,
            } => match entries.get(index) {
                Some(entry) => entry_row(*entry, label.to_owned(), depth),
                None => space::vertical().height(0).into(),
            },
        })
        .collect(),
    };
    let loading: Option<Element<'a, Message>> =
        match view.details.as_ref().and_then(|d| d.all_files.as_ref()) {
            Some(Loadable::Loading) if options.all_files => {
                Some(muted("Loading all files…").size(12).into())
            }
            Some(Loadable::Failed(error)) if options.all_files => Some(
                text(error.clone())
                    .size(12)
                    .style(theme::text_danger)
                    .into(),
            ),
            _ => None,
        };
    column![
        muted(changed_files(changes.len())).size(12),
        summary,
        toolbar.wrap().vertical_spacing(4),
    ]
    .push(loading)
    .push(Column::with_children(rows))
    .spacing(8)
    .into()
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
