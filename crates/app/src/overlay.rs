//! State of the window-wide overlays: modal dialog and context menu.

use iced::{Point, Size};

use crate::app::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    /// Destructive action, shown in the danger style.
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

    /// Plain message with a Close button.
    pub fn info(title: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            text: text.into(),
            details: None,
            buttons: vec![DialogButton {
                label: "Close".into(),
                kind: ButtonKind::Primary,
                message: None,
            }],
            default: Some(0),
        }
    }

    /// Replaces the summary line, e.g. with a hint how to fix the error.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Operation failed because another process holds a Git lock. The lock
    /// is never removed automatically; the user retries or deletes it.
    pub fn locked(label: &str, lock: &str, retry: Message) -> Self {
        Self {
            title: format!("{label} failed"),
            text: "Another Git process seems to be running in this repository. Wait until it \
                   has finished, then retry. If no Git process is running, a crashed process \
                   left the lock file behind; delete it manually."
                .into(),
            details: Some(lock.to_owned()),
            buttons: vec![
                DialogButton {
                    label: "Close".into(),
                    kind: ButtonKind::Secondary,
                    message: None,
                },
                DialogButton {
                    label: "Retry".into(),
                    kind: ButtonKind::Primary,
                    message: Some(retry),
                },
            ],
            default: Some(1),
        }
    }

    /// "Discard all" while changes are staged: keep them (default) or
    /// discard them as well.
    pub fn discard_all_with_staged(keep_staged: Message, everything: Message) -> Self {
        Self {
            title: "Discard all changes".into(),
            text: "Some changes are staged. Discard the staged changes as well? Unstaged \
                   changes are discarded and untracked files are deleted in both cases."
                .into(),
            details: None,
            buttons: vec![
                DialogButton {
                    label: "Cancel".into(),
                    kind: ButtonKind::Secondary,
                    message: None,
                },
                DialogButton {
                    label: "Keep staged".into(),
                    kind: ButtonKind::Secondary,
                    message: Some(keep_staged),
                },
                DialogButton {
                    label: "Discard staged too".into(),
                    kind: ButtonKind::Danger,
                    message: Some(everything),
                },
            ],
            default: Some(1),
        }
    }

    /// Asks before an action; destructive actions get the danger style.
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

/// Input of a [`Form`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    /// One of the given values (pick list).
    Choice(Vec<String>),
    /// Checkbox; the value is "true" or "false" (M7-10).
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    pub label: String,
    pub value: String,
    pub kind: FieldKind,
}

impl FormField {
    pub fn text(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            kind: FieldKind::Text,
        }
    }

    pub fn choice(
        label: impl Into<String>,
        value: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            kind: FieldKind::Choice(options),
        }
    }

    /// Checkbox field, checked if `checked`.
    pub fn check(label: impl Into<String>, checked: bool) -> Self {
        Self {
            label: label.into(),
            value: checked.to_string(),
            kind: FieldKind::Check,
        }
    }
}

/// What submitting a [`Form`] does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormAction {
    /// Push local branch `local` of tab `tab` and make the target its upstream.
    PushSetUpstream {
        tab: crate::app::TabId,
        local: String,
    },
    /// Make the chosen remote branch the upstream of `local`.
    SetUpstream {
        tab: crate::app::TabId,
        local: String,
    },
    /// Add a remote (name, URL).
    AddRemote { tab: crate::app::TabId },
    /// Add (`None`) or edit a saved identity (name, email; M8-03).
    Identity { index: Option<usize> },
    /// Finish Git flow branch `branch` (options; M7-10).
    FinishFlow {
        tab: crate::app::TabId,
        branch: String,
    },
    /// Start a Git flow branch of `kind` (name field; M7-08).
    StartFlow {
        tab: crate::app::TabId,
        kind: git_core::FlowKind,
    },
    /// Change the URL of remote `name`.
    SetRemoteUrl {
        tab: crate::app::TabId,
        name: String,
    },
    /// Rename remote `old`.
    RenameRemote { tab: crate::app::TabId, old: String },
    /// Create a tag (name, optional message) at commit `target`.
    CreateTag {
        tab: crate::app::TabId,
        target: String,
    },
    /// Push tag `tag` to the chosen remote.
    PushTag { tab: crate::app::TabId, tag: String },
    /// Delete tag `tag` on the chosen remote.
    DeleteRemoteTag { tab: crate::app::TabId, tag: String },
    /// Revert merge commit `id` against the chosen parent ("1: …").
    RevertMerge { tab: crate::app::TabId, id: String },
}

/// Modal dialog with inputs (e.g. remote and branch name for a first push).
#[derive(Debug, Clone)]
pub struct Form {
    pub title: String,
    pub text: String,
    pub fields: Vec<FormField>,
    pub submit: String,
    pub action: FormAction,
    /// Validation message shown above the buttons.
    pub error: Option<String>,
}

impl Form {
    /// Trimmed value of field `index` (empty if out of range).
    pub fn value(&self, index: usize) -> &str {
        self.fields.get(index).map_or("", |f| f.value.trim())
    }

    /// Checkbox field `index` is checked.
    pub fn checked(&self, index: usize) -> bool {
        self.value(index) == "true"
    }
}

/// What the commit message dialog does on submit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageAction {
    Reword { id: String },
    Squash { ids: Vec<String> },
}

/// Dialog with a multi-line commit message (reword, squash; M4-08).
#[derive(Debug)]
pub struct MessageDialog {
    pub title: String,
    pub submit: String,
    pub editor: iced::widget::text_editor::Content,
    pub action: MessageAction,
    pub error: Option<String>,
}

impl MessageDialog {
    pub fn new(
        title: impl Into<String>,
        submit: impl Into<String>,
        message: &str,
        action: MessageAction,
    ) -> Self {
        Self {
            title: title.into(),
            submit: submit.into(),
            editor: iced::widget::text_editor::Content::with_text(message),
            action,
            error: None,
        }
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
        /// Tooltip, e.g. "Not available yet" on disabled future actions.
        hint: Option<String>,
    },
    Separator,
}

impl MenuEntry {
    pub fn item(label: impl Into<String>, message: Message) -> Self {
        Self::Item {
            label: label.into(),
            shortcut: None,
            enabled: true,
            message: Box::new(message),
            hint: None,
        }
    }
}

/// Context menu opened at the cursor.
#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub position: Point,
    pub entries: Vec<MenuEntry>,
    /// Entry chosen with ↑/↓ (M6-07); Enter runs it.
    pub highlighted: Option<usize>,
    /// Width that fits the longest item (measured, see [`menu_width`]).
    pub width: f32,
}

/// Narrowest menu (short menus keep a calm, even look).
pub const MENU_MIN_WIDTH: f32 = 220.0;
/// Widest menu; longer labels end in "…" with the full text as tooltip.
pub const MENU_MAX_WIDTH: f32 = 520.0;
pub const MENU_ITEM_HEIGHT: f32 = 26.0;
pub const MENU_SEPARATOR_HEIGHT: f32 = 9.0;
/// Text sizes of the menu items (label, shortcut) as the view draws them.
pub const MENU_LABEL_SIZE: f32 = 13.0;
pub const MENU_SHORTCUT_SIZE: f32 = 11.0;
/// Horizontal space of an item besides its texts: button padding (2 × 10),
/// menu padding (2 × 4), a little slack for rounding.
const MENU_ITEM_CHROME: f32 = 20.0 + 8.0 + 4.0;
/// Gap between a label and its shortcut.
const MENU_SHORTCUT_GAP: f32 = 16.0;

/// Font the menu texts are drawn in: the app's default font. Under test the
/// `iced_test` simulator draws `Font::DEFAULT` as Fira Sans.
#[cfg(not(test))]
const DRAWN_FONT: iced::Font = iced::Font::DEFAULT;
#[cfg(test)]
const DRAWN_FONT: iced::Font = iced::Font::with_name("Fira Sans");

/// Width of `text` at `size` in the default font, measured with the font
/// system the renderer uses (so it is exact on every platform).
pub fn text_width(text: &str, size: f32) -> f32 {
    use iced::advanced::text::Paragraph as _;
    let paragraph = iced::advanced::graphics::text::Paragraph::with_text(iced::advanced::Text {
        content: text,
        bounds: Size::INFINITE,
        size: size.into(),
        line_height: iced::widget::text::LineHeight::default(),
        font: DRAWN_FONT,
        align_x: iced::advanced::text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: iced::widget::text::Shaping::default(),
        wrapping: iced::widget::text::Wrapping::None,
    });
    paragraph.min_bounds().width
}

/// Room an item needs: label, and shortcut with its gap.
fn item_width(label: &str, shortcut: Option<&str>) -> f32 {
    let shortcut = shortcut.map_or(0.0, |s| {
        text_width(s, MENU_SHORTCUT_SIZE) + MENU_SHORTCUT_GAP
    });
    text_width(label, MENU_LABEL_SIZE) + shortcut + MENU_ITEM_CHROME
}

/// Width of a menu with `entries`: the widest item, between
/// [`MENU_MIN_WIDTH`] and [`MENU_MAX_WIDTH`].
pub fn menu_width(entries: &[MenuEntry]) -> f32 {
    entries
        .iter()
        .filter_map(|entry| match entry {
            MenuEntry::Item {
                label, shortcut, ..
            } => Some(item_width(label, shortcut.as_deref())),
            MenuEntry::Separator => None,
        })
        .fold(MENU_MIN_WIDTH, f32::max)
        .ceil()
        .min(MENU_MAX_WIDTH)
}

/// Shortens `label` with "…" until its item fits `width`.
fn fit_label(label: &str, shortcut: Option<&str>, width: f32) -> String {
    if item_width(label, shortcut) <= width {
        return label.to_owned();
    }
    let chars: Vec<char> = label.chars().collect();
    // Longest prefix that fits (binary search over the char count).
    let (mut low, mut high) = (0, chars.len());
    while low < high {
        let mid = (low + high).div_ceil(2);
        let candidate: String = chars[..mid].iter().collect::<String>() + "…";
        if item_width(&candidate, shortcut) <= width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    chars[..low]
        .iter()
        .collect::<String>()
        .trim_end()
        .to_owned()
        + "…"
}

impl ContextMenu {
    /// Menu at `position`, as wide as its longest item; items longer than
    /// [`MENU_MAX_WIDTH`] are shortened with "…" and show their full text
    /// as tooltip.
    pub fn new(position: Point, mut entries: Vec<MenuEntry>) -> Self {
        let width = menu_width(&entries);
        for entry in &mut entries {
            if let MenuEntry::Item {
                label,
                shortcut,
                hint,
                ..
            } = entry
            {
                let fitted = fit_label(label, shortcut.as_deref(), width);
                if fitted != *label {
                    *hint = Some(match hint.take() {
                        Some(reason) => format!("{label}\n{reason}"),
                        None => label.clone(),
                    });
                    *label = fitted;
                }
            }
        }
        Self {
            position,
            entries,
            highlighted: None,
            width,
        }
    }

    /// Label, shortcut and the menu width of item `index` fit together.
    #[cfg(test)]
    pub fn item_fits(&self, index: usize) -> bool {
        match self.entries.get(index) {
            Some(MenuEntry::Item {
                label, shortcut, ..
            }) => item_width(label, shortcut.as_deref()) <= self.width,
            _ => true,
        }
    }

    fn enabled(&self, index: usize) -> bool {
        matches!(
            self.entries.get(index),
            Some(MenuEntry::Item { enabled: true, .. })
        )
    }

    /// Moves the keyboard highlight to the next (or previous) enabled item,
    /// wrapping around; the first press starts at the top (or bottom).
    pub fn move_highlight(&mut self, down: bool) {
        let count = self.entries.len();
        if count == 0 {
            return;
        }
        let mut index = match (self.highlighted, down) {
            (Some(i), true) => (i + 1) % count,
            (Some(i), false) => (i + count - 1) % count,
            (None, true) => 0,
            (None, false) => count - 1,
        };
        for _ in 0..count {
            if self.enabled(index) {
                self.highlighted = Some(index);
                return;
            }
            index = if down {
                (index + 1) % count
            } else {
                (index + count - 1) % count
            };
        }
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
    /// flips left/up when there is not enough room right/below the cursor,
    /// and stays as close to the cursor as the window allows when there is
    /// no room on either side (small window, high zoom).
    pub fn origin(&self, window: Size) -> Point {
        fn axis(cursor: f32, size: f32, room: f32) -> f32 {
            if cursor + size <= room {
                cursor
            } else if cursor - size >= 0.0 {
                cursor - size
            } else {
                (room - size).max(0.0)
            }
        }
        Point::new(
            axis(self.position.x, self.width, window.width),
            axis(self.position.y, self.height(), window.height),
        )
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
                    hint: None,
                },
            ],
        )
    }

    #[test]
    fn arrows_skip_separators_and_disabled_items() {
        let mut menu = ContextMenu::new(
            Point::ORIGIN,
            vec![
                MenuEntry::item("A", Message::DismissNotice),
                MenuEntry::Separator,
                MenuEntry::Item {
                    label: "Off".into(),
                    shortcut: None,
                    enabled: false,
                    message: Box::new(Message::Refresh),
                    hint: None,
                },
                MenuEntry::item("B", Message::Refresh),
            ],
        );
        menu.move_highlight(true);
        assert_eq!(menu.highlighted, Some(0));
        menu.move_highlight(true);
        assert_eq!(menu.highlighted, Some(3));
        menu.move_highlight(true);
        assert_eq!(menu.highlighted, Some(0), "wraps");
        menu.move_highlight(false);
        assert_eq!(menu.highlighted, Some(3));
        let mut up = ContextMenu::new(Point::ORIGIN, menu.entries.clone());
        up.move_highlight(false);
        assert_eq!(up.highlighted, Some(3), "starts at the bottom");
        let mut none = ContextMenu::new(Point::ORIGIN, vec![MenuEntry::Separator]);
        none.move_highlight(true);
        assert_eq!(none.highlighted, None);
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
        assert_eq!(origin.x, 790.0 - menu.width);
        assert!(origin.y + corner.height() <= 600.0);
        // No room above or below (high zoom): as low as the window allows.
        let height = corner.height();
        let short = Size::new(800.0, height + 40.0);
        let mut middle = corner.clone();
        middle.position = Point::new(100.0, 60.0);
        assert_eq!(middle.origin(short).y, 40.0);
        let tiny = Size::new(800.0, height - 10.0);
        assert_eq!(middle.origin(tiny).y, 0.0);
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
