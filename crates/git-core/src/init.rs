//! Start a local repository (M10-03, ADR-045): `git init -b <branch>`,
//! optional README and `.gitignore`, and an initial commit when an identity
//! resolves.
//!
//! Blocking: runs `git` and writes files.

use std::path::Path;

use crate::{CliBackend, Error, GitWriter, Result, RunOptions};

/// `.gitignore` templates bundled from github/gitignore (CC0-1.0, listed in
/// the third-party notices), by display name.
pub const GITIGNORE_TEMPLATES: &[(&str, &str)] = &[
    ("C++", include_str!("../assets/gitignore/C++.gitignore")),
    ("Go", include_str!("../assets/gitignore/Go.gitignore")),
    ("Java", include_str!("../assets/gitignore/Java.gitignore")),
    (
        "JetBrains",
        include_str!("../assets/gitignore/JetBrains.gitignore"),
    ),
    ("macOS", include_str!("../assets/gitignore/macOS.gitignore")),
    ("Node", include_str!("../assets/gitignore/Node.gitignore")),
    (
        "Python",
        include_str!("../assets/gitignore/Python.gitignore"),
    ),
    ("Rust", include_str!("../assets/gitignore/Rust.gitignore")),
    (
        "VisualStudio",
        include_str!("../assets/gitignore/VisualStudio.gitignore"),
    ),
    (
        "Windows",
        include_str!("../assets/gitignore/Windows.gitignore"),
    ),
];

/// Content of the bundled `.gitignore` template `name`.
pub fn gitignore_template(name: &str) -> Option<&'static str> {
    GITIGNORE_TEMPLATES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, content)| *content)
}

/// What "Start a local repository" creates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOptions {
    /// Initial branch (`git init -b`).
    pub branch: String,
    /// Write `README.md` with `# <name>`.
    pub readme: bool,
    /// Name of a bundled `.gitignore` template.
    pub gitignore: Option<String>,
}

/// How the new repository was left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitOutcome {
    /// No files were chosen: an empty repository (unborn branch).
    Empty,
    /// The chosen files were committed as "Initial commit".
    Committed,
    /// No Git identity is set: the files were written but not committed.
    NoIdentity,
}

/// Creates a repository in `target` (which must not exist or be empty),
/// named `name` in the README. A folder that did not exist before is
/// removed again if `git init` fails.
pub fn init(target: &Path, name: &str, options: &InitOptions) -> Result<InitOutcome> {
    init_with(CliBackend::new(target), target, name, options)
}

/// [`init`] with a prepared backend rooted at `target` (e.g. with
/// [`CliBackend::with_config`]).
pub fn init_with(
    git: CliBackend,
    target: &Path,
    name: &str,
    options: &InitOptions,
) -> Result<InitOutcome> {
    if let Some(problem) = crate::clone::target_problem(target) {
        return Err(Error::Backend(problem));
    }
    let branch = options.branch.trim();
    if let Some(problem) = crate::check_branch_name(branch) {
        return Err(Error::Backend(problem.into()));
    }
    let existed = target.exists();
    std::fs::create_dir_all(target)
        .map_err(|e| Error::Backend(format!("could not create {}: {e}", target.display())))?;
    if let Err(error) = git.run(&["init", "--quiet", "-b", branch], RunOptions::default()) {
        if !existed {
            let _ = std::fs::remove_dir_all(target);
        }
        return Err(error);
    }
    let mut files = Vec::new();
    if options.readme {
        write(target, "README.md", &format!("# {}\n", name.trim()))?;
        files.push("README.md".to_owned());
    }
    if let Some(template) = &options.gitignore {
        let content = gitignore_template(template)
            .ok_or_else(|| Error::NotFound(format!(".gitignore template {template}")))?;
        write(target, ".gitignore", content)?;
        files.push(".gitignore".to_owned());
    }
    if files.is_empty() {
        return Ok(InitOutcome::Empty);
    }
    if !has_identity(&git)? {
        return Ok(InitOutcome::NoIdentity);
    }
    git.stage(&files)?;
    git.commit("Initial commit", false)?;
    Ok(InitOutcome::Committed)
}

fn write(root: &Path, file: &str, content: &str) -> Result<()> {
    std::fs::write(root.join(file), content)
        .map_err(|e| Error::Backend(format!("could not write {file}: {e}")))
}

/// `user.name` and `user.email` resolve to a usable identity.
fn has_identity(git: &CliBackend) -> Result<bool> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    let value = |key: &str| -> Result<String> {
        let (code, output) = git.run_status(&["config", "--get", key], read)?;
        Ok(if code == Some(0) {
            output.stdout_text().trim().to_owned()
        } else {
            String::new()
        })
    };
    let (name, email) = (value("user.name")?, value("user.email")?);
    Ok(crate::identity_problem(&name, &email).is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_are_bundled_and_sorted() {
        let names: Vec<&str> = GITIGNORE_TEMPLATES.iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort_by_key(|n| n.to_lowercase());
        assert_eq!(names, sorted);
        assert_eq!(names.len(), 10);
        assert!(gitignore_template("Rust").unwrap().contains("target"));
        assert!(gitignore_template("Missing").is_none());
    }
}
