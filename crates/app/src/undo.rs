//! Undoable actions of a tab (ADR-014).

/// An action on the undo or redo stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoAction {
    /// Working tree content to restore (before a discard, or after it for redo).
    Discard {
        label: String,
        snapshot: git_core::DiscardSnapshot,
    },
    /// A commit or amend that moved HEAD from `before` (`None`: unborn) to `after`.
    Commit {
        amend: bool,
        before: Option<String>,
        after: String,
    },
    /// Checkout, branch or tag change (M3-12).
    Ref(git_core::RefChange),
    /// Merge, fast-forward, rebase, cherry-pick, revert, reset, reword or
    /// squash (M4-12).
    History(git_core::HistoryChange),
    /// A whole Git flow finish (M7-11): refs and checked-out branch.
    FlowFinish(Box<git_core::flow_finish::FinishRecord>),
    /// A stash pop or apply, with or without conflicts (M9-02).
    Stash(Box<git_core::StashRecord>),
    /// Stage, unstage or mark resolved (M12-07).
    Work(Box<git_core::work_undo::WorkChange>),
    /// "Stash" (M12-07).
    StashPush(Box<git_core::work_undo::StashPushRecord>),
    /// "Delete stash" (M12-07).
    StashDrop(git_core::work_undo::StashDropRecord),
    /// Remote added, removed, renamed or edited; upstream set or unset (M12-08).
    Remote(git_core::remote_undo::RemoteChange),
    /// "Rename stash" (M12-10).
    StashRename(git_core::work_undo::StashRename),
    /// Checkouts of a submodule update (M12-08).
    Submodules {
        label: String,
        record: git_core::submodule::UpdateRecord,
    },
}

impl UndoAction {
    /// Undoing or redoing it moves HEAD (blocked during merge/rebase).
    pub fn moves_head(&self) -> bool {
        matches!(
            self,
            Self::Commit { .. }
                | Self::Ref(git_core::RefChange::Checkout { .. })
                | Self::Ref(git_core::RefChange::CreateBranch { .. })
                | Self::FlowFinish(_)
        ) || matches!(self, Self::History(change)
            if !matches!(change.kind, git_core::HistoryUndo::MoveBranch { .. }))
    }

    /// Shown in the Undo/Redo tooltips, e.g. "Commit abc1234".
    pub fn label(&self) -> String {
        match self {
            Self::Discard { label, .. } => label.clone(),
            Self::Commit { amend, after, .. } => format!(
                "{} {}",
                if *amend { "Amend" } else { "Commit" },
                &after[..after.len().min(7)]
            ),
            Self::Ref(change) => change.label(),
            Self::History(change) => change.label.clone(),
            Self::FlowFinish(record) => record.label.clone(),
            Self::Stash(record) => record.label().to_owned(),
            Self::Work(change) => change.label.clone(),
            Self::StashPush(_) => "Stash".to_owned(),
            Self::StashDrop(_) => "Delete stash".to_owned(),
            Self::StashRename(_) => "Rename stash".to_owned(),
            Self::Remote(change) => change.label(),
            Self::Submodules { label, .. } => label.clone(),
        }
    }
}

/// How a finished write changes the undo and redo stacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum History {
    /// A new action: pushed onto the undo stack, the redo stack is cleared.
    Push(UndoAction),
    /// An undone action: pushed onto the redo stack.
    Undone(UndoAction),
    /// A redone action: pushed back onto the undo stack.
    Redone(UndoAction),
    /// An operation that stopped (conflicts, empty step): its entry is
    /// kept aside and pushed once the operation completes (M4-12).
    Started(UndoAction),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        let commit = UndoAction::Commit {
            amend: false,
            before: None,
            after: "abcdef1234".into(),
        };
        assert_eq!(commit.label(), "Commit abcdef1");
        let amend = UndoAction::Commit {
            amend: true,
            before: Some("1".into()),
            after: "1234567890".into(),
        };
        assert_eq!(amend.label(), "Amend 1234567");
        let rename = UndoAction::Ref(git_core::RefChange::RenameBranch {
            old: "a".into(),
            new: "b".into(),
        });
        assert_eq!(rename.label(), "Rename branch a → b");
        assert!(!rename.moves_head());
        assert!(commit.moves_head());
        let drop = UndoAction::StashDrop(git_core::work_undo::StashDropRecord {
            stash: "s".into(),
            message: "On main: x".into(),
            position: 0,
        });
        assert_eq!(drop.label(), "Delete stash");
        let unset = UndoAction::Remote(git_core::remote_undo::RemoteChange::Upstream {
            branch: "main".into(),
            before: Some(("origin".into(), "refs/heads/main".into())),
            after: None,
        });
        assert_eq!(unset.label(), "Unset upstream of main");
        assert!(!drop.moves_head());
    }
}
