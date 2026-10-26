//! Keyboard shortcuts (see `docs/UX-PARITY.md`). Ctrl on Windows/Linux, Cmd on macOS.

use iced::keyboard::{key::Named, Key, Modifiers};

use crate::nav::Move;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
    /// Zero-based tab index (Ctrl+1 … Ctrl+9).
    SwitchTab(usize),
    OpenRepository,
    Refresh,
    ToggleSidebar,
    TogglePanel,
    Navigate(Move),
    FocusFilter,
    /// S / U: stage or unstage the file whose diff is shown.
    StageSelected,
    UnstageSelected,
    StageAll,
    UnstageAll,
    /// Ctrl+L: fetch all remotes.
    FetchAll,
    /// Shift+↑/↓: extend the commit selection by one row.
    ExtendSelection(Move),
    /// Ctrl+B: new branch at the selected commit.
    CreateBranch,
    /// Ctrl+Z.
    Undo,
    /// Ctrl+Y or Ctrl+Shift+Z.
    Redo,
    /// Ctrl+Enter: commit staged changes.
    Commit,
    /// Ctrl+Shift+Enter: stage everything and commit.
    StageAllAndCommit,
    /// Ctrl+Shift+M: focus the commit summary.
    FocusSummary,
    /// Esc: close the dialog, the menu or the diff view (in that order).
    Close,
    /// Enter: confirm the default button of an open dialog.
    Confirm,
    /// P / R / F / D in the interactive rebase panel (S arrives as
    /// `StageSelected` and means squash there).
    TodoKey(char),
    /// Alt+↑ / Alt+↓: move the selected row of the interactive rebase panel.
    Reorder {
        up: bool,
    },
}

impl Shortcut {
    /// Shortcuts that still work while a text input has focus
    /// (Enter only acts while a dialog is open).
    pub fn works_in_text_input(self) -> bool {
        matches!(
            self,
            Self::Close
                | Self::Confirm
                | Self::Commit
                | Self::StageAllAndCommit
                | Self::FocusSummary
        )
    }
}

/// Maps a key press to a shortcut, if any.
pub fn shortcut(key: &Key, modifiers: Modifiers) -> Option<Shortcut> {
    let command = modifiers.command();
    let shift = modifiers.shift();
    let alt = modifiers.alt();
    let plain = !command && !alt && !modifiers.logo();
    // macOS: Cmd+↑ / Cmd+↓ jump to the first / last commit.
    let mac_jump = cfg!(target_os = "macos") && modifiers.logo() && !alt;
    match key.as_ref() {
        Key::Named(Named::F5) if !command && !alt => Some(Shortcut::Refresh),
        Key::Named(Named::Escape) if plain && !shift => Some(Shortcut::Close),
        Key::Named(Named::Enter) if plain && !shift => Some(Shortcut::Confirm),
        Key::Named(Named::Enter) if command && !alt => Some(if shift {
            Shortcut::StageAllAndCommit
        } else {
            Shortcut::Commit
        }),
        Key::Character("m" | "M") if command && shift && !alt => Some(Shortcut::FocusSummary),
        Key::Character("z" | "Z") if command && !alt => Some(if shift {
            Shortcut::Redo
        } else {
            Shortcut::Undo
        }),
        Key::Character("y" | "Y") if command && !alt && !shift => Some(Shortcut::Redo),
        Key::Named(Named::ArrowUp) if mac_jump => Some(Shortcut::Navigate(Move::First)),
        Key::Named(Named::ArrowDown) if mac_jump => Some(Shortcut::Navigate(Move::Last)),
        Key::Named(Named::ArrowUp) if alt && !command && !shift => {
            Some(Shortcut::Reorder { up: true })
        }
        Key::Named(Named::ArrowDown) if alt && !command && !shift => {
            Some(Shortcut::Reorder { up: false })
        }
        Key::Named(Named::Home) if command && !alt => Some(Shortcut::Navigate(Move::First)),
        Key::Named(Named::End) if command && !alt => Some(Shortcut::Navigate(Move::Last)),
        // Shift+↑/↓ extend the selection (M4-01); Shift+J/K stay parent/child.
        Key::Named(Named::ArrowDown) if plain => Some(if shift {
            Shortcut::ExtendSelection(Move::Next)
        } else {
            Shortcut::Navigate(Move::Next)
        }),
        Key::Named(Named::ArrowUp) if plain => Some(if shift {
            Shortcut::ExtendSelection(Move::Previous)
        } else {
            Shortcut::Navigate(Move::Previous)
        }),
        // iced reports the key without modifiers ("j" for Shift+J, from winit's
        // `key_without_modifiers`); upper case is accepted too (e.g. Caps Lock).
        Key::Character(c) if plain => match (c.to_lowercase().as_str(), shift) {
            ("j", false) => Some(Shortcut::Navigate(Move::Next)),
            ("k", false) => Some(Shortcut::Navigate(Move::Previous)),
            ("j", true) => Some(Shortcut::Navigate(Move::FirstParent)),
            ("k", true) => Some(Shortcut::Navigate(Move::Child)),
            ("s", false) => Some(Shortcut::StageSelected),
            (key @ ("p" | "r" | "f" | "d"), false) => key.chars().next().map(Shortcut::TodoKey),
            ("u", false) => Some(Shortcut::UnstageSelected),
            _ => None,
        },
        Key::Character("s" | "S") if command && shift && !alt => Some(Shortcut::StageAll),
        Key::Character("u" | "U") if command && shift && !alt => Some(Shortcut::UnstageAll),
        Key::Character("f" | "F") if command && alt => Some(Shortcut::FocusFilter),
        Key::Named(Named::Tab) if modifiers.control() && !alt => Some(if shift {
            Shortcut::PreviousTab
        } else {
            Shortcut::NextTab
        }),
        Key::Character(c) if command && !alt && !shift => match c {
            "t" | "T" => Some(Shortcut::NewTab),
            "w" | "W" => Some(Shortcut::CloseTab),
            "o" | "O" => Some(Shortcut::OpenRepository),
            "l" | "L" => Some(Shortcut::FetchAll),
            "b" | "B" => Some(Shortcut::CreateBranch),
            "j" | "J" => Some(Shortcut::ToggleSidebar),
            "k" | "K" => Some(Shortcut::TogglePanel),
            digit => digit
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=9).contains(n))
                .map(|n| Shortcut::SwitchTab(n - 1)),
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(c: &str) -> Key {
        Key::Character(c.into())
    }

    #[test]
    fn fetch_shortcut() {
        assert_eq!(
            shortcut(&ch("l"), Modifiers::COMMAND),
            Some(Shortcut::FetchAll)
        );
        assert_eq!(shortcut(&ch("l"), Modifiers::empty()), None);
        assert_eq!(
            shortcut(&ch("b"), Modifiers::COMMAND),
            Some(Shortcut::CreateBranch)
        );
    }

    #[test]
    fn tab_shortcuts() {
        let cmd = Modifiers::COMMAND;
        assert_eq!(shortcut(&ch("t"), cmd), Some(Shortcut::NewTab));
        assert_eq!(shortcut(&ch("w"), cmd), Some(Shortcut::CloseTab));
        assert_eq!(shortcut(&ch("o"), cmd), Some(Shortcut::OpenRepository));
        assert_eq!(shortcut(&ch("1"), cmd), Some(Shortcut::SwitchTab(0)));
        assert_eq!(shortcut(&ch("9"), cmd), Some(Shortcut::SwitchTab(8)));
        assert_eq!(shortcut(&ch("0"), cmd), None);
    }

    #[test]
    fn panel_toggles() {
        let cmd = Modifiers::COMMAND;
        assert_eq!(shortcut(&ch("j"), cmd), Some(Shortcut::ToggleSidebar));
        assert_eq!(shortcut(&ch("k"), cmd), Some(Shortcut::TogglePanel));
    }

    #[test]
    fn ctrl_tab_cycles() {
        let tab = Key::Named(Named::Tab);
        assert_eq!(shortcut(&tab, Modifiers::CTRL), Some(Shortcut::NextTab));
        assert_eq!(
            shortcut(&tab, Modifiers::CTRL | Modifiers::SHIFT),
            Some(Shortcut::PreviousTab)
        );
        assert_eq!(shortcut(&tab, Modifiers::empty()), None);
    }

    #[test]
    fn plain_letters_are_no_tab_shortcuts() {
        assert_eq!(shortcut(&ch("t"), Modifiers::empty()), None);
        assert_eq!(
            shortcut(&ch("t"), Modifiers::COMMAND | Modifiers::ALT),
            None
        );
    }

    #[test]
    fn graph_navigation() {
        let none = Modifiers::empty();
        let nav = |m| Some(Shortcut::Navigate(m));
        assert_eq!(
            shortcut(&Key::Named(Named::ArrowDown), none),
            nav(Move::Next)
        );
        assert_eq!(
            shortcut(&Key::Named(Named::ArrowUp), none),
            nav(Move::Previous)
        );
        assert_eq!(shortcut(&ch("j"), none), nav(Move::Next));
        assert_eq!(shortcut(&ch("k"), none), nav(Move::Previous));
        assert_eq!(
            shortcut(&Key::Named(Named::ArrowDown), Modifiers::SHIFT),
            Some(Shortcut::ExtendSelection(Move::Next))
        );
        assert_eq!(
            shortcut(&Key::Named(Named::ArrowUp), Modifiers::SHIFT),
            Some(Shortcut::ExtendSelection(Move::Previous))
        );
        // As iced delivers Shift+J: the key without modifiers plus Shift
        // (regression, M2-01 check on Windows).
        assert_eq!(shortcut(&ch("j"), Modifiers::SHIFT), nav(Move::FirstParent));
        assert_eq!(shortcut(&ch("k"), Modifiers::SHIFT), nav(Move::Child));
        assert_eq!(shortcut(&ch("J"), Modifiers::SHIFT), nav(Move::FirstParent));
        assert_eq!(shortcut(&ch("K"), Modifiers::SHIFT), nav(Move::Child));
        assert_eq!(
            shortcut(&ch("s"), Modifiers::SHIFT),
            None,
            "Shift+S stages nothing"
        );
        assert_eq!(
            shortcut(&Key::Named(Named::Home), Modifiers::COMMAND),
            nav(Move::First)
        );
        assert_eq!(
            shortcut(&Key::Named(Named::End), Modifiers::COMMAND),
            nav(Move::Last)
        );
        // Plain Home/End are not graph shortcuts; Alt+arrows reorder the
        // interactive rebase panel.
        assert_eq!(shortcut(&Key::Named(Named::Home), none), None);
        assert_eq!(
            shortcut(&Key::Named(Named::ArrowDown), Modifiers::ALT),
            Some(Shortcut::Reorder { up: false })
        );
        assert_eq!(shortcut(&ch("p"), none), Some(Shortcut::TodoKey('p')));
        assert_eq!(shortcut(&ch("d"), none), Some(Shortcut::TodoKey('d')));
    }

    #[test]
    fn filter_focus_and_close() {
        assert_eq!(
            shortcut(&ch("f"), Modifiers::COMMAND | Modifiers::ALT),
            Some(Shortcut::FocusFilter)
        );
        assert_eq!(
            shortcut(&Key::Named(Named::Escape), Modifiers::empty()),
            Some(Shortcut::Close)
        );
        assert!(Shortcut::Close.works_in_text_input());
        assert_eq!(
            shortcut(&Key::Named(Named::Enter), Modifiers::empty()),
            Some(Shortcut::Confirm)
        );
        assert!(!Shortcut::Navigate(Move::Next).works_in_text_input());
    }

    #[test]
    fn staging_shortcuts() {
        assert_eq!(
            shortcut(&ch("s"), Modifiers::empty()),
            Some(Shortcut::StageSelected)
        );
        assert_eq!(
            shortcut(&ch("u"), Modifiers::empty()),
            Some(Shortcut::UnstageSelected)
        );
        let all = Modifiers::COMMAND | Modifiers::SHIFT;
        assert_eq!(shortcut(&ch("S"), all), Some(Shortcut::StageAll));
        assert_eq!(shortcut(&ch("U"), all), Some(Shortcut::UnstageAll));
    }

    #[test]
    fn commit_shortcuts() {
        let enter = Key::Named(Named::Enter);
        assert_eq!(shortcut(&enter, Modifiers::COMMAND), Some(Shortcut::Commit));
        assert_eq!(
            shortcut(&enter, Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Shortcut::StageAllAndCommit)
        );
        assert_eq!(
            shortcut(&ch("M"), Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Shortcut::FocusSummary)
        );
        assert!(Shortcut::Commit.works_in_text_input());
    }

    #[test]
    fn undo_redo_shortcuts() {
        assert_eq!(shortcut(&ch("z"), Modifiers::COMMAND), Some(Shortcut::Undo));
        assert_eq!(shortcut(&ch("y"), Modifiers::COMMAND), Some(Shortcut::Redo));
        assert_eq!(
            shortcut(&ch("Z"), Modifiers::COMMAND | Modifiers::SHIFT),
            Some(Shortcut::Redo)
        );
        assert!(!Shortcut::Undo.works_in_text_input());
    }

    #[test]
    fn f5_refreshes() {
        assert_eq!(
            shortcut(&Key::Named(Named::F5), Modifiers::empty()),
            Some(Shortcut::Refresh)
        );
    }
}
