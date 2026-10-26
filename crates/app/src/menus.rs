//! Context menus of commits, branches, tags, remotes and stashes (M3-11),
//! built from plain facts so the enable rules are testable.

use crate::app::{CheckoutTarget, Message};
use crate::overlay::MenuEntry;

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
}

pub fn remote_branch(branch: RemoteBranch<'_>) -> Vec<MenuEntry> {
    let RemoteBranch {
        name,
        writable,
        head,
        head_can_fast_forward,
        behind_local,
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
        action("Copy name", true, Message::CopyToClipboard(name.into())),
    ]);
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

pub fn remote(name: &str, writable: bool) -> Vec<MenuEntry> {
    vec![
        action("Fetch", writable, Message::FetchRemote(name.into())),
        MenuEntry::Separator,
        action("Edit URL…", writable, Message::EditRemoteUrl(name.into())),
        action("Rename…", writable, Message::RenameRemoteStart(name.into())),
        action("Remove", writable, Message::RemoveRemote(name.into())),
    ]
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
        MenuEntry::Separator,
        action("Delete stash", writable, Message::StashDrop(id.into())),
    ]
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
        })
    }

    fn remote_menu(behind_local: Option<&str>, head_can_fast_forward: bool) -> Vec<MenuEntry> {
        remote_branch(RemoteBranch {
            name: "origin/topic",
            writable: true,
            head: "main",
            head_can_fast_forward,
            behind_local,
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
            Some(Message::Rebase { onto, autostash: false }) if onto == "refs/heads/topic"
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
        });
        assert_eq!(summary(&menu).0, ["Copy name"]);
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
        assert!(summary(&remote("origin", false)).0.is_empty());
    }

    #[test]
    fn stash_restore_is_blocked_during_operations() {
        assert_eq!(summary(&stash("s", true, false)).0, ["Delete stash"]);
        assert_eq!(
            summary(&stash("s", true, true)).0,
            ["Apply stash", "Pop stash", "Delete stash"]
        );
    }
}
