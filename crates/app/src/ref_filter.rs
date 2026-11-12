//! Hidden and soloed branches of a repository (M5-08), stored per
//! repository in the session file by full reference name.

use std::collections::BTreeSet;

use git_core::{RefKind, Reference};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefFilter {
    /// Full names (`refs/heads/x`, `refs/remotes/origin/x`) of hidden branches.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub hidden: BTreeSet<String>,
    /// Full names of soloed branches; while any exists only these (and
    /// HEAD) are shown.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub soloed: BTreeSet<String>,
}

/// Full reference name of a branch; `None` for tags.
pub fn full_name(name: &str, kind: RefKind) -> Option<String> {
    match kind {
        RefKind::LocalBranch => Some(format!("refs/heads/{name}")),
        RefKind::RemoteBranch => Some(format!("refs/remotes/{name}")),
        RefKind::Tag => None,
    }
}

fn reference_name(reference: &Reference) -> Option<String> {
    full_name(&reference.name, reference.kind)
}

impl RefFilter {
    /// Something is hidden or soloed: the graph is filtered.
    pub fn is_active(&self) -> bool {
        !self.hidden.is_empty() || !self.soloed.is_empty()
    }

    pub fn is_hidden(&self, name: &str, kind: RefKind) -> bool {
        full_name(name, kind).is_some_and(|full| self.hidden.contains(&full))
    }

    pub fn is_soloed(&self, name: &str, kind: RefKind) -> bool {
        full_name(name, kind).is_some_and(|full| self.soloed.contains(&full))
    }

    /// The branch's commits are shown: soloed (if anything is soloed) and
    /// not hidden.
    pub fn shows(&self, reference: &Reference) -> bool {
        let Some(full) = reference_name(reference) else {
            return false;
        };
        (self.soloed.is_empty() || self.soloed.contains(&full)) && !self.hidden.contains(&full)
    }

    /// The branch is not shown in the graph (sidebar entries are dimmed).
    pub fn dims(&self, name: &str, kind: RefKind) -> bool {
        let Some(full) = full_name(name, kind) else {
            return false;
        };
        self.hidden.contains(&full) || (!self.soloed.is_empty() && !self.soloed.contains(&full))
    }

    pub fn toggle_hidden(&mut self, name: &str, kind: RefKind) {
        if let Some(full) = full_name(name, kind) {
            if !self.hidden.remove(&full) {
                self.hidden.insert(full);
            }
        }
    }

    /// Hides (or shows) one branch (bulk "Hide in graph", M12-11).
    pub fn set_hidden(&mut self, name: &str, kind: RefKind, hide: bool) {
        if let Some(full) = full_name(name, kind) {
            if hide {
                self.hidden.insert(full);
            } else {
                self.hidden.remove(&full);
            }
        }
    }

    pub fn toggle_soloed(&mut self, name: &str, kind: RefKind) {
        if let Some(full) = full_name(name, kind) {
            if !self.soloed.remove(&full) {
                self.soloed.insert(full);
            }
        }
    }

    /// Hides or shows every branch of `remote`.
    pub fn set_remote_hidden(&mut self, refs: &[Reference], remote: &str, hide: bool) {
        for reference in refs
            .iter()
            .filter(|r| r.kind == RefKind::RemoteBranch && r.remote() == Some(remote))
        {
            if let Some(full) = reference_name(reference) {
                if hide {
                    self.hidden.insert(full);
                } else {
                    self.hidden.remove(&full);
                }
            }
        }
    }

    /// Drops names of branches that no longer exist; true if any were.
    pub fn prune(&mut self, refs: &[Reference]) -> bool {
        let existing: BTreeSet<String> = refs.iter().filter_map(reference_name).collect();
        let before = self.hidden.len() + self.soloed.len();
        self.hidden.retain(|n| existing.contains(n));
        self.soloed.retain(|n| existing.contains(n));
        before != self.hidden.len() + self.soloed.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(name: &str, kind: RefKind) -> Reference {
        Reference {
            name: name.into(),
            kind,
            target: "c".into(),
            is_head: false,
            upstream: None,
            ahead_behind: None,
        }
    }

    fn refs() -> Vec<Reference> {
        vec![
            branch("main", RefKind::LocalBranch),
            branch("topic", RefKind::LocalBranch),
            branch("origin/main", RefKind::RemoteBranch),
            branch("origin/topic", RefKind::RemoteBranch),
            branch("upstream/main", RefKind::RemoteBranch),
            branch("v1", RefKind::Tag),
        ]
    }

    #[test]
    fn hide_and_solo() {
        let refs = refs();
        let mut filter = RefFilter::default();
        assert!(!filter.is_active());
        assert!(refs[..5].iter().all(|r| filter.shows(r)));
        assert!(!filter.shows(&refs[5]), "tags are never tips");

        filter.toggle_hidden("topic", RefKind::LocalBranch);
        assert!(filter.is_active());
        assert!(filter.is_hidden("topic", RefKind::LocalBranch));
        assert!(!filter.shows(&refs[1]));
        assert!(filter.dims("topic", RefKind::LocalBranch));

        // Solo: only soloed branches are shown, hidden ones stay hidden.
        filter.toggle_soloed("main", RefKind::LocalBranch);
        filter.toggle_soloed("topic", RefKind::LocalBranch);
        assert!(filter.shows(&refs[0]));
        assert!(!filter.shows(&refs[1]), "solo plus hide: hidden wins");
        assert!(!filter.shows(&refs[2]));
        assert!(filter.dims("origin/main", RefKind::RemoteBranch));
        assert!(!filter.dims("main", RefKind::LocalBranch));

        filter.toggle_soloed("main", RefKind::LocalBranch);
        filter.toggle_soloed("topic", RefKind::LocalBranch);
        filter.toggle_hidden("topic", RefKind::LocalBranch);
        assert!(!filter.is_active());
    }

    #[test]
    fn remote_hide_all_and_show_all() {
        let refs = refs();
        let mut filter = RefFilter::default();
        filter.set_remote_hidden(&refs, "origin", true);
        assert_eq!(
            filter.hidden.iter().map(String::as_str).collect::<Vec<_>>(),
            ["refs/remotes/origin/main", "refs/remotes/origin/topic"]
        );
        assert!(filter.shows(&refs[4]), "other remotes stay");
        filter.set_remote_hidden(&refs, "origin", false);
        assert!(!filter.is_active());
    }

    #[test]
    fn prune_and_persistence() {
        let mut filter = RefFilter::default();
        filter.toggle_hidden("gone", RefKind::LocalBranch);
        filter.toggle_hidden("topic", RefKind::LocalBranch);
        filter.toggle_soloed("origin/old", RefKind::RemoteBranch);
        assert!(filter.prune(&refs()));
        assert_eq!(
            filter.hidden.iter().map(String::as_str).collect::<Vec<_>>(),
            ["refs/heads/topic"]
        );
        assert!(filter.soloed.is_empty());
        assert!(!filter.prune(&refs()), "nothing left to drop");

        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(json, r#"{"hidden":["refs/heads/topic"]}"#);
        let back: RefFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(back, filter);
        assert_eq!(
            serde_json::from_str::<RefFilter>("{}").unwrap(),
            RefFilter::default()
        );
    }
}
