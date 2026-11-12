//! External diff and merge tools (M5-12): Gitloom delegates to `git
//! difftool` / `git mergetool` with `--no-prompt`, so Git's tool
//! configuration applies. Stdin is closed and `GIT_EDITOR` stays `:`, so
//! nothing waits for a terminal.

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

/// `git difftool --no-prompt [--tool=X] …` arguments; `old_path` is the
/// path before a rename in a commit.
pub fn difftool_args(
    target: &DiffTarget,
    tool: Option<&str>,
    path: &str,
    old_path: Option<&str>,
    empty_tree: &str,
) -> Vec<String> {
    let mut args = vec!["difftool".to_owned(), "--no-prompt".to_owned()];
    if let Some(tool) = tool {
        args.push(format!("--tool={tool}"));
    }
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
pub fn mergetool_args(tool: Option<&str>, path: &str, delete_orig: bool) -> Vec<String> {
    let mut args = Vec::new();
    if delete_orig {
        args.extend(["-c".to_owned(), "mergetool.keepBackup=false".to_owned()]);
    }
    args.extend(["mergetool".to_owned(), "--no-prompt".to_owned()]);
    if let Some(tool) = tool {
        args.push(format!("--tool={tool}"));
    }
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
    tool: Option<&str>,
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
    tool: Option<&str>,
    path: &str,
    delete_orig: bool,
) -> Result<()> {
    let args = mergetool_args(tool, path, delete_orig);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    git.run(&args, RunOptions::default()).map(|_| ())
}

/// Id of the empty tree in this repository's hash format.
fn empty_tree(git: &dyn GitWriter) -> Result<String> {
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

    #[test]
    fn difftool_arguments() {
        let commit = DiffTarget::Commit {
            id: "c2".into(),
            parent: Some("c1".into()),
        };
        assert_eq!(
            difftool_args(&commit, Some("meld"), "b.txt", Some("a.txt"), ""),
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
            difftool_args(&root, None, "a.txt", None, "4b825dc"),
            ["difftool", "--no-prompt", "4b825dc", "c1", "--", "a.txt"]
        );
        assert_eq!(
            difftool_args(&DiffTarget::Unstaged, None, "a b.txt", None, ""),
            ["difftool", "--no-prompt", "--", "a b.txt"]
        );
        assert_eq!(
            difftool_args(&DiffTarget::Staged, Some("x"), "a.txt", Some("a.txt"), ""),
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
            mergetool_args(Some("kdiff3"), "c.txt", false),
            ["mergetool", "--no-prompt", "--tool=kdiff3", "--", "c.txt"]
        );
        assert_eq!(
            mergetool_args(None, "c.txt", true),
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
