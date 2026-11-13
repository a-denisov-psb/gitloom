//! Context menus of commits, branches, tags, remotes, stashes (M3-11) and
//! files (M13-07),
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

/// "Change author…" of the commit menu (M15-11): the commits it changes
/// (the multi-selection or the clicked commit) and why it is disabled.
#[derive(Debug, Clone, Default)]
pub struct AuthorChange {
    pub ids: Vec<String>,
    pub block: Option<String>,
}

/// `head`: checked-out branch name, or "HEAD" when detached; `author`:
/// "Change author…" (`None` for a stash).
pub fn commit(
    id: &str,
    message: &str,
    writable: bool,
    facts: CommitFacts,
    head: &str,
    author: Option<AuthorChange>,
) -> Vec<MenuEntry> {
    let mut entries = commit_menu_entries(id, message, writable, facts, head);
    if let Some(author) = author {
        // After the message entry (M15-11).
        let at = entries
            .iter()
            .position(
                |e| matches!(e, MenuEntry::Item { label, .. } if label == "Edit commit message"),
            )
            .map_or(entries.len(), |i| i + 1);
        let enabled = writable && author.block.is_none();
        entries.insert(
            at,
            MenuEntry::Item {
                label: "Change author…".into(),
                shortcut: None,
                enabled,
                hint: author.block.filter(|_| writable),
                message: Box::new(Message::ChangeAuthorStart(author.ids)),
            },
        );
    }
    entries
}

fn commit_menu_entries(
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
    /// HEAD can be reset to the branch's commit (`CommitFacts::resettable`).
    pub resettable: bool,
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

/// Entries every ref menu shares (M13-06): "Create branch here" and
/// "Reset <head> to this commit…" for the ref's commit `target`.
fn commit_entries(
    target: &str,
    label: &str,
    head: &str,
    writable: bool,
    resettable: bool,
) -> [MenuEntry; 2] {
    [
        action(
            "Create branch here",
            writable,
            Message::CreateBranchAtRef {
                target: target.into(),
                label: label.into(),
            },
        ),
        action(
            &format!("Reset {head} to this commit…"),
            writable && resettable,
            Message::ResetStart(target.into()),
        ),
    ]
}

/// Local branch menu like GitKraken's (M13-06).
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
        resettable,
    } = branch;
    let (pull, push) = if is_head {
        (Message::Pull(None), Message::Push { force: false })
    } else {
        (
            Message::PullBranch(name.into()),
            Message::PushBranch(name.into()),
        )
    };
    let mut entries = Vec::new();
    // Like GitKraken: no Checkout for the checked-out branch.
    if !is_head {
        entries.push(action(
            &format!("Checkout {name}"),
            writable,
            Message::Checkout(CheckoutTarget::Local(name.into())),
        ));
        entries.push(MenuEntry::Separator);
    }
    entries.extend([
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
    ]);
    // Merging the checked-out branch into itself makes no sense.
    if !is_head {
        entries.push(MenuEntry::Separator);
        entries.extend(merge_entries(
            name,
            &format!("refs/heads/{name}"),
            head,
            head_can_fast_forward,
            writable,
        ));
    }
    entries.push(MenuEntry::Separator);
    entries.extend(commit_entries(target, name, head, writable, resettable));
    entries.extend([
        MenuEntry::Separator,
        action(
            &format!("Rename {name}…"),
            writable,
            Message::RenameStart(name.into()),
        ),
        // The checked-out branch cannot be deleted.
        MenuEntry::Item {
            label: format!("Delete {name}"),
            shortcut: None,
            enabled: writable && !is_head,
            message: Box::new(Message::DeleteBranch(name.into())),
            hint: is_head.then(|| "Checked-out branch".to_owned()),
        },
        MenuEntry::Separator,
        action(
            "Copy branch name",
            true,
            Message::CopyToClipboard(name.into()),
        ),
        action(
            "Copy commit sha",
            true,
            Message::CopyToClipboard(target.into()),
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
        action(
            "Create tag here",
            writable,
            Message::CreateTagStart(target.into()),
        ),
    ]);
    entries
}

/// Facts about a remote branch for its menu.
#[derive(Debug, Clone, Copy)]
pub struct RemoteBranch<'a> {
    /// Short name (`origin/topic`).
    pub name: &'a str,
    /// Commit the branch points to.
    pub target: &'a str,
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
    /// HEAD can be reset to the branch's commit.
    pub resettable: bool,
}

/// Remote branch menu like GitKraken's (M13-06): merge entries first.
pub fn remote_branch(branch: RemoteBranch<'_>) -> Vec<MenuEntry> {
    let RemoteBranch {
        name,
        target,
        writable,
        head,
        head_can_fast_forward,
        behind_local,
        hidden,
        soloed,
        resettable,
    } = branch;
    let reference = format!("refs/remotes/{name}");
    let mut entries = Vec::new();
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
            &format!("Checkout {name}"),
            writable,
            Message::Checkout(CheckoutTarget::Remote(name.into())),
        ),
    ]);
    entries.extend(commit_entries(target, name, head, writable, resettable));
    entries.extend([
        action(
            &format!("Delete {name}"),
            writable,
            Message::DeleteRemoteBranch(name.into()),
        ),
        MenuEntry::Separator,
        action(
            "Copy branch name",
            true,
            Message::CopyToClipboard(name.into()),
        ),
        action(
            "Copy commit sha",
            true,
            Message::CopyToClipboard(target.into()),
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
        action(
            "Create tag here",
            writable,
            Message::CreateTagStart(target.into()),
        ),
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

/// Facts about a tag for its menu (M13-06).
#[derive(Debug, Clone, Copy)]
pub struct Tag<'a> {
    pub name: &'a str,
    /// The tag's commit (annotated tags peeled).
    pub target: &'a str,
    pub writable: bool,
    /// Checked-out branch name, or "HEAD" when detached.
    pub head: &'a str,
    pub head_can_fast_forward: bool,
    pub resettable: bool,
    /// The only remote, named in the push and delete labels.
    pub remote: Option<&'a str>,
}

/// Tag menu like GitKraken's (M13-06).
pub fn tag(tag: Tag<'_>) -> Vec<MenuEntry> {
    let Tag {
        name,
        target,
        writable,
        head,
        head_can_fast_forward,
        resettable,
        remote,
    } = tag;
    let (push, delete_remote) = match remote {
        Some(remote) => (
            format!("Push {name} to {remote}"),
            format!("Delete {name} from {remote}"),
        ),
        None => (
            format!("Push {name}…"),
            format!("Delete {name} from remote…"),
        ),
    };
    let mut entries = vec![
        action(&push, writable, Message::PushTag(name.into())),
        MenuEntry::Separator,
    ];
    entries.extend(merge_entries(
        name,
        &format!("refs/tags/{name}"),
        head,
        head_can_fast_forward,
        writable,
    ));
    entries.push(MenuEntry::Separator);
    entries.extend(commit_entries(target, name, head, writable, resettable));
    entries.extend([
        MenuEntry::Separator,
        action(
            &format!("Delete {name} locally"),
            writable,
            Message::DeleteTag(name.into()),
        ),
        action(
            &delete_remote,
            writable,
            Message::DeleteRemoteTag(name.into()),
        ),
        MenuEntry::Separator,
        action("Copy tag name", true, Message::CopyToClipboard(name.into())),
        action(
            "Copy commit sha",
            true,
            Message::CopyToClipboard(target.into()),
        ),
    ]);
    entries
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

/// Context menu on one of several selected left panel rows (M12-11):
/// delete them all; hide or show the branches among them.
pub fn bulk(
    plan: &crate::ref_selection::BulkPlan,
    branches: bool,
    all_hidden: bool,
    writable: bool,
) -> Vec<MenuEntry> {
    let mut entries = vec![action(
        &plan.menu_label(),
        writable && !plan.is_empty(),
        Message::BulkDeleteStart,
    )];
    if branches {
        entries.push(MenuEntry::Separator);
        entries.push(if all_hidden {
            MenuEntry::item("Show in graph", Message::HideSelected(false))
        } else {
            MenuEntry::item("Hide in graph", Message::HideSelected(true))
        });
    }
    entries
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

/// Hint of the diff tool entry when Preferences has none set.
pub const NO_DIFF_TOOL: &str = "Set a diff tool in Preferences → External tools";

/// Facts about a file row for its menu (M13-07).
#[derive(Debug, Clone)]
pub struct FileFacts<'a> {
    pub file: &'a git_core::FileChange,
    pub area: crate::app::FileArea,
    pub writable: bool,
    /// Commit of a file in the commit details (history and blame start there).
    pub commit: Option<&'a str>,
    /// First parent of `commit` (`None`: root commit).
    pub parent: Option<&'a str>,
    /// A diff tool is chosen in Preferences (or Git's configured one).
    pub diff_tool: bool,
    /// "Open in external editor" (built by the app from its settings).
    pub editor: MenuEntry,
    /// Absolute path with native separators for "Copy file path".
    pub copy_path: String,
}

fn item_with_hint(label: &str, reason: Option<&str>, message: Message) -> MenuEntry {
    MenuEntry::Item {
        label: label.into(),
        shortcut: None,
        enabled: reason.is_none(),
        message: Box::new(message),
        hint: reason.map(str::to_owned),
    }
}

/// File menus like GitKraken's (M13-07): unstaged, staged and commit files.
pub fn file(facts: FileFacts<'_>) -> Vec<MenuEntry> {
    use crate::app::{DiscardTarget, FileArea};
    use git_core::ChangeKind;
    let FileFacts {
        file,
        area,
        writable,
        commit,
        parent,
        diff_tool,
        editor,
        copy_path,
    } = facts;
    let path = file.path.clone();
    let write = |label: &str, shortcut: Option<&str>, message: Message| MenuEntry::Item {
        label: label.into(),
        shortcut: shortcut.map(str::to_owned),
        enabled: writable,
        message: Box::new(message),
        hint: None,
    };
    let mut entries = Vec::new();
    match area {
        FileArea::Unstaged => entries.extend([
            write("Stage", Some("S"), Message::Stage(file.clone())),
            write(
                "Discard changes",
                None,
                Message::Discard(DiscardTarget::File(file.clone())),
            ),
            write("Ignore", None, Message::IgnoreFile(path.clone())),
            MenuEntry::Separator,
        ]),
        FileArea::Staged => entries.extend([
            write("Unstage", Some("U"), Message::Unstage(file.clone())),
            write(
                "Discard changes",
                None,
                Message::Discard(DiscardTarget::Staged(file.clone())),
            ),
            action(
                "Copy commit message prompt…",
                true,
                Message::CopyPromptRequested,
            ),
            MenuEntry::Separator,
        ]),
        FileArea::Commit => {}
    }
    if file.kind == ChangeKind::Conflicted {
        entries.push(write(
            "Open in merge tool",
            None,
            Message::OpenMergeTool(path.clone()),
        ));
    }
    let untracked = file.kind == ChangeKind::Untracked;
    let untracked_hint = untracked.then_some("Untracked files have no history");
    // History of the file (M5-09); from a commit it starts there.
    entries.push(item_with_hint(
        "File history",
        untracked_hint,
        Message::FileHistoryOpened {
            path: path.clone(),
            start: commit.map(str::to_owned),
        },
    ));
    // Blame (M5-10): the working copy, or the file at the commit; a file
    // deleted there is blamed at the parent, where it still exists.
    let (blame_reason, blame_rev) = match (area, file.kind) {
        (_, ChangeKind::Untracked) => (untracked_hint, None),
        (FileArea::Commit, ChangeKind::Added) => (Some("Added in this commit"), None),
        (FileArea::Commit, ChangeKind::Deleted) => match parent {
            Some(parent) => (None, Some(parent.to_owned())),
            None => (Some("Deleted in this commit"), None),
        },
        (FileArea::Commit, _) => (None, commit.map(str::to_owned)),
        (_, ChangeKind::Added) => (Some("Not committed yet"), None),
        // Deleted in the working copy: blame the committed version.
        (_, ChangeKind::Deleted) => (None, Some("HEAD".to_owned())),
        _ => (None, None),
    };
    entries.push(item_with_hint(
        "File blame",
        blame_reason,
        Message::FileBlameOpened {
            path: path.clone(),
            start: commit.map(str::to_owned),
            rev: blame_rev,
        },
    ));
    entries.push(MenuEntry::Separator);
    // External diff tool (M5/M8): the sides of the diff the row shows.
    let target = match area {
        FileArea::Unstaged => git_core::DiffTarget::Unstaged,
        FileArea::Staged => git_core::DiffTarget::Staged,
        FileArea::Commit => git_core::DiffTarget::Commit {
            id: commit.unwrap_or_default().to_owned(),
            parent: parent.map(str::to_owned),
        },
    };
    let diff_reason = if !diff_tool {
        Some(NO_DIFF_TOOL)
    } else if untracked {
        Some("Untracked files have no previous version")
    } else if file.kind == ChangeKind::Conflicted {
        Some("Use the merge tool for conflicts")
    } else if area == FileArea::Commit && commit.is_none() {
        Some("Select one commit")
    } else {
        None
    };
    entries.push(item_with_hint(
        "Open in external diff tool",
        diff_reason,
        Message::ExternalDiffFile {
            target,
            path: path.clone(),
            old_path: file.old_path.clone(),
        },
    ));
    entries.push(editor);
    // A file deleted in a commit is not in the working tree; others are
    // checked when the entry runs (no file I/O on the UI thread).
    let location_reason = (area == FileArea::Commit && file.kind == ChangeKind::Deleted)
        .then_some(crate::app::NOT_IN_WORKTREE);
    entries.push(item_with_hint(
        "Open file location",
        location_reason,
        Message::OpenFileLocation(path),
    ));
    entries.extend([
        MenuEntry::Separator,
        action("Copy file path", true, Message::CopyToClipboard(copy_path)),
    ]);
    entries
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
            resettable: true,
        })
    }

    fn remote_menu(behind_local: Option<&str>, head_can_fast_forward: bool) -> Vec<MenuEntry> {
        remote_branch(RemoteBranch {
            name: "origin/topic",
            target: "r1",
            writable: true,
            head: "main",
            head_can_fast_forward,
            behind_local,
            hidden: false,
            soloed: false,
            resettable: true,
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
            resettable: true,
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

    /// All labels with "---" for separators.
    fn layout(menu: &[MenuEntry]) -> Vec<&str> {
        menu.iter()
            .map(|e| match e {
                MenuEntry::Item { label, .. } => label.as_str(),
                MenuEntry::Separator => "---",
                _ => "?",
            })
            .collect()
    }

    fn tag_menu(remote: Option<&'static str>, writable: bool, resettable: bool) -> Vec<MenuEntry> {
        tag(Tag {
            name: "v1",
            target: "t1",
            writable,
            head: "main",
            head_can_fast_forward: true,
            resettable,
            remote,
        })
    }

    #[test]
    fn local_branch_menu_is_ordered_like_gitkraken() {
        assert_eq!(
            layout(&branch(false, true)),
            [
                "Checkout topic",
                "---",
                "Pull",
                "Push",
                "Set upstream…",
                "Unset upstream",
                "---",
                "Merge topic into main",
                "Rebase main onto topic",
                "---",
                "Create branch here",
                "Reset main to this commit…",
                "---",
                "Rename topic…",
                "Delete topic",
                "---",
                "Copy branch name",
                "Copy commit sha",
                "---",
                "Hide",
                "Solo",
                "---",
                "Create tag here",
            ]
        );
        let menu = branch(false, true);
        assert!(matches!(
            message_of(&menu, "Create branch here"),
            Some(Message::CreateBranchAtRef { target, label }) if target == "c1" && label == "topic"
        ));
        assert!(matches!(
            message_of(&menu, "Reset main to this commit…"),
            Some(Message::ResetStart(id)) if id == "c1"
        ));
        assert!(matches!(
            message_of(&menu, "Copy commit sha"),
            Some(Message::CopyToClipboard(id)) if id == "c1"
        ));
        assert!(matches!(
            message_of(&menu, "Copy branch name"),
            Some(Message::CopyToClipboard(name)) if name == "topic"
        ));
    }

    #[test]
    fn checked_out_branch_cannot_be_checked_out_or_deleted() {
        let head = branch(true, true);
        // Like GitKraken: no Checkout entry at all for the checked-out branch.
        assert!(!layout(&head).iter().any(|l| l.starts_with("Checkout")));
        assert_eq!(layout(&head)[0], "Pull");
        assert_eq!(hint_of(&head, "Delete topic"), Some("Checked-out branch"));
        let other = branch(false, true);
        let (enabled, _) = summary(&other);
        assert!(enabled.contains(&"Checkout topic") && enabled.contains(&"Delete topic"));
    }

    #[test]
    fn remote_branch_menu_is_ordered_like_gitkraken() {
        assert_eq!(
            layout(&remote_menu(Some("topic"), true)),
            [
                "Fast-forward topic to origin/topic",
                "Fast-forward main to origin/topic",
                "Merge origin/topic into main",
                "Rebase main onto origin/topic",
                "---",
                "Checkout origin/topic",
                "Create branch here",
                "Reset main to this commit…",
                "Delete origin/topic",
                "---",
                "Copy branch name",
                "Copy commit sha",
                "---",
                "Hide",
                "Solo",
                "---",
                "Create tag here",
            ]
        );
        let menu = remote_menu(None, false);
        assert!(matches!(
            message_of(&menu, "Create branch here"),
            Some(Message::CreateBranchAtRef { target, .. }) if target == "r1"
        ));
        assert!(matches!(
            message_of(&menu, "Delete origin/topic"),
            Some(Message::DeleteRemoteBranch(name)) if name == "origin/topic"
        ));
    }

    #[test]
    fn tag_menu_is_ordered_like_gitkraken() {
        assert_eq!(
            layout(&tag_menu(Some("origin"), true, true)),
            [
                "Push v1 to origin",
                "---",
                "Fast-forward main to v1",
                "Merge v1 into main",
                "Rebase main onto v1",
                "---",
                "Create branch here",
                "Reset main to this commit…",
                "---",
                "Delete v1 locally",
                "Delete v1 from origin",
                "---",
                "Copy tag name",
                "Copy commit sha",
            ]
        );
        // Merge entries use the tag ref; git peels it to the commit.
        let menu = tag_menu(None, true, true);
        assert!(matches!(
            message_of(&menu, "Merge v1 into main"),
            Some(Message::Merge { source, .. }) if source == "refs/tags/v1"
        ));
        // Without exactly one remote the push and delete ask for one.
        assert!(matches!(
            message_of(&menu, "Push v1…"),
            Some(Message::PushTag(t)) if t == "v1"
        ));
        assert!(message_of(&menu, "Delete v1 from remote…").is_some());
        // Create branch here and Copy commit sha use the peeled commit.
        assert!(matches!(
            message_of(&menu, "Create branch here"),
            Some(Message::CreateBranchAtRef { target, label }) if target == "t1" && label == "v1"
        ));
        assert!(matches!(
            message_of(&menu, "Copy commit sha"),
            Some(Message::CopyToClipboard(id)) if id == "t1"
        ));
    }

    #[test]
    fn reset_from_a_ref_follows_the_commit_facts() {
        let menu = tag_menu(None, true, false);
        let (enabled, disabled) = summary(&menu);
        assert!(enabled.contains(&"Create branch here"));
        assert!(disabled
            .iter()
            .any(|(l, _)| *l == "Reset main to this commit…"));
        let mut facts = LocalBranch {
            name: "topic",
            target: "c1",
            is_head: false,
            has_upstream: false,
            writable: true,
            head: "main",
            head_can_fast_forward: false,
            hidden: false,
            soloed: false,
            resettable: false,
        };
        assert!(summary(&local_branch(facts))
            .1
            .iter()
            .any(|(l, _)| *l == "Reset main to this commit…"));
        facts.resettable = true;
        assert!(summary(&local_branch(facts))
            .0
            .contains(&"Reset main to this commit…"));
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
        let none = commit("c1", "msg", true, CommitFacts::default(), "main", None);
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
        let menu = commit("c1", "msg", true, all, "topic", None);
        let (_, disabled) = summary(&menu);
        assert!(disabled.is_empty(), "{disabled:?}");
        assert!(matches!(
            message_of(&commit("c1", "msg", true, all, "topic", None), "Reset topic to this commit…"),
            Some(Message::ResetStart(id)) if id == "c1"
        ));
    }

    /// M15-11: "Change author…" follows the message entry, for all
    /// selected commits; disabled with the reason of the first blocked one.
    #[test]
    fn commit_menu_change_author() {
        let ids = vec!["c1".to_owned(), "c3".to_owned()];
        let menu = commit(
            "c1",
            "msg",
            true,
            CommitFacts::default(),
            "main",
            Some(AuthorChange {
                ids: ids.clone(),
                block: None,
            }),
        );
        let labels: Vec<&str> = menu
            .iter()
            .filter_map(|e| match e {
                MenuEntry::Item { label, .. } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        let at = labels.iter().position(|l| *l == "Change author…").unwrap();
        assert_eq!(labels[at - 1], "Edit commit message");
        assert!(matches!(
            message_of(&menu, "Change author…"),
            Some(Message::ChangeAuthorStart(i)) if i == ids
        ));
        let blocked = commit(
            "c1",
            "msg",
            true,
            CommitFacts::default(),
            "main",
            Some(AuthorChange {
                ids,
                block: Some("c3c3c3c: Check out a branch that contains this commit".into()),
            }),
        );
        assert_eq!(
            hint_of(&blocked, "Change author…"),
            Some("c3c3c3c: Check out a branch that contains this commit")
        );
        // Stashes: no entry.
        let stash = commit("s", "msg", true, CommitFacts::default(), "main", None);
        assert!(message_of(&stash, "Change author…").is_none());
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
            resettable: true,
        });
        // Hiding only changes the view, not the repository.
        assert_eq!(
            summary(&menu).0,
            ["Copy branch name", "Copy commit sha", "Hide", "Solo"]
        );
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
                "main",
                None
            ))
            .0,
            ["Copy SHA", "Copy message"]
        );
        assert_eq!(
            summary(&tag_menu(Some("origin"), false, true)).0,
            ["Copy tag name", "Copy commit sha"]
        );
        let remote_read_only = remote_branch(RemoteBranch {
            name: "origin/topic",
            target: "r1",
            writable: false,
            head: "main",
            head_can_fast_forward: true,
            behind_local: Some("topic"),
            hidden: false,
            soloed: false,
            resettable: true,
        });
        assert_eq!(
            summary(&remote_read_only).0,
            ["Copy branch name", "Copy commit sha", "Hide", "Solo"]
        );
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
            resettable: true,
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

    fn change(kind: git_core::ChangeKind) -> git_core::FileChange {
        git_core::FileChange {
            path: "src/a.rs".into(),
            kind,
            old_path: None,
        }
    }

    fn file_menu(
        change: &git_core::FileChange,
        area: crate::app::FileArea,
        writable: bool,
        diff_tool: bool,
    ) -> Vec<MenuEntry> {
        let commit = (area == crate::app::FileArea::Commit).then_some("c2");
        super::file(FileFacts {
            file: change,
            area,
            writable,
            commit,
            parent: commit.map(|_| "c1"),
            diff_tool,
            editor: action("Open in external editor", true, Message::OpenInEditor(None)),
            copy_path: "/repo/src/a.rs".into(),
        })
    }

    #[test]
    fn file_menus_are_ordered_like_gitkraken() {
        use crate::app::FileArea;
        use git_core::ChangeKind;
        let modified = change(ChangeKind::Modified);
        let tail = [
            "---",
            "Open in external diff tool",
            "Open in external editor",
            "Open file location",
            "---",
            "Copy file path",
        ];
        let with_tail = |head: &[&'static str]| -> Vec<&'static str> {
            head.iter().copied().chain(tail).collect()
        };
        assert_eq!(
            layout(&file_menu(&modified, FileArea::Unstaged, true, true)),
            with_tail(&[
                "Stage",
                "Discard changes",
                "Ignore",
                "---",
                "File history",
                "File blame",
            ])
        );
        assert_eq!(
            layout(&file_menu(&modified, FileArea::Staged, true, true)),
            with_tail(&[
                "Unstage",
                "Discard changes",
                "Copy commit message prompt…",
                "---",
                "File history",
                "File blame",
            ])
        );
        assert_eq!(
            layout(&file_menu(&modified, FileArea::Commit, true, true)),
            with_tail(&["File history", "File blame"])
        );
        // Conflicted files add the merge tool before the history.
        let conflicted = change(ChangeKind::Conflicted);
        assert_eq!(
            layout(&file_menu(&conflicted, FileArea::Unstaged, true, true))[4..7],
            ["Open in merge tool", "File history", "File blame"]
        );
        let menu = file_menu(&conflicted, FileArea::Unstaged, true, true);
        assert_eq!(
            hint_of(&menu, "Open in external diff tool"),
            Some("Use the merge tool for conflicts")
        );
    }

    #[test]
    fn file_menu_messages() {
        use crate::app::{DiscardTarget, FileArea};
        use git_core::{ChangeKind, DiffTarget};
        let modified = change(ChangeKind::Modified);
        let staged = file_menu(&modified, FileArea::Staged, true, true);
        assert!(matches!(
            message_of(&staged, "Discard changes"),
            Some(Message::Discard(DiscardTarget::Staged(f))) if f.path == "src/a.rs"
        ));
        assert!(matches!(
            message_of(&staged, "Open in external diff tool"),
            Some(Message::ExternalDiffFile {
                target: DiffTarget::Staged,
                ..
            })
        ));
        assert!(matches!(
            message_of(&staged, "File blame"),
            Some(Message::FileBlameOpened {
                start: None,
                rev: None,
                ..
            })
        ));
        // Commit files: parent against commit, blame and history at the commit.
        let commit = file_menu(&modified, FileArea::Commit, true, true);
        assert!(matches!(
            message_of(&commit, "Open in external diff tool"),
            Some(Message::ExternalDiffFile {
                target: DiffTarget::Commit { id, parent: Some(p) },
                ..
            }) if id == "c2" && p == "c1"
        ));
        assert!(matches!(
            message_of(&commit, "File blame"),
            Some(Message::FileBlameOpened { start: Some(s), rev: Some(r), .. })
                if s == "c2" && r == "c2"
        ));
        assert!(matches!(
            message_of(&commit, "Copy file path"),
            Some(Message::CopyToClipboard(p)) if p == "/repo/src/a.rs"
        ));
        // Deleted in the commit: blame at the parent, not in the worktree.
        let deleted_file = change(ChangeKind::Deleted);
        let deleted = file_menu(&deleted_file, FileArea::Commit, true, true);
        assert!(matches!(
            message_of(&deleted, "File blame"),
            Some(Message::FileBlameOpened { rev: Some(r), .. }) if r == "c1"
        ));
        assert_eq!(
            hint_of(&deleted, "Open file location"),
            Some(crate::app::NOT_IN_WORKTREE)
        );
        // Deleted in the working copy: blame the committed version.
        let gone = file_menu(&deleted_file, FileArea::Unstaged, true, true);
        assert!(matches!(
            message_of(&gone, "File blame"),
            Some(Message::FileBlameOpened { rev: Some(r), .. }) if r == "HEAD"
        ));
    }

    #[test]
    fn file_menu_enable_rules() {
        use crate::app::FileArea;
        use git_core::ChangeKind;
        let untracked_file = change(ChangeKind::Untracked);
        let untracked = file_menu(&untracked_file, FileArea::Unstaged, true, true);
        for label in ["File history", "File blame"] {
            assert_eq!(
                hint_of(&untracked, label),
                Some("Untracked files have no history")
            );
        }
        assert!(hint_of(&untracked, "Open in external diff tool").is_some());
        let added_file = change(ChangeKind::Added);
        let added = file_menu(&added_file, FileArea::Commit, true, true);
        assert_eq!(hint_of(&added, "File blame"), Some("Added in this commit"));
        assert!(summary(&added).0.contains(&"File history"));
        // No diff tool chosen.
        let modified = change(ChangeKind::Modified);
        let menu = file_menu(&modified, FileArea::Unstaged, true, false);
        assert_eq!(
            hint_of(&menu, "Open in external diff tool"),
            Some(NO_DIFF_TOOL)
        );
        // Read-only: only the write entries are disabled.
        for area in [FileArea::Unstaged, FileArea::Staged] {
            let menu = file_menu(&modified, area, false, true);
            let (enabled, disabled) = summary(&menu);
            let disabled: Vec<_> = disabled.iter().map(|(l, _)| *l).collect();
            assert!(disabled.contains(&"Discard changes"), "{area:?}");
            assert!(enabled.contains(&"File blame") && enabled.contains(&"Copy file path"));
            assert!(enabled.contains(&"Open in external diff tool"));
        }
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
