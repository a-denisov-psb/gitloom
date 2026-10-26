//! State of the conflict resolution view (M4-05, ADR-022): the loaded
//! conflict, the user's picks and the output editor. Logic only; the view
//! lives in `ui::conflict`.

use git_core::{ConflictContent, ConflictText, LoadedConflict, Picks, Side};
use iced::widget::text_editor;

use crate::repo_view::Loadable;

/// One conflicted file opened in the center pane.
#[derive(Debug)]
pub struct ConflictView {
    pub path: String,
    /// Marker labels for ours and theirs (e.g. "HEAD", "topic").
    pub labels: (String, String),
    pub loaded: Loadable<LoadedConflict>,
    pub picks: Picks,
    pub output: text_editor::Content,
    /// The output was edited by hand; it wins over the picks until reset.
    pub edited: bool,
}

impl ConflictView {
    pub fn new(path: String, labels: (String, String)) -> Self {
        Self {
            path,
            labels,
            loaded: Loadable::Loading,
            picks: Picks::default(),
            output: text_editor::Content::new(),
            edited: false,
        }
    }

    pub fn set_loaded(&mut self, result: Result<LoadedConflict, String>) {
        self.loaded = match result {
            Ok(loaded) => Loadable::Ready(loaded),
            Err(error) => Loadable::Failed(error),
        };
        self.picks = self.text().map(Picks::new).unwrap_or_default();
        self.edited = false;
        self.refresh_output();
    }

    /// The conflict as text, if hunks and lines can be picked.
    pub fn text(&self) -> Option<&ConflictText> {
        match &self.loaded {
            Loadable::Ready(LoadedConflict {
                content: ConflictContent::Text(text),
                ..
            }) => Some(text),
            _ => None,
        }
    }

    fn model_output(&self) -> Option<String> {
        let (ours, theirs) = &self.labels;
        self.text()
            .map(|text| text.output(&self.picks, (ours.as_str(), theirs.as_str())))
    }

    /// Shows the picks' result unless the output was edited by hand.
    fn refresh_output(&mut self) {
        if self.edited {
            return;
        }
        let output = self.model_output().unwrap_or_default();
        self.output = text_editor::Content::with_text(&output);
    }

    pub fn toggle_line(&mut self, hunk: usize, side: Side, line: usize) {
        self.picks.toggle_line(hunk, side, line);
        self.refresh_output();
    }

    pub fn toggle_hunk(&mut self, hunk: usize, side: Side) {
        self.picks.toggle_hunk(hunk, side);
        self.refresh_output();
    }

    pub fn take_all(&mut self, side: Side) {
        self.picks.take_all(side);
        self.refresh_output();
    }

    /// An action in the output editor; edits make the output win.
    pub fn edit(&mut self, action: text_editor::Action) {
        if action.is_edit() {
            self.edited = true;
        }
        self.output.perform(action);
    }

    /// Back to the output of the picks.
    pub fn reset_output(&mut self) {
        self.edited = false;
        self.refresh_output();
    }

    /// Why saving is not possible yet, if it is not.
    pub fn save_blocker(&self) -> Option<String> {
        if self.text().is_none() {
            return Some("The conflict is not loaded".into());
        }
        let open = self.picks.open_count();
        (!self.edited && open > 0).then(|| match open {
            1 => "Pick lines for the remaining conflict, or edit the output".into(),
            n => format!("Pick lines for the remaining {n} conflicts, or edit the output"),
        })
    }

    /// Content to save as the resolved file: the edited output with the
    /// file's line endings, or the picks' result byte for byte.
    pub fn resolved_content(&self) -> Result<String, String> {
        if let Some(reason) = self.save_blocker() {
            return Err(reason);
        }
        let text = self.text().ok_or("The conflict is not loaded")?;
        if self.edited {
            Ok(git_core::with_line_endings(&self.output.text(), text.eol))
        } else {
            self.model_output()
                .ok_or_else(|| "The conflict is not loaded".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::{Chunk, ConflictHunk, ConflictStages};

    fn loaded(eol: &'static str) -> LoadedConflict {
        let line = |s: &str| format!("{s}{eol}");
        LoadedConflict {
            path: "a.txt".into(),
            stages: ConflictStages::default(),
            content: ConflictContent::Text(ConflictText {
                chunks: vec![
                    Chunk::Clean(vec![line("a")]),
                    Chunk::Conflict(ConflictHunk {
                        ours: vec![line("o1"), line("o2")],
                        base: vec![],
                        theirs: vec![line("t")],
                    }),
                    Chunk::Clean(vec![line("z")]),
                ],
                eol,
            }),
        }
    }

    fn view(eol: &'static str) -> ConflictView {
        let mut view = ConflictView::new("a.txt".into(), ("HEAD".into(), "topic".into()));
        view.set_loaded(Ok(loaded(eol)));
        view
    }

    fn type_text(view: &mut ConflictView, text: &str) {
        use std::sync::Arc;
        view.edit(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        view.edit(text_editor::Action::Edit(text_editor::Edit::Paste(
            Arc::new(text.into()),
        )));
    }

    #[test]
    fn picks_update_the_output_until_it_is_edited() {
        let mut view = view("\n");
        assert_eq!(
            view.output.text(),
            "a\n<<<<<<< HEAD\no1\no2\n=======\nt\n>>>>>>> topic\nz\n"
        );
        assert!(view.save_blocker().is_some());
        view.toggle_line(0, Side::Ours, 1);
        view.toggle_hunk(0, Side::Theirs);
        assert_eq!(view.output.text(), "a\no2\nt\nz\n");
        assert_eq!(view.resolved_content().unwrap(), "a\no2\nt\nz\n");

        type_text(&mut view, "added");
        assert!(view.edited);
        view.take_all(Side::Ours);
        assert_eq!(view.output.text(), "a\no2\nt\nz\nadded", "edits win");
        assert_eq!(view.resolved_content().unwrap(), "a\no2\nt\nz\nadded");

        view.reset_output();
        assert!(!view.edited);
        assert_eq!(view.output.text(), "a\no1\no2\nz\n");
    }

    #[test]
    fn edited_output_can_be_saved_with_open_conflicts() {
        let mut view = view("\n");
        assert_eq!(
            view.save_blocker().as_deref(),
            Some("Pick lines for the remaining conflict, or edit the output")
        );
        type_text(&mut view, "x");
        assert!(view.save_blocker().is_none());
    }

    #[test]
    fn crlf_is_kept_for_picks_and_edits() {
        let mut view = view("\r\n");
        view.take_all(Side::Theirs);
        assert_eq!(view.resolved_content().unwrap(), "a\r\nt\r\nz\r\n");
        type_text(&mut view, "new\n");
        assert_eq!(view.resolved_content().unwrap(), "a\r\nt\r\nz\r\nnew\r\n");
    }

    #[test]
    fn failed_load_cannot_be_saved() {
        let mut view = ConflictView::new("a.txt".into(), ("HEAD".into(), "x".into()));
        view.set_loaded(Err("boom".into()));
        assert_eq!(view.loaded, Loadable::Failed("boom".into()));
        assert!(view.resolved_content().is_err());
    }
}
