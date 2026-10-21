//! Topological ordering equivalent to `git log --date-order`.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::{check_ids, Error, GraphCommit, Id};

/// Result of [`topo_order`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopoOrder {
    /// Every input commit exactly once; children before parents, newest first otherwise.
    pub order: Vec<Id>,
    /// `(child, missing parent)` for every parent not contained in the input,
    /// in emission order of the child.
    pub open_ends: Vec<(Id, Id)>,
}

/// Orders commits with Kahn's algorithm.
///
/// Among commits whose children are all emitted, the one with the newest
/// commit time comes first; equal times keep input order. Clock skew cannot
/// break topology because a parent is only ready after all its children.
pub fn topo_order(commits: &[GraphCommit]) -> Result<TopoOrder, Error> {
    check_ids(commits)?;
    let count = commits.len();

    // Number of not yet emitted children per commit (duplicate parent entries count twice).
    let mut pending = vec![0u32; count];
    for parent in commits.iter().flat_map(|c| &c.parents) {
        if let Some(n) = pending.get_mut(parent.index()) {
            *n += 1;
        }
    }

    // Max-heap by time; `Reverse(id)` makes the earlier input position win ties.
    let mut ready: BinaryHeap<(i64, Reverse<Id>)> = commits
        .iter()
        .filter(|c| pending[c.id.index()] == 0)
        .map(|c| (c.time, Reverse(c.id)))
        .collect();

    let mut result = TopoOrder {
        order: Vec::with_capacity(count),
        open_ends: Vec::new(),
    };
    while let Some((_, Reverse(id))) = ready.pop() {
        let commit = &commits[id.index()];
        result.order.push(id);
        for &parent in &commit.parents {
            match pending.get_mut(parent.index()) {
                Some(n) => {
                    *n -= 1;
                    if *n == 0 {
                        ready.push((commits[parent.index()].time, Reverse(parent)));
                    }
                }
                None => result.open_ends.push((id, parent)),
            }
        }
    }

    // Commits on (or below) a cycle never become ready. Git histories cannot
    // contain cycles, but corrupt input must not lose commits: append them
    // newest first.
    if result.order.len() < count {
        let mut rest: Vec<&GraphCommit> = commits
            .iter()
            .filter(|c| pending[c.id.index()] > 0)
            .collect();
        rest.sort_by_key(|c| (Reverse(c.time), c.id));
        for commit in rest {
            result.order.push(commit.id);
            result.open_ends.extend(
                commit
                    .parents
                    .iter()
                    .filter(|p| p.index() >= count)
                    .map(|&p| (commit.id, p)),
            );
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::history;

    /// Orders and checks the invariants every result must satisfy.
    fn order(commits: &[GraphCommit]) -> TopoOrder {
        let result = topo_order(commits).unwrap();
        let mut position = vec![usize::MAX; commits.len()];
        for (row, id) in result.order.iter().enumerate() {
            assert_eq!(position[id.index()], usize::MAX, "{id:?} emitted twice");
            position[id.index()] = row;
        }
        assert!(position.iter().all(|&p| p != usize::MAX), "commit missing");
        for commit in commits {
            for parent in commit.parents.iter().filter(|p| p.index() < commits.len()) {
                assert!(
                    position[commit.id.index()] < position[parent.index()],
                    "{:?} not before its parent {parent:?}",
                    commit.id
                );
            }
        }
        result
    }

    fn ids(result: &TopoOrder) -> Vec<u32> {
        result.order.iter().map(|id| id.0).collect()
    }

    #[test]
    fn empty_input() {
        assert_eq!(order(&[]), TopoOrder::default());
    }

    #[test]
    fn linear() {
        let commits = history(&[(&[1], 3), (&[2], 2), (&[], 1)]);
        assert_eq!(ids(&order(&commits)), [0, 1, 2]);
    }

    #[test]
    fn linear_in_reverse_input_order() {
        let commits = history(&[(&[], 1), (&[0], 2), (&[1], 3)]);
        assert_eq!(ids(&order(&commits)), [2, 1, 0]);
    }

    #[test]
    fn fork() {
        // 1 and 2 both branch off 0.
        let commits = history(&[(&[], 1), (&[0], 2), (&[0], 3)]);
        assert_eq!(ids(&order(&commits)), [2, 1, 0]);
    }

    #[test]
    fn merge() {
        // 3 merges 2 into 1.
        let commits = history(&[(&[], 1), (&[0], 2), (&[0], 3), (&[1, 2], 4)]);
        assert_eq!(ids(&order(&commits)), [3, 2, 1, 0]);
    }

    #[test]
    fn octopus_merge() {
        let commits = history(&[(&[], 1), (&[0], 2), (&[0], 4), (&[0], 3), (&[1, 2, 3], 5)]);
        assert_eq!(ids(&order(&commits)), [4, 2, 3, 1, 0]);
    }

    #[test]
    fn criss_cross_merge() {
        // 3 = merge(1, 2), 4 = merge(2, 1).
        let commits = history(&[(&[], 1), (&[0], 2), (&[0], 3), (&[1, 2], 4), (&[2, 1], 5)]);
        assert_eq!(ids(&order(&commits)), [4, 3, 2, 1, 0]);
    }

    #[test]
    fn clock_skew_keeps_parent_after_child() {
        // Parent 0 is newer than its child 1 and than the unrelated tip 2.
        let commits = history(&[(&[], 100), (&[0], 50), (&[], 75)]);
        assert_eq!(ids(&order(&commits)), [2, 1, 0]);
    }

    #[test]
    fn multiple_roots_and_disconnected_histories() {
        // Chain 1 -> 0 and chain 3 -> 2, interleaved by time.
        let commits = history(&[(&[], 1), (&[0], 3), (&[], 2), (&[2], 4)]);
        assert_eq!(ids(&order(&commits)), [3, 1, 2, 0]);
    }

    #[test]
    fn merge_of_two_root_commits() {
        let commits = history(&[(&[], 1), (&[], 2), (&[0, 1], 3)]);
        assert_eq!(ids(&order(&commits)), [2, 1, 0]);
    }

    #[test]
    fn missing_parents_are_open_ends() {
        // 0 has a missing parent 5; merge 1 has a missing second parent 7.
        let commits = history(&[(&[5], 2), (&[0, 7], 3)]);
        let result = order(&commits);
        assert_eq!(ids(&result), [1, 0]);
        assert_eq!(result.open_ends, [(Id(1), Id(7)), (Id(0), Id(5))]);
    }

    #[test]
    fn equal_times_keep_input_order() {
        let commits = history(&[(&[], 5), (&[], 5), (&[], 5)]);
        assert_eq!(ids(&order(&commits)), [0, 1, 2]);

        // Tips of a fork with equal times.
        let commits = history(&[(&[2], 5), (&[2], 5), (&[], 1)]);
        assert_eq!(ids(&order(&commits)), [0, 1, 2]);
    }

    #[test]
    fn duplicate_parent_entries() {
        let commits = history(&[(&[], 1), (&[0, 0], 2)]);
        assert_eq!(ids(&order(&commits)), [1, 0]);
    }

    #[test]
    fn cycles_do_not_lose_commits() {
        // 0 <-> 1 form a cycle, 3 is a self-loop with a missing parent 9.
        let commits = history(&[(&[1], 1), (&[0], 2), (&[], 3), (&[3, 9], 4)]);
        let result = topo_order(&commits).unwrap();
        assert_eq!(ids(&result), [2, 3, 1, 0]);
        assert_eq!(result.open_ends, [(Id(3), Id(9))]);
    }

    #[test]
    fn rejects_id_not_matching_position() {
        let mut commits = history(&[(&[], 1), (&[0], 2)]);
        commits[1].id = Id(0);
        assert_eq!(
            topo_order(&commits),
            Err(Error::IdMismatch {
                position: 1,
                id: Id(0)
            })
        );
    }
}
