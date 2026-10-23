//! Left sidebar: local/remote branches, tags, stashes, submodules.

use std::collections::BTreeMap;

use git_core::{RefKind, Reference};
use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Element, Fill};

use crate::app::Message;
use crate::repo_view::{RepoView, Section};
use crate::theme;
use crate::ui::muted;

const INDENT: f32 = 14.0;

pub fn view(repo: &RepoView) -> Element<'_, Message> {
    let needle = repo.filter.to_lowercase();
    let matches = |name: &str| needle.is_empty() || name.to_lowercase().contains(&needle);
    let refs_of = |kind: RefKind| {
        repo.snapshot
            .refs
            .iter()
            .filter(move |r| r.kind == kind)
            .filter(|r| matches(&r.name))
    };

    let local: Vec<_> = refs_of(RefKind::LocalBranch)
        .map(|r| match &repo.rename {
            Some(rename) if rename.old == r.name => rename_input(rename),
            _ => ref_item(r, &r.name, 1),
        })
        .collect();

    let mut by_remote: BTreeMap<&str, Vec<&Reference>> = BTreeMap::new();
    for r in refs_of(RefKind::RemoteBranch) {
        by_remote
            .entry(r.remote().unwrap_or("?"))
            .or_default()
            .push(r);
    }
    let remote_count = by_remote.values().map(Vec::len).sum();
    let remote: Vec<_> = by_remote
        .into_iter()
        .flat_map(|(name, refs)| {
            std::iter::once(group_label(name))
                .chain(refs.into_iter().map(|r| ref_item(r, r.local_name(), 2)))
        })
        .collect();

    let tags: Vec<_> = refs_of(RefKind::Tag)
        .map(|r| ref_item(r, &r.name, 1))
        .collect();

    // Click selects the stash row in the graph; right click opens its menu.
    let stashes: Vec<_> = repo
        .snapshot
        .stashes
        .iter()
        .filter(|s| matches(&s.message))
        .map(|s| {
            let item = list_button(
                text(&s.message).wrapping(text::Wrapping::None).into(),
                1,
                Message::CommitSelected(s.id.clone()),
            );
            let id = s.id.clone();
            crate::ui::context_area::context_area(item, move |position| Message::StashContextMenu {
                id: id.clone(),
                position,
            })
        })
        .collect();

    let sections = column![
        section(repo, Section::Local, "LOCAL", local.len(), local),
        section(repo, Section::Remote, "REMOTE", remote_count, remote),
        section(repo, Section::Tags, "TAGS", tags.len(), tags),
        section(repo, Section::Stashes, "STASHES", stashes.len(), stashes),
        submodules(),
    ]
    .spacing(6);

    let filter = text_input("Filter", &repo.filter)
        .id(crate::app::FILTER_INPUT)
        .on_input(Message::FilterChanged)
        .padding([4, 8]);

    let content = column![
        container(filter).padding(8),
        scrollable(container(sections).padding([0, 4])).height(Fill),
    ];

    container(content)
        .width(Fill)
        .height(Fill)
        .style(theme::panel)
        .into()
}

fn section<'a>(
    repo: &RepoView,
    section: Section,
    title: &'a str,
    count: usize,
    items: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let collapsed = repo.collapsed.contains(&section);
    let arrow = if collapsed { "▸" } else { "▾" };

    let header = button(
        row![
            text(arrow).width(INDENT),
            text(title).size(11),
            space::horizontal(),
            text(count.to_string()).size(11),
        ]
        .align_y(iced::Center),
    )
    .on_press(Message::SectionToggled(section))
    .padding([4, 6])
    .width(Fill)
    .style(theme::section_header);

    if collapsed {
        header.into()
    } else {
        column![header].extend(items).into()
    }
}

/// Submodules are listed from M2 on; the header stays visible but inactive.
fn submodules<'a>() -> Element<'a, Message> {
    crate::ui::panel::with_tooltip(
        container(
            row![
                text("▸").width(INDENT),
                text("SUBMODULES").size(11),
                space::horizontal(),
            ]
            .align_y(iced::Center),
        )
        .padding([4, 6])
        .width(Fill)
        .style(|_| container::Style {
            text_color: Some(theme::TEXT_MUTED.scale_alpha(0.6)),
            ..container::Style::default()
        })
        .into(),
    )
}

fn ref_item<'a>(reference: &'a Reference, label: &'a str, depth: u16) -> Element<'a, Message> {
    let marker = if reference.is_head { "✓" } else { "" };
    let mut content = row![
        text(marker).width(INDENT).color(theme::ACCENT),
        text(label).wrapping(text::Wrapping::None),
        space::horizontal(),
    ]
    .align_y(iced::Center);
    // Like GitKraken: commits ahead/behind the upstream as small badges.
    if let Some(badge) = ahead_behind_badge(reference.ahead_behind) {
        content = content.push(muted(badge).size(11));
    }
    let content = content.into();

    let item = list_button(content, depth, Message::RefClicked(reference.name.clone()));
    let item = match checkout_target(reference) {
        Some(target) => crate::ui::context_area::double_click_area(item, Message::Checkout(target)),
        None => item,
    };
    let (name, kind) = (reference.name.clone(), reference.kind);
    crate::ui::context_area::context_area(item, move |position| Message::BranchContextMenu {
        name: name.clone(),
        kind,
        position,
    })
}

/// Inline rename input in place of the branch entry; red border on an error.
fn rename_input(rename: &crate::repo_view::RenameBranch) -> Element<'_, Message> {
    let field = text_input("Branch name", &rename.name)
        .id(crate::ui::RENAME_INPUT)
        .on_input(Message::RenameInput)
        .on_submit(Message::RenameSubmit)
        .size(13)
        .padding([2, 4])
        .style(theme::inline_input(rename.error.is_some()));
    let field = container(field).padding(iced::Padding::from([1, 6]).left(6.0 + INDENT));
    match &rename.error {
        Some(error) => column![field, text(error.clone()).size(11).color(theme::DANGER)]
            .spacing(2)
            .into(),
        None => field.into(),
    }
}

/// Double click checks out branches (remote ones as tracking branch).
fn checkout_target(reference: &Reference) -> Option<crate::app::CheckoutTarget> {
    match reference.kind {
        git_core::RefKind::LocalBranch => {
            Some(crate::app::CheckoutTarget::Local(reference.name.clone()))
        }
        git_core::RefKind::RemoteBranch => {
            Some(crate::app::CheckoutTarget::Remote(reference.name.clone()))
        }
        git_core::RefKind::Tag => None,
    }
}

/// `↑2 ↓1` style badge; nothing when in sync or without upstream.
pub fn ahead_behind_badge(ahead_behind: Option<(usize, usize)>) -> Option<String> {
    let (ahead, behind) = ahead_behind?;
    let parts: Vec<String> = [(ahead, '↑'), (behind, '↓')]
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, arrow)| format!("{arrow}{n}"))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn group_label<'a>(name: &'a str) -> Element<'a, Message> {
    container(muted(name))
        .padding(iced::Padding::from([3, 6]).left(6.0 + INDENT))
        .into()
}

fn list_button<'a>(
    content: Element<'a, Message>,
    depth: u16,
    on_press: Message,
) -> Element<'a, Message> {
    button(content)
        .on_press(on_press)
        .padding(iced::Padding::from([3, 6]).left(6.0 + INDENT * f32::from(depth)))
        .width(Fill)
        .clip(true)
        .style(theme::list_item(false))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ahead_behind_badges() {
        assert_eq!(ahead_behind_badge(None), None);
        assert_eq!(ahead_behind_badge(Some((0, 0))), None);
        assert_eq!(ahead_behind_badge(Some((2, 0))).as_deref(), Some("↑2"));
        assert_eq!(ahead_behind_badge(Some((0, 5))).as_deref(), Some("↓5"));
        assert_eq!(ahead_behind_badge(Some((3, 1))).as_deref(), Some("↑3 ↓1"));
    }
}
