//! Plain-language explanations of Git errors (M12-02, ADR-049): Git's
//! stderr is matched against known patterns; the UI shows the explanation
//! and keeps Git's own output under "Details".

/// What an error means, in plain language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Explanation {
    /// Short name of the problem.
    pub title: &'static str,
    /// One sentence on what happened.
    pub explanation: &'static str,
    /// What to do about it.
    pub hint: &'static str,
}

impl Explanation {
    /// Explanation and hint as one text.
    pub fn text(&self) -> String {
        format!("{} {}", self.explanation, self.hint)
    }
}

/// Text for errors the catalog does not know.
pub const UNKNOWN: &str = "Git reported an error.";

/// (patterns, explanation): any of the lowercase patterns matches.
const CATALOG: &[(&[&str], Explanation)] = &[
    (
        &["index.lock': file exists", ".lock': file exists"],
        Explanation {
            title: "Repository locked",
            explanation: "Another Git process seems to be running in this repository.",
            hint: "Wait for it to finish (or close the other program), then try again.",
        },
    ),
    (
        &[
            "you need to resolve your current index first",
            "unmerged paths",
            "is unmerged",
            "because you have unmerged files",
        ],
        Explanation {
            title: "Unresolved conflicts",
            explanation: "There are files with unresolved conflicts.",
            hint: "Resolve the conflicts and stage the files, or abort the operation.",
        },
    ),
    (
        &[
            "your local changes to the following files would be overwritten",
            "untracked working tree files would be overwritten",
            "would be overwritten by merge",
            "would be overwritten by checkout",
        ],
        Explanation {
            title: "Local changes in the way",
            explanation: "Your changes to some files would be overwritten.",
            hint: "Commit or stash them first.",
        },
    ),
    (
        &[
            "non-fast-forward",
            "[rejected]",
            "fetch first",
            "updates were rejected",
        ],
        Explanation {
            title: "Push rejected",
            explanation: "The remote branch has commits that you do not have.",
            hint: "Pull first, then push again.",
        },
    ),
    (
        &[
            "authentication failed",
            "could not read username",
            "could not read password",
            "invalid username or password",
            "permission denied (publickey",
        ],
        Explanation {
            title: "Authentication failed",
            explanation: "The remote did not accept your credentials or SSH key.",
            hint: "Check your credentials, credential helper or SSH key.",
        },
    ),
    (
        &[
            "could not resolve host",
            "could not resolve hostname",
            "network is unreachable",
            "connection timed out",
            "connection refused",
            "unable to access",
        ],
        Explanation {
            title: "Remote not reachable",
            explanation: "The remote could not be reached.",
            hint: "Check your network connection and the remote URL.",
        },
    ),
    (
        &["nothing to commit", "no changes added to commit"],
        Explanation {
            title: "Nothing to commit",
            explanation: "There are no staged changes.",
            hint: "Stage the changes you want to commit first.",
        },
    ),
    (
        &["not a git repository"],
        Explanation {
            title: "Not a Git repository",
            explanation: "This folder is not a Git repository (or it was removed).",
            hint: "Open another folder or initialize a repository.",
        },
    ),
    (
        &[
            "gpg failed to sign",
            "error: gpg failed",
            "failed to write commit object",
            "signing failed",
        ],
        Explanation {
            title: "Signing failed",
            explanation: "Git could not sign.",
            hint: "Check Preferences → Signing (key, GPG program, agent).",
        },
    ),
    (
        &["detected dubious ownership"],
        Explanation {
            title: "Repository not trusted",
            explanation: "Git refuses to work in this folder because it belongs to another user.",
            hint: "Trust the repository if you know the folder (Trust this repository…).",
        },
    ),
    (
        &[
            "you are not currently on a branch",
            "head detached",
            "not on a branch",
        ],
        Explanation {
            title: "No branch checked out",
            explanation: "This needs a checked-out branch, but HEAD is detached.",
            hint: "Check out a branch first.",
        },
    ),
];

/// The explanation of Git's error output `stderr`, if the catalog knows it.
pub fn explain(stderr: &str) -> Option<Explanation> {
    let lower = stderr.to_lowercase();
    CATALOG
        .iter()
        .find(|(patterns, _)| patterns.iter().any(|p| lower.contains(p)))
        .map(|(_, explanation)| *explanation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_sample_per_entry() {
        let samples = [
            ("fatal: Unable to create '/r/.git/index.lock': File exists.", "Repository locked"),
            ("error: you need to resolve your current index first", "Unresolved conflicts"),
            (
                "error: Your local changes to the following files would be overwritten by checkout:\n\ta.txt",
                "Local changes in the way",
            ),
            (
                " ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs",
                "Push rejected",
            ),
            ("fatal: Authentication failed for 'https://example.org/r.git/'", "Authentication failed"),
            ("git@example.org: Permission denied (publickey).", "Authentication failed"),
            (
                "fatal: unable to access 'https://example.org/': Could not resolve host: example.org",
                "Remote not reachable",
            ),
            ("ssh: Could not resolve hostname example.org", "Remote not reachable"),
            ("nothing to commit, working tree clean", "Nothing to commit"),
            ("fatal: not a git repository (or any of the parent directories): .git", "Not a Git repository"),
            ("error: gpg failed to sign the data", "Signing failed"),
            ("fatal: You are not currently on a branch.", "No branch checked out"),
            ("fatal: detected dubious ownership in repository at '/r'", "Repository not trusted"),
        ];
        for (stderr, title) in samples {
            assert_eq!(explain(stderr).map(|e| e.title), Some(title), "{stderr}");
        }
        assert_eq!(
            explain("nothing to commit").unwrap().text(),
            "There are no staged changes. Stage the changes you want to commit first."
        );
    }

    #[test]
    fn unknown_errors_fall_back() {
        assert_eq!(explain("fatal: something unusual happened"), None);
        assert_eq!(explain(""), None);
        assert_eq!(UNKNOWN, "Git reported an error.");
    }
}
