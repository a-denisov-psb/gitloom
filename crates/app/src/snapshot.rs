//! Immutable view of a repository at one point in time, built off the UI thread.

use std::collections::HashMap;
use std::sync::Arc;

use git_core::{Commit, GitBackend, Head, Log, Reference, Stash, WorkingTree};
use git_graph::{GraphCommit, Id, Layout, Wip};

/// Maximum number of commits loaded per repository.
pub const LOG_LIMIT: usize = 100_000;

/// Input of the lane layout, kept to recompute it when the WIP row appears
/// or disappears without reloading the history.
#[derive(Debug)]
struct GraphInput {
    commits: Vec<GraphCommit>,
    order: Vec<Id>,
    head: Option<Id>,
}

/// History parts are shared between snapshots, so a status-only refresh
/// does not copy the commits.
#[derive(Debug, Clone)]
pub struct RepoSnapshot {
    /// Commits in display order (topological, newest first).
    pub commits: Arc<Vec<Commit>>,
    /// Commit id → index into `commits`.
    pub index: Arc<HashMap<String, usize>>,
    graph: Arc<GraphInput>,
    /// Graph rows; row 0 is the WIP row if `has_wip`.
    pub layout: Arc<Layout>,
    /// The working tree has changes; a WIP row is shown above the commits.
    pub has_wip: bool,
    pub refs: Vec<Reference>,
    /// Commit index → indices into `refs` pointing at it.
    pub refs_by_commit: HashMap<usize, Vec<usize>>,
    pub stashes: Vec<Stash>,
    pub working_tree: WorkingTree,
    pub head: Head,
    /// More commits exist than [`LOG_LIMIT`].
    pub truncated: bool,
    /// Prepared message of a merge in progress (`MERGE_MSG`).
    pub merge_message: Option<String>,
    /// Configured `commit.template` content.
    pub commit_template: Option<String>,
}

impl RepoSnapshot {
    /// Reads everything from the backend. Blocking; call via `spawn_blocking`.
    pub fn load(backend: &dyn GitBackend) -> Result<Self, String> {
        let log = backend.log(LOG_LIMIT).map_err(|e| e.to_string())?;
        let head = backend.head().map_err(|e| e.to_string())?;
        let refs = backend.references().map_err(|e| e.to_string())?;
        let stashes = backend.stashes().map_err(|e| e.to_string())?;
        let working_tree = backend.working_tree().map_err(|e| e.to_string())?;
        let mut snapshot = Self::build(log, head, refs, stashes, working_tree)?;
        snapshot.merge_message = backend.merge_message().map_err(|e| e.to_string())?;
        snapshot.commit_template = backend.commit_template().map_err(|e| e.to_string())?;
        Ok(snapshot)
    }

    /// Orders the commits, computes the graph layout and the lookup tables.
    pub fn build(
        log: Log,
        head: Head,
        refs: Vec<Reference>,
        stashes: Vec<Stash>,
        working_tree: WorkingTree,
    ) -> Result<Self, String> {
        let count = log.commits.len();
        let position: HashMap<&str, u32> = log
            .commits
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.as_str(), i as u32))
            .collect();

        // Parents outside the loaded set get distinct ids after the commits.
        let mut missing: HashMap<String, u32> = HashMap::new();
        let mut id_of = |oid: &str| -> Id {
            match position.get(oid) {
                Some(&i) => Id(i),
                None => {
                    let next = (count + missing.len()) as u32;
                    Id(*missing.entry(oid.to_owned()).or_insert(next))
                }
            }
        };
        let graph: Vec<GraphCommit> = log
            .commits
            .iter()
            .enumerate()
            .map(|(i, c)| GraphCommit {
                id: Id(i as u32),
                parents: c.parents.iter().map(|p| id_of(p)).collect(),
                time: c.commit_time,
            })
            .collect();
        let head_id = head.target().map(&mut id_of);

        let order = git_graph::topo_order(&graph)
            .map_err(|e| e.to_string())?
            .order;
        let has_wip = !working_tree.is_empty();
        let wip = has_wip.then_some(Wip { head: head_id });
        let layout = git_graph::layout(&graph, &order, wip).map_err(|e| e.to_string())?;

        let mut slots: Vec<Option<Commit>> = log.commits.into_iter().map(Some).collect();
        let commits: Vec<Commit> = order
            .iter()
            .filter_map(|id| slots.get_mut(id.index()).and_then(Option::take))
            .collect();
        let index: HashMap<String, usize> = commits
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.clone(), i))
            .collect();

        let mut refs_by_commit: HashMap<usize, Vec<usize>> = HashMap::new();
        for (r, reference) in refs.iter().enumerate() {
            if let Some(&commit) = index.get(&reference.target) {
                refs_by_commit.entry(commit).or_default().push(r);
            }
        }

        Ok(Self {
            commits: Arc::new(commits),
            index: Arc::new(index),
            graph: Arc::new(GraphInput {
                commits: graph,
                order,
                head: head_id,
            }),
            layout: Arc::new(layout),
            has_wip,
            refs,
            refs_by_commit,
            stashes,
            working_tree,
            head,
            truncated: log.truncated,
            merge_message: None,
            commit_template: None,
        })
    }

    /// Same history with a new working tree status; the layout is only
    /// recomputed when the WIP row appears or disappears.
    pub fn with_working_tree(&self, working_tree: WorkingTree) -> Result<Self, String> {
        let has_wip = !working_tree.is_empty();
        let layout = if has_wip == self.has_wip {
            Arc::clone(&self.layout)
        } else {
            let wip = has_wip.then_some(Wip {
                head: self.graph.head,
            });
            Arc::new(
                git_graph::layout(&self.graph.commits, &self.graph.order, wip)
                    .map_err(|e| e.to_string())?,
            )
        };
        Ok(Self {
            layout,
            has_wip,
            working_tree,
            ..self.clone()
        })
    }

    /// Layout row of the commit at `index`.
    pub fn row_of(&self, index: usize) -> usize {
        index + usize::from(self.has_wip)
    }

    /// Checked-out branch name, or `HEAD (abc1234)` when detached.
    pub fn branch_label(&self) -> String {
        match &self.head {
            Head::Branch { name, .. } | Head::Unborn { name } => name.clone(),
            Head::Detached { target } => format!("HEAD ({})", &target[..target.len().min(7)]),
        }
    }

    pub fn commit(&self, id: &str) -> Option<&Commit> {
        self.index.get(id).and_then(|&i| self.commits.get(i))
    }

    pub fn refs_at(&self, index: usize) -> impl Iterator<Item = &Reference> {
        self.refs_by_commit
            .get(&index)
            .into_iter()
            .flatten()
            .filter_map(|&r| self.refs.get(r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::{ChangeKind, FileChange, RefKind};

    fn commit(id: &str, parents: &[&str], time: i64) -> Commit {
        Commit {
            id: id.into(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
            summary: format!("commit {id}"),
            body: String::new(),
            author: "A".into(),
            email: "a@example.com".into(),
            author_time: time,
            commit_time: time,
        }
    }

    fn reference(name: &str, target: &str) -> Reference {
        Reference {
            name: name.into(),
            kind: RefKind::LocalBranch,
            target: target.into(),
            is_head: name == "main",
            upstream: None,
            ahead_behind: None,
        }
    }

    fn dirty() -> WorkingTree {
        WorkingTree {
            unstaged: vec![FileChange {
                path: "a".into(),
                kind: ChangeKind::Modified,
                old_path: None,
            }],
            staged: Vec::new(),
        }
    }

    #[test]
    fn orders_commits_topologically_and_indexes_them() {
        // Input newest-first by time, but "c" has clock skew (older than its parent "b").
        let log = Log {
            commits: vec![
                commit("b", &["a"], 30),
                commit("a", &[], 20),
                commit("c", &["b"], 10),
            ],
            truncated: false,
        };
        let head = Head::Branch {
            name: "main".into(),
            target: "c".into(),
        };
        let snapshot = RepoSnapshot::build(
            log,
            head,
            vec![reference("main", "c")],
            Vec::new(),
            WorkingTree::default(),
        )
        .unwrap();

        let ids: Vec<_> = snapshot.commits.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"]);
        assert_eq!(snapshot.index["a"], 2);
        assert_eq!(snapshot.layout.len(), 3);
        assert!(!snapshot.has_wip);
        assert_eq!(
            snapshot
                .refs_at(0)
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            ["main"]
        );
        assert_eq!(snapshot.refs_at(1).count(), 0);
    }

    #[test]
    fn dirty_worktree_adds_wip_row() {
        let log = Log {
            commits: vec![commit("a", &[], 1)],
            truncated: false,
        };
        let head = Head::Branch {
            name: "main".into(),
            target: "a".into(),
        };
        let snapshot = RepoSnapshot::build(log, head, Vec::new(), Vec::new(), dirty()).unwrap();
        assert!(snapshot.has_wip);
        assert_eq!(snapshot.layout.len(), 2);
        assert_eq!(snapshot.row_of(0), 1);
    }

    #[test]
    fn missing_parents_stay_distinct() {
        // Truncated history: two commits with different parents outside the set.
        let log = Log {
            commits: vec![commit("m", &["x", "y"], 3)],
            truncated: true,
        };
        let snapshot = RepoSnapshot::build(
            log,
            Head::Detached { target: "m".into() },
            Vec::new(),
            Vec::new(),
            WorkingTree::default(),
        )
        .unwrap();
        // Two separate open lanes, not one merged lane.
        assert_eq!(snapshot.layout.row(0).unwrap().width, 2);
        assert!(snapshot.truncated);
    }

    #[test]
    fn branch_label_shows_detached_head() {
        let snapshot = RepoSnapshot::build(
            Log::default(),
            Head::Detached {
                target: "0123456789abcdef".into(),
            },
            Vec::new(),
            Vec::new(),
            WorkingTree::default(),
        )
        .unwrap();
        assert_eq!(snapshot.branch_label(), "HEAD (0123456)");
    }

    #[test]
    fn status_refresh_reuses_history_and_toggles_the_wip_row() {
        let log = Log {
            commits: vec![commit("b", &["a"], 2), commit("a", &[], 1)],
            truncated: false,
        };
        let head = Head::Branch {
            name: "main".into(),
            target: "b".into(),
        };
        let clean =
            RepoSnapshot::build(log, head, Vec::new(), Vec::new(), WorkingTree::default()).unwrap();
        assert!(!clean.has_wip);

        let dirty = clean.with_working_tree(dirty()).unwrap();
        assert!(dirty.has_wip);
        assert_eq!(dirty.layout.len(), 3);
        assert!(Arc::ptr_eq(&clean.commits, &dirty.commits));
        // WIP row is connected to HEAD (dashed edge into row 1).
        let wip = dirty.layout.row(0).unwrap();
        assert!(wip
            .segments
            .iter()
            .any(|s| matches!(s, git_graph::Segment::FromNode { dashed: true, .. })));

        let still_dirty = dirty.with_working_tree(dirty.working_tree.clone()).unwrap();
        assert!(Arc::ptr_eq(&dirty.layout, &still_dirty.layout));

        let clean_again = dirty.with_working_tree(WorkingTree::default()).unwrap();
        assert_eq!(clean_again.layout.len(), 2);
    }

    #[test]
    fn empty_repository() {
        let snapshot = RepoSnapshot::build(
            Log::default(),
            Head::Unborn {
                name: "main".into(),
            },
            Vec::new(),
            Vec::new(),
            dirty(),
        )
        .unwrap();
        assert!(snapshot.commits.is_empty());
        assert_eq!(snapshot.layout.len(), 1);
    }
}

/// Manual memory check against a large repository:
/// `GITLOOM_PERF_REPO=<path> cargo test -p gitloom --release snapshot_memory -- --ignored --nocapture`
#[cfg(test)]
mod perf {
    use super::*;

    fn rss_kb() -> u64 {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
            })
            .unwrap_or(0)
    }

    #[test]
    #[ignore = "manual memory check, needs GITLOOM_PERF_REPO"]
    fn snapshot_memory() {
        let Ok(path) = std::env::var("GITLOOM_PERF_REPO") else {
            return;
        };
        let before = rss_kb();
        let backend = git_core::GixBackend::discover(path).unwrap();
        let start = std::time::Instant::now();
        let snapshot = RepoSnapshot::load(&backend).unwrap();
        let elapsed = start.elapsed();
        let after = rss_kb();

        let text: usize = snapshot
            .commits
            .iter()
            .map(|c| c.summary.len() + c.body.len() + c.author.len() + c.email.len())
            .sum();
        let bodies: usize = snapshot.commits.iter().map(|c| c.body.len()).sum();
        let segments: usize = snapshot.layout.rows().map(|r| r.segments.len()).sum();
        eprintln!(
            "snapshot of {} commits in {elapsed:?}: RSS +{} MB, text {} MB (bodies {} MB), \
             {segments} segments if stored per row ({} MB), max width {}",
            snapshot.commits.len(),
            (after.saturating_sub(before)) / 1024,
            text / 1_000_000,
            bodies / 1_000_000,
            segments * std::mem::size_of::<git_graph::Segment>() / 1_000_000,
            snapshot.layout.max_width,
        );
    }
}
