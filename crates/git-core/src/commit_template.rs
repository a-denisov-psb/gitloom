//! Commit template (M10-06): Git's `commit.template` file, edited from
//! Preferences → Repository → Commit, and the options "apply" and "remove
//! comments" in the repository config (`gitloom.applyCommitTemplate`,
//! `gitloom.removeComments`).
//!
//! Blocking: runs `git` and reads/writes the template file.

use std::path::{Path, PathBuf};

use crate::{CliBackend, Error, GitWriter, Result, RunOptions};

/// How the commit panel uses the template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateOptions {
    /// Prefill empty messages from the template (default on).
    pub apply: bool,
    /// Remove comment lines from the prefill and the commit message
    /// (default on).
    pub remove_comments: bool,
    /// `core.commentChar` (`#` by default and for `auto`).
    pub comment_char: String,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        Self {
            apply: true,
            remove_comments: true,
            comment_char: "#".into(),
        }
    }
}

impl TemplateOptions {
    /// From the raw config values (`None`: not set).
    pub fn from_config(apply: Option<bool>, remove: Option<bool>, comment: Option<&str>) -> Self {
        let comment_char = match comment.map(str::trim) {
            Some(c) if !c.is_empty() && !c.eq_ignore_ascii_case("auto") => c.to_owned(),
            _ => "#".to_owned(),
        };
        Self {
            apply: apply.unwrap_or(true),
            remove_comments: remove.unwrap_or(true),
            comment_char,
        }
    }
}

/// Lines starting with `comment` removed; surrounding blank lines trimmed.
pub fn strip_comment_lines(text: &str, comment: &str) -> String {
    let kept: Vec<&str> = text.lines().filter(|l| !l.starts_with(comment)).collect();
    kept.join("\n").trim().to_owned()
}

/// Template file format: summary line, blank line, description.
pub fn split_template(text: &str) -> (String, String) {
    let text = text.trim_start_matches(['\n', '\r']);
    match text.split_once('\n') {
        Some((summary, rest)) => (summary.trim().to_owned(), rest.trim().to_owned()),
        None => (text.trim().to_owned(), String::new()),
    }
}

/// The file text for `summary` and `description`.
pub fn join_template(summary: &str, description: &str) -> String {
    let (summary, description) = (summary.trim(), description.trim());
    if description.is_empty() {
        format!("{summary}\n")
    } else {
        format!("{summary}\n\n{description}\n")
    }
}

/// Where `commit.template` comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFile {
    pub path: PathBuf,
    /// Git's scope name: "local", "global", "system", "worktree", "command".
    pub scope: String,
}

impl TemplateFile {
    /// Changing it changes every repository (needs a confirmation).
    pub fn shared(&self) -> bool {
        self.scope != "local" && self.scope != "worktree"
    }
}

/// What Preferences → Repository → Commit shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSettings {
    pub file: Option<TemplateFile>,
    /// Content of the file (empty when there is none or it is missing).
    pub content: String,
    pub options: TemplateOptions,
}

fn read() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

fn config_value(git: &CliBackend, args: &[&str]) -> Result<Option<String>> {
    let (code, output) = git.run_status(args, read())?;
    Ok((code == Some(0)).then(|| {
        output
            .stdout_text()
            .trim_end_matches(['\n', '\r'])
            .to_owned()
    }))
}

fn config_bool(git: &CliBackend, key: &str) -> Result<Option<bool>> {
    Ok(config_value(git, &["config", "--type=bool", "--get", key])?.map(|v| v == "true"))
}

/// Reads the template file and options of the repository at `root`.
pub fn read_template(root: &Path) -> Result<TemplateSettings> {
    let git = CliBackend::new(root);
    let file = config_value(
        &git,
        &[
            "config",
            "--show-scope",
            "--type=path",
            "--get",
            "commit.template",
        ],
    )?
    .and_then(|line| {
        let (scope, path) = line.split_once('\t')?;
        let path = PathBuf::from(path);
        let path = if path.is_relative() {
            root.join(path)
        } else {
            path
        };
        Some(TemplateFile {
            path,
            scope: scope.to_owned(),
        })
    });
    let content = file
        .as_ref()
        .and_then(|f| std::fs::read_to_string(&f.path).ok())
        .unwrap_or_default();
    let options = TemplateOptions::from_config(
        config_bool(&git, &crate::identity::config_key("applyCommitTemplate"))?,
        config_bool(&git, &crate::identity::config_key("removeComments"))?,
        config_value(&git, &["config", "--get", "core.commentChar"])?.as_deref(),
    );
    Ok(TemplateSettings {
        file,
        content,
        options,
    })
}

/// Saves the template: into the configured file, or, without one, into
/// `.git/gitloom/commit-template.txt` with `commit.template` set in the
/// repository config. A shared (global) file is written only with
/// `shared_confirmed`. Returns the file written.
pub fn save_template(
    root: &Path,
    summary: &str,
    description: &str,
    shared_confirmed: bool,
) -> Result<PathBuf> {
    let git = CliBackend::new(root);
    let current = read_template(root)?.file;
    let text = join_template(summary, description);
    let path = match current {
        Some(file) if file.shared() && !shared_confirmed => {
            return Err(Error::Backend(
                "the commit template is used by all repositories; confirm the change first".into(),
            ))
        }
        Some(file) => file.path,
        None => {
            let git_dir = git
                .run(&["rev-parse", "--absolute-git-dir"], read())?
                .stdout_text()
                .trim()
                .to_owned();
            let path = PathBuf::from(git_dir)
                .join(crate::identity::BINARY_NAME)
                .join("commit-template.txt");
            git.run(
                &[
                    "config",
                    "--local",
                    "commit.template",
                    &path.to_string_lossy(),
                ],
                RunOptions::default(),
            )?;
            path
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Backend(format!("could not create {}: {e}", parent.display())))?;
    }
    std::fs::write(&path, text)
        .map_err(|e| Error::Backend(format!("could not write {}: {e}", path.display())))?;
    Ok(path)
}

/// Sets `gitloom.applyCommitTemplate` (`apply`) or `gitloom.removeComments`
/// in the repository config.
pub fn set_template_option(root: &Path, apply: bool, on: bool) -> Result<()> {
    let key = crate::identity::config_key(if apply {
        "applyCommitTemplate"
    } else {
        "removeComments"
    });
    CliBackend::new(root)
        .run(
            &["config", "--local", &key, if on { "true" } else { "false" }],
            RunOptions::default(),
        )
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_from_config() {
        assert_eq!(
            TemplateOptions::from_config(None, None, None),
            TemplateOptions::default()
        );
        let options = TemplateOptions::from_config(Some(false), Some(false), Some(";"));
        assert!(!options.apply && !options.remove_comments);
        assert_eq!(options.comment_char, ";");
        assert_eq!(
            TemplateOptions::from_config(None, None, Some("auto")).comment_char,
            "#"
        );
    }

    #[test]
    fn split_join_and_strip() {
        assert_eq!(
            split_template("Summary\n\nLine 1\nLine 2\n"),
            ("Summary".into(), "Line 1\nLine 2".into())
        );
        assert_eq!(split_template("Only\n"), ("Only".into(), String::new()));
        assert_eq!(join_template("S", ""), "S\n");
        assert_eq!(join_template(" S ", " D \n"), "S\n\nD\n");
        assert_eq!(
            strip_comment_lines("Fix\n# hint\n\nBody\n; other\n", "#"),
            "Fix\n\nBody\n; other"
        );
        assert_eq!(
            strip_comment_lines("Fix\n; hint\n# keep", ";"),
            "Fix\n# keep"
        );
    }
}
