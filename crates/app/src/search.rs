//! Commit search (M5-07): which loaded commits match a query. Pure; the app
//! runs it off the UI thread.

use git_core::Commit;

/// A query of at least this many hex digits also matches SHA prefixes.
pub const MIN_SHA_PREFIX: usize = 4;

/// Indices into `commits` (ascending, i.e. display order) of the commits
/// whose summary, message body, author name or email contains `query`
/// (case-insensitive), or whose id starts with it (≥ 4 hex digits).
pub fn matches(commits: &[Commit], query: &str) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let needle = Needle::new(query);
    let sha = (query.len() >= MIN_SHA_PREFIX && query.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| query.to_ascii_lowercase());
    commits
        .iter()
        .enumerate()
        .filter(|(_, commit)| {
            sha.as_deref().is_some_and(|sha| commit.id.starts_with(sha))
                || needle.found_in(&commit.summary)
                || needle.found_in(&commit.author)
                || needle.found_in(&commit.email)
                || needle.found_in(&commit.body)
        })
        .map(|(index, _)| index)
        .collect()
}

/// Case-insensitive substring search without allocating per haystack for
/// ASCII queries (the common case).
struct Needle {
    ascii: Option<Vec<u8>>,
    lower: String,
}

impl Needle {
    fn new(query: &str) -> Self {
        let ascii = query
            .is_ascii()
            .then(|| query.bytes().map(|b| b.to_ascii_lowercase()).collect());
        Self {
            ascii,
            lower: query.to_lowercase(),
        }
    }

    fn found_in(&self, haystack: &str) -> bool {
        match &self.ascii {
            Some(needle) => contains_ignore_ascii_case(haystack.as_bytes(), needle),
            None => haystack.to_lowercase().contains(&self.lower),
        }
    }
}

/// `needle` is lower case ASCII.
fn contains_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    let Some((&first, rest)) = needle.split_first() else {
        return true;
    };
    if haystack.len() < needle.len() {
        return false;
    }
    let last_start = haystack.len() - needle.len();
    let upper = first.to_ascii_uppercase();
    let mut start = 0;
    while start <= last_start {
        // Find the next candidate start, then compare the rest.
        let Some(offset) = haystack[start..=last_start]
            .iter()
            .position(|&b| b == first || b == upper)
        else {
            return false;
        };
        let at = start + offset;
        if haystack[at + 1..at + needle.len()]
            .iter()
            .zip(rest)
            .all(|(h, n)| h.to_ascii_lowercase() == *n)
        {
            return true;
        }
        start = at + 1;
    }
    false
}

/// Search bar state of a tab.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchState {
    pub query: String,
    /// Commit indices of the last finished search, ascending.
    pub matches: Vec<usize>,
    /// Index into `matches` of the match jumped to last.
    pub current: Option<usize>,
    /// Number of the latest search request; older results are dropped.
    pub seq: u64,
    /// The pending results come from typing: jump to the first match (a
    /// re-run after a refresh keeps the selection).
    pub jump: bool,
}

impl SearchState {
    pub fn active(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// Commit `index` does not match the active query (its row is dimmed).
    pub fn dims(&self, index: usize) -> bool {
        self.active() && self.matches.binary_search(&index).is_err()
    }

    /// "3 of 12", "0 of 0", or nothing without a query.
    pub fn counter(&self) -> String {
        if !self.active() {
            return String::new();
        }
        let position = self.current.map_or(0, |i| i + 1);
        format!("{position} of {}", self.matches.len())
    }

    /// Takes new results; the match at commit `selected` (if any) becomes
    /// the current one, otherwise none is.
    pub fn set_matches(&mut self, matches: Vec<usize>, selected: Option<usize>) {
        self.current = selected.and_then(|s| matches.binary_search(&s).ok());
        self.matches = matches;
    }

    /// Moves to the next or previous match; returns its commit index.
    pub fn step(&mut self, forward: bool) -> Option<usize> {
        self.current = step(self.current, self.matches.len(), forward);
        self.current.map(|i| self.matches[i])
    }
}

/// Next match after `current` (or before it, `forward == false`), wrapping
/// around; `current` is an index into `matches`.
pub fn step(current: Option<usize>, count: usize, forward: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match (current, forward) {
        (None, true) => 0,
        (None, false) => count - 1,
        (Some(i), true) => (i + 1) % count,
        (Some(i), false) => (i + count - 1) % count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str, summary: &str, body: &str, author: &str, email: &str) -> Commit {
        Commit {
            id: id.into(),
            parents: Vec::new(),
            summary: summary.into(),
            body: body.into(),
            author: author.into(),
            email: email.into(),
            author_time: 0,
            commit_time: 0,
        }
    }

    fn sample() -> Vec<Commit> {
        vec![
            commit(
                "a1b2c3d4",
                "Fix login bug",
                "",
                "Ada Lovelace",
                "ada@example.com",
            ),
            commit(
                "b2c3d4e5",
                "Add search",
                "Closes #12.\nSearch is FAST.",
                "Bob",
                "bob@corp.io",
            ),
            commit("c3d4e5f6", "Grüße an Ünïcode", "", "Zoë", "zoe@example.com"),
            commit(
                "deadbeef",
                "Bump version",
                "",
                "Ada Lovelace",
                "ada@example.com",
            ),
        ]
    }

    #[test]
    fn summary_body_author_and_email_case_insensitive() {
        let commits = sample();
        assert_eq!(matches(&commits, "LOGIN"), [0]);
        assert_eq!(matches(&commits, "fast"), [1], "body");
        assert_eq!(matches(&commits, "lovelace"), [0, 3], "author");
        assert_eq!(matches(&commits, "corp.io"), [1], "email");
        assert_eq!(matches(&commits, "  bump  "), [3], "trimmed");
        assert!(matches(&commits, "").is_empty());
        assert!(matches(&commits, "nothing like this").is_empty());
    }

    #[test]
    fn sha_prefix_needs_four_hex_digits() {
        let commits = sample();
        assert_eq!(matches(&commits, "dead"), [3]);
        assert_eq!(matches(&commits, "DEADBE"), [3], "upper case hex");
        // Prefix of b2c3… only; a1b2c3… contains it but does not start with it.
        assert_eq!(matches(&commits, "b2c3"), [1]);
        // Three digits are too short for a SHA prefix: "a1b" matches nothing else.
        assert!(matches(&commits, "a1b").is_empty());
        // Only prefixes count, not SHA substrings.
        assert!(matches(&commits, "beef").is_empty());
    }

    #[test]
    fn non_ascii_queries() {
        let commits = sample();
        assert_eq!(matches(&commits, "grüsse").len(), 0);
        assert_eq!(matches(&commits, "GRÜßE"), [2]);
        assert_eq!(matches(&commits, "zoë"), [2]);
    }

    #[test]
    fn ascii_search_edges() {
        assert!(contains_ignore_ascii_case(b"Hello", b"hello"));
        assert!(contains_ignore_ascii_case(b"xxHeLLo", b"llo"));
        assert!(!contains_ignore_ascii_case(b"hel", b"hello"));
        assert!(
            contains_ignore_ascii_case(b"aaab", b"aab"),
            "restart after a partial match"
        );
        assert!(contains_ignore_ascii_case(b"", b""));
        // Multi-byte text around an ASCII needle.
        assert!(contains_ignore_ascii_case("Größe fix".as_bytes(), b"fix"));
    }

    #[test]
    fn state_counter_dimming_and_steps() {
        let mut state = SearchState::default();
        assert!(!state.dims(0), "no query, nothing dimmed");
        assert_eq!(state.counter(), "");
        state.query = "fix".into();
        state.set_matches(vec![2, 5, 9], None);
        assert_eq!(state.counter(), "0 of 3");
        assert!(state.dims(0) && !state.dims(5));
        assert_eq!(state.step(true), Some(2));
        assert_eq!(state.step(true), Some(5));
        assert_eq!(state.counter(), "2 of 3");
        assert_eq!(state.step(false), Some(2));
        assert_eq!(state.step(false), Some(9), "wraps to the last");
        // New results keep the selected commit as current if it still matches.
        state.set_matches(vec![1, 9], Some(9));
        assert_eq!(state.counter(), "2 of 2");
        state.set_matches(vec![1, 3], Some(9));
        assert_eq!(state.current, None);
        state.set_matches(Vec::new(), None);
        assert_eq!(state.step(true), None);
        assert_eq!(state.counter(), "0 of 0");
    }

    #[test]
    fn stepping_wraps_around() {
        assert_eq!(step(None, 3, true), Some(0));
        assert_eq!(step(None, 3, false), Some(2));
        assert_eq!(step(Some(2), 3, true), Some(0));
        assert_eq!(step(Some(0), 3, false), Some(2));
        assert_eq!(step(Some(1), 3, true), Some(2));
        assert_eq!(step(None, 0, true), None);
    }

    /// Acceptance M5-07: 100 000 commits < 50 ms (release).
    /// `cargo test --release -p gitloom search_speed -- --ignored --nocapture`
    #[test]
    #[ignore = "timing; run in release"]
    fn search_speed() {
        let words = [
            "fix", "add", "refactor", "update", "graph", "lane", "palette", "merge",
        ];
        let commits: Vec<Commit> = (0..100_000_usize)
            .map(|i| {
                commit(
                    &format!("{:040x}", i as u64 * 2_654_435_761),
                    &format!("{} the {} of module {i}", words[i % 8], words[(i / 8) % 8]),
                    &format!(
                        "Longer body text for commit {i}.\n\nIt explains the {} change in \
                         some detail so the body is a few hundred bytes long, like real \
                         commit messages tend to be.\n\nSigned-off-by: Dev {} <dev{}@example.com>",
                        words[(i / 3) % 8],
                        i % 50,
                        i % 50
                    ),
                    &format!("Developer {}", i % 50),
                    &format!("dev{}@example.com", i % 50),
                )
            })
            .collect();
        let mut worst = std::time::Duration::ZERO;
        for query in [
            "palette",
            "Module 99999",
            "dev7@",
            "zzz-no-match",
            "a1b2c3",
            "Größe",
        ] {
            let start = std::time::Instant::now();
            let found = matches(&commits, query).len();
            let elapsed = start.elapsed();
            eprintln!("{query:>14}: {elapsed:?}, {found} matches");
            worst = worst.max(elapsed);
        }
        assert!(worst.as_millis() < 50, "worst {worst:?}");
    }
}
