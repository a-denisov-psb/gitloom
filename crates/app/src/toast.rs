//! Toast notifications (M12-02, ADR-049): informational messages at the
//! bottom left of the window, stacked up to [`Toasts::MAX`], newest at the
//! bottom. Errors and warnings stay until closed; info and success close
//! after [`TIMEOUT`] unless the pointer rests on them. Decisions stay modal
//! dialogs. Pure state; the view and `App::update` use it.

use std::time::Duration;

use crate::app::Message;

/// How long info and success toasts stay.
pub const TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Warning, Info and Success are used by the next tasks (submodules M12-06).
#[allow(dead_code)]
pub enum ToastKind {
    Error,
    Warning,
    Info,
    Success,
}

impl ToastKind {
    /// Kinds that close by themselves.
    pub fn times_out(self) -> bool {
        matches!(self, Self::Info | Self::Success)
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub title: String,
    /// Plain-language text.
    pub text: String,
    /// Git's command output, shown under "Details" and copied by "Copy".
    pub details: Option<String>,
    pub expanded: bool,
    /// The pointer rests on it: the timeout waits.
    pub hovered: bool,
    /// Counts timer restarts; an older timer is ignored.
    pub seq: u64,
    /// An optional button (label, message), e.g. "Update" (M12-06).
    pub action: Option<(String, Box<Message>)>,
}

impl Toast {
    pub fn new(kind: ToastKind, title: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: 0,
            kind,
            title: title.into(),
            text: text.into(),
            details: None,
            expanded: false,
            hovered: false,
            seq: 0,
            action: None,
        }
    }

    /// An error from Git's output: the plain-language text from the catalog
    /// in git-core ("Git reported an error." when unknown), the output as
    /// details.
    pub fn git_error(title: impl Into<String>, output: impl Into<String>) -> Self {
        let output = output.into();
        let text = git_core::explain::explain(&output)
            .map_or_else(|| git_core::explain::UNKNOWN.to_owned(), |e| e.text());
        Self::error_with(title, text, output)
    }

    /// An error with an explanation of the caller's own.
    pub fn error_with(
        title: impl Into<String>,
        text: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        let output = output.into();
        Self {
            details: (!output.trim().is_empty()).then_some(output),
            ..Self::new(ToastKind::Error, title, text)
        }
    }

    #[allow(dead_code)] // first used by the submodule notice (M12-06)
    pub fn with_action(mut self, label: impl Into<String>, message: Message) -> Self {
        self.action = Some((label.into(), Box::new(message)));
        self
    }
}

/// A timer to start: `ToastExpired { id, seq }` after [`TIMEOUT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    pub id: u64,
    pub seq: u64,
}

/// The shown toasts, oldest first.
#[derive(Debug, Default)]
pub struct Toasts {
    pub list: Vec<Toast>,
    next: u64,
}

impl Toasts {
    /// Toasts shown at most; a new one pushes out the oldest.
    pub const MAX: usize = 3;

    /// Shows `toast`; returns the timer to start for kinds that time out.
    pub fn push(&mut self, mut toast: Toast) -> Option<Timer> {
        self.next += 1;
        toast.id = self.next;
        toast.seq = 0;
        let timer = toast.kind.times_out().then_some(Timer {
            id: toast.id,
            seq: 0,
        });
        self.list.push(toast);
        while self.list.len() > Self::MAX {
            self.list.remove(0);
        }
        timer
    }

    pub fn dismiss(&mut self, id: u64) {
        self.list.retain(|t| t.id != id);
    }

    /// A timer ran out: the toast closes unless the pointer rests on it or
    /// the timer was restarted since.
    pub fn expired(&mut self, timer: Timer) {
        self.list.retain(|t| {
            !(t.id == timer.id && t.seq == timer.seq && !t.hovered && t.kind.times_out())
        });
    }

    /// The pointer entered or left a toast; leaving restarts its timer.
    pub fn hover(&mut self, id: u64, hovered: bool) -> Option<Timer> {
        let toast = self.list.iter_mut().find(|t| t.id == id)?;
        toast.hovered = hovered;
        if hovered || !toast.kind.times_out() {
            return None;
        }
        toast.seq += 1;
        Some(Timer { id, seq: toast.seq })
    }

    pub fn toggle_details(&mut self, id: u64) {
        if let Some(toast) = self.list.iter_mut().find(|t| t.id == id) {
            toast.expanded = !toast.expanded;
        }
    }

    pub fn get(&self, id: u64) -> Option<&Toast> {
        self.list.iter().find(|t| t.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(title: &str) -> Toast {
        Toast::new(ToastKind::Info, title, "text")
    }

    #[test]
    fn at_most_three_newest_last() {
        let mut toasts = Toasts::default();
        for title in ["a", "b", "c", "d"] {
            toasts.push(info(title));
        }
        let titles: Vec<&str> = toasts.list.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["b", "c", "d"]);
    }

    #[test]
    fn info_times_out_errors_stay() {
        let mut toasts = Toasts::default();
        let timer = toasts.push(info("saved")).unwrap();
        assert!(toasts
            .push(Toast::git_error("Push failed", "fatal: weird"))
            .is_none());
        toasts.expired(timer);
        assert_eq!(toasts.list.len(), 1);
        assert_eq!(toasts.list[0].kind, ToastKind::Error);
        assert_eq!(toasts.list[0].text, git_core::explain::UNKNOWN);
    }

    #[test]
    fn hover_pauses_the_timeout() {
        let mut toasts = Toasts::default();
        let timer = toasts.push(info("x")).unwrap();
        let id = timer.id;
        assert!(toasts.hover(id, true).is_none());
        toasts.expired(timer);
        assert_eq!(toasts.list.len(), 1, "paused while hovered");
        let restarted = toasts.hover(id, false).unwrap();
        assert_eq!(restarted.seq, 1);
        // The old timer no longer counts; the new one does.
        toasts.expired(timer);
        assert_eq!(toasts.list.len(), 1);
        toasts.expired(restarted);
        assert!(toasts.list.is_empty());
    }

    #[test]
    fn details_and_catalog_text() {
        let mut toasts = Toasts::default();
        toasts.push(Toast::git_error(
            "Push failed",
            "fatal: Authentication failed for 'https://example.org/'",
        ));
        let toast = &toasts.list[0];
        assert!(toast.text.starts_with("The remote did not accept"));
        assert!(toast
            .details
            .as_deref()
            .unwrap()
            .contains("Authentication failed"));
        let id = toast.id;
        toasts.toggle_details(id);
        assert!(toasts.get(id).unwrap().expanded);
        toasts.dismiss(id);
        assert!(toasts.list.is_empty());
    }
}
