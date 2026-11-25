//! "Commit message" modal (finding #43, ADR-051 amendment): reads the
//! message of the selected commit and, where ADR-051 allows it, edits the
//! summary, the description and the author in one step. It replaces the
//! inline editor of M14-05.

use iced::widget::text_editor::Content;

use crate::app::TabId;
use crate::repo_view::Loadable;

/// Reason shown in the read-only modal of a stash.
pub const STASH_REASON: &str = "Stashes cannot be edited";

/// Focus target of the modal's summary field.
pub const SUMMARY_INPUT: &str = "message-modal-summary";

pub struct MessageModal {
    pub tab: TabId,
    /// The commit whose message is shown.
    pub id: String,
    /// `None`: editable; `Some(reason)`: read-only with the reason, which
    /// is empty while the check still runs.
    pub read_only: Option<String>,
    /// Summary field (edit mode).
    pub summary: String,
    /// Description field (edit mode) or the read-only description.
    pub body: Content,
    /// Read-only summary (selectable text).
    pub summary_view: Content,
    original_summary: String,
    original_body: String,
    /// Original author, for "Authored by …".
    pub author_name: String,
    pub author_email: String,
    /// New author (M15-11); `None` keeps the original one.
    pub author: Option<git_core::Identity>,
    /// Moved branch and dropped signatures, for the note.
    pub facts: Loadable<git_core::reword::EditFacts>,
    /// Validation message ("Enter a summary").
    pub error: Option<String>,
    /// "Discard changes?" is asked.
    pub confirm_discard: bool,
}

impl std::fmt::Debug for MessageModal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageModal")
            .field("id", &self.id)
            .field("read_only", &self.read_only)
            .field("summary", &self.summary)
            .finish()
    }
}

impl MessageModal {
    /// The modal for `commit`; `read_only` names why it cannot be edited.
    pub fn new(tab: TabId, commit: &git_core::Commit, read_only: Option<String>) -> Self {
        Self {
            tab,
            id: commit.id.clone(),
            facts: if read_only.is_some() {
                Loadable::Ready(git_core::reword::EditFacts::default())
            } else {
                Loadable::Loading
            },
            read_only,
            summary: commit.summary.clone(),
            body: Content::with_text(&commit.body),
            summary_view: Content::with_text(&commit.summary),
            original_summary: commit.summary.clone(),
            original_body: commit.body.clone(),
            author_name: commit.author.clone(),
            author_email: commit.email.clone(),
            author: None,
            error: None,
            confirm_discard: false,
        }
    }

    pub fn editable(&self) -> bool {
        self.read_only.is_none()
    }

    /// Text or author changed since the modal opened.
    pub fn dirty(&self) -> bool {
        self.editable()
            && (self.summary != self.original_summary
                || self.body.text().trim_end() != self.original_body.trim_end()
                || self.author.is_some())
    }

    /// The edited message: summary, a blank line and the description.
    pub fn message(&self) -> String {
        join(self.summary.trim(), self.body.text().trim_end())
    }

    /// The original message for "Copy message".
    pub fn original_message(&self) -> String {
        join(&self.original_summary, self.original_body.trim_end())
    }

    /// Whether the edited message equals the original one.
    pub fn message_unchanged(&self) -> bool {
        self.message().trim_end() == self.original_message().trim_end()
    }
}

fn join(summary: &str, body: &str) -> String {
    if body.trim().is_empty() {
        summary.to_owned()
    } else {
        format!("{summary}\n\n{body}")
    }
}

/// What a close request (Esc, Cancel, ×, a click beside the modal) does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseRequest {
    /// Nothing changed: close right away.
    Close,
    /// Changes: ask "Discard changes?".
    Ask,
    /// The question is open: Esc keeps editing.
    KeepEditing,
}

/// [`CloseRequest`] for `modal`.
pub fn close_request(modal: &MessageModal) -> CloseRequest {
    if modal.confirm_discard {
        CloseRequest::KeepEditing
    } else if modal.dirty() {
        CloseRequest::Ask
    } else {
        CloseRequest::Close
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::text_editor::{Action, Edit};

    fn commit(body: &str) -> git_core::Commit {
        git_core::Commit {
            id: "c1".into(),
            summary: "Fix it".into(),
            body: body.into(),
            author: "Ana Lyst".into(),
            email: "ana@example.com".into(),
            parents: Vec::new(),
            author_time: 0,
            commit_time: 0,
            committer: String::new(),
        }
    }

    #[test]
    fn dirty_follows_text_and_author() {
        let tab = crate::app::tests::tab_id(1);
        let mut modal = MessageModal::new(tab, &commit("Body"), None);
        assert!(!modal.dirty());
        assert_eq!(close_request(&modal), CloseRequest::Close);
        modal.summary.push('!');
        assert!(modal.dirty());
        assert_eq!(close_request(&modal), CloseRequest::Ask);
        modal.confirm_discard = true;
        assert_eq!(close_request(&modal), CloseRequest::KeepEditing);
        modal.confirm_discard = false;
        modal.summary.pop();
        modal.body.perform(Action::Edit(Edit::Insert('x')));
        assert!(modal.dirty());
        let mut author = MessageModal::new(tab, &commit("Body"), None);
        author.author = Some(git_core::Identity {
            name: "Bo".into(),
            email: "bo@example.com".into(),
        });
        assert!(author.dirty());
        assert!(author.message_unchanged());
        // Read-only never has changes.
        let mut read = MessageModal::new(tab, &commit(""), Some(STASH_REASON.into()));
        read.summary.push('x');
        assert!(!read.dirty());
    }

    #[test]
    fn messages_join_summary_and_description() {
        let tab = crate::app::tests::tab_id(1);
        let modal = MessageModal::new(tab, &commit("Line 1\nLine 2"), None);
        assert_eq!(modal.original_message(), "Fix it\n\nLine 1\nLine 2");
        assert!(modal.message_unchanged());
        let empty = MessageModal::new(tab, &commit(""), None);
        assert_eq!(empty.original_message(), "Fix it");
    }
}
