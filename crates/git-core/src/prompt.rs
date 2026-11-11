//! "Copy commit message prompt" (M10-07): a text the user pastes into a
//! tool of their choice to get a commit message for the staged changes.
//! No network, no AI inside Gitloom.
//!
//! [`gather`] is blocking (runs `git`); [`build`] is pure.

use crate::{GitWriter, Result, RunOptions};

/// Default instruction (Preferences → Templates, "Reset to default").
pub const DEFAULT_INSTRUCTION: &str = "Write a Git commit message for the staged changes below. \
Follow the commit template if one is given and the style of the recent commits. Summary: \
imperative mood, at most 72 characters. Then a blank line and a short description of what \
changed and why. Reply with the commit message only.";

/// Characters of diff text the prompt carries at most.
pub const MAX_DIFF_CHARS: usize = 60_000;

/// Staged files larger than this are listed without their diff.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;

/// One staged file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptFile {
    /// Git's status letter (A, M, D, T, …).
    pub status: String,
    pub path: String,
    /// Added and removed lines; `None` for binary files.
    pub counts: Option<(u64, u64)>,
    /// The file's staged diff, or why it is left out.
    pub diff: std::result::Result<String, Omitted>,
}

impl PromptFile {
    pub fn new(
        status: &str,
        path: &str,
        counts: Option<(u64, u64)>,
        diff: std::result::Result<String, Omitted>,
    ) -> Self {
        Self {
            status: status.to_owned(),
            path: path.to_owned(),
            counts,
            diff,
        }
    }
}

/// Why a file's diff is not in the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Omitted {
    Binary,
    TooLarge,
    /// It did not fit into [`MAX_DIFF_CHARS`] any more.
    NoRoom,
}

impl Omitted {
    fn reason(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::TooLarge => "larger than 256 KB",
            Self::NoRoom => "size limit reached",
        }
    }
}

/// Everything the prompt is built from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptInput {
    /// Checked-out branch, or "detached HEAD".
    pub branch: String,
    /// Summaries of the last commits of HEAD, newest first.
    pub recent: Vec<String>,
    pub files: Vec<PromptFile>,
}

/// The finished prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub text: String,
    /// Notes for the dialog, e.g. "2 diffs omitted (size limit reached)".
    pub notes: Vec<String>,
}

/// Builds the prompt: instruction; commit template; branch; recent commits;
/// files with +/- counts; the staged diffs within [`MAX_DIFF_CHARS`].
pub fn build(instruction: &str, template: Option<&str>, input: &PromptInput) -> Prompt {
    let mut text = String::new();
    text.push_str(instruction.trim());
    text.push_str("\n\n");
    if let Some(template) = template.map(str::trim).filter(|t| !t.is_empty()) {
        text.push_str("Commit template:\n");
        text.push_str(template);
        text.push_str("\n\n");
    }
    text.push_str(&format!("Branch: {}\n\n", input.branch));
    if !input.recent.is_empty() {
        text.push_str("Recent commits:\n");
        for summary in &input.recent {
            text.push_str(&format!("- {summary}\n"));
        }
        text.push('\n');
    }
    text.push_str("Files:\n");
    for file in &input.files {
        let counts = match file.counts {
            Some((added, removed)) => format!(" (+{added} -{removed})"),
            None => " (binary)".to_owned(),
        };
        text.push_str(&format!("{} {}{counts}\n", file.status, file.path));
    }
    text.push_str("\nStaged diff:\n");
    let mut used = 0usize;
    let mut omitted: Vec<(String, Omitted)> = Vec::new();
    for file in &input.files {
        let diff = match &file.diff {
            Ok(diff) if used + diff.chars().count() <= MAX_DIFF_CHARS => diff,
            Ok(_) => {
                omitted.push((file.path.clone(), Omitted::NoRoom));
                text.push_str(&format!("{}: (diff omitted)\n", file.path));
                continue;
            }
            Err(reason) => {
                omitted.push((file.path.clone(), *reason));
                text.push_str(&format!("{}: (diff omitted)\n", file.path));
                continue;
            }
        };
        used += diff.chars().count();
        text.push_str(diff);
        if !diff.ends_with('\n') {
            text.push('\n');
        }
    }
    let mut notes = Vec::new();
    for reason in [Omitted::Binary, Omitted::TooLarge, Omitted::NoRoom] {
        let count = omitted.iter().filter(|(_, r)| *r == reason).count();
        if count > 0 {
            let what = if count == 1 { "diff" } else { "diffs" };
            notes.push(format!("{count} {what} omitted ({})", reason.reason()));
        }
    }
    Prompt { text, notes }
}

fn read() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

fn nul_fields(stdout: &[u8]) -> Vec<String> {
    stdout
        .split(|&b| b == 0)
        .filter(|f| !f.is_empty())
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect()
}

/// Collects branch, recent commits and the staged files with their diffs.
pub fn gather(git: &dyn GitWriter) -> Result<PromptInput> {
    let (code, output) = git.run_status(&["symbolic-ref", "--short", "-q", "HEAD"], read())?;
    let branch = if code == Some(0) {
        output.stdout_text().trim().to_owned()
    } else {
        "detached HEAD".to_owned()
    };
    let (code, output) = git.run_status(&["log", "-10", "--format=%s"], read())?;
    let recent = if code == Some(0) {
        output.stdout_text().lines().map(str::to_owned).collect()
    } else {
        Vec::new() // unborn branch
    };
    // `<added>\t<removed>\t<path>\0`, "-" for binary files.
    let numstat = git.run(
        &["diff", "--cached", "--no-renames", "--numstat", "-z"],
        read(),
    )?;
    let counts: Vec<(String, Option<(u64, u64)>)> = nul_fields(&numstat.stdout)
        .into_iter()
        .filter_map(|record| {
            let mut parts = record.splitn(3, '\t');
            let (added, removed, path) = (parts.next()?, parts.next()?, parts.next()?);
            let counts = added.parse().ok().zip(removed.parse().ok());
            Some((path.to_owned(), counts))
        })
        .collect();
    let status = git.run(
        &["diff", "--cached", "--no-renames", "--name-status", "-z"],
        read(),
    )?;
    let fields = nul_fields(&status.stdout);
    let mut files = Vec::new();
    for pair in fields.chunks(2) {
        let [letter, path] = pair else { continue };
        let counts = counts.iter().find(|(p, _)| p == path).and_then(|(_, c)| *c);
        let diff = if counts.is_none() {
            Err(Omitted::Binary)
        } else if staged_size(git, path)? > MAX_FILE_BYTES {
            Err(Omitted::TooLarge)
        } else {
            Ok(git
                .run(&["diff", "--cached", "--no-renames", "--", path], read())?
                .stdout_text())
        };
        files.push(PromptFile::new(letter, path, counts, diff));
    }
    Ok(PromptInput {
        branch,
        recent,
        files,
    })
}

/// Size of the staged blob of `path` (0 for a deleted file).
fn staged_size(git: &dyn GitWriter, path: &str) -> Result<u64> {
    let (code, output) = git.run_status(&["cat-file", "-s", &format!(":{path}")], read())?;
    Ok(if code == Some(0) {
        output.stdout_text().trim().parse().unwrap_or(0)
    } else {
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(files: Vec<PromptFile>) -> PromptInput {
        PromptInput {
            branch: "main".into(),
            recent: vec!["Add parser".into(), "Fix tests".into()],
            files,
        }
    }

    #[test]
    fn parts_in_order() {
        let prompt = build(
            "Instruction.",
            Some("feat: \n\nWhy:"),
            &input(vec![PromptFile::new(
                "M",
                "src/a.rs",
                Some((2, 1)),
                Ok("diff --git a/src/a.rs b/src/a.rs\n+x\n".into()),
            )]),
        );
        let text = &prompt.text;
        let order = [
            "Instruction.",
            "Commit template:\nfeat:",
            "Branch: main",
            "Recent commits:\n- Add parser\n- Fix tests",
            "Files:\nM src/a.rs (+2 -1)",
            "Staged diff:\ndiff --git a/src/a.rs",
        ]
        .map(|part| {
            text.find(part)
                .unwrap_or_else(|| panic!("{part:?} in {text}"))
        });
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
        assert!(prompt.notes.is_empty());
    }

    #[test]
    fn no_template_no_recent_commits() {
        let mut input = input(Vec::new());
        input.recent.clear();
        let prompt = build("I", None, &input);
        assert!(!prompt.text.contains("Commit template"));
        assert!(!prompt.text.contains("Recent commits"));
    }

    #[test]
    fn limits_and_notes() {
        let big = "+".repeat(MAX_DIFF_CHARS - 10);
        let prompt = build(
            "I",
            None,
            &input(vec![
                PromptFile::new("M", "big.txt", Some((1, 0)), Ok(big)),
                PromptFile::new("M", "small.txt", Some((1, 0)), Ok("+0123456789abc".into())),
                PromptFile::new("A", "logo.png", None, Err(Omitted::Binary)),
                PromptFile::new("M", "dump.sql", Some((9, 9)), Err(Omitted::TooLarge)),
            ]),
        );
        assert!(prompt.text.contains("A logo.png (binary)"));
        assert!(prompt.text.contains("small.txt: (diff omitted)"));
        assert!(prompt.text.contains("logo.png: (diff omitted)"));
        assert!(prompt.text.contains("dump.sql: (diff omitted)"));
        assert_eq!(
            prompt.notes,
            [
                "1 diff omitted (binary)",
                "1 diff omitted (larger than 256 KB)",
                "1 diff omitted (size limit reached)"
            ]
        );
    }
}
