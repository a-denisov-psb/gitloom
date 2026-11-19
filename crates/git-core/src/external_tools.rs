//! External diff and merge tools (M5-12): Gitloom delegates to `git
//! difftool` / `git mergetool` with `--no-prompt`, so Git's tool
//! configuration applies. Stdin is closed and `GIT_EDITOR` stays `:`, so
//! nothing waits for a terminal.
//!
//! A command template from Gitloom's settings (fix after M15, ADR-055)
//! runs the same way: it is handed to Git per call as the tool
//! [`CUSTOM_TOOL`] with `-c difftool.<name>.cmd=…`, so Git still writes
//! and removes the temporary files and nothing is written to any Git
//! config.

use crate::{Error, GitWriter, Result, RunOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Diff,
    Merge,
}

impl ToolKind {
    fn command(self) -> &'static str {
        match self {
            Self::Diff => "difftool",
            Self::Merge => "mergetool",
        }
    }
}

pub use crate::identity::CUSTOM_TOOL;

/// Placeholders of a command template and the shell variables Git sets.
const PLACEHOLDERS: [(&str, &str); 4] = [
    ("{local}", "LOCAL"),
    ("{remote}", "REMOTE"),
    ("{base}", "BASE"),
    ("{merged}", "MERGED"),
];

/// Which tool `git difftool` / `git mergetool` runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSelection {
    /// Git's own choice (`diff.tool` / `merge.tool`).
    Configured,
    /// A tool Git knows, as `--tool=<name>`.
    Named(String),
    /// A shell command for Git (from [`tool_shell_command`]).
    Command(String),
}

impl ToolSelection {
    /// Gitloom names the tool (Git's configuration is not asked).
    pub fn is_explicit(&self) -> bool {
        !matches!(self, Self::Configured)
    }
}

/// The shell command Git runs for a command template already split into
/// `words` (program first): every word is quoted for Git's `sh`, and the
/// placeholders `{local}`, `{remote}`, `{base}` and `{merged}` become
/// `"$LOCAL"` and so on, also inside a word (`--output={merged}`). On
/// Windows the program's backslashes become slashes, as Git for Windows
/// runs tools through its POSIX shell (like the editor command, ADR-021).
pub fn tool_shell_command(words: &[String], windows: bool) -> String {
    words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let word = if windows && index == 0 {
                word.replace('\\', "/")
            } else {
                word.clone()
            };
            shell_word(&word)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `word` for `sh`: literal parts in single quotes, placeholders as
/// double-quoted variables.
fn shell_word(word: &str) -> String {
    let mut out = String::new();
    let mut literal = String::new();
    let flush = |literal: &mut String, out: &mut String| {
        if !literal.is_empty() {
            out.push('\'');
            out.push_str(&literal.replace('\'', r"'\''"));
            out.push('\'');
            literal.clear();
        }
    };
    let mut rest = word;
    'outer: while !rest.is_empty() {
        for (placeholder, variable) in PLACEHOLDERS {
            if let Some(after) = rest.strip_prefix(placeholder) {
                flush(&mut literal, &mut out);
                out.push_str(&format!("\"${variable}\""));
                rest = after;
                continue 'outer;
            }
        }
        let mut chars = rest.chars();
        if let Some(c) = chars.next() {
            literal.push(c);
        }
        rest = chars.as_str();
    }
    flush(&mut literal, &mut out);
    if out.is_empty() {
        out.push_str("''");
    }
    out
}

/// Tools Git knows for one kind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolInfo {
    /// `diff.tool` (falling back to `merge.tool` like Git) or `merge.tool`.
    pub configured: Option<String>,
    /// Installed tools and user-defined `difftool.<name>.cmd` ones.
    pub available: Vec<String>,
}

/// What `git difftool` compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffTarget {
    /// The file in commit `id` against its first parent (`None`: root commit).
    Commit { id: String, parent: Option<String> },
    /// Working tree against the index.
    Unstaged,
    /// Index against HEAD.
    Staged,
}

/// Reads the configured tool and the available ones.
pub fn tool_info(git: &dyn GitWriter, kind: ToolKind) -> Result<ToolInfo> {
    let help = git.run(&[kind.command(), "--tool-help"], read_only())?;
    Ok(ToolInfo {
        configured: configured_tool(git, kind)?,
        available: parse_tool_help(&help.stdout_text()),
    })
}

/// The tool Git uses without `--tool`, if any.
pub fn configured_tool(git: &dyn GitWriter, kind: ToolKind) -> Result<Option<String>> {
    let keys: &[&str] = match kind {
        ToolKind::Diff => &["diff.tool", "merge.tool"],
        ToolKind::Merge => &["merge.tool"],
    };
    for key in keys {
        if let Some(value) = config_value(git, key)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn config_value(git: &dyn GitWriter, key: &str) -> Result<Option<String>> {
    match git.run(&["config", "--get", key], read_only()) {
        Ok(output) => {
            let value = output.stdout_text().trim().to_owned();
            Ok((!value.is_empty()).then_some(value))
        }
        // Exit code 1: the key is not set.
        Err(Error::Command { code: Some(1), .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Tools of the first section of `--tool-help` ("may be set to one of the
/// following"); user-defined ones are listed as `<name>.cmd …`.
pub fn parse_tool_help(text: &str) -> Vec<String> {
    let mut tools = Vec::new();
    for line in text.lines() {
        if line.starts_with("The following tools are valid, but not currently available")
            || line.starts_with("Some of the tools listed above")
        {
            break;
        }
        let Some(entry) = line.strip_prefix("\t\t") else {
            continue;
        };
        let Some(name) = entry.split_whitespace().next() else {
            continue;
        };
        let name = name.strip_suffix(".cmd").unwrap_or(name);
        if !tools.iter().any(|t| t == name) {
            tools.push(name.to_owned());
        }
    }
    tools
}

/// `-c …` arguments for a command template and the `--tool` argument.
fn tool_args(kind: ToolKind, tool: &ToolSelection) -> (Vec<String>, Option<String>) {
    match tool {
        ToolSelection::Configured => (Vec::new(), None),
        ToolSelection::Named(name) => (Vec::new(), Some(format!("--tool={name}"))),
        ToolSelection::Command(command) => {
            let section = format!("{}.{CUSTOM_TOOL}", kind.command());
            let mut config = vec!["-c".to_owned(), format!("{section}.cmd={command}")];
            if kind == ToolKind::Merge {
                // Resolved when the tool saved the file, whatever it exits
                // with; unchanged: Git asks on stdin, which is closed, so
                // the file stays conflicted instead of waiting.
                config.extend(["-c".to_owned(), format!("{section}.trustExitCode=false")]);
            }
            (config, Some(format!("--tool={CUSTOM_TOOL}")))
        }
    }
}

/// `git difftool --no-prompt [--tool=X] …` arguments; `old_path` is the
/// path before a rename in a commit.
pub fn difftool_args(
    target: &DiffTarget,
    tool: &ToolSelection,
    path: &str,
    old_path: Option<&str>,
    empty_tree: &str,
) -> Vec<String> {
    let (mut args, tool) = tool_args(ToolKind::Diff, tool);
    args.extend(["difftool".to_owned(), "--no-prompt".to_owned()]);
    args.extend(tool);
    match target {
        DiffTarget::Commit { id, parent } => {
            args.push(parent.clone().unwrap_or_else(|| empty_tree.to_owned()));
            args.push(id.clone());
        }
        DiffTarget::Unstaged => {}
        DiffTarget::Staged => args.push("--cached".to_owned()),
    }
    args.push("--".to_owned());
    if let Some(old) = old_path.filter(|old| *old != path) {
        args.push(old.to_owned());
    }
    args.push(path.to_owned());
    args
}

/// `delete_orig`: no `.orig` backup is left after merging
/// (`mergetool.keepBackup=false`, M10-08).
pub fn mergetool_args(tool: &ToolSelection, path: &str, delete_orig: bool) -> Vec<String> {
    let (mut args, tool) = tool_args(ToolKind::Merge, tool);
    if delete_orig {
        args.extend(["-c".to_owned(), "mergetool.keepBackup=false".to_owned()]);
    }
    args.extend(["mergetool".to_owned(), "--no-prompt".to_owned()]);
    args.extend(tool);
    args.extend(["--".to_owned(), path.to_owned()]);
    args
}

/// Turns the Git flow layout of the repository on or off
/// (`gitloom.flowLayout` in its config, M13-03).
pub fn set_flow_layout(git: &dyn GitWriter, on: bool) -> Result<()> {
    let key = crate::identity::config_key("flowLayout");
    git.run(
        &["config", "--local", &key, if on { "true" } else { "false" }],
        RunOptions::default(),
    )
    .map(drop)
}

/// Sets the repository's external editor (`gitloom.externalEditor` in its
/// config, M10-11); `None` removes it (the global setting applies).
pub fn set_repository_editor(git: &dyn GitWriter, command: Option<&str>) -> Result<()> {
    let key = crate::identity::config_key("externalEditor");
    match command.map(str::trim).filter(|c| !c.is_empty()) {
        Some(command) => git
            .run(&["config", "--local", &key, command], RunOptions::default())
            .map(drop),
        None => {
            let (code, output) = git.run_status(
                &["config", "--local", "--unset-all", &key],
                RunOptions::default(),
            )?;
            // 5: the key was not set.
            if matches!(code, Some(0 | 5)) {
                Ok(())
            } else {
                Err(Error::Command {
                    command: "git config --unset-all".into(),
                    code,
                    stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                })
            }
        }
    }
}

/// Opens the diff of `path` in the external tool and waits until it exits.
pub fn run_difftool(
    git: &dyn GitWriter,
    target: &DiffTarget,
    tool: &ToolSelection,
    path: &str,
    old_path: Option<&str>,
) -> Result<()> {
    let empty_tree = match target {
        DiffTarget::Commit { parent: None, .. } => empty_tree(git)?,
        _ => String::new(),
    };
    let args = difftool_args(target, tool, path, old_path, &empty_tree);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    git.run(&args, RunOptions::default()).map(|_| ())
}

/// Opens the conflicted `path` in the merge tool and waits until it exits.
pub fn run_mergetool(
    git: &dyn GitWriter,
    tool: &ToolSelection,
    path: &str,
    delete_orig: bool,
) -> Result<()> {
    let args = mergetool_args(tool, path, delete_orig);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    git.run(&args, RunOptions::default()).map(|_| ())
}

/// Id of the empty tree in this repository's hash format.
pub(crate) fn empty_tree(git: &dyn GitWriter) -> Result<String> {
    let output = git.run(
        &["mktree"],
        RunOptions {
            stdin: Some(b""),
            read_only: false,
        },
    )?;
    Ok(output.stdout_text().trim().to_owned())
}

fn read_only() -> RunOptions<'static> {
    RunOptions {
        stdin: None,
        read_only: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_help_lists_installed_and_user_defined_tools() {
        let text = "'git difftool --tool=<tool>' may be set to one of the following:\n\
                    \t\tmeld             Use Meld\n\
                    \t\tvimdiff          Use Vim\n\
                    \tuser-defined:\n\
                    \t\tfake.cmd echo \"$LOCAL\"\n\
                    \n\
                    The following tools are valid, but not currently available:\n\
                    \t\tbc               Use Beyond Compare\n\
                    \n\
                    Some of the tools listed above only work in a windowed\n";
        assert_eq!(parse_tool_help(text), ["meld", "vimdiff", "fake"]);
        let none = "No suitable tool for 'git difftool --tool=<tool>' found.\n\n\
                    The following tools are valid, but not currently available:\n\
                    \t\tmeld             Use Meld\n";
        assert!(parse_tool_help(none).is_empty());
    }

    fn named(name: &str) -> ToolSelection {
        ToolSelection::Named(name.into())
    }

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn templates_become_quoted_shell_commands() {
        // A preset from PATH.
        assert_eq!(
            tool_shell_command(
                &words(&["code", "--wait", "--diff", "{local}", "{remote}"]),
                false
            ),
            r#"'code' '--wait' '--diff' "$LOCAL" "$REMOTE""#
        );
        // Placeholders inside a word, and all four of a merge.
        assert_eq!(
            tool_shell_command(
                &words(&["meld", "--output={merged}", "{local}", "{base}", "{remote}"]),
                false
            ),
            r#"'meld' '--output='"$MERGED" "$LOCAL" "$BASE" "$REMOTE""#
        );
        // A Windows program path with spaces and backslashes (Git's sh):
        // slashes in the program, quotes kept, arguments untouched.
        assert_eq!(
            tool_shell_command(
                &words(&[
                    r"C:\Program Files\Microsoft VS Code\Code.exe",
                    "--wait",
                    "--diff",
                    "{local}",
                    r"C:\it's\{remote}",
                ]),
                true
            ),
            r#"'C:/Program Files/Microsoft VS Code/Code.exe' '--wait' '--diff' "$LOCAL" 'C:\it'\''s\'"$REMOTE""#
        );
        // Unknown braces stay literal; an empty word stays a word.
        assert_eq!(
            tool_shell_command(&words(&["tool", "{path}", ""]), false),
            "'tool' '{path}' ''"
        );
    }

    #[test]
    fn command_templates_run_as_the_custom_tool() {
        let command = ToolSelection::Command("'tool' \"$LOCAL\" \"$REMOTE\"".into());
        assert_eq!(
            difftool_args(&DiffTarget::Staged, &command, "a.txt", None, ""),
            [
                "-c",
                "difftool.gitloom-custom.cmd='tool' \"$LOCAL\" \"$REMOTE\"",
                "difftool",
                "--no-prompt",
                "--tool=gitloom-custom",
                "--cached",
                "--",
                "a.txt"
            ]
        );
        // Merge: the exit code is not trusted; with delete_orig the backup
        // setting comes too.
        assert_eq!(
            mergetool_args(&command, "c.txt", true),
            [
                "-c",
                "mergetool.gitloom-custom.cmd='tool' \"$LOCAL\" \"$REMOTE\"",
                "-c",
                "mergetool.gitloom-custom.trustExitCode=false",
                "-c",
                "mergetool.keepBackup=false",
                "mergetool",
                "--no-prompt",
                "--tool=gitloom-custom",
                "--",
                "c.txt"
            ]
        );
        assert!(command.is_explicit() && named("x").is_explicit());
        assert!(!ToolSelection::Configured.is_explicit());
    }

    #[test]
    fn difftool_arguments() {
        let commit = DiffTarget::Commit {
            id: "c2".into(),
            parent: Some("c1".into()),
        };
        assert_eq!(
            difftool_args(&commit, &named("meld"), "b.txt", Some("a.txt"), ""),
            [
                "difftool",
                "--no-prompt",
                "--tool=meld",
                "c1",
                "c2",
                "--",
                "a.txt",
                "b.txt"
            ]
        );
        let root = DiffTarget::Commit {
            id: "c1".into(),
            parent: None,
        };
        assert_eq!(
            difftool_args(&root, &ToolSelection::Configured, "a.txt", None, "4b825dc"),
            ["difftool", "--no-prompt", "4b825dc", "c1", "--", "a.txt"]
        );
        assert_eq!(
            difftool_args(
                &DiffTarget::Unstaged,
                &ToolSelection::Configured,
                "a b.txt",
                None,
                ""
            ),
            ["difftool", "--no-prompt", "--", "a b.txt"]
        );
        assert_eq!(
            difftool_args(&DiffTarget::Staged, &named("x"), "a.txt", Some("a.txt"), ""),
            [
                "difftool",
                "--no-prompt",
                "--tool=x",
                "--cached",
                "--",
                "a.txt"
            ]
        );
        assert_eq!(
            mergetool_args(&named("kdiff3"), "c.txt", false),
            ["mergetool", "--no-prompt", "--tool=kdiff3", "--", "c.txt"]
        );
        assert_eq!(
            mergetool_args(&ToolSelection::Configured, "c.txt", true),
            [
                "-c",
                "mergetool.keepBackup=false",
                "mergetool",
                "--no-prompt",
                "--",
                "c.txt"
            ]
        );
    }
}
