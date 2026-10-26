//! State of the interactive rebase panel (M4-07): the plan in todo order
//! (oldest first, shown newest on top), the selected row, dragging and the
//! messages for rewords and squash chains. Logic only; the view lives in
//! `ui::rebase`.

use std::collections::HashMap;

use git_core::{EditorInput, Oid, TodoAction, TodoEntry};
use iced::widget::text_editor;

use crate::repo_view::Loadable;

#[derive(Debug)]
pub struct RebasePanel {
    /// Commit the plan is replayed onto (exclusive).
    pub base: Oid,
    /// Short id and summary of the base, for the header.
    pub base_label: String,
    pub loaded: Loadable<()>,
    /// Todo order: oldest first.
    pub entries: Vec<TodoEntry>,
    /// Index into `entries`.
    pub selected: usize,
    /// Entry being dragged (its current index).
    pub dragging: Option<usize>,
    /// Message editors by chain head commit id.
    pub messages: HashMap<Oid, text_editor::Content>,
    /// Merge commits after the base; the rebase drops them (M4-13).
    pub flattened_merges: usize,
}

impl RebasePanel {
    pub fn new(base: Oid, base_label: String) -> Self {
        Self {
            base,
            base_label,
            loaded: Loadable::Loading,
            entries: Vec::new(),
            selected: 0,
            dragging: None,
            messages: HashMap::new(),
            flattened_merges: 0,
        }
    }

    pub fn set_loaded(&mut self, result: Result<Vec<(Oid, String)>, String>) {
        match result {
            Ok(commits) if commits.is_empty() => {
                self.loaded = Loadable::Failed("There are no commits to rebase".into());
            }
            Ok(commits) => {
                self.entries = commits
                    .into_iter()
                    .map(|(id, message)| TodoEntry {
                        id,
                        action: TodoAction::Pick,
                        message,
                    })
                    .collect();
                // The newest commit is on top.
                self.selected = self.entries.len() - 1;
                self.loaded = Loadable::Ready(());
            }
            Err(error) => self.loaded = Loadable::Failed(error),
        }
    }

    /// Indices in display order: newest on top.
    pub fn display_order(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.entries.len()).rev()
    }

    pub fn set_action(&mut self, index: usize, action: TodoAction) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.action = action;
            self.selected = index;
        }
        self.sync_messages();
    }

    pub fn set_selected_action(&mut self, action: TodoAction) {
        self.set_action(self.selected, action);
    }

    pub fn select(&mut self, index: usize) {
        if index < self.entries.len() {
            self.selected = index;
        }
    }

    /// ↑ / ↓ in the newest-on-top list.
    pub fn move_selection(&mut self, up: bool) {
        if up {
            if self.selected + 1 < self.entries.len() {
                self.selected += 1;
            }
        } else {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    /// Moves entry `from` to position `to` (both todo indices).
    pub fn move_entry(&mut self, from: usize, to: usize) {
        if from >= self.entries.len() || to >= self.entries.len() || from == to {
            return;
        }
        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        if self.selected == from {
            self.selected = to;
        }
        self.sync_messages();
    }

    /// Alt+↑ / Alt+↓: moves the selected entry up or down in the list.
    pub fn move_selected(&mut self, up: bool) {
        let from = self.selected;
        let to = if up {
            from + 1
        } else {
            match from.checked_sub(1) {
                Some(to) => to,
                None => return,
            }
        };
        self.move_entry(from, to);
        self.selected = to.min(self.entries.len().saturating_sub(1));
    }

    pub fn drag_start(&mut self, index: usize) {
        self.select(index);
        self.dragging = Some(index);
    }

    /// The dragged entry follows the pointer over row `index`.
    pub fn drag_over(&mut self, index: usize) {
        if let Some(from) = self.dragging {
            if from != index {
                self.move_entry(from, index);
                self.selected = index;
                self.dragging = Some(index);
            }
        }
    }

    pub fn drag_end(&mut self) {
        self.dragging = None;
    }

    /// Creates editors (prefilled) for chain heads that need a message.
    fn sync_messages(&mut self) {
        for chain in git_core::chains(&self.entries) {
            if git_core::needs_message(&self.entries, chain.head) {
                let id = self.entries[chain.head].id.clone();
                let prefill = git_core::default_message(&self.entries, chain.head);
                self.messages
                    .entry(id)
                    .or_insert_with(|| text_editor::Content::with_text(&prefill));
            }
        }
    }

    /// Editor for the chain headed by entry `index`, if it needs a message.
    pub fn message_editor(&self, index: usize) -> Option<&text_editor::Content> {
        if !git_core::needs_message(&self.entries, index) {
            return None;
        }
        self.messages.get(&self.entries.get(index)?.id)
    }

    pub fn edit_message(&mut self, id: &str, action: text_editor::Action) {
        if let Some(content) = self.messages.get_mut(id) {
            content.perform(action);
        }
    }

    /// Why the plan cannot start, if it cannot.
    pub fn problem(&self) -> Option<String> {
        if !matches!(self.loaded, Loadable::Ready(())) {
            return Some("The commits are not loaded".into());
        }
        git_core::validate_todo(&self.entries).err()
    }

    /// Todo list and messages for `git rebase -i`.
    pub fn build(&self) -> Result<EditorInput, String> {
        if let Some(problem) = self.problem() {
            return Err(problem);
        }
        git_core::build_todo(&self.entries, |head| {
            self.entries
                .get(head)
                .and_then(|entry| self.messages.get(&entry.id))
                .map(text_editor::Content::text)
                .unwrap_or_else(|| git_core::default_message(&self.entries, head))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TodoAction::*;

    fn panel(count: usize) -> RebasePanel {
        let mut panel = RebasePanel::new("base".into(), "base".into());
        panel.set_loaded(Ok((0..count)
            .map(|i| (format!("c{i}"), format!("m{i}\n")))
            .collect()));
        panel
    }

    fn ids(panel: &RebasePanel) -> Vec<&str> {
        panel.entries.iter().map(|e| e.id.as_str()).collect()
    }

    #[test]
    fn loads_with_the_newest_selected() {
        let panel = panel(3);
        assert_eq!(panel.selected, 2);
        assert_eq!(panel.display_order().collect::<Vec<_>>(), [2, 1, 0]);
        assert!(panel.build().is_ok());

        let mut empty = RebasePanel::new("b".into(), "b".into());
        empty.set_loaded(Ok(Vec::new()));
        assert!(matches!(empty.loaded, Loadable::Failed(_)));
    }

    #[test]
    fn keyboard_selection_and_reordering_follow_the_display() {
        let mut panel = panel(3);
        panel.move_selection(true);
        assert_eq!(panel.selected, 2, "already on top");
        panel.move_selection(false);
        assert_eq!(panel.selected, 1);
        // Alt+↑ moves "c1" above "c2" (newer position).
        panel.move_selected(true);
        assert_eq!(ids(&panel), ["c0", "c2", "c1"]);
        assert_eq!(panel.selected, 2);
        panel.move_selected(true);
        assert_eq!(ids(&panel), ["c0", "c2", "c1"], "top stays");
        panel.select(0);
        panel.move_selected(false);
        assert_eq!(ids(&panel), ["c0", "c2", "c1"], "bottom stays");
    }

    #[test]
    fn dragging_moves_the_entry_along() {
        let mut panel = panel(4);
        panel.drag_start(3);
        panel.drag_over(2);
        panel.drag_over(1);
        panel.drag_end();
        assert_eq!(ids(&panel), ["c0", "c3", "c1", "c2"]);
        assert_eq!(panel.selected, 1);
        assert!(panel.dragging.is_none());
        panel.drag_over(3);
        assert_eq!(ids(&panel), ["c0", "c3", "c1", "c2"], "no drag active");
    }

    #[test]
    fn messages_are_prefilled_for_rewords_and_squash_chains() {
        let mut panel = panel(3);
        panel.set_action(1, Reword);
        assert_eq!(panel.message_editor(1).unwrap().text(), "m1");
        assert!(panel.message_editor(0).is_none());
        panel.set_action(2, Squash);
        // c1 heads a squash chain now: its editor keeps the reword text.
        assert_eq!(panel.message_editor(1).unwrap().text(), "m1");
        panel.set_action(1, Pick);
        assert!(panel.message_editor(1).is_some(), "squash chain");

        let mut panel = panel_with_squash();
        assert_eq!(panel.message_editor(0).unwrap().text(), "m0\n\nm1");
        let input = panel.build().unwrap();
        assert_eq!(input.messages, ["m0\n\nm1\n"]);
        panel.set_action(0, Squash);
        assert!(panel.problem().is_some());
        assert!(panel.build().is_err());
    }

    fn panel_with_squash() -> RebasePanel {
        let mut panel = panel(2);
        panel.set_action(1, Squash);
        panel
    }

    #[test]
    fn edited_message_is_used() {
        let mut panel = panel(2);
        panel.set_action(1, Reword);
        panel.edit_message(
            "c1",
            text_editor::Action::Edit(text_editor::Edit::Paste(std::sync::Arc::new("new ".into()))),
        );
        let input = panel.build().unwrap();
        assert_eq!(input.messages, ["new m1\n"]);
        assert_eq!(input.sequence.as_deref(), Some("pick c0\nreword c1\n"));
    }
}
