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
    /// Esc: close the dialog, the menu or the diff view (in that order).
    Close,
    /// Enter: confirm the default button of an open dialog.
    Confirm,
}

impl Shortcut {
    /// Shortcuts that still work while a text input has focus
    /// (Enter only acts while a dialog is open).
    pub fn works_in_text_input(self) -> bool {
        matches!(self, Self::Close | Self::Confirm)
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
        Key::Named(Named::ArrowUp) if mac_jump => Some(Shortcut::Navigate(Move::First)),
        Key::Named(Named::ArrowDown) if mac_jump => Some(Shortcut::Navigate(Move::Last)),
        Key::Named(Named::Home) if command && !alt => Some(Shortcut::Navigate(Move::First)),
        Key::Named(Named::End) if command && !alt => Some(Shortcut::Navigate(Move::Last)),
        Key::Named(Named::ArrowDown) if plain => Some(Shortcut::Navigate(if shift {
            Move::FirstParent
        } else {
            Move::Next
        })),
        Key::Named(Named::ArrowUp) if plain => Some(Shortcut::Navigate(if shift {
            Move::Child
        } else {
            Move::Previous
        })),
        Key::Character(c) if plain => match c {
            "j" => Some(Shortcut::Navigate(Move::Next)),
            "k" => Some(Shortcut::Navigate(Move::Previous)),
            "J" => Some(Shortcut::Navigate(Move::FirstParent)),
            "K" => Some(Shortcut::Navigate(Move::Child)),
            _ => None,
        },
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
            nav(Move::FirstParent)
        );
        assert_eq!(
            shortcut(&Key::Named(Named::ArrowUp), Modifiers::SHIFT),
            nav(Move::Child)
        );
        assert_eq!(shortcut(&ch("J"), Modifiers::SHIFT), nav(Move::FirstParent));
        assert_eq!(shortcut(&ch("K"), Modifiers::SHIFT), nav(Move::Child));
        assert_eq!(
            shortcut(&Key::Named(Named::Home), Modifiers::COMMAND),
            nav(Move::First)
        );
        assert_eq!(
            shortcut(&Key::Named(Named::End), Modifiers::COMMAND),
            nav(Move::Last)
        );
        // Plain Home/End and Alt+arrows are not graph shortcuts.
        assert_eq!(shortcut(&Key::Named(Named::Home), none), None);
        assert_eq!(
            shortcut(&Key::Named(Named::ArrowDown), Modifiers::ALT),
            None
        );
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
    fn f5_refreshes() {
        assert_eq!(
            shortcut(&Key::Named(Named::F5), Modifiers::empty()),
            Some(Shortcut::Refresh)
        );
    }
}
