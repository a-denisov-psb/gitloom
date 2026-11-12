use std::path::Path;

use crate::{
    diff, ChangeKind, Commit, DiffRequest, Error, FileChange, FileDiff, GitBackend, Head, Log, Oid,
    RefKind, Reference, RepoState, Result, Stash, StashChanges, WorkingTree,
};

/// Static sample repository for UI development without a real repo.
pub struct MockBackend {
    name: String,
    commits: Vec<Commit>,
}

/// (summary, body, parent indices, author, hours before `BASE_TIME`)
type Row = (
    &'static str,
    &'static str,
    &'static [usize],
    &'static str,
    i64,
);

const BASE_TIME: i64 = 1_792_000_000;

const HISTORY: &[Row] = &[
    ("Graph: draft lane assignment", "", &[2], "Alice Meyer", 1),
    ("Update README", "", &[2], "Bob Schulz", 3),
    (
        "Merge branch 'feature/sidebar'",
        "",
        &[3, 5],
        "Alice Meyer",
        20,
    ),
    ("Fix tab close button alignment", "", &[4], "Bob Schulz", 26),
    (
        "Add status bar",
        "Shows the current branch and commit count.",
        &[6],
        "Alice Meyer",
        30,
    ),
    ("Sidebar: collapsible sections", "", &[7], "Chris Wolf", 41),
    (
        "Toolbar: action buttons",
        "Undo, redo, pull, push, branch, stash and pop.",
        &[8],
        "Alice Meyer",
        50,
    ),
    ("Sidebar: filter input", "", &[8], "Chris Wolf", 55),
    ("Layout: resizable pane grid", "", &[9], "Alice Meyer", 72),
    ("Add repository tabs", "", &[10], "Bob Schulz", 96),
    (
        "Define GitBackend trait",
        "Read-only for now, write ops follow.",
        &[11],
        "Alice Meyer",
        120,
    ),
    ("Set up cargo workspace", "", &[12], "Alice Meyer", 140),
    ("Initial commit", "", &[], "Alice Meyer", 160),
];

const FILE_POOL: &[&str] = &[
    "crates/app/src/ui/graph.rs",
    "crates/app/src/ui/sidebar.rs",
    "crates/app/src/ui/toolbar.rs",
    "crates/app/src/app.rs",
    "crates/git-core/src/lib.rs",
    "README.md",
    "Cargo.toml",
];

impl MockBackend {
    pub fn new(name: impl Into<String>) -> Self {
        let ids: Vec<Oid> = (0..HISTORY.len()).map(fake_oid).collect();

        let commits = HISTORY
            .iter()
            .enumerate()
            .map(|(i, (summary, body, parents, author, hours))| Commit {
                id: ids[i].clone(),
                parents: parents.iter().map(|&p| ids[p].clone()).collect(),
                summary: summary.to_string(),
                body: body.to_string(),
                author: author.to_string(),
                email: format!("{}@example.com", author.to_lowercase().replace(' ', ".")),
                author_time: BASE_TIME - hours * 3600,
                commit_time: BASE_TIME - hours * 3600,
                committer: String::new(),
            })
            .collect();

        Self {
            name: name.into(),
            commits,
        }
    }

    fn reference(&self, name: &str, kind: RefKind, index: usize, is_head: bool) -> Reference {
        Reference {
            name: name.to_string(),
            kind,
            target: self.commits[index].id.clone(),
            is_head,
            upstream: None,
            ahead_behind: None,
        }
    }
}

impl GitBackend for MockBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &Path {
        Path::new(".")
    }

    fn log(&self, limit: usize) -> Result<Log> {
        Ok(Log {
            commits: self.commits.iter().take(limit).cloned().collect(),
            truncated: self.commits.len() > limit,
        })
    }

    fn head(&self) -> Result<Head> {
        Ok(Head::Branch {
            name: "main".into(),
            target: self.commits[1].id.clone(),
        })
    }

    fn references(&self) -> Result<Vec<Reference>> {
        Ok(vec![
            self.reference("main", RefKind::LocalBranch, 1, true),
            self.reference("feature/graph", RefKind::LocalBranch, 0, false),
            self.reference("feature/sidebar", RefKind::LocalBranch, 5, false),
            self.reference("origin/main", RefKind::RemoteBranch, 2, false),
            self.reference("origin/feature/sidebar", RefKind::RemoteBranch, 5, false),
            self.reference("upstream/main", RefKind::RemoteBranch, 8, false),
            self.reference("v0.1.0", RefKind::Tag, 8, false),
        ])
    }

    fn remotes(&self) -> Result<Vec<String>> {
        Ok(vec!["origin".into(), "upstream".into()])
    }

    fn remote_url(&self, name: &str) -> Result<Option<String>> {
        Ok(Some(format!("https://example.com/{name}.git")))
    }

    fn stashes(&self) -> Result<Vec<Stash>> {
        Ok(vec![Stash {
            index: 0,
            message: "WIP on main: try icon font".into(),
            id: fake_oid(HISTORY.len()),
            base: Some(self.commits[1].id.clone()),
            time: BASE_TIME,
            author: "Alice Meyer".into(),
        }])
    }

    fn working_tree(&self) -> Result<WorkingTree> {
        let change = |path: &str, kind| FileChange {
            path: path.into(),
            kind,
            old_path: None,
        };
        Ok(WorkingTree {
            unstaged: vec![
                change("crates/app/src/ui/graph.rs", ChangeKind::Modified),
                change("crates/app/src/ui/lanes.rs", ChangeKind::Untracked),
                change("README.md", ChangeKind::Modified),
            ],
            staged: vec![change("Cargo.toml", ChangeKind::Modified)],
        })
    }

    fn commit_changes(&self, id: &str) -> Result<Vec<FileChange>> {
        let index = self
            .commits
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| Error::NotFound(format!("commit {id}")))?;

        let count = 1 + index % 3;
        Ok((0..count)
            .map(|n| FileChange {
                path: FILE_POOL[(index + n * 2) % FILE_POOL.len()].into(),
                kind: if index == HISTORY.len() - 1 {
                    ChangeKind::Added
                } else {
                    ChangeKind::Modified
                },
                old_path: None,
            })
            .collect())
    }

    fn stash_changes(&self, _id: &str) -> Result<StashChanges> {
        Ok(StashChanges::default())
    }

    fn commit_template(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn state(&self) -> Result<RepoState> {
        Ok(RepoState::Clean)
    }

    fn merge_message(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn worktree_content(&self, _path: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn file_diff_with(
        &self,
        request: &DiffRequest,
        _context: crate::DiffContext,
    ) -> Result<FileDiff> {
        Ok(Self::sample_diff(request))
    }
}

impl MockBackend {
    /// Same sample diff for every file.
    fn sample_diff(request: &DiffRequest) -> FileDiff {
        let text = diff::text_diff(b"fn main() {}\n", b"fn main() {\n    run();\n}\n");
        FileDiff {
            old_path: Some(request.path.clone()),
            new_path: Some(request.path.clone()),
            binary: text.binary,
            too_large: text.too_large,
            encoding: None,
            hunks: text.hunks,
        }
    }
}

/// Deterministic pseudo object id (splitmix64).
fn fake_oid(seed: usize) -> Oid {
    let mut state = seed as u64 ^ 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    format!("{:016x}{:016x}{:08x}", next(), next(), next() as u32)
}
