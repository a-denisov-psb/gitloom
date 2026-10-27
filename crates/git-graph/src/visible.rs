//! Reachability filter for hidden and soloed branches (M5-08): which
//! commits remain when only some branch tips are shown.

use crate::{GraphCommit, Id};

/// Marks every commit reachable from `tips` by following parents.
///
/// The result is indexed like `commits`. Tips and parents outside the input
/// (missing parents of a truncated history) are ignored.
pub fn visible_commits(commits: &[GraphCommit], tips: &[Id]) -> Vec<bool> {
    let mut visible = vec![false; commits.len()];
    let mut stack: Vec<Id> = tips.to_vec();
    while let Some(id) = stack.pop() {
        let index = id.index();
        let Some(seen) = visible.get_mut(index) else {
            continue;
        };
        if std::mem::replace(seen, true) {
            continue;
        }
        stack.extend(
            commits[index]
                .parents
                .iter()
                .filter(|p| visible.get(p.index()) == Some(&false)),
        );
    }
    visible
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::history;

    fn shown(visible: &[bool]) -> Vec<u32> {
        visible
            .iter()
            .enumerate()
            .filter(|(_, v)| **v)
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// ```text
    /// 0 main    3 topic
    /// |         |
    /// 1         4
    ///  \       /
    ///   2 base
    /// ```
    fn fork() -> Vec<GraphCommit> {
        history(&[(&[1], 50), (&[2], 40), (&[], 10), (&[4], 45), (&[2], 30)])
    }

    #[test]
    fn shared_history_stays_with_either_branch() {
        let commits = fork();
        assert_eq!(shown(&visible_commits(&commits, &[Id(0)])), [0, 1, 2]);
        assert_eq!(shown(&visible_commits(&commits, &[Id(3)])), [2, 3, 4]);
        assert_eq!(
            shown(&visible_commits(&commits, &[Id(0), Id(3)])),
            [0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn merges_bring_in_both_parents() {
        // 0 merges 1 (first parent) and 2; 3 is a branch on its own.
        let commits = history(&[(&[1, 2], 50), (&[4], 40), (&[4], 30), (&[], 60), (&[], 10)]);
        assert_eq!(shown(&visible_commits(&commits, &[Id(0)])), [0, 1, 2, 4]);
        // Hiding the merged branch's tip does not hide what the merge reaches.
        assert_eq!(
            shown(&visible_commits(&commits, &[Id(0), Id(2)])),
            [0, 1, 2, 4]
        );
    }

    #[test]
    fn nothing_visible_but_head() {
        let commits = fork();
        // Every branch hidden: only HEAD's history (here: topic) remains.
        assert_eq!(shown(&visible_commits(&commits, &[Id(3)])), [2, 3, 4]);
        assert_eq!(shown(&visible_commits(&commits, &[])), Vec::<u32>::new());
    }

    #[test]
    fn missing_parents_and_unknown_tips_are_ignored() {
        // 1 has a parent outside the input (truncated history).
        let commits = history(&[(&[1], 20), (&[7], 10)]);
        assert_eq!(shown(&visible_commits(&commits, &[Id(0), Id(9)])), [0, 1]);
    }

    #[test]
    fn long_linear_history_does_not_recurse() {
        let spec: Vec<(Vec<u32>, i64)> = (0..200_000u32)
            .map(|i| (if i + 1 < 200_000 { vec![i + 1] } else { vec![] }, 0))
            .collect();
        let spec: Vec<(&[u32], i64)> = spec.iter().map(|(p, t)| (p.as_slice(), *t)).collect();
        let commits = history(&spec);
        let visible = visible_commits(&commits, &[Id(0)]);
        assert!(visible.iter().all(|v| *v));
    }
}
