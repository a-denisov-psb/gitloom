//! Git flow classification (M13-01): which branch type every commit belongs
//! to, also for branches deleted long ago, so the graph can be drawn like
//! the classic Git flow diagram. Pure: commits, merge subjects, ref tips
//! and the Git flow names in; a segment per first-parent chain out.
//!
//! Steps (decisions table of M13):
//! 1. production and development are the first-parent chains of their tips
//!    (production wins where both meet);
//! 2. every other live branch tip: its first-parent chain down to a
//!    classified commit, typed by its prefix;
//! 3. every remaining first-parent chain (newest first) is a segment of a
//!    deleted branch: name from the subjects of the merges that merge it,
//!    type from the name's prefix, else from the topology (where it forks
//!    off, where it is merged), resolved until nothing changes;
//! 4. chains no merge reaches are unknown.

use std::collections::HashMap;

use crate::{topo_order, Error, GraphCommit, Id};

/// Branch type of a commit, also its lane group (left to right).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowKind {
    Production,
    Hotfix,
    Release,
    Development,
    Feature,
    /// Not reached by any merge or live branch (e.g. only by a tag).
    Unknown,
}

impl FlowKind {
    /// Lane group: 0 production … 4 feature (also unknown).
    pub fn group(self) -> u16 {
        match self {
            Self::Production => 0,
            Self::Hotfix => 1,
            Self::Release => 2,
            Self::Development => 3,
            Self::Feature | Self::Unknown => 4,
        }
    }

    /// Lower case name for labels and tooltips.
    pub fn label(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Hotfix => "hotfix",
            Self::Release => "release",
            Self::Development => "development",
            Self::Feature => "feature",
            Self::Unknown => "unknown",
        }
    }
}

/// Where the type of a segment comes from (priority in this order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowSource {
    LiveRef,
    MergeMessage,
    Topology,
    Unknown,
}

/// One first-parent chain of commits with the same type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowSegment {
    /// Newest commit of the chain.
    pub tip: Id,
    pub kind: FlowKind,
    /// Branch name (without remote), if known.
    pub name: Option<String>,
    pub source: FlowSource,
}

/// Git flow branch names (`FlowConfig` of M7-06).
#[derive(Debug, Clone, Copy)]
pub struct FlowNames<'a> {
    pub production: &'a str,
    pub development: &'a str,
    pub feature: &'a str,
    pub release: &'a str,
    pub hotfix: &'a str,
}

impl FlowNames<'_> {
    /// Type of a branch name: development or production by name, else the
    /// prefix; `None` without a Git flow prefix.
    fn kind_of(&self, name: &str) -> Option<FlowKind> {
        let prefixed = |prefix: &str| !prefix.is_empty() && name.starts_with(prefix);
        if name == self.production {
            Some(FlowKind::Production)
        } else if name == self.development {
            Some(FlowKind::Development)
        } else if prefixed(self.hotfix) {
            Some(FlowKind::Hotfix)
        } else if prefixed(self.release) {
            Some(FlowKind::Release)
        } else if prefixed(self.feature) {
            Some(FlowKind::Feature)
        } else {
            None
        }
    }
}

/// A branch tip: short name without remote, commit, and whether it is a
/// remote-tracking branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowTip {
    pub name: String,
    pub commit: Id,
    pub remote: bool,
}

/// Input of [`classify`].
#[derive(Debug, Clone, Copy)]
pub struct FlowInput<'a> {
    pub commits: &'a [GraphCommit],
    /// Subject of every merge commit, indexed by [`Id`] (`None` for others).
    pub subjects: &'a [Option<String>],
    pub tips: &'a [FlowTip],
    pub names: FlowNames<'a>,
}

/// Result of [`classify`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Classification {
    pub segments: Vec<FlowSegment>,
    /// Segment of every commit, indexed by [`Id`].
    pub commit_segment: Vec<u32>,
}

impl Classification {
    /// The segment of commit `id`.
    pub fn segment_of(&self, id: Id) -> Option<&FlowSegment> {
        let index = *self.commit_segment.get(id.index())?;
        self.segments.get(index as usize)
    }
}

/// A parsed merge subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeSubject {
    /// Merged branch without remote or owner.
    pub name: String,
    /// `into <target>` when Git wrote it.
    pub into: Option<String>,
}

/// Parses Git's and the hosts' default merge subjects:
/// `Merge branch '<name>'[ of <url>][ into <target>]`,
/// `Merge remote-tracking branch '<remote>/<name>'[ into <target>]`,
/// `Merge pull request #<n> from <owner>/<name>`.
pub fn parse_merge_subject(subject: &str) -> Option<MergeSubject> {
    let subject = subject.lines().next()?.trim();
    let into = |rest: &str| {
        rest.find(" into ")
            .map(|i| rest[i + " into ".len()..].trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    let quoted = |rest: &str| -> Option<(String, String)> {
        let rest = rest.strip_prefix('\'')?;
        let end = rest.find('\'')?;
        Some((rest[..end].to_owned(), rest[end + 1..].to_owned()))
    };
    if let Some(rest) = subject.strip_prefix("Merge branch ") {
        let (name, rest) = quoted(rest)?;
        return (!name.is_empty()).then(|| MergeSubject {
            name,
            into: into(&rest),
        });
    }
    if let Some(rest) = subject.strip_prefix("Merge remote-tracking branch ") {
        let (full, rest) = quoted(rest)?;
        let (_, name) = full.split_once('/')?;
        return (!name.is_empty()).then(|| MergeSubject {
            name: name.to_owned(),
            into: into(&rest),
        });
    }
    if let Some(rest) = subject.strip_prefix("Merge pull request #") {
        let (number, rest) = rest.split_once(" from ")?;
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let source = rest.split_whitespace().next()?;
        let (_, name) = source.split_once('/')?;
        return (!name.is_empty()).then(|| MergeSubject {
            name: name.to_owned(),
            into: None,
        });
    }
    None
}

const NONE: u32 = u32::MAX;

/// Classifies every commit (see the module documentation).
pub fn classify(input: &FlowInput<'_>) -> Result<Classification, Error> {
    let commits = input.commits;
    let order = topo_order(commits)?.order;
    let count = commits.len();
    let mut segment_of = vec![NONE; count];
    let mut segments: Vec<FlowSegment> = Vec::new();
    // Segments typed later (step 3): kind decided by `resolve`.
    let mut pending: Vec<bool> = Vec::new();

    let parent = |id: Id| -> Option<Id> {
        commits
            .get(id.index())
            .and_then(|c| c.parents.first().copied())
            .filter(|p| p.index() < count)
    };
    // Walks the first-parent chain from `tip` while commits are free.
    let walk = |segment_of: &mut Vec<u32>, tip: Id, index: u32| {
        let mut at = Some(tip);
        while let Some(id) = at {
            if segment_of[id.index()] != NONE {
                break;
            }
            segment_of[id.index()] = index;
            at = parent(id);
        }
    };

    // Step 1 and 2: live branches, production and development first; local
    // branches before remote-tracking ones without a local twin.
    let names = input.names;
    let local = |name: &str| input.tips.iter().any(|t| !t.remote && t.name == name);
    let mut tips: Vec<&FlowTip> = input
        .tips
        .iter()
        .filter(|t| t.commit.index() < count && t.name != "HEAD")
        .filter(|t| !t.remote || !local(&t.name))
        .collect();
    let rank = |t: &FlowTip| match names.kind_of(&t.name) {
        Some(FlowKind::Production) => 0,
        Some(FlowKind::Development) => 1,
        _ => 2 + usize::from(t.remote),
    };
    tips.sort_by_key(|t| rank(t));
    let mut seen_names: Vec<&str> = Vec::new();
    for tip in tips {
        // One segment per name (e.g. origin/main next to upstream/main).
        if seen_names.contains(&tip.name.as_str()) || segment_of[tip.commit.index()] != NONE {
            continue;
        }
        seen_names.push(&tip.name);
        let index = segments.len() as u32;
        segments.push(FlowSegment {
            tip: tip.commit,
            kind: names.kind_of(&tip.name).unwrap_or(FlowKind::Feature),
            name: Some(tip.name.clone()),
            source: FlowSource::LiveRef,
        });
        pending.push(false);
        walk(&mut segment_of, tip.commit, index);
    }

    // Step 3: every remaining chain, newest first.
    for &id in &order {
        if segment_of[id.index()] != NONE {
            continue;
        }
        let index = segments.len() as u32;
        segments.push(FlowSegment {
            tip: id,
            kind: FlowKind::Unknown,
            name: None,
            source: FlowSource::Unknown,
        });
        pending.push(true);
        walk(&mut segment_of, id, index);
    }

    // Merges into each pending segment (any of its commits as a non-first
    // parent), and its fork point (first parent of its oldest commit).
    let mut targets: HashMap<u32, Vec<Id>> = HashMap::new();
    for commit in commits {
        for p in commit.parents.iter().skip(1) {
            if let Some(&segment) = segment_of.get(p.index()) {
                if pending.get(segment as usize) == Some(&true) {
                    targets.entry(segment).or_default().push(commit.id);
                }
            }
        }
    }
    let mut fork: HashMap<u32, Id> = HashMap::new();
    for commit in commits {
        let own = segment_of[commit.id.index()];
        if pending.get(own as usize) != Some(&true) {
            continue;
        }
        if let Some(p) = parent(commit.id) {
            if segment_of[p.index()] != own {
                fork.insert(own, p);
            }
        }
    }

    // Names and prefix types from the merge subjects.
    for (segment, merges) in &targets {
        let name = merges.iter().find_map(|m| {
            input
                .subjects
                .get(m.index())
                .and_then(|s| s.as_deref())
                .and_then(parse_merge_subject)
                .map(|s| s.name)
        });
        let entry = &mut segments[*segment as usize];
        if let Some(name) = name {
            if let Some(kind) = names.kind_of(&name) {
                entry.kind = kind;
                entry.source = FlowSource::MergeMessage;
                pending[*segment as usize] = false;
            }
            entry.name = Some(name);
        }
    }

    // Topology, repeated until nothing changes (nested features); then the
    // rest with whatever is known.
    let kind_at = |segments: &[FlowSegment], pending: &[bool], id: Id| -> Option<FlowKind> {
        let segment = segment_of[id.index()] as usize;
        (!pending[segment]).then(|| segments[segment].kind)
    };
    for last_round in [false, true] {
        loop {
            let mut changed = false;
            for segment in 0..segments.len() {
                if !pending[segment] {
                    continue;
                }
                let Some(merges) = targets.get(&(segment as u32)) else {
                    continue;
                };
                let kinds: Vec<Option<FlowKind>> = merges
                    .iter()
                    .map(|m| kind_at(&segments, &pending, *m))
                    .collect();
                let origin = fork
                    .get(&(segment as u32))
                    .map(|f| kind_at(&segments, &pending, *f));
                // Wait for unresolved neighbours, except in the last round.
                let waiting = kinds.iter().any(Option::is_none) || origin == Some(None);
                if waiting && !last_round {
                    continue;
                }
                let known: Vec<FlowKind> = kinds.into_iter().flatten().collect();
                segments[segment].kind = by_topology(&known, origin.flatten());
                segments[segment].source = FlowSource::Topology;
                pending[segment] = false;
                changed = true;
            }
            if !changed {
                break;
            }
        }
    }
    // Step 4: no merge reaches them.
    for (segment, open) in pending.iter().enumerate() {
        if *open {
            segments[segment].kind = FlowKind::Unknown;
            segments[segment].source = FlowSource::Unknown;
        }
    }
    Ok(Classification {
        segments,
        commit_segment: segment_of,
    })
}

/// Type of a deleted branch from the types of the commits that merge it
/// and of its fork point.
fn by_topology(targets: &[FlowKind], origin: Option<FlowKind>) -> FlowKind {
    let into = |kind: FlowKind| targets.contains(&kind);
    if into(FlowKind::Production) {
        match origin {
            Some(FlowKind::Production) => FlowKind::Hotfix,
            _ => FlowKind::Release,
        }
    } else if into(FlowKind::Development) && origin == Some(FlowKind::Production) {
        FlowKind::Hotfix
    } else {
        FlowKind::Feature
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::history;

    fn names() -> FlowNames<'static> {
        FlowNames {
            production: "main",
            development: "develop",
            feature: "feature/",
            release: "release/",
            hotfix: "hotfix/",
        }
    }

    fn tip(name: &str, commit: u32, remote: bool) -> FlowTip {
        FlowTip {
            name: name.into(),
            commit: Id(commit),
            remote,
        }
    }

    /// "id kind/name/source" per commit, oldest first.
    fn render(spec: &[(&[u32], i64)], subjects: &[(u32, &str)], tips: &[FlowTip]) -> Vec<String> {
        let commits = history(spec);
        let mut by_id = vec![None; commits.len()];
        for (id, subject) in subjects {
            by_id[*id as usize] = Some((*subject).to_owned());
        }
        let result = classify(&FlowInput {
            commits: &commits,
            subjects: &by_id,
            tips,
            names: names(),
        })
        .unwrap();
        (0..commits.len())
            .map(|i| {
                let s = result.segment_of(Id(i as u32)).unwrap();
                let source = match s.source {
                    FlowSource::LiveRef => "ref",
                    FlowSource::MergeMessage => "msg",
                    FlowSource::Topology => "topo",
                    FlowSource::Unknown => "?",
                };
                format!(
                    "{i} {}/{}/{source}",
                    s.kind.label(),
                    s.name.as_deref().unwrap_or("-")
                )
            })
            .collect()
    }

    #[test]
    fn merge_subjects() {
        let parse = |s: &str| parse_merge_subject(s).map(|m| (m.name, m.into));
        assert_eq!(
            parse("Merge branch 'feature/a' into develop"),
            Some(("feature/a".into(), Some("develop".into())))
        );
        assert_eq!(
            parse("Merge branch 'release/2.0'"),
            Some(("release/2.0".into(), None))
        );
        assert_eq!(
            parse("Merge branch 'develop' of github.com:o/r into develop"),
            Some(("develop".into(), Some("develop".into())))
        );
        assert_eq!(
            parse("Merge remote-tracking branch 'origin/feature/x' into feature/y"),
            Some(("feature/x".into(), Some("feature/y".into())))
        );
        assert_eq!(
            parse("Merge pull request #42 from octo/feature/login\n\nAdd login"),
            Some(("feature/login".into(), None))
        );
        for no in [
            "Fix merge of branch 'x'",
            "Merge branch ''",
            "Merge branch 'unterminated",
            "Merge pull request #x from o/b",
            "Merge pull request #1 from nobranch",
            "Merge branches 'a' and 'b'",
            "",
        ] {
            assert_eq!(parse(no), None, "{no}");
        }
    }

    /// The classic diagram, all topic branches deleted:
    /// 0 v1.0 on main; develop forks (1); feature/a (2, 3) and feature/b
    /// (4) merged into develop (5, 6); release/2.0 (7) from develop, merged
    /// into main (8, v2.0) and develop (9); hotfix/2.0.1 (10) from main,
    /// merged into main (11, v2.1) and develop (12).
    type Spec = Vec<(&'static [u32], i64)>;
    type Subjects = Vec<(u32, &'static str)>;

    fn classic() -> (Spec, Subjects) {
        (
            vec![
                (&[], 0),       // 0 main: initial (v1.0)
                (&[0], 1),      // 1 develop
                (&[1], 2),      // 2 feature/a
                (&[2], 3),      // 3 feature/a
                (&[1], 4),      // 4 feature/b
                (&[1, 3], 5),   // 5 develop: merge feature/a
                (&[5, 4], 6),   // 6 develop: merge feature/b
                (&[6], 7),      // 7 release/2.0
                (&[0, 7], 8),   // 8 main: merge release/2.0 (v2.0)
                (&[6, 7], 9),   // 9 develop: merge release/2.0
                (&[8], 10),     // 10 hotfix/2.0.1
                (&[8, 10], 11), // 11 main: merge hotfix (v2.1)
                (&[9, 10], 12), // 12 develop: merge hotfix
            ],
            vec![
                (5, "Merge branch 'feature/a' into develop"),
                (6, "Merge branch 'feature/b' into develop"),
                (8, "Merge branch 'release/2.0'"),
                (9, "Merge branch 'release/2.0' into develop"),
                (11, "Merge branch 'hotfix/2.0.1'"),
                (12, "Merge branch 'hotfix/2.0.1' into develop"),
            ],
        )
    }

    #[test]
    fn classic_history_by_merge_messages() {
        let (spec, subjects) = classic();
        let tips = [tip("main", 11, false), tip("develop", 12, false)];
        assert_eq!(
            render(&spec, &subjects, &tips),
            [
                "0 production/main/ref",
                "1 development/develop/ref",
                "2 feature/feature/a/msg",
                "3 feature/feature/a/msg",
                "4 feature/feature/b/msg",
                "5 development/develop/ref",
                "6 development/develop/ref",
                "7 release/release/2.0/msg",
                "8 production/main/ref",
                "9 development/develop/ref",
                "10 hotfix/hotfix/2.0.1/msg",
                "11 production/main/ref",
                "12 development/develop/ref",
            ]
        );
    }

    #[test]
    fn classic_history_by_topology() {
        // Without merge messages (e.g. edited), the shape decides.
        let (spec, _) = classic();
        let tips = [tip("main", 11, false), tip("develop", 12, false)];
        assert_eq!(
            render(&spec, &[], &tips),
            [
                "0 production/main/ref",
                "1 development/develop/ref",
                "2 feature/-/topo",
                "3 feature/-/topo",
                "4 feature/-/topo",
                "5 development/develop/ref",
                "6 development/develop/ref",
                "7 release/-/topo",
                "8 production/main/ref",
                "9 development/develop/ref",
                "10 hotfix/-/topo",
                "11 production/main/ref",
                "12 development/develop/ref",
            ]
        );
    }

    #[test]
    fn live_branches_win_and_remote_twins_are_skipped() {
        // 0 main, 1 develop, 2-3 feature/x (live, also on origin), 4 a
        // branch without prefix, 5 a hotfix branched from develop by mistake.
        let spec: Vec<(&[u32], i64)> = vec![
            (&[], 0),
            (&[0], 1),
            (&[1], 2),
            (&[2], 3),
            (&[1], 4),
            (&[1], 5),
        ];
        let tips = [
            tip("main", 0, false),
            tip("develop", 1, false),
            tip("feature/x", 3, true),
            tip("feature/x", 3, false),
            tip("experiment", 4, false),
            tip("hotfix/oops", 5, false),
        ];
        assert_eq!(
            render(&spec, &[], &tips),
            [
                "0 production/main/ref",
                "1 development/develop/ref",
                "2 feature/feature/x/ref",
                "3 feature/feature/x/ref",
                "4 feature/experiment/ref",
                "5 hotfix/hotfix/oops/ref",
            ]
        );
    }

    #[test]
    fn production_wins_and_remote_main_counts() {
        // develop's first-parent chain runs into main's history: those
        // commits stay production. main only exists as origin/main.
        let spec: Vec<(&[u32], i64)> = vec![(&[], 0), (&[0], 1), (&[1], 2)];
        let tips = [tip("main", 1, true), tip("develop", 2, false)];
        assert_eq!(
            render(&spec, &[], &tips),
            [
                "0 production/main/ref",
                "1 production/main/ref",
                "2 development/develop/ref",
            ]
        );
    }

    #[test]
    fn pull_merges_nested_features_fast_forward_and_unknown() {
        // 0 main; 1 develop; 2 remote develop work merged by a pull (3);
        // 4 feature/outer, 5 nested feature merged into it (6), outer
        // merged into develop (7) without a message (topology); 8 a
        // fast-forwarded commit on develop; 9 a commit only a tag reaches.
        let spec: Vec<(&[u32], i64)> = vec![
            (&[], 0),
            (&[0], 1),
            (&[1], 2),
            (&[1, 2], 3),
            (&[3], 4),
            (&[4], 5),
            (&[4, 5], 6),
            (&[3, 6], 7),
            (&[7], 8),
            (&[0], 9),
        ];
        let subjects = [
            (3, "Merge branch 'develop' of github.com:o/r into develop"),
            (6, "Merge branch 'feature/inner' into feature/outer"),
        ];
        let tips = [tip("main", 0, false), tip("develop", 8, false)];
        assert_eq!(
            render(&spec, &subjects, &tips),
            [
                "0 production/main/ref",
                "1 development/develop/ref",
                "2 development/develop/msg",
                "3 development/develop/ref",
                "4 feature/-/topo",
                "5 feature/feature/inner/msg",
                "6 feature/-/topo",
                "7 development/develop/ref",
                "8 development/develop/ref",
                "9 unknown/-/?",
            ]
        );
    }

    #[test]
    fn hotfix_into_develop_only_and_missing_parents() {
        // A hotfix from main merged only into develop; the history is cut
        // off below commit 0 (missing parent 99).
        let spec: Vec<(&[u32], i64)> = vec![(&[99], 0), (&[0], 1), (&[0], 2), (&[1, 2], 3)];
        let tips = [tip("main", 0, false), tip("develop", 3, false)];
        assert_eq!(
            render(&spec, &[], &tips),
            [
                "0 production/main/ref",
                "1 development/develop/ref",
                "2 hotfix/-/topo",
                "3 development/develop/ref",
            ]
        );
    }
}
