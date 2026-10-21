//! State of the window-wide overlays: modal dialog and context menu.

use iced::{Point, Size};

use crate::app::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    /// Destructive action, shown in the danger style.
    // First used by discard confirmations (M2-08).
    #[allow(dead_code)]
    Danger,
}

#[derive(Debug, Clone)]
pub struct DialogButton {
    pub label: String,
    pub kind: ButtonKind,
    /// Sent after the dialog closed; `None` only closes it.
    pub message: Option<Message>,
}

/// Modal dialog over the whole window.
#[derive(Debug, Clone)]
pub struct Modal {
    pub title: String,
    pub text: String,
    /// Monospace, scrollable details (e.g. Git output).
    pub details: Option<String>,
    pub buttons: Vec<DialogButton>,
    /// Button triggered by Enter.
    pub default: Option<usize>,
}

impl Modal {
    /// Failed operation: summary plus the full Git output.
    pub fn error(title: impl Into<String>, details: impl Into<String>) -> Self {
        let details = details.into();
        Self {
            title: title.into(),
            text: "Git reported an error. The full output is shown below.".into(),
            details: Some(details.clone()),
            buttons: vec![
                DialogButton {
                    label: "Copy".into(),
                    kind: ButtonKind::Secondary,
                    message: Some(Message::CopyToClipboard(details)),
                },
                DialogButton {
                    label: "Close".into(),
                    kind: ButtonKind::Primary,
                    message: None,
                },
            ],
            default: Some(1),
        }
    }

    /// Asks before an action; destructive actions get the danger style.
    // First used by discard confirmations (M2-08).
    #[allow(dead_code)]
    pub fn confirm(
        title: impl Into<String>,
        text: impl Into<String>,
        confirm_label: impl Into<String>,
        destructive: bool,
        on_confirm: Message,
    ) -> Self {
        Self {
            title: title.into(),
            text: text.into(),
            details: None,
            buttons: vec![
                DialogButton {
                    label: "Cancel".into(),
                    kind: ButtonKind::Secondary,
                    message: None,
                },
                DialogButton {
                    label: confirm_label.into(),
                    kind: if destructive {
                        ButtonKind::Danger
                    } else {
                        ButtonKind::Primary
                    },
                    message: Some(on_confirm),
                },
            ],
            default: Some(1),
        }
    }

    /// Message of button `index`, if it has one (closing happens in any case).
    pub fn action(&self, index: usize) -> Option<Message> {
        self.buttons.get(index).and_then(|b| b.message.clone())
    }

    /// Message of the default button (Enter).
    pub fn default_action(&self) -> Option<Message> {
        self.default.and_then(|i| self.action(i))
    }
}

#[derive(Debug, Clone)]
pub enum MenuEntry {
    Item {
        label: String,
        /// Shortcut hint shown on the right, e.g. "S".
        shortcut: Option<String>,
        enabled: bool,
        message: Box<Message>,
    },
    // First used by the file and stash menus (M2-08, M2-11).
    #[allow(dead_code)]
    Separator,
}

impl MenuEntry {
    pub fn item(label: impl Into<String>, message: Message) -> Self {
        Self::Item {
            label: label.into(),
            shortcut: None,
            enabled: true,
            message: Box::new(message),
        }
    }
}

/// Context menu opened at the cursor.
#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub position: Point,
    pub entries: Vec<MenuEntry>,
}

/// Approximate menu size used to keep it inside the window.
pub const MENU_WIDTH: f32 = 220.0;
pub const MENU_ITEM_HEIGHT: f32 = 26.0;
pub const MENU_SEPARATOR_HEIGHT: f32 = 9.0;

impl ContextMenu {
    pub fn new(position: Point, entries: Vec<MenuEntry>) -> Self {
        Self { position, entries }
    }

    /// Message of entry `index` if it is an enabled item.
    pub fn select(&self, index: usize) -> Option<Message> {
        match self.entries.get(index)? {
            MenuEntry::Item {
                enabled: true,
                message,
                ..
            } => Some(message.as_ref().clone()),
            _ => None,
        }
    }

    pub fn height(&self) -> f32 {
        self.entries
            .iter()
            .map(|e| match e {
                MenuEntry::Item { .. } => MENU_ITEM_HEIGHT,
                MenuEntry::Separator => MENU_SEPARATOR_HEIGHT,
            })
            .sum::<f32>()
            + 8.0
    }

    /// Top-left corner keeping the menu inside a window of `window` size:
    /// flips left/up when there is not enough room right/below the cursor.
    pub fn origin(&self, window: Size) -> Point {
        let height = self.height();
        let mut x = self.position.x;
        let mut y = self.position.y;
        if x + MENU_WIDTH > window.width {
            x = (x - MENU_WIDTH).max(0.0);
        }
        if y + height > window.height {
            y = (y - height).max(0.0);
        }
        Point::new(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> ContextMenu {
        ContextMenu::new(
            Point::new(100.0, 100.0),
            vec![
                MenuEntry::item("First", Message::DismissNotice),
                MenuEntry::Separator,
                MenuEntry::Item {
                    label: "Disabled".into(),
                    shortcut: Some("D".into()),
                    enabled: false,
                    message: Box::new(Message::Refresh),
                },
            ],
        )
    }

    #[test]
    fn only_enabled_items_can_be_selected() {
        let menu = menu();
        assert!(matches!(menu.select(0), Some(Message::DismissNotice)));
        assert!(menu.select(1).is_none(), "separator");
        assert!(menu.select(2).is_none(), "disabled");
        assert!(menu.select(9).is_none(), "out of range");
    }

    #[test]
    fn menu_stays_inside_the_window() {
        let menu = menu();
        let window = Size::new(800.0, 600.0);
        assert_eq!(menu.origin(window), Point::new(100.0, 100.0));

        let corner = ContextMenu::new(Point::new(790.0, 590.0), menu.entries.clone());
        let origin = corner.origin(window);
        assert_eq!(origin.x, 790.0 - MENU_WIDTH);
        assert!(origin.y + corner.height() <= 600.0);
    }

    #[test]
    fn confirm_dialog_defaults_to_the_action() {
        let modal = Modal::confirm("Discard?", "Really?", "Discard", true, Message::Refresh);
        assert_eq!(modal.buttons[1].kind, ButtonKind::Danger);
        assert!(matches!(modal.default_action(), Some(Message::Refresh)));
        assert!(modal.action(0).is_none(), "cancel only closes");
    }

    #[test]
    fn error_dialog_offers_copy() {
        let modal = Modal::error("Stage failed", "fatal: boom");
        assert_eq!(modal.details.as_deref(), Some("fatal: boom"));
        assert!(
            matches!(modal.action(0), Some(Message::CopyToClipboard(text)) if text == "fatal: boom")
        );
        assert!(modal.default_action().is_none(), "Enter closes");
    }
}
