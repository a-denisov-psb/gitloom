//! Submodules (M12-06): status from `git submodule status --recursive`,
//! dirty working trees from `git status` inside each submodule.

use std::path::Path;

use crate::{GitWriter, Oid, Result, RunOptions};

/// State of a submodule, like the icons of GitKraken's SUBMODULES section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmoduleState {
    /// Checked out at the recorded commit, no local changes.
    Current,
    /// Not cloned / not initialized (`-`).
    NotInitialized,
    /// The checked-out commit differs from the one the superproject records (`+`).
    OutOfDate,
    /// Changes in the submodule's working tree.
    Modified,
    /// Merge conflict on the submodule entry (`U`).
    Conflict,
}

impl SubmoduleState {
    /// Tooltip text.
    pub fn description(self) -> &'static str {
        match self {
            Self::Current => "Up to date",
            Self::NotInitialized => "Not initialized",
            Self::OutOfDate => "Out of date: the checked-out commit differs from the recorded one",
            Self::Modified => "Modified: the submodule has local changes",
            Self::Conflict => "Conflict",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submodule {
    /// Path relative to the top-level working tree, `/`-separated (nested
    /// submodules include their parents' paths).
    pub path: String,
    /// Commit shown by `git submodule status` (checked out, or recorded if
    /// not initialized).
    pub commit: Oid,
    pub state: SubmoduleState,
}

impl Submodule {
    /// Nesting level: 0 for submodules of the repository itself.
    pub fn depth(&self, all: &[Submodule]) -> usize {
        all.iter()
            .filter(|other| {
                other.path != self.path
                    && self
                        .path
                        .strip_prefix(other.path.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .count()
    }

    /// Last path segment.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

/// Parses `git submodule status --recursive`: `<flag><sha> <path>[ (<describe>)]`.
pub fn parse_status(output: &str) -> Vec<Submodule> {
    output
        .lines()
        .filter_map(|line| {
            let mut chars = line.chars();
            let flag = chars.next()?;
            let rest = chars.as_str();
            let (commit, rest) = rest.split_once(' ')?;
            // A describe suffix " (v1.0-3-gabc)" follows the path.
            let path = match rest.rfind(" (") {
                Some(index) if rest.ends_with(')') => &rest[..index],
                _ => rest,
            };
            let state = match flag {
                '-' => SubmoduleState::NotInitialized,
                '+' => SubmoduleState::OutOfDate,
                'U' => SubmoduleState::Conflict,
                _ => SubmoduleState::Current,
            };
            Some(Submodule {
                path: path.replace('\\', "/"),
                commit: commit.to_owned(),
                state,
            })
        })
        .collect()
}

/// The submodules of the repository at `root` (empty without `.gitmodules`);
/// checked out ones with local changes are [`SubmoduleState::Modified`].
pub fn submodules(git: &dyn GitWriter, root: &Path) -> Result<Vec<Submodule>> {
    if !root.join(".gitmodules").is_file() {
        return Ok(Vec::new());
    }
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    let output = git.run(&["submodule", "status", "--recursive"], read)?;
    let mut list = parse_status(&output.stdout_text());
    for submodule in &mut list {
        if submodule.state != SubmoduleState::Current {
            continue;
        }
        let status = git.run(
            &[
                "-C",
                &submodule.path,
                "status",
                "--porcelain",
                "--ignore-submodules=dirty",
            ],
            read,
        );
        if status.is_ok_and(|s| !s.stdout.is_empty()) {
            submodule.state = SubmoduleState::Modified;
        }
    }
    Ok(list)
}

/// Arguments of `git submodule update --init --recursive` for `paths` (all
/// submodules when empty), with progress for the network display.
pub fn update_args(paths: &[String]) -> Vec<String> {
    let mut args: Vec<String> = ["submodule", "update", "--init", "--recursive", "--progress"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    if !paths.is_empty() {
        args.push("--".into());
        args.extend(paths.iter().cloned());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lines() {
        let sha = "a".repeat(40);
        let output = format!(
            " {sha} libs/core (v1.0-2-gaaaaaaa)\n-{sha} vendor/x\n+{sha} libs/ui (heads/main)\nU{sha} conflicted\n {sha} libs/core/nested\n"
        );
        let list = parse_status(&output);
        let states: Vec<(&str, SubmoduleState)> =
            list.iter().map(|s| (s.path.as_str(), s.state)).collect();
        assert_eq!(
            states,
            [
                ("libs/core", SubmoduleState::Current),
                ("vendor/x", SubmoduleState::NotInitialized),
                ("libs/ui", SubmoduleState::OutOfDate),
                ("conflicted", SubmoduleState::Conflict),
                ("libs/core/nested", SubmoduleState::Current),
            ]
        );
        assert_eq!(list[0].commit, sha);
        assert_eq!(list[4].depth(&list), 1);
        assert_eq!(list[0].depth(&list), 0);
        assert_eq!(list[4].name(), "nested");
        // A path with a space and no describe suffix.
        assert_eq!(parse_status(&format!(" {sha} my lib"))[0].path, "my lib");
    }

    #[test]
    fn update_arguments() {
        assert_eq!(
            update_args(&[]).join(" "),
            "submodule update --init --recursive --progress"
        );
        assert_eq!(
            update_args(&["libs/ui".into()]).join(" "),
            "submodule update --init --recursive --progress -- libs/ui"
        );
    }
}
