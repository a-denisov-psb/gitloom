//! Keyboard shortcuts (see `docs/UX-PARITY.md`). Ctrl on Windows/Linux, Cmd on macOS.

use iced::keyboard::{key::Named, Key, Modifiers};

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
}

/// Maps a key press to a shortcut, if any.
pub fn shortcut(key: &Key, modifiers: Modifiers) -> Option<Shortcut> {
    let command = modifiers.command();
    let shift = modifiers.shift();
    let alt = modifiers.alt();
    match key.as_ref() {
        Key::Named(Named::F5) if !command && !alt => Some(Shortcut::Refresh),
        Key::Named(Named::Tab) if modifiers.control() && !alt => Some(if shift {
            Shortcut::PreviousTab
        } else {
            Shortcut::NextTab
        }),
        Key::Character(c) if command && !alt && !shift => match c {
            "t" | "T" => Some(Shortcut::NewTab),
            "w" | "W" => Some(Shortcut::CloseTab),
            "o" | "O" => Some(Shortcut::OpenRepository),
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
    fn f5_refreshes() {
        assert_eq!(
            shortcut(&Key::Named(Named::F5), Modifiers::empty()),
            Some(Shortcut::Refresh)
        );
    }
}
