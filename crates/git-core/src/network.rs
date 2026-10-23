//! Long-running (network) Git commands: progress parsing and cancellation
//! (ADR-020).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// One progress update parsed from Git's `--progress` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Phase, e.g. "Receiving objects" (a `remote: ` prefix is dropped).
    pub phase: String,
    /// Percentage, if the phase reports one (counting phases do not).
    pub percent: Option<u8>,
}

/// Parses one progress segment (text between `\r` / `\n`), e.g.
/// `Receiving objects:  45% (450/1000), 1.20 MiB | 1.00 MiB/s`.
/// Other output (ref updates, errors) yields `None`.
pub fn parse_progress(segment: &str) -> Option<Progress> {
    let line = segment.trim();
    let line = line.strip_prefix("remote: ").unwrap_or(line).trim();
    let (phase, rest) = line.split_once(':')?;
    let rest = rest.trim_start();
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    // Phases are words; "fatal: …" or "To host:path" have no number after the colon.
    if phase.is_empty() || digits == 0 || !phase.chars().all(|c| c.is_alphabetic() || c == ' ') {
        return None;
    }
    let percent = match rest[digits..].starts_with('%') {
        true => rest[..digits].parse::<u8>().ok(),
        false => None,
    };
    Some(Progress {
        phase: phase.trim().to_owned(),
        percent,
    })
}

/// Git's stderr without progress segments, for error messages.
pub(crate) fn strip_progress(stderr: &str) -> String {
    stderr
        .split(['\r', '\n'])
        .map(str::trim_end)
        .filter(|s| !s.is_empty() && parse_progress(s).is_none())
        .collect::<Vec<_>>()
        .join("\n")
}

/// How `git pull` integrates the upstream branch (toolbar Pull dropdown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PullMode {
    /// Fast-forward if possible, otherwise a merge commit (`--ff`).
    #[default]
    FastForwardIfPossible,
    /// Only fast-forward; fails if the branches diverged (`--ff-only`).
    FastForwardOnly,
    /// Rebase local commits onto the upstream branch (`--rebase`).
    Rebase,
}

impl PullMode {
    pub const ALL: [PullMode; 3] = [
        PullMode::FastForwardIfPossible,
        PullMode::FastForwardOnly,
        PullMode::Rebase,
    ];

    /// Menu label, as in GitKraken.
    pub fn label(self) -> &'static str {
        match self {
            Self::FastForwardIfPossible => "Pull (fast-forward if possible)",
            Self::FastForwardOnly => "Pull (fast-forward only)",
            Self::Rebase => "Pull (rebase)",
        }
    }

    /// `git` arguments with progress output.
    pub fn args(self) -> [&'static str; 3] {
        let mode = match self {
            Self::FastForwardIfPossible => "--ff",
            Self::FastForwardOnly => "--ff-only",
            Self::Rebase => "--rebase",
        };
        ["pull", mode, "--progress"]
    }
}

/// Shared flag that asks a running command to stop.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(phase: &str, percent: Option<u8>) -> Option<Progress> {
        Some(Progress {
            phase: phase.into(),
            percent,
        })
    }

    /// Recorded from `git fetch --progress` / `git push --progress`
    /// (Git 2.47; the format is unchanged since Git 2.34).
    #[test]
    fn parses_recorded_progress_segments() {
        let cases = [
            (
                "remote: Enumerating objects: 302, done.",
                progress("Enumerating objects", None),
            ),
            (
                "remote: Counting objects:  10% (31/302)        ",
                progress("Counting objects", Some(10)),
            ),
            (
                "remote: Compressing objects: 100% (302/302), done.",
                progress("Compressing objects", Some(100)),
            ),
            (
                "Receiving objects:   0% (1/302)",
                progress("Receiving objects", Some(0)),
            ),
            (
                "Receiving objects:  45% (450/1000), 1.20 MiB | 1.00 MiB/s",
                progress("Receiving objects", Some(45)),
            ),
            (
                "Resolving deltas: 100% (3/3), done.",
                progress("Resolving deltas", Some(100)),
            ),
            (
                "Writing objects:  33% (1/3)",
                progress("Writing objects", Some(33)),
            ),
            (
                "Updating files:  50% (1/2)",
                progress("Updating files", Some(50)),
            ),
            (
                "remote: Total 302 (delta 0), reused 0 (delta 0), pack-reused 0",
                None,
            ),
            ("From ../netbare", None),
            (" * [new branch]      main       -> origin/main", None),
            ("To git@github.com:a/b.git", None),
            ("fatal: could not read from remote repository.", None),
            ("Delta compression using up to 16 threads", None),
        ];
        for (segment, expected) in cases {
            assert_eq!(parse_progress(segment), expected, "{segment}");
        }
    }

    #[test]
    fn error_text_keeps_messages_and_drops_progress() {
        let stderr =
            "remote: Counting objects:  50% (1/2)\rremote: Counting objects: 100% (2/2), done.\n\
                      To ../bare.git\n ! [rejected]        main -> main (fetch first)\n\
                      error: failed to push some refs to '../bare.git'\n";
        assert_eq!(
            strip_progress(stderr),
            "To ../bare.git\n ! [rejected]        main -> main (fetch first)\n\
             error: failed to push some refs to '../bare.git'"
        );
    }

    #[test]
    fn cancel_token_is_shared() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }
}
