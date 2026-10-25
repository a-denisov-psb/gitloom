//! Fetch and auto-fetch (ADR-020).

use std::time::{Duration, Instant};

use crate::repo_view::{NetworkKind, NetworkRequest};

/// Default auto-fetch interval in minutes (planning decision M3).
pub const DEFAULT_INTERVAL_MINUTES: u32 = 5;

/// Delay before checking again when a due auto-fetch had to be skipped
/// (busy tab, merge in progress).
pub const RETRY: Duration = Duration::from_secs(60);

/// `git fetch --all` (pruning follows the user's `fetch.prune` setting).
pub fn fetch_all(background: bool) -> NetworkRequest {
    NetworkRequest {
        label: if background { "Auto-fetch" } else { "Fetch" }.into(),
        kind: NetworkKind::Fetch,
        args: vec!["fetch".into(), "--all".into(), "--progress".into()],
        background,
    }
}

/// `git pull` in `mode`.
pub fn pull(mode: git_core::PullMode) -> NetworkRequest {
    NetworkRequest {
        label: "Pull".into(),
        kind: NetworkKind::Pull,
        args: mode.args().iter().map(|s| s.to_string()).collect(),
        background: false,
    }
}

/// What the scheduler needs to know about one tab.
#[derive(Debug, Clone, Copy)]
pub struct TabState {
    /// Last fetch (or when the tab was opened).
    pub last_fetch: Instant,
    /// Another operation is running.
    pub busy: bool,
    /// A merge, rebase, cherry-pick or revert is in progress.
    pub operation_in_progress: bool,
    pub has_remotes: bool,
}

/// Whether the tab should auto-fetch now; `interval_minutes == 0` disables it.
pub fn auto_fetch_due(now: Instant, interval_minutes: u32, tab: TabState) -> bool {
    interval_minutes > 0
        && tab.has_remotes
        && !tab.busy
        && !tab.operation_in_progress
        && now.duration_since(tab.last_fetch)
            >= Duration::from_secs(u64::from(interval_minutes) * 60)
}

/// When the next auto-fetch check is needed: the earliest due time of the
/// tabs, or `now + RETRY` if one is overdue (it was skipped). `None` if
/// auto-fetch is off or no tab can fetch. A single timer replaces a
/// periodic tick: every message redraws the window, which costs CPU
/// with software rendering.
pub fn next_check(now: Instant, interval_minutes: u32, tabs: &[TabState]) -> Option<Instant> {
    if interval_minutes == 0 {
        return None;
    }
    let interval = Duration::from_secs(u64::from(interval_minutes) * 60);
    let due = tabs
        .iter()
        .filter(|t| t.has_remotes)
        .map(|t| t.last_fetch + interval)
        .min()?;
    Some(if due <= now { now + RETRY } else { due })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(minutes_ago: u64) -> (Instant, TabState) {
        let start = Instant::now();
        let now = start + Duration::from_secs(minutes_ago * 60);
        (
            now,
            TabState {
                last_fetch: start,
                busy: false,
                operation_in_progress: false,
                has_remotes: true,
            },
        )
    }

    #[test]
    fn due_after_the_interval() {
        let (now, state) = tab(4);
        assert!(!auto_fetch_due(now, 5, state));
        let (now, state) = tab(5);
        assert!(auto_fetch_due(now, 5, state));
    }

    #[test]
    fn skip_rules() {
        let (now, state) = tab(10);
        assert!(!auto_fetch_due(now, 0, state), "0 = off");
        let busy = TabState {
            busy: true,
            ..state
        };
        assert!(!auto_fetch_due(now, 5, busy), "another operation runs");
        let merging = TabState {
            operation_in_progress: true,
            ..state
        };
        assert!(!auto_fetch_due(now, 5, merging), "merge/rebase in progress");
        let local = TabState {
            has_remotes: false,
            ..state
        };
        assert!(!auto_fetch_due(now, 5, local), "nothing to fetch from");
    }

    #[test]
    fn next_check_is_the_earliest_due_time() {
        let (now, state) = tab(2);
        let older = TabState {
            last_fetch: state.last_fetch - Duration::from_secs(60),
            ..state
        };
        // Fetched 2 and 3 minutes ago, every 5 minutes: due in 2 minutes.
        assert_eq!(
            next_check(now, 5, &[state, older]),
            Some(older.last_fetch + Duration::from_secs(300))
        );
        assert_eq!(next_check(now, 0, &[state]), None, "off");
        let local = TabState {
            has_remotes: false,
            ..state
        };
        assert_eq!(next_check(now, 5, &[local]), None, "nothing to fetch");
        // Overdue (skipped while busy): retry later instead of spinning.
        let (late, state) = tab(10);
        assert_eq!(next_check(late, 5, &[state]), Some(late + RETRY));
    }

    #[test]
    fn fetch_request_is_background_only_for_auto_fetch() {
        assert!(fetch_all(true).background);
        assert_eq!(fetch_all(false).label, "Fetch");
        assert_eq!(fetch_all(false).args, ["fetch", "--all", "--progress"]);
    }
}
