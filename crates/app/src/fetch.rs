//! Fetch and auto-fetch (ADR-020).

use std::time::{Duration, Instant};

use crate::repo_view::NetworkRequest;

/// Default auto-fetch interval in minutes (planning decision M3).
pub const DEFAULT_INTERVAL_MINUTES: u32 = 5;

/// How often open tabs are checked for a due auto-fetch.
pub const TICK: Duration = Duration::from_secs(30);

/// `git fetch --all` (pruning follows the user's `fetch.prune` setting).
pub fn fetch_all(background: bool) -> NetworkRequest {
    NetworkRequest {
        label: if background { "Auto-fetch" } else { "Fetch" }.into(),
        args: vec!["fetch".into(), "--all".into(), "--progress".into()],
        background,
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
    fn fetch_request_is_background_only_for_auto_fetch() {
        assert!(fetch_all(true).background);
        assert_eq!(fetch_all(false).label, "Fetch");
        assert_eq!(fetch_all(false).args, ["fetch", "--all", "--progress"]);
    }
}
