//! File list of the commit details (M14-03): summary per change kind,
//! sorting, Path/Tree mode and "View all files". Pure: no iced types.

use git_core::{ChangeKind, FileChange};
use serde::{Deserialize, Serialize};

/// Global view options of the details' file list (`settings.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FileListOptions {
    /// Z→A instead of A→Z.
    pub descending: bool,
    /// Tree instead of Path.
    pub tree: bool,
    /// All files of the commit's tree, not only the changed ones.
    pub all_files: bool,
}

/// Kinds in the order of the summary line.
const ORDER: [ChangeKind; 6] = [
    ChangeKind::Added,
    ChangeKind::Modified,
    ChangeKind::Deleted,
    ChangeKind::Renamed,
    ChangeKind::Untracked,
    ChangeKind::Conflicted,
];

/// Count per change kind, in summary order, without empty kinds.
pub fn counts(files: &[FileChange]) -> Vec<(ChangeKind, usize)> {
    ORDER
        .iter()
        .map(|kind| (*kind, files.iter().filter(|f| f.kind == *kind).count()))
        .filter(|(_, count)| *count > 0)
        .collect()
}

/// "2 added", "1 modified", "3 renamed"; the words do not change with the
/// number, like GitKraken's summary.
pub fn count_label(kind: ChangeKind, count: usize) -> String {
    let word = match kind {
        ChangeKind::Added => "added",
        ChangeKind::Modified => "modified",
        ChangeKind::Deleted => "deleted",
        ChangeKind::Renamed => "renamed",
        ChangeKind::Untracked => "untracked",
        ChangeKind::Conflicted => "conflicted",
    };
    format!("{count} {word}")
}

/// The whole summary line: "2 added · 1 modified", "No changes".
pub fn summary(files: &[FileChange]) -> String {
    let parts: Vec<String> = counts(files)
        .into_iter()
        .map(|(kind, count)| count_label(kind, count))
        .collect();
    if parts.is_empty() {
        "No changes".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// A row of the list: a changed file, or an unchanged one ("View all files").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry<'a> {
    Changed(&'a FileChange),
    Unchanged(&'a str),
}

impl Entry<'_> {
    pub fn path(&self) -> &str {
        match self {
            Self::Changed(file) => &file.path,
            Self::Unchanged(path) => path,
        }
    }
}

/// Entries sorted by path (case-insensitive, then exact), A→Z or Z→A;
/// with `all` the unchanged files of the commit's tree are added.
pub fn entries<'a>(
    changes: &'a [FileChange],
    all: Option<&'a [String]>,
    descending: bool,
) -> Vec<Entry<'a>> {
    let mut entries: Vec<Entry<'a>> = changes.iter().map(Entry::Changed).collect();
    if let Some(all) = all {
        let changed: std::collections::HashSet<&str> =
            changes.iter().map(|f| f.path.as_str()).collect();
        entries.extend(
            all.iter()
                .filter(|p| !changed.contains(p.as_str()))
                .map(|p| Entry::Unchanged(p)),
        );
    }
    entries.sort_by(|a, b| compare(a.path(), b.path()));
    if descending {
        entries.reverse();
    }
    entries
}

fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

/// Folder tree of `entries` (index into them, path) with the left panel's
/// rules (M11-11): files first, then folders, case-insensitive; Z→A
/// reverses every level.
pub fn tree(entries: &[Entry<'_>], descending: bool) -> Vec<crate::branch_tree::Node> {
    let mut nodes =
        crate::branch_tree::build(entries.iter().enumerate().map(|(i, e)| (i, e.path())));
    if descending {
        reverse(&mut nodes);
    }
    nodes
}

fn reverse(nodes: &mut [crate::branch_tree::Node]) {
    use crate::branch_tree::Node;
    // Files stay before folders; each group reverses.
    let files = nodes
        .iter()
        .filter(|n| matches!(n, Node::Branch { .. }))
        .count();
    nodes[..files].reverse();
    nodes[files..].reverse();
    for node in nodes {
        if let Node::Folder { children, .. } = node {
            reverse(children);
        }
    }
}

/// Every folder path of `nodes`, for "Expand all" / "Collapse all".
pub fn folder_paths(nodes: &[crate::branch_tree::Node]) -> Vec<String> {
    use crate::branch_tree::Node;
    let mut out = Vec::new();
    for node in nodes {
        if let Node::Folder { path, children, .. } = node {
            out.push(path.clone());
            out.extend(folder_paths(children));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch_tree::{rows, Row};

    fn change(path: &str, kind: ChangeKind) -> FileChange {
        FileChange {
            path: path.into(),
            kind,
            old_path: None,
        }
    }

    #[test]
    fn summary_counts_every_kind() {
        let files = [
            change("a", ChangeKind::Added),
            change("b", ChangeKind::Added),
            change("c", ChangeKind::Modified),
            change("d", ChangeKind::Deleted),
            change("e", ChangeKind::Renamed),
        ];
        assert_eq!(
            summary(&files),
            "2 added · 1 modified · 1 deleted · 1 renamed"
        );
        assert_eq!(summary(&files[2..3]), "1 modified");
        assert_eq!(summary(&[]), "No changes");
        assert_eq!(
            counts(&[
                change("x", ChangeKind::Conflicted),
                change("y", ChangeKind::Untracked)
            ]),
            [(ChangeKind::Untracked, 1), (ChangeKind::Conflicted, 1)]
        );
    }

    #[test]
    fn entries_sort_and_add_unchanged_files() {
        let changes = [
            change("src/b.rs", ChangeKind::Modified),
            change("README.md", ChangeKind::Added),
        ];
        let all: Vec<String> = ["a.txt", "README.md", "src/b.rs", "src/C.rs"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let paths = |e: &[Entry<'_>]| e.iter().map(|e| e.path().to_owned()).collect::<Vec<_>>();
        assert_eq!(
            paths(&entries(&changes, None, false)),
            ["README.md", "src/b.rs"]
        );
        assert_eq!(
            paths(&entries(&changes, None, true)),
            ["src/b.rs", "README.md"]
        );
        let with_all = entries(&changes, Some(&all), false);
        assert_eq!(
            paths(&with_all),
            ["a.txt", "README.md", "src/b.rs", "src/C.rs"]
        );
        assert!(matches!(with_all[0], Entry::Unchanged("a.txt")));
        assert!(matches!(with_all[1], Entry::Changed(_)));
    }

    #[test]
    fn tree_lists_files_before_folders_and_reverses() {
        let changes = [
            change("src/ui/b.rs", ChangeKind::Modified),
            change("src/a.rs", ChangeKind::Modified),
            change("Cargo.toml", ChangeKind::Modified),
            change("docs/x.md", ChangeKind::Added),
        ];
        let list = entries(&changes, None, false);
        let labels = |nodes: &[crate::branch_tree::Node]| -> Vec<String> {
            rows(nodes, &|_| false, false)
                .iter()
                .map(|row| match row {
                    Row::Folder { depth, name, .. } => format!("{depth}:{name}/"),
                    Row::Branch { depth, label, .. } => format!("{depth}:{label}"),
                })
                .collect()
        };
        let nodes = tree(&list, false);
        assert_eq!(
            labels(&nodes),
            [
                "0:Cargo.toml",
                "0:docs/",
                "1:x.md",
                "0:src/",
                "1:a.rs",
                "1:ui/",
                "2:b.rs"
            ]
        );
        assert_eq!(folder_paths(&nodes), ["docs", "src", "src/ui"]);
        let list = entries(&changes, None, true);
        assert_eq!(
            labels(&tree(&list, true)),
            [
                "0:Cargo.toml",
                "0:src/",
                "1:a.rs",
                "1:ui/",
                "2:b.rs",
                "0:docs/",
                "1:x.md"
            ]
        );
        // Collapsed folders hide their files.
        let collapsed = rows(&nodes, &|p| p == "src", false);
        assert_eq!(collapsed.len(), 4);
    }

    #[test]
    fn options_round_trip() {
        let options = FileListOptions {
            descending: true,
            tree: true,
            all_files: false,
        };
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(
            serde_json::from_str::<FileListOptions>(&json).unwrap(),
            options
        );
        assert_eq!(
            serde_json::from_str::<FileListOptions>("{}").unwrap(),
            FileListOptions::default()
        );
    }
}
