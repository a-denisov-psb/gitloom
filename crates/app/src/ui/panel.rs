//! Right panel: staging area (WIP selected) or commit details.

use git_core::{Commit, FileChange};
use iced::widget::{
    button, column, container, row, scrollable, space, text, text_editor, text_input, Column,
};
use iced::{Element, Fill, FillPortion};

use crate::app::{CenterView, DiffSource, DiffTarget, Message, Repo, Selection};
use crate::format;
use crate::theme;
use crate::ui::{change_badge, muted};

const SUMMARY_LIMIT: usize = 72;

pub fn view(repo: &Repo) -> Element<'_, Message> {
    let content = match repo.selection {
        Selection::Wip => staging(repo),
        Selection::Commit(index) => match repo.commits.get(index) {
            Some(commit) => details(repo, index, commit),
            None => muted("Nothing selected").into(),
        },
    };

    container(content)
        .width(Fill)
        .height(Fill)
        .style(theme::panel)
        .into()
}

fn staging(repo: &Repo) -> Element<'_, Message> {
    let tree = &repo.working_tree;
    let branch = repo.head().map_or("(detached)", |r| r.name.as_str());

    let header =
        container(text(format!("{} file changes on {branch}", tree.len()))).padding([10, 12]);

    let unstaged = file_section(
        format!("Unstaged Files ({})", tree.unstaged.len()),
        button(text("Stage all changes").size(12))
            .on_press_maybe((!tree.unstaged.is_empty()).then_some(Message::StageAll))
            .padding([3, 8])
            .style(button::success),
        tree.unstaged.iter().map(|file| {
            file_row(
                repo,
                file,
                DiffSource::Unstaged,
                Some(("Stage", Message::Stage(file.path.clone()))),
            )
        }),
    );

    let staged = file_section(
        format!("Staged Files ({})", tree.staged.len()),
        button(text("Unstage all changes").size(12))
            .on_press_maybe((!tree.staged.is_empty()).then_some(Message::UnstageAll))
            .padding([3, 8])
            .style(button::danger),
        tree.staged.iter().map(|file| {
            file_row(
                repo,
                file,
                DiffSource::Staged,
                Some(("Unstage", Message::Unstage(file.path.clone()))),
            )
        }),
    );

    column![header, unstaged, staged, commit_box(repo)].into()
}

fn commit_box(repo: &Repo) -> Element<'_, Message> {
    let staged = repo.working_tree.staged.len();
    let remaining = SUMMARY_LIMIT as i64 - repo.summary.chars().count() as i64;
    let can_commit = staged > 0 && !repo.summary.trim().is_empty();

    let label = match staged {
        0 => "Stage files to commit".to_string(),
        1 => "Commit changes to 1 file".to_string(),
        n => format!("Commit changes to {n} files"),
    };

    let summary = row![
        text_input("Summary", &repo.summary)
            .on_input(Message::SummaryChanged)
            .padding(6),
        muted(remaining.to_string())
            .size(11)
            .width(28)
            .align_x(iced::Right),
    ]
    .spacing(6)
    .align_y(iced::Center);

    let description = text_editor(&repo.description)
        .placeholder("Description")
        .on_action(Message::DescriptionEdited)
        .padding(6)
        .height(90);

    let commit = button(text(label).width(Fill).align_x(iced::Center))
        .on_press_maybe(can_commit.then_some(Message::CommitRequested))
        .width(Fill)
        .padding(8)
        .style(button::primary);

    column![muted("Commit"), summary, description, commit]
        .spacing(8)
        .padding(12)
        .into()
}

fn details<'a>(repo: &'a Repo, index: usize, commit: &'a Commit) -> Element<'a, Message> {
    let id = row![
        muted("commit:"),
        text(commit.short_id()).font(iced::Font::MONOSPACE),
    ]
    .spacing(6);

    let body = (!commit.body.is_empty()).then(|| text(&commit.body));

    let author = column![
        text(&commit.author),
        muted(format!(
            "{} · authored {}",
            commit.email,
            format::date_time(commit.time)
        ))
        .size(12),
    ]
    .spacing(2);

    let parents = commit.parents.iter().map(|p| {
        muted(format!("parent: {}", &p[..p.len().min(7)]))
            .font(iced::Font::MONOSPACE)
            .size(12)
            .into()
    });

    let files = repo
        .commit_changes
        .iter()
        .map(|file| file_row(repo, file, DiffSource::Commit(index), None));

    let content = column![id, text(&commit.summary).size(16),]
        .push(body)
        .push(author)
        .push(row(parents).spacing(12))
        .push(space::vertical().height(4))
        .push(muted(format!(
            "{} changed files",
            repo.commit_changes.len()
        )))
        .push(Column::with_children(files))
        .spacing(8)
        .padding(12);

    scrollable(content).height(Fill).into()
}

fn file_section<'a>(
    title: String,
    action: impl Into<Element<'a, Message>>,
    files: impl Iterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    let header = row![text(title), space::horizontal(), action.into()]
        .padding([6, 12])
        .align_y(iced::Center);

    column![
        header,
        scrollable(Column::with_children(files)).height(Fill)
    ]
    .height(FillPortion(1))
    .into()
}

fn file_row<'a>(
    repo: &'a Repo,
    file: &'a FileChange,
    source: DiffSource,
    quick_action: Option<(&'a str, Message)>,
) -> Element<'a, Message> {
    let target = DiffTarget {
        path: file.path.clone(),
        source,
    };
    let selected = repo.center == CenterView::Diff(target.clone());

    let open = button(
        row![
            change_badge(file.kind),
            text(&file.path).wrapping(text::Wrapping::None),
        ]
        .align_y(iced::Center),
    )
    .on_press(Message::DiffOpened(target))
    .padding([3, 12])
    .width(Fill)
    .clip(true)
    .style(theme::list_item(selected));

    // TODO: show the quick action only on hover, like the original.
    let action = quick_action.map(|(label, message)| {
        button(text(label).size(11))
            .on_press(message)
            .padding([2, 6])
            .style(theme::flat)
    });

    row![open].push(action).align_y(iced::Center).into()
}
