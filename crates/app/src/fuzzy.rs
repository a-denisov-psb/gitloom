//! Fuzzy matching for the command palette (M5-06): the query's characters
//! must appear in order in the candidate; matches at word starts, runs of
//! consecutive characters and exact case score higher. Pure, no allocation
//! per candidate beyond the query.

/// Points per matched character.
const MATCH: i32 = 1;
/// Matched character starts a word (after a separator, or camelCase hump).
const WORD_START: i32 = 8;
/// Matched character directly follows the previous match.
const CONSECUTIVE: i32 = 5;
/// Matched character has the same case as in the query.
const SAME_CASE: i32 = 1;
/// First character of the candidate matched.
const FIRST: i32 = 4;
/// Per character skipped between the first match and the end of the match.
const GAP: i32 = 1;

/// Query prepared once for many candidates.
pub struct Query {
    chars: Vec<char>,
    lower: Vec<char>,
}

impl Query {
    pub fn new(query: &str) -> Self {
        let chars: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).collect();
        let lower = chars.iter().map(|c| lower(*c)).collect();
        Self { chars, lower }
    }

    /// Score of `candidate`, or `None` when the query is not a subsequence.
    /// Greedy from the left, but a later word-start occurrence is preferred
    /// over a mid-word one for each query character.
    pub fn score(&self, candidate: &str) -> Option<i32> {
        if self.chars.is_empty() {
            return Some(0);
        }
        let mut score = 0;
        let mut query = 0;
        let mut previous: Option<char> = None;
        let mut last_match: Option<usize> = None;
        let mut first_match: Option<usize> = None;
        let mut chars = candidate.chars().enumerate().peekable();
        while let Some((index, c)) = chars.next() {
            let Some(&wanted) = self.lower.get(query) else {
                break;
            };
            let word_start = match previous {
                None => true,
                Some(p) => is_separator(p) || (p.is_lowercase() && c.is_uppercase()),
            };
            previous = Some(c);
            if lower(c) != wanted {
                continue;
            }
            // Mid-word match while the same character starts a word later
            // in the candidate: skip it, unless it continues a run.
            let continues = last_match.is_some_and(|last| last + 1 == index);
            if !word_start && !continues && later_word_start(chars.clone(), c, wanted) {
                continue;
            }
            score += MATCH;
            if word_start {
                score += WORD_START;
            }
            if continues {
                score += CONSECUTIVE;
            }
            if c == self.chars[query] {
                score += SAME_CASE;
            }
            if index == 0 {
                score += FIRST;
            }
            first_match.get_or_insert(index);
            last_match = Some(index);
            query += 1;
        }
        if query < self.lower.len() {
            return None;
        }
        let span = last_match.unwrap_or(0) - first_match.unwrap_or(0) + 1;
        Some(score - GAP * (span - self.chars.len()) as i32)
    }
}

/// True if `wanted` occurs at a word start in the rest of the candidate.
fn later_word_start(
    rest: std::iter::Peekable<std::iter::Enumerate<std::str::Chars<'_>>>,
    current: char,
    wanted: char,
) -> bool {
    let mut previous = current;
    for (_, c) in rest {
        let word_start = is_separator(previous) || (previous.is_lowercase() && c.is_uppercase());
        if word_start && lower(c) == wanted {
            return true;
        }
        previous = c;
    }
    false
}

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn is_separator(c: char) -> bool {
    matches!(c, ' ' | '/' | '\\' | '-' | '_' | '.' | ':' | '(' | '[')
}

/// Indices of `candidates` matching `query`, best first; ties keep the
/// candidates' order.
pub fn rank<'a>(query: &str, candidates: impl IntoIterator<Item = &'a str>) -> Vec<usize> {
    let query = Query::new(query);
    let mut scored: Vec<(i32, usize)> = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| query.score(candidate).map(|score| (score, index)))
        .collect();
    // Stable: equal scores stay in candidate order.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranked<'a>(query: &str, candidates: &[&'a str]) -> Vec<&'a str> {
        rank(query, candidates.iter().copied())
            .into_iter()
            .map(|i| candidates[i])
            .collect()
    }

    #[test]
    fn subsequence_required() {
        let q = Query::new("fch");
        assert!(q.score("Fetch all").is_some());
        assert!(q.score("Fetch").is_some());
        assert!(q.score("Cherry-pick").is_none());
        assert!(Query::new("xyz").score("Fetch all").is_none());
        assert_eq!(Query::new("").score("anything"), Some(0));
        // Whitespace in the query is ignored.
        assert!(Query::new("fe al").score("Fetch all").is_some());
    }

    #[test]
    fn word_starts_win() {
        // "ca" as initials of "Create … branch"? No: "Close all" starts both words.
        assert_eq!(
            ranked("ca", &["Reload cache", "Close all"]),
            ["Close all", "Reload cache"]
        );
        // Initials of path segments beat scattered letters.
        assert_eq!(
            ranked("ar", &["src/library.rs", "crates/app/README.md"]),
            ["crates/app/README.md", "src/library.rs"]
        );
        // camelCase humps count as word starts.
        assert_eq!(
            ranked("fh", &["fetchHelper", "fishing"]),
            ["fetchHelper", "fishing"]
        );
    }

    #[test]
    fn consecutive_characters_win() {
        assert_eq!(
            ranked("fetch", &["Find external tool cache", "Fetch all"]),
            ["Fetch all", "Find external tool cache"]
        );
        assert_eq!(
            ranked(
                "app.rs",
                &["crates/app/src/actions.rs", "crates/app/src/app.rs"]
            ),
            ["crates/app/src/app.rs", "crates/app/src/actions.rs"]
        );
    }

    #[test]
    fn case_breaks_ties() {
        assert_eq!(ranked("R", &["reset", "Reset"]), ["Reset", "reset"]);
    }

    #[test]
    fn ties_keep_candidate_order() {
        assert_eq!(ranked("a", &["xa", "ya", "za"]), ["xa", "ya", "za"]);
        assert_eq!(ranked("", &["b", "a", "c"]), ["b", "a", "c"]);
    }

    #[test]
    fn later_word_start_is_preferred_over_mid_word() {
        // "b" inside "Stash" would be skipped for "Branch".
        let q = Query::new("b");
        let mid = q.score("abc").unwrap();
        let start = q.score("a b").unwrap();
        assert!(start > mid);
        // Both are initials; the shorter gap wins.
        assert_eq!(
            ranked("cb", &["Checkout branch", "Create branch"]),
            ["Create branch", "Checkout branch"]
        );
    }

    #[test]
    fn unicode_does_not_panic() {
        assert!(Query::new("ü").score("Grüße/Übersicht").is_some());
        assert!(Query::new("ß").score("straße").is_some());
    }
}
