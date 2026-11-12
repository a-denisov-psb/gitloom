//! Multi-selection in the left panel and bulk delete (M12-11): rows of
//! LOCAL, REMOTE, GITFLOW and TAGS. Click selects one row, Ctrl+click
//! toggles, Shift+click selects the range from the anchor in visible order.
//! Pure; the sidebar draws the selection, `App::update` runs the plan.

use git_core::RefKind;

/// A selected row: kind and full name (`main`, `origin/main`, `v1.0`).
pub type RefKey = (RefKind, String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefSelection {
    /// Selected rows in selection order.
    pub items: Vec<RefKey>,
    /// Start of a Shift+click range.
    pub anchor: Option<RefKey>,
}

impl RefSelection {
    /// A click on `key`; `order` is the visible order of the rows.
    pub fn click(&mut self, key: RefKey, order: &[RefKey], toggle: bool, range: bool) {
        if range {
            let anchor = self.anchor.clone().unwrap_or_else(|| key.clone());
            let position = |k: &RefKey| order.iter().position(|o| o == k);
            match (position(&anchor), position(&key)) {
                (Some(a), Some(b)) => {
                    let (from, to) = (a.min(b), a.max(b));
                    self.items = order[from..=to].to_vec();
                }
                _ => self.items = vec![key],
            }
            self.anchor = Some(anchor);
            return;
        }
        if toggle {
            match self.items.iter().position(|k| *k == key) {
                Some(index) => {
                    self.items.remove(index);
                }
                None => self.items.push(key.clone()),
            }
        } else {
            self.items = vec![key.clone()];
        }
        self.anchor = Some(key);
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.anchor = None;
    }

    pub fn contains(&self, kind: RefKind, name: &str) -> bool {
        self.items.iter().any(|(k, n)| *k == kind && n == name)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Drops rows that are no longer visible (filter, collapsed folders,
    /// deleted refs).
    pub fn retain_visible(&mut self, order: &[RefKey]) {
        self.items.retain(|k| order.contains(k));
        if self.anchor.as_ref().is_some_and(|a| !order.contains(a)) {
            self.anchor = None;
        }
    }
}

/// What a bulk delete of the selection does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BulkPlan {
    pub local: Vec<String>,
    /// The checked-out branch, listed but skipped.
    pub skipped: Option<String>,
    /// Remote branches per remote: (remote, branch names on the remote).
    pub remote: Vec<(String, Vec<String>)>,
    pub tags: Vec<String>,
}

impl BulkPlan {
    /// Groups `items`; `head` is the checked-out branch; `remotes` the
    /// configured remotes (to split `origin/feature/x`).
    pub fn new(items: &[RefKey], head: Option<&str>, remotes: &[String]) -> Self {
        let mut plan = Self::default();
        for (kind, name) in items {
            match kind {
                RefKind::LocalBranch if Some(name.as_str()) == head => {
                    plan.skipped = Some(name.clone());
                }
                RefKind::LocalBranch => plan.local.push(name.clone()),
                RefKind::Tag => plan.tags.push(name.clone()),
                RefKind::RemoteBranch => {
                    // The longest configured remote name that prefixes it.
                    let remote = remotes
                        .iter()
                        .filter(|r| {
                            name.strip_prefix(r.as_str())
                                .is_some_and(|rest| rest.starts_with('/'))
                        })
                        .max_by_key(|r| r.len());
                    let Some(remote) = remote else { continue };
                    let branch = name[remote.len() + 1..].to_owned();
                    match plan.remote.iter_mut().find(|(r, _)| r == remote) {
                        Some((_, branches)) => branches.push(branch),
                        None => plan.remote.push((remote.clone(), vec![branch])),
                    }
                }
            }
        }
        plan
    }

    fn branches(&self) -> usize {
        self.local.len() + self.remote.iter().map(|(_, b)| b.len()).sum::<usize>()
    }

    /// "Delete 3 branches…", "Delete 2 tags…", "Delete 4 branches and tags…".
    pub fn menu_label(&self) -> String {
        let (branches, tags) = (self.branches(), self.tags.len());
        match (branches, tags) {
            (0, n) => format!("Delete {n} tag{}…", plural(n)),
            (n, 0) => format!("Delete {n} branch{}…", if n == 1 { "" } else { "es" }),
            (b, t) => format!("Delete {} branches and tags…", b + t),
        }
    }

    /// Label of the undo entry: "Delete 3 branches".
    pub fn undo_label(&self) -> String {
        let local = self.local.len() + self.tags.len();
        match (self.local.len(), self.tags.len()) {
            (n, 0) => format!("Delete {n} branch{}", if n == 1 { "" } else { "es" }),
            (0, n) => format!("Delete {n} tag{}", plural(n)),
            _ => format!("Delete {local} branches and tags"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.local.is_empty() && self.remote.is_empty() && self.tags.is_empty()
    }

    /// The confirmation text: items grouped, the skipped checked-out branch,
    /// and the warning for remote branches.
    pub fn dialog_text(&self) -> String {
        let mut text = Vec::new();
        if !self.local.is_empty() || self.skipped.is_some() {
            let mut lines = vec!["Local branches:".to_owned()];
            lines.extend(self.local.iter().map(|b| format!("• {b}")));
            if let Some(head) = &self.skipped {
                lines.push(format!("• {head} (checked out, skipped)"));
            }
            text.push(lines.join("\n"));
        }
        for (remote, branches) in &self.remote {
            let mut lines = vec![format!("Remote branches on {remote}:")];
            lines.extend(branches.iter().map(|b| format!("• {b}")));
            text.push(lines.join("\n"));
        }
        if !self.tags.is_empty() {
            let mut lines = vec!["Tags:".to_owned()];
            lines.extend(self.tags.iter().map(|t| format!("• {t}")));
            text.push(lines.join("\n"));
        }
        if !self.remote.is_empty() {
            text.push("Remote branches cannot be restored with Undo.".to_owned());
        }
        text.join("\n\n")
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(kind: RefKind, name: &str) -> RefKey {
        (kind, name.to_owned())
    }

    fn local(name: &str) -> RefKey {
        key(RefKind::LocalBranch, name)
    }

    #[test]
    fn click_toggle_range_and_visibility() {
        let order = vec![
            local("a"),
            local("b"),
            local("c"),
            key(RefKind::RemoteBranch, "origin/a"),
            key(RefKind::Tag, "v1"),
        ];
        let mut selection = RefSelection::default();
        selection.click(local("b"), &order, false, false);
        assert_eq!(selection.items, [local("b")]);
        // Shift: from the anchor b to the tag, across sections.
        selection.click(key(RefKind::Tag, "v1"), &order, false, true);
        assert_eq!(selection.items, order[1..].to_vec());
        // Shift again from the same anchor upwards.
        selection.click(local("a"), &order, false, true);
        assert_eq!(selection.items, order[..2].to_vec());
        // Ctrl toggles single rows and moves the anchor.
        selection.click(local("a"), &order, true, false);
        assert_eq!(selection.items, [local("b")]);
        selection.click(key(RefKind::Tag, "v1"), &order, true, false);
        assert_eq!(selection.items, [local("b"), key(RefKind::Tag, "v1")]);
        // A row hidden by a collapsed folder or the filter drops out; rows
        // in collapsed folders are not part of a range.
        let shorter = vec![local("a"), local("c"), key(RefKind::Tag, "v1")];
        selection.retain_visible(&shorter);
        assert_eq!(selection.items, [key(RefKind::Tag, "v1")]);
        selection.click(local("a"), &shorter, false, true);
        assert_eq!(selection.items, shorter);
        selection.clear();
        assert_eq!(selection.len(), 0);
        assert!(selection.anchor.is_none());
    }

    #[test]
    fn plan_groups_and_labels() {
        let items = vec![
            local("main"),
            local("topic"),
            key(RefKind::RemoteBranch, "origin/feature/x"),
            key(RefKind::RemoteBranch, "up/stream/old"),
            key(RefKind::RemoteBranch, "origin/y"),
            key(RefKind::Tag, "v1"),
        ];
        let remotes = vec!["origin".to_owned(), "up".into(), "up/stream".into()];
        let plan = BulkPlan::new(&items, Some("main"), &remotes);
        assert_eq!(plan.local, ["topic"]);
        assert_eq!(plan.skipped.as_deref(), Some("main"));
        assert_eq!(
            plan.remote,
            [
                (
                    "origin".to_owned(),
                    vec!["feature/x".to_owned(), "y".into()]
                ),
                ("up/stream".to_owned(), vec!["old".to_owned()]),
            ]
        );
        assert_eq!(plan.menu_label(), "Delete 5 branches and tags…");
        assert_eq!(plan.undo_label(), "Delete 2 branches and tags");
        let text = plan.dialog_text();
        assert!(text.starts_with("Local branches:\n• topic\n• main (checked out, skipped)"));
        assert!(text.contains("Remote branches on origin:\n• feature/x\n• y"));
        assert!(text.ends_with("Tags:\n• v1\n\nRemote branches cannot be restored with Undo."));

        let tags = BulkPlan::new(&[key(RefKind::Tag, "a"), key(RefKind::Tag, "b")], None, &[]);
        assert_eq!(tags.menu_label(), "Delete 2 tags…");
        let branches = BulkPlan::new(&[local("a"), local("b")], None, &[]);
        assert_eq!(branches.menu_label(), "Delete 2 branches…");
        assert_eq!(branches.undo_label(), "Delete 2 branches");
        assert!(!branches.dialog_text().contains("Undo"));
    }
}
