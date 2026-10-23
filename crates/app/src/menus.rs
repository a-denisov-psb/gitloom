//! Context menus of commits, branches, tags, remotes and stashes (M3-11),
//! built from plain facts so the enable rules are testable.

use crate::app::{CheckoutTarget, Message};
use crate::overlay::MenuEntry;

/// Tooltip of actions that arrive with merge and rebase support (M4).
const LATER: &str = "Not available yet";

fn action(label: &str, enabled: bool, message: Message) -> MenuEntry {
    MenuEntry::Item {
        label: label.into(),
        shortcut: None,
        enabled,
        message: Box::new(message),
        hint: None,
    }
}

/// Visible but disabled until M4.
fn later(label: &str) -> MenuEntry {
    MenuEntry::Item {
        label: label.into(),
        shortcut: None,
        enabled: false,
        message: Box::new(Message::Refresh),
        hint: Some(LATER.into()),
    }
}

pub fn commit(id: &str, message: &str, writable: bool) -> Vec<MenuEntry> {
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
        later("Cherry-pick commit"),
        later("Revert commit"),
        later("Reset to this commit"),
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
}

pub fn local_branch(branch: LocalBranch<'_>) -> Vec<MenuEntry> {
    let LocalBranch {
        name,
        target,
        is_head,
        has_upstream,
        writable,
    } = branch;
    let (pull, push) = if is_head {
        (Message::Pull(None), Message::Push { force: false })
    } else {
        (
            Message::PullBranch(name.into()),
            Message::PushBranch(name.into()),
        )
    };
    vec![
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
        later("Merge into current branch"),
        later("Rebase current branch onto this"),
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
    ]
}

/// Remote branch by full name (`origin/topic`).
pub fn remote_branch(name: &str, writable: bool) -> Vec<MenuEntry> {
    vec![
        action(
            "Checkout",
            writable,
            Message::Checkout(CheckoutTarget::Remote(name.into())),
        ),
        MenuEntry::Separator,
        later("Merge into current branch"),
        later("Rebase current branch onto this"),
        MenuEntry::Separator,
        action(
            "Delete on remote",
            writable,
            Message::DeleteRemoteBranch(name.into()),
        ),
        action("Copy name", true, Message::CopyToClipboard(name.into())),
    ]
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
        })
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
    fn m4_actions_are_disabled_with_a_hint() {
        for menu in [
            commit("c1", "msg", true),
            branch(false, true),
            remote_branch("origin/topic", true),
        ] {
            let (_, disabled) = summary(&menu);
            assert!(!disabled.is_empty());
            for (label, hint) in disabled {
                if ["Checkout", "Delete", "Pull", "Unset upstream"].contains(&label) {
                    continue;
                }
                assert_eq!(hint, Some(LATER), "{label}");
            }
        }
    }

    #[test]
    fn read_only_keeps_only_copy_actions() {
        let menu = local_branch(LocalBranch {
            name: "topic",
            target: "c1",
            is_head: false,
            has_upstream: true,
            writable: false,
        });
        assert_eq!(summary(&menu).0, ["Copy name"]);
        assert_eq!(
            summary(&commit("c1", "msg", false)).0,
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
