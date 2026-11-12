//! Context menus of commits, branches, tags, remotes and stashes (M3-11),
//! built from plain facts so the enable rules are testable.

use crate::app::{CheckoutTarget, Message};
use crate::overlay::MenuEntry;

/// Hide/Show and Solo/Unsolo of a branch (M5-08); the checked-out branch
/// cannot be hidden.
fn visibility_entries(
    name: &str,
    kind: git_core::RefKind,
    hidden: bool,
    soloed: bool,
    is_head: bool,
) -> [MenuEntry; 2] {
    let hide = MenuEntry::Item {
        label: if hidden { "Show" } else { "Hide" }.into(),
        shortcut: None,
        enabled: !is_head,
        message: Box::new(Message::ToggleHidden {
            name: name.into(),
            kind,
        }),
        hint: is_head.then(|| "Checked-out branch".to_owned()),
    };
    let solo = action(
        if soloed { "Unsolo" } else { "Solo" },
        true,
        Message::ToggleSolo {
            name: name.into(),
            kind,
        },
    );
    [hide, solo]
}

fn action(label: &str, enabled: bool, message: Message) -> MenuEntry {
    MenuEntry::Item {
        label: label.into(),
        shortcut: None,
        enabled,
        message: Box::new(message),
        hint: None,
    }
}

/// History actions on a commit that depend on its position.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommitFacts {
    /// A proper ancestor of HEAD and no operation in progress: the commits
    /// after it can be rebased interactively.
    pub rebase_from: bool,
    /// Its message can be edited (HEAD, or linear history up to HEAD).
    pub rewordable: bool,
    /// It can be cherry-picked onto HEAD (not HEAD itself, no operation).
    pub pickable: bool,
    /// It can be reverted (HEAD exists, no operation).
    pub revertable: bool,
    /// HEAD can be reset to it (not HEAD itself, no operation).
    pub resettable: bool,
}

/// `head`: checked-out branch name, or "HEAD" when detached.
pub fn commit(
    id: &str,
    message: &str,
    writable: bool,
    facts: CommitFacts,
    head: &str,
) -> Vec<MenuEntry> {
    vec![
        action(
            "Checkout this commit",
            writable,
            Message::Checkout(CheckoutTarget::Commit(id.into())),
        ),
        action(
            "Create branch here",
            writable,
            Message::NewBranchStart(Some(id.into())),
        ),
        action(
            "Create tag here",
            writable,
            Message::CreateTagStart(id.into()),
        ),
        MenuEntry::Separator,
        action(
            "Edit commit message",
            writable && facts.rewordable,
            Message::RewordStart(id.into()),
        ),
        action(
            "Interactive rebase from here",
            writable && facts.rebase_from,
            Message::RebasePanelOpened(id.into()),
        ),
        action(
            "Cherry-pick commit",
            writable && facts.pickable,
            Message::CherryPick(vec![id.into()]),
        ),
        action(
            "Revert commit",
            writable && facts.revertable,
            Message::RevertStart(id.into()),
        ),
        action(
            &format!("Reset {head} to this commit…"),
            writable && facts.resettable,
            Message::ResetStart(id.into()),
        ),
        MenuEntry::Separator,
        action("Copy SHA", true, Message::CopyToClipboard(id.into())),
        action(
            "Copy message",
            true,
            Message::CopyToClipboard(message.into()),
        ),
    ]
}

/// Facts about a local branch for its menu.
#[derive(Debug, Clone, Copy)]
pub struct LocalBranch<'a> {
    pub name: &'a str,
    /// Commit the branch points to (for "Create tag here").
    pub target: &'a str,
    pub is_head: bool,
    pub has_upstream: bool,
    pub writable: bool,
    /// Checked-out branch name, or "HEAD" when detached.
    pub head: &'a str,
    /// HEAD can be fast-forwarded to this branch.
    pub head_can_fast_forward: bool,
    /// Hidden / soloed in the graph (M5-08).
    pub hidden: bool,
    pub soloed: bool,
}

/// "Merge X into Y", "Rebase Y onto X" and, if possible, "Fast-forward Y
/// to X" for the branch with full reference name `reference` and display
/// name `name`.
fn merge_entries(
    name: &str,
    reference: &str,
    head: &str,
    head_can_fast_forward: bool,
    writable: bool,
) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    if head_can_fast_forward {
        entries.push(action(
            &format!("Fast-forward {head} to {name}"),
            writable,
            Message::FastForward {
                branch: None,
                source: reference.into(),
                autostash: false,
            },
        ));
    }
    entries.push(action(
        &format!("Merge {name} into {head}"),
        writable,
        Message::Merge {
            source: reference.into(),
            autostash: false,
        },
    ));
    entries.push(action(
        &format!("Rebase {head} onto {name}"),
        writable,
        Message::Rebase {
            onto: reference.into(),
            autostash: false,
            confirmed: false,
        },
    ));
    entries
}

pub fn local_branch(branch: LocalBranch<'_>) -> Vec<MenuEntry> {
    let LocalBranch {
        name,
        target,
        is_head,
        has_upstream,
        writable,
        head,
        head_can_fast_forward,
        hidden,
        soloed,
    } = branch;
    let (pull, push) = if is_head {
        (Message::Pull(None), Message::Push { force: false })
    } else {
        (
            Message::PullBranch(name.into()),
            Message::PushBranch(name.into()),
        )
    };
    // Merging the checked-out branch into itself makes no sense.
    let merge = if is_head {
        Vec::new()
    } else {
        merge_entries(
            name,
            &format!("refs/heads/{name}"),
            head,
            head_can_fast_forward,
            writable,
        )
    };
    let mut entries = vec![
        action(
            "Checkout",
            writable && !is_head,
            Message::Checkout(CheckoutTarget::Local(name.into())),
        ),
        MenuEntry::Separator,
        action("Pull", writable && has_upstream, pull),
        action("Push", writable, push),
        action(
            "Set upstream…",
            writable,
            Message::SetUpstreamStart(name.into()),
        ),
        action(
            "Unset upstream",
            writable && has_upstream,
            Message::UnsetUpstream(name.into()),
        ),
        MenuEntry::Separator,
    ];
    entries.extend(merge);
    entries.extend([
        MenuEntry::Separator,
        action("Rename…", writable, Message::RenameStart(name.into())),
        // The checked-out branch cannot be deleted.
        action(
            "Delete",
            writable && !is_head,
            Message::DeleteBranch(name.into()),
        ),
        action(
            "Create tag here",
            writable,
            Message::CreateTagStart(target.into()),
        ),
        MenuEntry::Separator,
    ]);
    entries.extend(visibility_entries(
        name,
        git_core::RefKind::LocalBranch,
        hidden,
        soloed,
        is_head,
    ));
    entries.extend([
        MenuEntry::Separator,
        action("Copy name", true, Message::CopyToClipboard(name.into())),
    ]);
    entries
}

/// Facts about a remote branch for its menu.
#[derive(Debug, Clone, Copy)]
pub struct RemoteBranch<'a> {
    /// Short name (`origin/topic`).
    pub name: &'a str,
    pub writable: bool,
    /// Checked-out branch name, or "HEAD" when detached.
    pub head: &'a str,
    /// HEAD can be fast-forwarded to this branch.
    pub head_can_fast_forward: bool,
    /// Local branch tracking this one that is not checked out and can be
    /// fast-forwarded to it.
    pub behind_local: Option<&'a str>,
    pub hidden: bool,
    pub soloed: bool,
}

pub fn remote_branch(branch: RemoteBranch<'_>) -> Vec<MenuEntry> {
    let RemoteBranch {
        name,
        writable,
        head,
        head_can_fast_forward,
        behind_local,
        hidden,
        soloed,
    } = branch;
    let reference = format!("refs/remotes/{name}");
    let mut entries = vec![
        action(
            "Checkout",
            writable,
            Message::Checkout(CheckoutTarget::Remote(name.into())),
        ),
        MenuEntry::Separator,
    ];
    if let Some(local) = behind_local {
        entries.push(action(
            &format!("Fast-forward {local} to {name}"),
            writable,
            Message::FastForward {
                branch: Some(local.into()),
                source: reference.clone(),
                autostash: false,
            },
        ));
    }
    entries.extend(merge_entries(
        name,
        &reference,
        head,
        head_can_fast_forward,
        writable,
    ));
    entries.extend([
        MenuEntry::Separator,
        action(
            "Delete on remote",
            writable,
            Message::DeleteRemoteBranch(name.into()),
        ),
        MenuEntry::Separator,
    ]);
    entries.extend(visibility_entries(
        name,
        git_core::RefKind::RemoteBranch,
        hidden,
        soloed,
        false,
    ));
    entries.extend([
        MenuEntry::Separator,
        action("Copy name", true, Message::CopyToClipboard(name.into())),
    ]);
    entries
}

/// Facts about a branch dropped onto another one (M4-11).
#[derive(Debug, Clone, Copy)]
pub struct DropFacts<'a> {
    /// Dragged branch (short name) and whether it is a remote branch.
    pub source: &'a str,
    pub source_remote: bool,
    /// Branch it was dropped on.
    pub target: &'a str,
    pub target_remote: bool,
    /// Commit the target points to (base of an interactive rebase onto it).
    pub target_commit: &'a str,
    /// Checked-out branch, if any.
    pub head: Option<&'a str>,
    /// The target is a local branch that is a proper ancestor of the source.
    pub can_fast_forward: bool,
    /// Writing is possible and no operation is in progress.
    pub writable: bool,
}

fn needs_checkout(label: &str, enabled: bool, branch: &str, message: Message) -> MenuEntry {
    if enabled {
        return action(label, true, message);
    }
    MenuEntry::Item {
        label: label.into(),
        shortcut: None,
        enabled: false,
        message: Box::new(message),
        hint: Some(format!("Check out {branch} first")),
    }
}

/// Menu after dropping branch S onto branch T (planning decision M4):
/// fast-forward T to S (if possible), merge S into T, rebase S onto T,
/// interactive rebase S onto T. Items that need another checked-out branch
/// are disabled with "Check out <branch> first".
pub fn drop_menu(facts: DropFacts<'_>) -> Vec<MenuEntry> {
    let DropFacts {
        source,
        source_remote,
        target,
        target_remote,
        target_commit,
        head,
        can_fast_forward,
        writable,
    } = facts;
    let reference = |name: &str, remote: bool| {
        if remote {
            format!("refs/remotes/{name}")
        } else {
            format!("refs/heads/{name}")
        }
    };
    let source_ref = reference(source, source_remote);
    let target_checked_out = !target_remote && head == Some(target);
    let source_checked_out = !source_remote && head == Some(source);
    let mut entries = Vec::new();
    if can_fast_forward && !target_remote {
        entries.push(action(
            &format!("Fast-forward {target} to {source}"),
            writable,
            Message::FastForward {
                branch: (!target_checked_out).then(|| target.to_owned()),
                source: source_ref.clone(),
                autostash: false,
            },
        ));
    }
    entries.push(needs_checkout(
        &format!("Merge {source} into {target}"),
        writable && target_checked_out,
        target,
        Message::Merge {
            source: source_ref,
            autostash: false,
        },
    ));
    entries.push(needs_checkout(
        &format!("Rebase {source} onto {target}"),
        writable && source_checked_out,
        source,
        Message::Rebase {
            onto: reference(target, target_remote),
            autostash: false,
            confirmed: false,
        },
    ));
    entries.push(needs_checkout(
        &format!("Interactive rebase {source} onto {target}"),
        writable && source_checked_out,
        source,
        Message::RebasePanelOpened(target_commit.to_owned()),
    ));
    entries
}

pub fn tag(name: &str, writable: bool) -> Vec<MenuEntry> {
    vec![
        action("Push to remote", writable, Message::PushTag(name.into())),
        action("Delete local", writable, Message::DeleteTag(name.into())),
        action(
            "Delete on remote",
            writable,
            Message::DeleteRemoteTag(name.into()),
        ),
        MenuEntry::Separator,
        action("Copy name", true, Message::CopyToClipboard(name.into())),
    ]
}

/// `any_shown` / `any_hidden`: some of the remote's branches are shown /
/// hidden in the graph.
/// Facts for the remote menu.
#[derive(Debug, Clone)]
pub struct RemoteFacts {
    pub writable: bool,
    pub any_shown: bool,
    pub any_hidden: bool,
    /// Defined outside the repository config: `git remote` cannot change it.
    pub external: Option<git_core::ConfigScope>,
    pub has_url: bool,
    /// The remote's website (M12-03), if its URL has one.
    pub website: Option<String>,
}

pub fn remote(name: &str, facts: &RemoteFacts) -> Vec<MenuEntry> {
    let RemoteFacts {
        writable,
        any_shown,
        any_hidden,
        external,
        has_url,
        ref website,
    } = facts;
    let (writable, any_shown, any_hidden, external, has_url) =
        (*writable, *any_shown, *any_hidden, *external, *has_url);
    // Outside the repository config: only Fetch (if it has a URL).
    let config_entry = |label: &str, message: Message| match external {
        Some(scope) => MenuEntry::Item {
            label: label.into(),
            shortcut: None,
            enabled: false,
            message: Box::new(message),
            hint: Some(scope.hint().into()),
        },
        None => action(label, writable, message),
    };
    let mut entries = vec![action(
        "Fetch",
        writable && (external.is_none() || has_url),
        Message::FetchRemote(name.into()),
    )];
    if let Some(url) = website {
        entries.push(MenuEntry::item(
            git_core::remote_web::web_label(url),
            Message::OpenLink(url.clone()),
        ));
    }
    entries.extend([
        MenuEntry::Separator,
        config_entry("Edit URL…", Message::EditRemoteUrl(name.into())),
        config_entry("Rename…", Message::RenameRemoteStart(name.into())),
        config_entry("Remove", Message::RemoveRemote(name.into())),
        MenuEntry::Separator,
        action(
            "Hide all",
            any_shown,
            Message::HideRemote {
                remote: name.into(),
                hide: true,
            },
        ),
        action(
            "Show all",
            any_hidden,
            Message::HideRemote {
                remote: name.into(),
                hide: false,
            },
        ),
    ]);
    entries
}

/// `restorable`: apply/pop are allowed (no merge or rebase in progress).
pub fn stash(id: &str, writable: bool, restorable: bool) -> Vec<MenuEntry> {
    vec![
        action(
            "Apply stash",
            writable && restorable,
            Message::StashApply(id.into()),
        ),
        action(
            "Pop stash",
            writable && restorable,
            Message::StashPop(id.into()),
        ),
        action(
            "Rename stash…",
            writable,
            Message::RenameStashStart(id.into()),
        ),
        MenuEntry::Separator,
        action("Delete stash", writable, Message::StashDrop(id.into())),
    ]
}

/// Context menu of a submodule row (M12-06).
pub fn submodule(path: &str, writable: bool) -> Vec<MenuEntry> {
    vec![
        action(
            "Update",
            writable,
            Message::SubmoduleUpdate(Some(path.into())),
        ),
        MenuEntry::item("Open in new tab", Message::OpenSubmodule(path.into())),
        MenuEntry::item(
            "Open in file manager",
            Message::OpenFileLocation(path.into()),
        ),
        MenuEntry::Separator,
        MenuEntry::item("Copy path", Message::CopyToClipboard(path.into())),
    ]
}

/// Context menu of the SUBMODULES header (M12-06).
pub fn submodules_header(enabled: bool) -> Vec<MenuEntry> {
    vec![action(
        "Update all submodules",
        enabled,
        Message::SubmoduleUpdate(None),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Labels of enabled items, and of disabled items with their hint.
    fn summary(menu: &[MenuEntry]) -> (Vec<&str>, Vec<(&str, Option<&str>)>) {
        let mut enabled = Vec::new();
        let mut disabled = Vec::new();
        for entry in menu {
            if let MenuEntry::Item {
                label,
                enabled: on,
                hint,
                ..
            } = entry
            {
                if *on {
                    enabled.push(label.as_str());
                } else {
                    disabled.push((label.as_str(), hint.as_deref()));
                }
            }
        }
        (enabled, disabled)
    }

    fn branch(is_head: bool, has_upstream: bool) -> Vec<MenuEntry> {
        local_branch(LocalBranch {
            name: "topic",
            target: "c1",
            is_head,
            has_upstream,
            writable: true,
            head: "main",
            head_can_fast_forward: false,
            hidden: false,
            soloed: false,
        })
    }

    fn remote_menu(behind_local: Option<&str>, head_can_fast_forward: bool) -> Vec<MenuEntry> {
        remote_branch(RemoteBranch {
            name: "origin/topic",
            writable: true,
            head: "main",
            head_can_fast_forward,
            behind_local,
            hidden: false,
            soloed: false,
        })
    }

    fn message_of(menu: &[MenuEntry], label: &str) -> Option<Message> {
        menu.iter().find_map(|e| match e {
            MenuEntry::Item {
                label: l, message, ..
            } if l == label => Some(message.as_ref().clone()),
            _ => None,
        })
    }

    #[test]
    fn other_branches_merge_into_head() {
        let menu = branch(false, false);
        assert!(matches!(
            message_of(&menu, "Merge topic into main"),
            Some(Message::Merge { source, autostash: false }) if source == "refs/heads/topic"
        ));
        assert!(message_of(&menu, "Fast-forward main to topic").is_none());
        assert!(matches!(
            message_of(&menu, "Rebase main onto topic"),
            Some(Message::Rebase { onto, autostash: false, .. }) if onto == "refs/heads/topic"
        ));
        // The checked-out branch offers no merge.
        let head = branch(true, false);
        assert!(!summary(&head).0.iter().any(|l| l.starts_with("Merge")));

        let ahead = local_branch(LocalBranch {
            name: "topic",
            target: "c1",
            is_head: false,
            has_upstream: false,
            writable: true,
            head: "main",
            head_can_fast_forward: true,
            hidden: false,
            soloed: false,
        });
        assert!(matches!(
            message_of(&ahead, "Fast-forward main to topic"),
            Some(Message::FastForward { branch: None, source, .. }) if source == "refs/heads/topic"
        ));
    }

    #[test]
    fn remote_branch_merges_and_fast_forwards() {
        let menu = remote_menu(None, false);
        assert!(matches!(
            message_of(&menu, "Merge origin/topic into main"),
            Some(Message::Merge { source, .. }) if source == "refs/remotes/origin/topic"
        ));
        assert!(!summary(&menu)
            .0
            .iter()
            .any(|l| l.starts_with("Fast-forward")));

        let menu = remote_menu(Some("topic"), true);
        assert!(matches!(
            message_of(&menu, "Fast-forward topic to origin/topic"),
            Some(Message::FastForward { branch: Some(b), source, .. })
                if b == "topic" && source == "refs/remotes/origin/topic"
        ));
        assert!(message_of(&menu, "Fast-forward main to origin/topic").is_some());
    }

    #[test]
    fn checked_out_branch_cannot_be_checked_out_or_deleted() {
        let head = branch(true, true);
        let (_, disabled) = summary(&head);
        let labels: Vec<_> = disabled.iter().map(|(l, _)| *l).collect();
        assert!(labels.contains(&"Checkout") && labels.contains(&"Delete"));
        let other = branch(false, true);
        let (enabled, _) = summary(&other);
        assert!(enabled.contains(&"Checkout") && enabled.contains(&"Delete"));
    }

    #[test]
    fn pull_and_unset_need_an_upstream() {
        let plain = branch(false, false);
        let (enabled, _) = summary(&plain);
        assert!(!enabled.contains(&"Pull") && !enabled.contains(&"Unset upstream"));
        assert!(enabled.contains(&"Push") && enabled.contains(&"Set upstream…"));
        let tracking = branch(false, true);
        let (enabled, _) = summary(&tracking);
        assert!(enabled.contains(&"Pull") && enabled.contains(&"Unset upstream"));
    }

    #[test]
    fn head_branch_pulls_and_pushes_like_the_toolbar() {
        let menu = branch(true, true);
        let message = |label: &str| {
            menu.iter().find_map(|e| match e {
                MenuEntry::Item {
                    label: l, message, ..
                } if l == label => Some(message.as_ref().clone()),
                _ => None,
            })
        };
        assert!(matches!(message("Pull"), Some(Message::Pull(None))));
        assert!(matches!(
            message("Push"),
            Some(Message::Push { force: false })
        ));
        let other = branch(false, true);
        assert!(other.iter().any(|e| matches!(e, MenuEntry::Item { message, .. } if matches!(message.as_ref(), Message::PullBranch(b) if b == "topic"))));
    }

    #[test]
    fn commit_menu_history_actions_follow_the_facts() {
        let none = commit("c1", "msg", true, CommitFacts::default(), "main");
        let (enabled, _) = summary(&none);
        assert_eq!(
            enabled,
            [
                "Checkout this commit",
                "Create branch here",
                "Create tag here",
                "Copy SHA",
                "Copy message"
            ]
        );
        let all = CommitFacts {
            rebase_from: true,
            rewordable: true,
            pickable: true,
            revertable: true,
            resettable: true,
        };
        let menu = commit("c1", "msg", true, all, "topic");
        let (_, disabled) = summary(&menu);
        assert!(disabled.is_empty(), "{disabled:?}");
        assert!(matches!(
            message_of(&commit("c1", "msg", true, all, "topic"), "Reset topic to this commit…"),
            Some(Message::ResetStart(id)) if id == "c1"
        ));
    }

    fn drop_facts<'a>(head: Option<&'a str>, can_fast_forward: bool) -> DropFacts<'a> {
        DropFacts {
            source: "feature",
            source_remote: false,
            target: "main",
            target_remote: false,
            target_commit: "c0",
            head,
            can_fast_forward,
            writable: true,
        }
    }

    fn hint_of<'a>(menu: &'a [MenuEntry], label: &str) -> Option<&'a str> {
        menu.iter().find_map(|e| match e {
            MenuEntry::Item { label: l, hint, .. } if l == label => hint.as_deref(),
            _ => None,
        })
    }

    #[test]
    fn drop_onto_the_checked_out_target_merges() {
        let menu = drop_menu(drop_facts(Some("main"), true));
        let (enabled, disabled) = summary(&menu);
        assert_eq!(
            enabled,
            ["Fast-forward main to feature", "Merge feature into main"]
        );
        assert_eq!(
            hint_of(&menu, "Rebase feature onto main"),
            Some("Check out feature first")
        );
        assert_eq!(disabled.len(), 2);
        assert!(matches!(
            message_of(&menu, "Fast-forward main to feature"),
            Some(Message::FastForward { branch: None, source, .. }) if source == "refs/heads/feature"
        ));
    }

    #[test]
    fn drop_with_the_source_checked_out_rebases() {
        let menu = drop_menu(drop_facts(Some("feature"), false));
        let (enabled, _) = summary(&menu);
        assert_eq!(
            enabled,
            [
                "Rebase feature onto main",
                "Interactive rebase feature onto main"
            ]
        );
        assert_eq!(
            hint_of(&menu, "Merge feature into main"),
            Some("Check out main first")
        );
        assert!(matches!(
            message_of(&menu, "Interactive rebase feature onto main"),
            Some(Message::RebasePanelOpened(base)) if base == "c0"
        ));
    }

    #[test]
    fn fast_forward_of_a_branch_that_is_not_checked_out() {
        let menu = drop_menu(drop_facts(Some("other"), true));
        let (enabled, _) = summary(&menu);
        assert_eq!(enabled, ["Fast-forward main to feature"]);
        assert!(matches!(
            message_of(&menu, "Fast-forward main to feature"),
            Some(Message::FastForward { branch: Some(b), .. }) if b == "main"
        ));
        // Remote targets cannot be fast-forwarded or merged into.
        let mut facts = drop_facts(Some("feature"), true);
        facts.target = "origin/main";
        facts.target_remote = true;
        let menu = drop_menu(facts);
        assert!(message_of(&menu, "Fast-forward origin/main to feature").is_none());
        assert!(matches!(
            message_of(&menu, "Rebase feature onto origin/main"),
            Some(Message::Rebase { onto, .. }) if onto == "refs/remotes/origin/main"
        ));
    }

    #[test]
    fn read_only_keeps_only_copy_actions() {
        let menu = local_branch(LocalBranch {
            name: "topic",
            target: "c1",
            is_head: false,
            has_upstream: true,
            writable: false,
            head: "main",
            head_can_fast_forward: true,
            hidden: false,
            soloed: false,
        });
        // Hiding only changes the view, not the repository.
        assert_eq!(summary(&menu).0, ["Hide", "Solo", "Copy name"]);
        assert_eq!(
            summary(&commit(
                "c1",
                "msg",
                false,
                CommitFacts {
                    rebase_from: true,
                    rewordable: true,
                    pickable: true,
                    revertable: true,
                    resettable: true,
                },
                "main"
            ))
            .0,
            ["Copy SHA", "Copy message"]
        );
        assert_eq!(summary(&tag("v1", false)).0, ["Copy name"]);
        let read_only = RemoteFacts {
            any_shown: false,
            ..facts(false, None)
        };
        assert!(summary(&remote("origin", &read_only)).0.is_empty());
    }

    #[test]
    fn hide_and_solo_entries() {
        let head = branch(true, false);
        assert_eq!(hint_of(&head, "Hide"), Some("Checked-out branch"));
        assert!(summary(&head).0.contains(&"Solo"));
        let hidden = local_branch(LocalBranch {
            name: "topic",
            target: "c1",
            is_head: false,
            has_upstream: false,
            writable: true,
            head: "main",
            head_can_fast_forward: false,
            hidden: true,
            soloed: true,
        });
        assert!(matches!(
            message_of(&hidden, "Show"),
            Some(Message::ToggleHidden { name, .. }) if name == "topic"
        ));
        assert!(message_of(&hidden, "Unsolo").is_some());
        assert!(message_of(&remote_menu(None, false), "Hide").is_some());

        let menu = remote("origin", &facts(true, None));
        let (enabled, disabled) = summary(&menu);
        assert!(enabled.contains(&"Hide all"));
        assert!(disabled.iter().any(|(l, _)| *l == "Show all"));
    }

    fn facts(writable: bool, external: Option<git_core::ConfigScope>) -> RemoteFacts {
        RemoteFacts {
            writable,
            any_shown: true,
            any_hidden: false,
            external,
            has_url: true,
            website: None,
        }
    }

    #[test]
    fn remote_outside_the_repository_config_can_only_be_fetched() {
        let hint = "Defined in the global Git configuration";
        let menu = remote("glob", &facts(true, Some(git_core::ConfigScope::Global)));
        let (enabled, _) = summary(&menu);
        assert!(enabled.contains(&"Fetch"));
        for label in ["Edit URL…", "Rename…", "Remove"] {
            assert!(!enabled.contains(&label), "{label}");
            assert_eq!(hint_of(&menu, label), Some(hint));
        }
        // Without a URL there is nothing to fetch from.
        let no_url = RemoteFacts {
            has_url: false,
            ..facts(true, Some(git_core::ConfigScope::Global))
        };
        assert!(!summary(&remote("glob", &no_url)).0.contains(&"Fetch"));
        // Repository remotes keep every entry.
        let menu = remote("origin", &facts(true, None));
        let (enabled, _) = summary(&menu);
        for label in ["Fetch", "Edit URL…", "Rename…", "Remove"] {
            assert!(enabled.contains(&label), "{label}");
        }
    }

    #[test]
    fn stash_restore_is_blocked_during_operations() {
        assert_eq!(
            summary(&stash("s", true, false)).0,
            ["Rename stash…", "Delete stash"]
        );
        assert_eq!(
            summary(&stash("s", true, true)).0,
            ["Apply stash", "Pop stash", "Rename stash…", "Delete stash"]
        );
    }
}
