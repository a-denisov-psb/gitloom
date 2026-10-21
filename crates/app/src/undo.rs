//! Undoable actions of a tab (ADR-014). The stack is filled by write
//! operations; undo and redo themselves follow in M2-12.

/// An action that can be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoAction {
    /// Discarded working tree changes with the content before the discard.
    Discard {
        label: String,
        snapshot: git_core::DiscardSnapshot,
    },
}

impl UndoAction {
    /// Shown in the Undo tooltip, e.g. "Discard src/main.rs".
    // Used by the Undo button (M2-12).
    #[allow(dead_code)]
    pub fn label(&self) -> &str {
        match self {
            Self::Discard { label, .. } => label,
        }
    }
}
