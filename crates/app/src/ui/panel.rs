//! Right panel: staging area (WIP selected) or commit details.

use git_core::{Commit, DiffKind, DiffRequest, FileChange};
use iced::widget::{
    button, column, container, row, scrollable, space, text, text_editor, text_input, tooltip,
    Column,
};
use iced::{Element, Fill, FillPortion};

use crate::app::Message;
use crate::format;
use crate::repo_view::{CenterView, Loadable, RepoView, Selection};
use crate::theme;
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
        disabled_button("Stage all changes", button::success),
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
        disabled_button("Unstage all changes", button::danger),
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
    let summary = text_input("Summary", "").padding(6);
    let description = text_editor(&view.description)
        .placeholder("Description")
        .padding(6)
        .height(90);
    let commit = button(text("Commit changes").width(Fill).align_x(iced::Center))
        .width(Fill)
        .padding(8)
        .style(button::primary);

    column![
        muted("Commit"),
        summary,
        description,
        with_tooltip(commit.into()),
        muted("Committing is available from version 0.3.").size(11),
    ]
    .spacing(8)
    .padding(12)
    .into()
}

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
    let header = row![text(title), space::horizontal(), action]
        .padding([6, 12])
        .align_y(iced::Center);

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

    button(
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
    .style(theme::list_item(selected))
    .into()
}

fn disabled_button<'a>(
    label: &'a str,
    style: fn(&iced::Theme, button::Status) -> button::Style,
) -> Element<'a, Message> {
    with_tooltip(
        button(text(label).size(12))
            .padding([3, 8])
            .style(style)
            .into(),
    )
}

/// Wraps a disabled control in the standard "Not available yet" tooltip.
pub(crate) fn with_tooltip(content: Element<'_, Message>) -> Element<'_, Message> {
    tooltip(
        content,
        container(text(NOT_AVAILABLE).size(12))
            .padding([4, 8])
            .style(theme::tooltip),
        tooltip::Position::Bottom,
    )
    .into()
}
