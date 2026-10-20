//! Left sidebar: local/remote branches, tags, stashes, submodules.

use std::collections::BTreeMap;

use git_core::{RefKind, Reference};
use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Element, Fill};

use crate::app::{Message, Repo, Section};
use crate::theme;
use crate::ui::muted;

const INDENT: f32 = 14.0;

pub fn view(repo: &Repo) -> Element<'_, Message> {
    let needle = repo.filter.to_lowercase();
    let matches = |name: &str| needle.is_empty() || name.to_lowercase().contains(&needle);
    let refs_of = |kind: RefKind| {
        repo.refs
            .iter()
            .filter(move |r| r.kind == kind)
            .filter(|r| matches(&r.name))
    };

    let local: Vec<_> = refs_of(RefKind::LocalBranch)
        .map(|r| ref_item(r, &r.name, 1))
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

    let stashes: Vec<_> = repo
        .stashes
        .iter()
        .filter(|s| matches(&s.message))
        .map(|s| list_button(text(&s.message).into(), 1, Message::StashClicked(s.index)))
        .collect();

    let sections = column![
        section(repo, Section::Local, "LOCAL", local.len(), local),
        section(repo, Section::Remote, "REMOTE", remote_count, remote),
        section(repo, Section::Tags, "TAGS", tags.len(), tags),
        section(repo, Section::Stashes, "STASHES", stashes.len(), stashes),
        section(repo, Section::Submodules, "SUBMODULES", 0, Vec::new()),
    ]
    .spacing(6);

    let filter = text_input("Filter", &repo.filter)
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
    repo: &Repo,
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

fn ref_item<'a>(reference: &'a Reference, label: &'a str, depth: u16) -> Element<'a, Message> {
    let marker = if reference.is_head { "✓" } else { "" };
    let content = row![
        text(marker).width(INDENT).color(theme::ACCENT),
        text(label).wrapping(text::Wrapping::None),
    ]
    .into();

    list_button(content, depth, Message::RefClicked(reference.name.clone()))
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
