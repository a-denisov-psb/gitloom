//! Branch folders of the left panel (M7-14): branch names are split at `/`
//! into folders like GitKraken. Pure; the sidebar renders the rows.

use std::cmp::Ordering;

/// A folder or a branch of one group (local branches, or one remote).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Folder {
        /// Last segment, e.g. `ui` for `feature/ui`.
        name: String,
        /// Folder path inside the group, e.g. `feature/ui`.
        path: String,
        children: Vec<Node>,
    },
    Branch {
        /// Index of the reference in the snapshot's refs.
        index: usize,
        /// Last segment of the branch name.
        label: String,
    },
}

impl Node {
    fn name(&self) -> &str {
        match self {
            Self::Folder { name, .. } => name,
            Self::Branch { label, .. } => label,
        }
    }

    fn is_folder(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }
}

/// Builds the folder tree of `branches` (index into the refs, name inside
/// the group: `feature/ui/x` for a local branch, `feature/x` for
/// `origin/feature/x`). Folders come before branches, both sorted
/// case-insensitively.
pub fn build<'a>(branches: impl IntoIterator<Item = (usize, &'a str)>) -> Vec<Node> {
    let mut root = Vec::new();
    for (index, name) in branches {
        insert(&mut root, "", index, name);
    }
    sort(&mut root);
    root
}

fn insert(nodes: &mut Vec<Node>, parent: &str, index: usize, rest: &str) {
    match rest.split_once('/') {
        Some((folder, tail)) if !folder.is_empty() && !tail.is_empty() => {
            let position = nodes
                .iter()
                .position(|n| n.is_folder() && n.name() == folder);
            let position = match position {
                Some(position) => position,
                None => {
                    let path = if parent.is_empty() {
                        folder.to_owned()
                    } else {
                        format!("{parent}/{folder}")
                    };
                    nodes.push(Node::Folder {
                        name: folder.to_owned(),
                        path,
                        children: Vec::new(),
                    });
                    nodes.len() - 1
                }
            };
            if let Some(Node::Folder { path, children, .. }) = nodes.get_mut(position) {
                let path = path.clone();
                insert(children, &path, index, tail);
            }
        }
        _ => nodes.push(Node::Branch {
            index,
            label: rest.to_owned(),
        }),
    }
}

fn sort(nodes: &mut [Node]) {
    nodes.sort_by(|a, b| {
        b.is_folder()
            .cmp(&a.is_folder())
            .then_with(|| compare_names(a.name(), b.name()))
    });
    for node in nodes {
        if let Node::Folder { children, .. } = node {
            sort(children);
        }
    }
}

/// Case-insensitive, ties in byte order (stable across platforms).
fn compare_names(a: &str, b: &str) -> Ordering {
    let lower = |s: &str| s.chars().flat_map(char::to_lowercase).collect::<String>();
    lower(a).cmp(&lower(b)).then_with(|| a.cmp(b))
}

/// GITFLOW section (M9-06): the production and development branches and
/// every branch with a Git flow prefix, the prefix as a folder ("feature" ›
/// "f2"). Like GitKraken, folders and branches are one case-insensitive
/// alphabetical list. `branches` are (index into the refs, local branch
/// name) after the text filter. Folder paths are the collapse keys
/// (`gitflow:feature`). Returns the tree and the number of branches.
pub fn gitflow<'a>(
    branches: impl IntoIterator<Item = (usize, &'a str)>,
    config: &git_core::FlowConfig,
) -> (Vec<Node>, usize) {
    let mut root: Vec<Node> = Vec::new();
    let mut count = 0;
    for (index, name) in branches {
        if name == config.production || name == config.development {
            root.push(Node::Branch {
                index,
                label: name.to_owned(),
            });
            count += 1;
            continue;
        }
        let Some(branch) = config.classify(name) else {
            continue;
        };
        count += 1;
        let prefix = config.prefix(branch.kind);
        let folder = prefix.trim_end_matches('/');
        let folder = if folder.is_empty() { prefix } else { folder };
        let path = format!("gitflow:{}", branch.kind.noun());
        let position = root
            .iter()
            .position(|n| matches!(n, Node::Folder { path: p, .. } if *p == path));
        let position = position.unwrap_or_else(|| {
            root.push(Node::Folder {
                name: folder.to_owned(),
                path: path.clone(),
                children: Vec::new(),
            });
            root.len() - 1
        });
        if let Some(Node::Folder { children, .. }) = root.get_mut(position) {
            children.push(Node::Branch {
                index,
                label: branch.name,
            });
        }
    }
    sort_mixed(&mut root);
    (root, count)
}

/// Sorts folders and branches together, case-insensitively.
fn sort_mixed(nodes: &mut [Node]) {
    nodes.sort_by(|a, b| compare_names(a.name(), b.name()));
    for node in nodes {
        if let Node::Folder { children, .. } = node {
            sort_mixed(children);
        }
    }
}

/// One visible row of a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row<'t> {
    Folder {
        depth: usize,
        name: &'t str,
        path: &'t str,
        expanded: bool,
    },
    Branch {
        depth: usize,
        index: usize,
        label: &'t str,
    },
}

/// Visible rows of `nodes`: children of collapsed folders are skipped.
/// `expand_all` (an active text filter) shows every folder expanded
/// without changing the stored state.
pub fn rows<'t>(
    nodes: &'t [Node],
    collapsed: &dyn Fn(&str) -> bool,
    expand_all: bool,
) -> Vec<Row<'t>> {
    let mut out = Vec::new();
    push_rows(&mut out, nodes, 0, collapsed, expand_all);
    out
}

fn push_rows<'t>(
    out: &mut Vec<Row<'t>>,
    nodes: &'t [Node],
    depth: usize,
    collapsed: &dyn Fn(&str) -> bool,
    expand_all: bool,
) {
    for node in nodes {
        match node {
            Node::Folder {
                name,
                path,
                children,
            } => {
                let expanded = expand_all || !collapsed(path);
                out.push(Row::Folder {
                    depth,
                    name,
                    path,
                    expanded,
                });
                if expanded {
                    push_rows(out, children, depth + 1, collapsed, expand_all);
                }
            }
            Node::Branch { index, label } => out.push(Row::Branch {
                depth,
                index: *index,
                label,
            }),
        }
    }
}

/// Key of a folder in the stored collapse state: section, remote and
/// folder path (`local:feature`, `remote:origin:feature/ui`). The remote
/// node itself is `remote:origin`.
pub fn folder_key(remote: Option<&str>, path: &str) -> String {
    match (remote, path.is_empty()) {
        (None, _) => format!("local:{path}"),
        (Some(remote), true) => format!("remote:{remote}"),
        (Some(remote), false) => format!("remote:{remote}:{path}"),
    }
}

/// Branches shown in the graph out of all (hidden and soloed ones count
/// as not shown; the checked-out branch always is).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub visible: usize,
    pub total: usize,
}

impl Counts {
    fn add(&mut self, shown: bool) {
        self.total += 1;
        self.visible += usize::from(shown);
    }

    /// Section count: "visible/total" while branches are hidden or soloed.
    pub fn label(self, filtered: bool) -> String {
        if filtered {
            format!("{}/{}", self.visible, self.total)
        } else {
            self.total.to_string()
        }
    }
}

/// The LOCAL and REMOTE sections of the left panel for one snapshot, text
/// filter and hide/solo state. Built once per change, not per frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Groups {
    pub local: Vec<Node>,
    /// Per remote (sorted by name): its branches. Configured remotes
    /// without fetched branches are listed too.
    pub remotes: Vec<(String, Vec<Node>)>,
    /// Matches of the text filter in LOCAL and REMOTE.
    pub local_count: Counts,
    pub remote_count: Counts,
    /// All branches, regardless of the text filter ("Viewing x/y").
    pub all: Counts,
}

impl Groups {
    pub fn build(
        refs: &[git_core::Reference],
        remotes: &[String],
        needle: &str,
        filter: &crate::ref_filter::RefFilter,
    ) -> Self {
        use git_core::RefKind;

        let needle = needle.to_lowercase();
        let matches = |name: &str| needle.is_empty() || name.to_lowercase().contains(&needle);
        let shown = |r: &git_core::Reference| r.is_head || !filter.dims(&r.name, r.kind);
        let mut groups = Self::default();
        let mut local = Vec::new();
        let mut by_remote: std::collections::BTreeMap<&str, Vec<(usize, &str)>> =
            std::collections::BTreeMap::new();
        for remote in remotes {
            if matches(remote) {
                by_remote.entry(remote.as_str()).or_default();
            }
        }
        for (index, reference) in refs.iter().enumerate() {
            let branch = match reference.kind {
                RefKind::LocalBranch | RefKind::RemoteBranch => true,
                RefKind::Tag => false,
            };
            if !branch {
                continue;
            }
            groups.all.add(shown(reference));
            if !matches(&reference.name) {
                continue;
            }
            if reference.kind == RefKind::LocalBranch {
                groups.local_count.add(shown(reference));
                local.push((index, reference.name.as_str()));
            } else {
                groups.remote_count.add(shown(reference));
                by_remote
                    .entry(reference.remote().unwrap_or("?"))
                    .or_default()
                    .push((index, reference.local_name()));
            }
        }
        groups.local = build(local);
        groups.remotes = by_remote
            .into_iter()
            .map(|(remote, branches)| (remote.to_owned(), build(branches)))
            .collect();
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow() -> git_core::FlowConfig {
        git_core::FlowConfig::defaults(&["master"])
    }

    /// Rows as compact text: folders with "▾"/"▸", branches with their index.
    fn flow_text(names: &[&str], collapsed: &[&str], filtering: bool) -> (String, usize) {
        let (tree, count) = gitflow(names.iter().copied().enumerate(), &flow());
        let text = rows(&tree, &|path| collapsed.contains(&path), filtering)
            .into_iter()
            .map(|row| match row {
                Row::Folder {
                    depth,
                    name,
                    expanded,
                    ..
                } => format!(
                    "{}{}{name}",
                    "  ".repeat(depth),
                    if expanded { "▾" } else { "▸" }
                ),
                Row::Branch {
                    depth,
                    index,
                    label,
                } => format!("{}{label}#{index}", "  ".repeat(depth)),
            })
            .collect::<Vec<_>>()
            .join("\n");
        (text, count)
    }

    #[test]
    fn gitflow_mixes_folders_and_branches_alphabetically() {
        let names = [
            "master",
            "feature/f2",
            "develop",
            "release/1.2.0",
            "feature/Alpha",
            "topic",
            "hotfix/1.1.1",
            "feature/ui/toolbar",
        ];
        let (text, count) = flow_text(&names, &[], false);
        assert_eq!(
            text,
            "develop#2\n▾feature\n  Alpha#4\n  f2#1\n  ui/toolbar#7\n▾hotfix\n  1.1.1#6\n\
             master#0\n▾release\n  1.2.0#3"
        );
        assert_eq!(count, 7, "topic is not a Git flow branch");
    }

    #[test]
    fn gitflow_without_main_branches_and_collapsed_folders() {
        // Missing production/development branches are not shown.
        let (text, count) = flow_text(&["feature/a", "feature/b"], &["gitflow:feature"], false);
        assert_eq!(text, "▸feature");
        assert_eq!(count, 2);
        // A text filter expands everything.
        let (text, _) = flow_text(&["feature/a"], &["gitflow:feature"], true);
        assert_eq!(text, "▾feature\n  a#0");
        let (text, count) = flow_text(&[], &[], false);
        assert_eq!((text.as_str(), count), ("", 0));
    }

    #[test]
    fn gitflow_uses_custom_prefixes_as_folders() {
        let mut config = flow();
        config.feature = "feat-".into();
        let (tree, count) = gitflow([(0, "feat-x"), (1, "main")], &config);
        assert_eq!(count, 1);
        assert!(matches!(&tree[0], Node::Folder { name, .. } if name == "feat-"));
    }

    /// Compact text of the rows: indentation per depth, folders end in `/`.
    fn render(nodes: &[Node], collapsed: &[&str], expand_all: bool) -> String {
        let is_collapsed = |path: &str| collapsed.contains(&path);
        rows(nodes, &is_collapsed, expand_all)
            .iter()
            .map(|row| match row {
                Row::Folder {
                    depth,
                    name,
                    expanded,
                    ..
                } => format!(
                    "{}{}{name}/",
                    "  ".repeat(*depth),
                    if *expanded { "" } else { "+" }
                ),
                Row::Branch {
                    depth,
                    index,
                    label,
                } => format!("{}{label} #{index}", "  ".repeat(*depth)),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn tree(names: &[&str]) -> Vec<Node> {
        build(names.iter().copied().enumerate())
    }

    #[test]
    fn nests_folders_of_any_depth() {
        let nodes = tree(&[
            "main",
            "feature/ui/toolbar",
            "feature/ui/sidebar",
            "feature/api",
        ]);
        assert_eq!(
            render(&nodes, &[], false),
            "feature/\n  ui/\n    sidebar #2\n    toolbar #1\n  api #3\nmain #0"
        );
    }

    #[test]
    fn folders_before_branches_case_insensitive() {
        let nodes = tree(&["Zeta", "alpha", "beta/x", "Alpha/y", "Beta"]);
        assert_eq!(
            render(&nodes, &[], false),
            "Alpha/\n  y #3\nbeta/\n  x #2\nalpha #1\nBeta #4\nZeta #0"
        );
    }

    #[test]
    fn a_branch_and_a_folder_with_the_same_name() {
        // Possible across groups and for odd names; both stay visible.
        let nodes = tree(&["x", "x/y"]);
        assert_eq!(render(&nodes, &[], false), "x/\n  y #1\nx #0");
    }

    #[test]
    fn odd_slashes_stay_in_the_label() {
        let nodes = tree(&["/lead", "trail/"]);
        assert_eq!(render(&nodes, &[], false), "/lead #0\ntrail/ #1");
    }

    #[test]
    fn collapsed_folders_hide_their_children() {
        let nodes = tree(&["feature/ui/a", "feature/b", "hotfix/c"]);
        assert_eq!(
            render(&nodes, &["feature/ui", "hotfix"], false),
            "feature/\n  +ui/\n  b #1\n+hotfix/"
        );
        assert_eq!(
            render(&nodes, &["feature"], false),
            "+feature/\nhotfix/\n  c #2"
        );
    }

    #[test]
    fn filter_shows_matches_with_expanded_ancestors() {
        // The sidebar builds the tree from the matches only.
        let names = ["feature/ui/toolbar", "feature/api", "main"];
        let matches: Vec<_> = names
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, n)| n.contains("tool"))
            .collect();
        let nodes = build(matches);
        assert_eq!(
            render(&nodes, &["feature", "feature/ui"], true),
            "feature/\n  ui/\n    toolbar #0"
        );
    }

    #[test]
    fn remote_with_and_without_folders() {
        // Names inside the remote group (`origin/` stripped).
        assert_eq!(
            render(&tree(&["main", "dev"]), &[], false),
            "dev #1\nmain #0"
        );
        assert_eq!(
            render(&tree(&["main", "release/1.0"]), &[], false),
            "release/\n  1.0 #1\nmain #0"
        );
    }

    #[test]
    fn folder_keys() {
        assert_eq!(folder_key(None, "feature/ui"), "local:feature/ui");
        assert_eq!(folder_key(Some("origin"), ""), "remote:origin");
        assert_eq!(
            folder_key(Some("origin"), "feature"),
            "remote:origin:feature"
        );
    }

    #[test]
    fn two_thousand_branches_build_quickly() {
        let names: Vec<String> = (0..2_000)
            .map(|i| format!("team{}/feature/topic-{i}", i % 20))
            .collect();
        let start = std::time::Instant::now();
        let nodes = build(names.iter().enumerate().map(|(i, n)| (i, n.as_str())));
        let shown = rows(&nodes, &|_| false, false).len();
        let elapsed = start.elapsed();
        assert_eq!(shown, 2_000 + 20 * 2);
        // Target 5 ms (release); generous for debug builds on CI.
        assert!(elapsed.as_millis() < 200, "{elapsed:?}");
    }

    fn reference(name: &str, kind: git_core::RefKind, is_head: bool) -> git_core::Reference {
        git_core::Reference {
            name: name.into(),
            kind,
            target: "c".into(),
            is_head,
            upstream: None,
            ahead_behind: None,
        }
    }

    fn sample() -> Vec<git_core::Reference> {
        use git_core::RefKind::{LocalBranch, RemoteBranch, Tag};
        vec![
            reference("main", LocalBranch, true),
            reference("feature/ui", LocalBranch, false),
            reference("feature/api", LocalBranch, false),
            reference("origin/main", RemoteBranch, false),
            reference("origin/feature/ui", RemoteBranch, false),
            reference("v1", Tag, false),
        ]
    }

    #[test]
    fn groups_split_local_and_remotes() {
        let remotes = vec!["origin".to_owned(), "backup".to_owned()];
        let groups = Groups::build(&sample(), &remotes, "", &Default::default());
        assert_eq!(
            render(&groups.local, &[], false),
            "feature/\n  api #2\n  ui #1\nmain #0"
        );
        let names: Vec<_> = groups.remotes.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["backup", "origin"], "remotes without branches too");
        assert_eq!(
            render(&groups.remotes[1].1, &[], false),
            "feature/\n  ui #4\nmain #3"
        );
        assert_eq!(
            groups.local_count,
            Counts {
                visible: 3,
                total: 3
            }
        );
        assert_eq!(
            groups.remote_count,
            Counts {
                visible: 2,
                total: 2
            }
        );
        assert_eq!(
            groups.all,
            Counts {
                visible: 5,
                total: 5
            }
        );
    }

    #[test]
    fn counts_with_hidden_and_soloed_branches() {
        use git_core::RefKind::{LocalBranch, RemoteBranch};
        let mut filter = crate::ref_filter::RefFilter::default();
        filter.toggle_hidden("feature/api", LocalBranch);
        filter.toggle_hidden("origin/main", RemoteBranch);
        let groups = Groups::build(&sample(), &[], "", &filter);
        assert_eq!(
            groups.local_count,
            Counts {
                visible: 2,
                total: 3
            }
        );
        assert_eq!(
            groups.remote_count,
            Counts {
                visible: 1,
                total: 2
            }
        );
        assert_eq!(
            groups.all,
            Counts {
                visible: 3,
                total: 5
            }
        );
        assert_eq!(groups.local_count.label(true), "2/3");
        assert_eq!(groups.local_count.label(false), "3");

        // Solo: only soloed branches and the checked-out one are shown.
        let mut solo = crate::ref_filter::RefFilter::default();
        solo.toggle_soloed("feature/ui", LocalBranch);
        let groups = Groups::build(&sample(), &[], "", &solo);
        assert_eq!(
            groups.all,
            Counts {
                visible: 2,
                total: 5
            }
        );
    }

    #[test]
    fn text_filter_narrows_sections_but_not_the_viewing_count() {
        let remotes = vec!["origin".to_owned()];
        let groups = Groups::build(&sample(), &remotes, "UI", &Default::default());
        assert_eq!(render(&groups.local, &[], true), "feature/\n  ui #1");
        assert_eq!(groups.local_count.total, 1);
        assert_eq!(groups.remote_count.total, 1);
        assert_eq!(groups.all.total, 5);
        // A remote whose name does not match is only listed with matches.
        let groups = Groups::build(&sample(), &remotes, "api", &Default::default());
        assert!(groups.remotes.is_empty());
    }
}
