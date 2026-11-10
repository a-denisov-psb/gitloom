//! Left sidebar: local/remote branches, tags, stashes, submodules.

use git_core::{RefKind, Reference};
use iced::widget::{button, column, container, row, space, text, text_input};
use iced::{Element, Fill};

use crate::app::Message;
use crate::branch_tree::{folder_key, Row};
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

    // LOCAL and REMOTE as folder trees (M7-14), cached per snapshot,
    // text filter and hide/solo state.
    let groups = repo.sidebar_groups();
    let filtering = !needle.is_empty();
    let local: Vec<_> =
        crate::branch_tree::rows(&groups.local, &is_collapsed(repo, None), filtering)
            .into_iter()
            .map(|row| tree_row(repo, row, None, 1))
            .collect();

    let mut remote = Vec::new();
    for (name, nodes) in &groups.remotes {
        let key = folder_key(Some(name), "");
        let expanded = filtering || !repo.folders_collapsed.contains(&key);
        let external = repo.snapshot.external_remote(name);
        remote.push(remote_label(name.clone(), key, expanded, external));
        if expanded {
            let remote_name = Some(name.as_str());
            remote.extend(
                crate::branch_tree::rows(nodes, &is_collapsed(repo, remote_name), filtering)
                    .into_iter()
                    .map(|row| tree_row(repo, row, remote_name, 2)),
            );
        }
    }

    let tags: Vec<_> = refs_of(RefKind::Tag)
        .map(|r| ref_item(repo, r, r.name.clone(), 1))
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

    let hide_active = repo.ref_filter.is_active();
    let local = section(
        repo,
        Section::Local,
        "LOCAL",
        groups.local_count.label(hide_active),
        local,
    );
    // GITFLOW between LOCAL and REMOTE, only when initialized (maintainer
    // decision, M7-15).
    let sections = match &repo.snapshot.flow {
        Some(config) => column![local, gitflow(repo, config, &needle)],
        None => column![local],
    };
    let sections = sections
        .push(section_with_action(
            repo,
            Section::Remote,
            "REMOTE",
            groups.remote_count.label(hide_active),
            remote,
            Some((
                Icon::Plus,
                "Add remote",
                HeaderAction::Press(Box::new(Message::AddRemoteStart)),
            )),
        ))
        .push(section(
            repo,
            Section::Tags,
            "TAGS",
            tags.len().to_string(),
            tags,
        ))
        .push(section(
            repo,
            Section::Stashes,
            "STASHES",
            stashes.len().to_string(),
            stashes,
        ))
        .push(submodules())
        .spacing(6);

    let filter = text_input("Filter", &repo.filter)
        .id(crate::app::FILTER_INPUT)
        .on_input(Message::FilterChanged)
        .padding([4, 8])
        .style(theme::input);

    let mut content = column![];
    // Like GitKraken: while branches are hidden or soloed, a line above the
    // filter says how many are shown and offers to show all again.
    if hide_active {
        content = content.push(viewing_line(groups.all));
    }
    let content = content
        .push(container(filter).padding(8))
        .push(crate::ui::vscroll(container(sections).padding([0, 4])).height(Fill));

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
    count: String,
    items: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    section_with_action(repo, section, title, count, items, None)
}

/// Section with an optional small action button in the header (e.g. "+").
fn section_with_action<'a>(
    repo: &RepoView,
    section: Section,
    title: &'a str,
    count: String,
    items: Vec<Element<'a, Message>>,
    action: Option<(Icon, &'a str, HeaderAction)>,
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
        Section::Gitflow => Icon::Merge,
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
    if let Some((icon, tip, action)) = action {
        let writable = repo.write_block.is_none() && repo.busy.is_none();
        let control: Element<'a, Message> = match action {
            HeaderAction::Press(message) => button(icons::button_icon(icon, 13.0, writable))
                .on_press_maybe(writable.then_some(*message))
                .padding([1, 5])
                .style(theme::flat)
                .into(),
            // Green "+" opening a menu at the pointer (GITFLOW, M7-08).
            HeaderAction::Menu(menu) => {
                let plus = container(if writable {
                    icons::tinted(icon, 14.0, |p| p.success)
                } else {
                    icons::button_icon(icon, 14.0, false)
                })
                .padding([1, 5]);
                if writable {
                    crate::ui::context_area::click_area(
                        iced::widget::mouse_area(plus)
                            .interaction(iced::mouse::Interaction::Pointer),
                        menu,
                    )
                } else {
                    plus.into()
                }
            }
        };
        line = line.push(crate::ui::panel::with_tooltip_text(control, tip.to_owned()));
    }
    line = line.push(text(count).size(11));
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

/// Small control in a section header: a button, or a menu at the pointer.
enum HeaderAction {
    Press(Box<Message>),
    Menu(fn(iced::Point) -> Message),
}

/// GITFLOW section (M7-08): Features / Releases / Hotfixes with the branch
/// names without prefix; the same rows as LOCAL; a green "+" starts one.
fn gitflow<'a>(
    repo: &'a RepoView,
    config: &git_core::FlowConfig,
    needle: &str,
) -> Element<'a, Message> {
    let filtering = !needle.is_empty();
    let mut items = Vec::new();
    let mut count = 0;
    for (kind, title) in [
        (git_core::FlowKind::Feature, "Features"),
        (git_core::FlowKind::Release, "Releases"),
        (git_core::FlowKind::Hotfix, "Hotfixes"),
    ] {
        let branches: Vec<(&Reference, String)> = repo
            .snapshot
            .refs
            .iter()
            .filter(|r| r.kind == RefKind::LocalBranch)
            .filter(|r| needle.is_empty() || r.name.to_lowercase().contains(needle))
            .filter_map(|r| {
                config
                    .classify(&r.name)
                    .filter(|b| b.kind == kind)
                    .map(|b| (r, b.name))
            })
            .collect();
        count += branches.len();
        // Like GitKraken: a group is listed only when it has branches.
        if branches.is_empty() {
            continue;
        }
        let key = format!("gitflow:{}", kind.noun());
        let expanded = filtering || !repo.folders_collapsed.contains(&key);
        items.push(folder_row(
            format!("{title} ({})", branches.len()),
            key,
            expanded,
            1,
        ));
        if expanded {
            items.extend(
                branches
                    .into_iter()
                    .map(|(reference, name)| ref_item(repo, reference, name, 2)),
            );
        }
    }
    section_with_action(
        repo,
        Section::Gitflow,
        "GITFLOW",
        count.to_string(),
        items,
        Some((
            Icon::Plus,
            "Start a feature, release or hotfix",
            HeaderAction::Menu(Message::FlowStartMenu),
        )),
    )
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

/// Whether a folder of the local group (`remote` `None`) or of a remote is
/// collapsed.
fn is_collapsed<'a>(repo: &'a RepoView, remote: Option<&'a str>) -> impl Fn(&str) -> bool + 'a {
    move |path| repo.folders_collapsed.contains(&folder_key(remote, path))
}

/// "Viewing x/y · Show All" above the filter (M7-14).
fn viewing_line<'a>(all: crate::branch_tree::Counts) -> Element<'a, Message> {
    let show_all = button(
        text("Show All")
            .size(12)
            .style(|theme: &iced::Theme| text::Style {
                color: Some(theme::palette(theme).accent),
            }),
    )
    .on_press(Message::ShowAllBranches)
    .padding([1, 4])
    .style(theme::flat);
    container(
        row![
            muted(format!("Viewing {}/{}", all.visible, all.total)).size(12),
            muted("·").size(12),
            crate::ui::panel::with_tooltip_text(
                show_all.into(),
                "Show all hidden and soloed branches again".to_owned(),
            ),
        ]
        .spacing(4)
        .align_y(iced::Center),
    )
    .padding(iced::Padding::from([4, 8]).bottom(0))
    .into()
}

/// One row of a branch tree: a folder (toggles on click) or a branch.
/// `base` is the depth of the tree's top level.
fn tree_row<'a>(
    repo: &'a RepoView,
    row: Row<'_>,
    remote: Option<&str>,
    base: u16,
) -> Element<'a, Message> {
    let depth = |d: usize| base + u16::try_from(d).unwrap_or(u16::MAX - base);
    match row {
        Row::Folder {
            depth: d,
            name,
            path,
            expanded,
        } => folder_row(
            name.to_owned(),
            folder_key(remote, path),
            expanded,
            depth(d),
        ),
        Row::Branch {
            depth: d,
            index,
            label,
        } => {
            let Some(reference) = repo.snapshot.refs.get(index) else {
                return space::horizontal().width(0).into();
            };
            match &repo.rename {
                Some(rename)
                    if reference.kind == RefKind::LocalBranch && rename.old == reference.name =>
                {
                    rename_input(rename)
                }
                _ => ref_item(repo, reference, label.to_owned(), depth(d)),
            }
        }
    }
}

/// Branch folder: chevron, folder icon, name; a click toggles it.
fn folder_row<'a>(name: String, key: String, expanded: bool, depth: u16) -> Element<'a, Message> {
    let (chevron, folder) = if expanded {
        (Icon::ChevronDown, Icon::FolderOpen)
    } else {
        (Icon::ChevronRight, Icon::Folder)
    };
    let tip = name.clone();
    let content = row![
        container(icons::tinted(chevron, 12.0, |p| p.text_muted)).width(INDENT),
        icons::tinted(folder, 13.0, |p| p.text_muted),
        crate::ui::panel::with_tooltip_text(text(name).wrapping(text::Wrapping::None).into(), tip,),
        space::horizontal(),
    ]
    .spacing(4)
    .align_y(iced::Center);
    list_button(content.into(), depth, Message::FolderToggled(key))
}

fn ref_item<'a>(
    repo: &RepoView,
    reference: &'a Reference,
    label: String,
    depth: u16,
) -> Element<'a, Message> {
    // Branches carry a branch icon; the checked-out one a checkbox-style
    // check on a highlighted row (M7-14). Tags keep their plain layout.
    let marker: Element<'a, Message> = match reference.kind {
        _ if reference.is_head => icons::tinted(Icon::CheckSquare, 13.0, |p| p.accent).into(),
        RefKind::LocalBranch | RefKind::RemoteBranch => {
            icons::tinted(Icon::Branch, 13.0, |p| p.text_muted).into()
        }
        RefKind::Tag => space::horizontal().width(0).into(),
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
    // The full name, since the row shows only the last segment.
    let label: Element<'a, Message> = if reference.kind == RefKind::Tag {
        label.into()
    } else {
        crate::ui::panel::with_tooltip_text(label.into(), reference.name.clone())
    };
    let mut content = row![container(marker).width(INDENT), label, space::horizontal(),]
        .spacing(4)
        .align_y(iced::Center);
    // Like GitKraken: commits ahead/behind the upstream as small badges.
    if let Some(badge) = ahead_behind_badge(reference.ahead_behind) {
        // Muted text is not AA on the highlight of the checked-out row.
        let badge = if reference.is_head {
            text(badge).size(11)
        } else {
            muted(badge).size(11)
        };
        content = content.push(badge);
    }
    if filter.is_soloed(&reference.name, reference.kind) {
        content = content.push(icons::tinted(Icon::Solo, 13.0, |p| p.accent));
    }
    let hidden = filter.is_hidden(&reference.name, reference.kind);
    if hidden {
        content = content.push(icons::tinted(Icon::EyeOff, 13.0, |p| p.text_muted));
    }
    let content = content.into();

    let message = Message::RefClicked(reference.name.clone());
    let item = if reference.is_head {
        head_button(content, depth, message)
    } else {
        list_button(content, depth, message)
    };
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

/// Remote node above its branches: a click collapses it, right click
/// opens the remote menu; `external`: defined outside the repository config
/// (the tooltip says where, M6-18).
fn remote_label<'a>(
    name: String,
    key: String,
    expanded: bool,
    external: Option<git_core::ConfigScope>,
) -> Element<'a, Message> {
    let chevron = if expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };
    let content = row![
        container(icons::tinted(chevron, 12.0, |p| p.text_muted)).width(INDENT),
        icons::tinted(Icon::Cloud, 13.0, |p| p.text_muted),
        muted(name.clone()),
        space::horizontal(),
    ]
    .spacing(4)
    .align_y(iced::Center);
    let row = list_button(content.into(), 1, Message::FolderToggled(key));
    let area =
        crate::ui::context_area::context_area(row, move |position| Message::RemoteContextMenu {
            name: name.clone(),
            position,
        });
    match external {
        Some(scope) => crate::ui::panel::with_tooltip_text(area, scope.hint().to_owned()),
        None => area,
    }
}

/// Row of the checked-out branch: always highlighted (M7-14).
fn head_button<'a>(
    content: Element<'a, Message>,
    depth: u16,
    on_press: Message,
) -> Element<'a, Message> {
    button(content)
        .on_press(on_press)
        .padding(iced::Padding::from([3, 6]).left(6.0 + INDENT * f32::from(depth)))
        .width(Fill)
        .clip(true)
        .style(theme::list_item(true))
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
