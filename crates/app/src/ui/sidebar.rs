//! Left sidebar: local/remote branches, tags, stashes, submodules.

use std::collections::BTreeMap;

use git_core::{RefKind, Reference};
use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Element, Fill};

use crate::app::Message;
use crate::icons::{self, Icon};
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
            _ => ref_item(repo, r, &r.name, 1),
        })
        .collect();

    let mut by_remote: BTreeMap<&str, Vec<&Reference>> = BTreeMap::new();
    // Remotes without fetched branches are listed too.
    for remote in &repo.snapshot.remotes {
        if matches(remote) || needle.is_empty() {
            by_remote.entry(remote.as_str()).or_default();
        }
    }
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
            std::iter::once(remote_label(name)).chain(
                refs.into_iter()
                    .map(|r| ref_item(repo, r, r.local_name(), 2)),
            )
        })
        .collect();

    let tags: Vec<_> = refs_of(RefKind::Tag)
        .map(|r| ref_item(repo, r, &r.name, 1))
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
        section_with_action(
            repo,
            Section::Remote,
            "REMOTE",
            remote_count,
            remote,
            Some((Icon::Plus, "Add remote", Message::AddRemoteStart)),
        ),
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
    section_with_action(repo, section, title, count, items, None)
}

/// Section with an optional small action button in the header (e.g. "+").
fn section_with_action<'a>(
    repo: &RepoView,
    section: Section,
    title: &'a str,
    count: usize,
    items: Vec<Element<'a, Message>>,
    action: Option<(Icon, &'a str, Message)>,
) -> Element<'a, Message> {
    let collapsed = repo.collapsed.contains(&section);
    let arrow = if collapsed {
        Icon::ChevronRight
    } else {
        Icon::ChevronDown
    };
    let kind = match section {
        Section::Local => Icon::Local,
        Section::Remote => Icon::Cloud,
        Section::Tags => Icon::Tag,
        Section::Stashes => Icon::Stash,
        Section::Submodules => Icon::Package,
    };

    let mut line = row![
        container(icons::tinted(arrow, 12.0, |p| p.text_muted)).width(INDENT),
        icons::tinted(kind, 13.0, |p| p.text_muted),
        text(title).size(11),
        space::horizontal(),
    ]
    .spacing(6)
    .align_y(iced::Center);
    if let Some((icon, tip, message)) = action {
        let writable = repo.write_block.is_none() && repo.busy.is_none();
        line = line.push(crate::ui::panel::with_tooltip_text(
            button(icons::button_icon(icon, 13.0, writable))
                .on_press_maybe(writable.then_some(message))
                .padding([1, 5])
                .style(theme::flat)
                .into(),
            tip.to_owned(),
        ));
    }
    line = line.push(text(count.to_string()).size(11));
    let header = button(line)
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
                container(icons::tinted(Icon::ChevronRight, 12.0, |p| p
                    .text_muted
                    .scale_alpha(0.6)))
                .width(INDENT),
                icons::tinted(Icon::Package, 13.0, |p| p.text_muted.scale_alpha(0.6)),
                text("SUBMODULES").size(11),
                space::horizontal(),
            ]
            .spacing(6)
            .align_y(iced::Center),
        )
        .padding([4, 6])
        .width(Fill)
        .style(theme::faded)
        .into(),
    )
}

fn ref_item<'a>(
    repo: &RepoView,
    reference: &'a Reference,
    label: &'a str,
    depth: u16,
) -> Element<'a, Message> {
    let marker: Element<'a, Message> = if reference.is_head {
        icons::tinted(Icon::Check, 13.0, |p| p.accent).into()
    } else {
        space::horizontal().width(0).into()
    };
    // Branches not shown in the graph are dimmed (M5-08).
    let filter = &repo.ref_filter;
    // The checked-out branch is always shown (HEAD).
    let dimmed = !reference.is_head && filter.dims(&reference.name, reference.kind);
    let label = text(label).wrapping(text::Wrapping::None);
    let label = if dimmed {
        label.style(theme::text_faint)
    } else {
        label
    };
    let mut content = row![container(marker).width(INDENT), label, space::horizontal(),]
        .spacing(4)
        .align_y(iced::Center);
    // Like GitKraken: commits ahead/behind the upstream as small badges.
    if let Some(badge) = ahead_behind_badge(reference.ahead_behind) {
        content = content.push(muted(badge).size(11));
    }
    if filter.is_soloed(&reference.name, reference.kind) {
        content = content.push(icons::tinted(Icon::Solo, 13.0, |p| p.accent));
    }
    let hidden = filter.is_hidden(&reference.name, reference.kind);
    if hidden {
        content = content.push(icons::tinted(Icon::EyeOff, 13.0, |p| p.text_muted));
    }
    let content = content.into();

    let item = list_button(content, depth, Message::RefClicked(reference.name.clone()));
    let item = match checkout_target(reference) {
        Some(target) => crate::ui::context_area::double_click_area(item, Message::Checkout(target)),
        None => item,
    };
    let (name, kind) = (reference.name.clone(), reference.kind);
    let item =
        crate::ui::context_area::context_area(item, move |position| Message::BranchContextMenu {
            name: name.clone(),
            kind,
            position,
        });
    // Branches can be dragged onto each other (M4-11); tags cannot.
    if reference.kind == RefKind::Tag {
        return item;
    }
    let highlight = repo
        .drag
        .as_ref()
        .is_some_and(|d| d.targets(&reference.name, reference.kind));
    let item = crate::ui::draggable_branch(item, &reference.name, reference.kind, highlight);
    // Eye on hover hides or shows the branch; the checked-out one stays.
    if reference.is_head {
        return item;
    }
    // The eye shows the current state (like GitKraken): open = shown.
    let (icon, tip) = if hidden {
        (Icon::EyeOff, "Show in the graph")
    } else {
        (Icon::Eye, "Hide in the graph")
    };
    let eye = crate::ui::panel::with_tooltip_text(
        button(icons::tinted(icon, 13.0, |p| p.text))
            .on_press(Message::ToggleHidden {
                name: reference.name.clone(),
                kind: reference.kind,
            })
            .padding([3, 6])
            .style(theme::flat)
            .into(),
        tip.to_owned(),
    );
    iced::widget::hover(
        item,
        row![space::horizontal(), eye]
            .padding(iced::Padding::default().right(4))
            .align_y(iced::Center),
    )
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
        Some(error) => column![
            field,
            text(error.clone()).size(11).style(theme::text_danger)
        ]
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

/// Remote name above its branches; right click opens the remote menu.
fn remote_label<'a>(name: &'a str) -> Element<'a, Message> {
    let label = container(muted(name))
        .padding(iced::Padding::from([3, 6]).left(6.0 + INDENT))
        .width(Fill);
    let name = name.to_owned();
    crate::ui::context_area::context_area(label, move |position| Message::RemoteContextMenu {
        name: name.clone(),
        position,
    })
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
