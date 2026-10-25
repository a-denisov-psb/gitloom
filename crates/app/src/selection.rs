//! Multi-selection of commits in the graph (M4-01): Ctrl+click toggles,
//! Shift+click and Shift+↑/↓ select a range from the anchor. Kept by commit
//! id so it survives refreshes.

/// Selected commits in graph order. `order` arguments list the selectable
/// commit ids in graph order (stashes and the WIP row are not selectable).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultiSelection {
    ids: Vec<String>,
    /// Start of Shift ranges: the last plainly clicked or toggled commit.
    anchor: Option<String>,
}

impl MultiSelection {
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.iter().any(|i| i == id)
    }

    /// Plain click: only `id`, which becomes the anchor.
    pub fn single(&mut self, id: &str) {
        self.ids = vec![id.to_owned()];
        self.anchor = Some(id.to_owned());
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchor = None;
    }

    /// Ctrl+click: adds or removes `id`; it becomes the anchor.
    pub fn toggle(&mut self, id: &str, order: &[&str]) {
        if let Some(pos) = self.ids.iter().position(|i| i == id) {
            self.ids.remove(pos);
        } else {
            self.ids.push(id.to_owned());
        }
        self.anchor = Some(id.to_owned());
        self.sort(order);
    }

    /// Shift+click: everything between the anchor and `id` (inclusive),
    /// replacing the previous selection; the anchor stays.
    pub fn range(&mut self, id: &str, order: &[&str]) {
        let anchor = self.anchor.clone().unwrap_or_else(|| id.to_owned());
        let index = |target: &str| order.iter().position(|i| *i == target);
        match (index(&anchor), index(id)) {
            (Some(a), Some(b)) => {
                let (from, to) = (a.min(b), a.max(b));
                self.ids = order[from..=to].iter().map(|i| (*i).to_owned()).collect();
                self.anchor = Some(anchor);
            }
            _ => self.single(id),
        }
    }

    /// Drops commits that are no longer listed (after a refresh).
    pub fn retain(&mut self, order: &[&str]) {
        self.ids.retain(|id| order.contains(&id.as_str()));
        if self
            .anchor
            .as_deref()
            .is_some_and(|anchor| !order.contains(&anchor))
        {
            self.anchor = self.ids.first().cloned();
        }
        self.sort(order);
    }

    fn sort(&mut self, order: &[&str]) {
        self.ids
            .sort_by_key(|id| order.iter().position(|i| i == id).unwrap_or(usize::MAX));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [&str; 5] = ["e", "d", "c", "b", "a"];

    fn ids(selection: &MultiSelection) -> Vec<&str> {
        selection.ids().iter().map(String::as_str).collect()
    }

    #[test]
    fn toggle_adds_and_removes_in_graph_order() {
        let mut s = MultiSelection::default();
        s.single("b");
        s.toggle("d", &ORDER);
        s.toggle("a", &ORDER);
        assert_eq!(ids(&s), ["d", "b", "a"]);
        s.toggle("b", &ORDER);
        assert_eq!(ids(&s), ["d", "a"]);
    }

    #[test]
    fn range_spans_from_the_anchor_in_both_directions() {
        let mut s = MultiSelection::default();
        s.single("c");
        s.range("a", &ORDER);
        assert_eq!(ids(&s), ["c", "b", "a"]);
        // The anchor stays: a second Shift+click replaces the range.
        s.range("e", &ORDER);
        assert_eq!(ids(&s), ["e", "d", "c"]);
        // Ctrl+click moves the anchor.
        s.toggle("a", &ORDER);
        s.range("b", &ORDER);
        assert_eq!(ids(&s), ["b", "a"]);
    }

    #[test]
    fn range_without_anchor_selects_one() {
        let mut s = MultiSelection::default();
        s.range("c", &ORDER);
        assert_eq!(ids(&s), ["c"]);
    }

    #[test]
    fn refresh_keeps_existing_commits() {
        let mut s = MultiSelection::default();
        s.single("d");
        s.range("b", &ORDER);
        s.retain(&["e", "d", "b", "a"]);
        assert_eq!(ids(&s), ["d", "b"]);
        s.retain(&["e", "a"]);
        assert!(s.is_empty());
        // The anchor fell back; a range from a new click still works.
        s.single("e");
        s.range("a", &["e", "a"]);
        assert_eq!(ids(&s), ["e", "a"]);
    }
}
