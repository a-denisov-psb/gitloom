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
}

impl UndoAction {
    /// Shown in the Undo/Redo tooltips, e.g. "Commit abc1234".
    pub fn label(&self) -> String {
        match self {
            Self::Discard { label, .. } => label.clone(),
            Self::Commit { amend, after, .. } => format!(
                "{} {}",
                if *amend { "Amend" } else { "Commit" },
                &after[..after.len().min(7)]
            ),
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
    }
}
