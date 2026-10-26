//! Keyboard shortcuts (see `docs/UX-PARITY.md`). Ctrl on Windows/Linux, Cmd on macOS.

use iced::keyboard::{Key, Modifiers};

/// Keyboard dispatch works on the registered actions (M5-01).
pub use crate::actions::ActionId as Shortcut;

/// Maps a key press to a shortcut, if any.
#[cfg(test)]
pub fn shortcut(key: &Key, modifiers: Modifiers) -> Option<Shortcut> {
    crate::actions::action_for_key(key, key, modifiers)
}

/// Like [`shortcut`], with the key as the layout produces it with Shift
/// (for Ctrl+/ and Ctrl++ on layouts that need Shift for them).
pub fn shortcut_with_modified(
    key: &Key,
    modified_key: &Key,
    modifiers: Modifiers,
) -> Option<Shortcut> {
    crate::actions::action_for_key(key, modified_key, modifiers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::Move;
    use iced::keyboard::key::Named;

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
        // Ctrl+0 resets the zoom (M5-04).
        assert_eq!(shortcut(&ch("0"), cmd), Some(Shortcut::ZoomReset));
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
