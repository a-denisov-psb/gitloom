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

/// Checked-out commit per submodule path (`None`: not initialized).
pub type Heads = Vec<(String, Option<Oid>)>;

/// The checked-out commits of all submodules (M12-08).
pub fn heads(git: &dyn GitWriter, root: &Path) -> Result<Heads> {
    if !root.join(".gitmodules").is_file() {
        return Ok(Vec::new());
    }
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    let output = git.run(&["submodule", "status", "--recursive"], read)?;
    Ok(parse_status(&output.stdout_text())
        .into_iter()
        .map(|s| {
            let head = (s.state != SubmoduleState::NotInitialized).then_some(s.commit);
            (s.path, head)
        })
        .collect())
}

/// What a submodule update changed: `(path, before, after)` of every
/// submodule whose checkout moved (M12-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateRecord {
    pub changes: Vec<(String, Option<Oid>, Option<Oid>)>,
}

/// Compares the heads before and after an update; `None` when nothing moved.
pub fn update_record(before: &Heads, after: &Heads) -> Option<UpdateRecord> {
    let changes: Vec<_> = after
        .iter()
        .filter_map(|(path, now)| {
            let was = before
                .iter()
                .find(|(p, _)| p == path)
                .and_then(|(_, head)| head.clone());
            (was != *now).then(|| (path.clone(), was, now.clone()))
        })
        .collect();
    (!changes.is_empty()).then_some(UpdateRecord { changes })
}

/// The submodule's working tree has local changes (or cannot be read).
fn dirty(git: &dyn GitWriter, path: &str) -> bool {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    git.run(&["-C", path, "status", "--porcelain"], read)
        .map_or(true, |s| !s.stdout.is_empty())
}

/// The innermost other changed submodule containing `path`, as (parent,
/// path relative to it); `None` for a submodule of the repository itself.
fn parent_of<'a>(
    path: &'a str,
    all: &'a [(String, Option<Oid>, Option<Oid>)],
) -> Option<(&'a str, &'a str)> {
    all.iter()
        .filter_map(|(other, _, _)| {
            let rest = path.strip_prefix(other.as_str())?.strip_prefix('/')?;
            Some((other.as_str(), rest))
        })
        .max_by_key(|(other, _)| other.len())
}

/// Undoes (`undo`) or redoes a submodule update: checks the recorded
/// commits out again (detached, as Git leaves submodules), and
/// de-initializes submodules that were not initialized before. Refuses
/// without changes when a submodule moved or has local changes since.
pub fn switch_update(
    git: &dyn GitWriter,
    root: &Path,
    record: &UpdateRecord,
    undo: bool,
) -> Result<()> {
    let current = heads(git, root)?;
    let head_of = |path: &str| {
        current
            .iter()
            .find(|(p, _)| p == path)
            .and_then(|(_, head)| head.clone())
    };
    for (path, before, after) in &record.changes {
        let (expected, _) = if undo {
            (after, before)
        } else {
            (before, after)
        };
        if head_of(path) != *expected || (expected.is_some() && dirty(git, path)) {
            return Err(crate::Error::Backend(format!(
                "Cannot {}: the submodule {path} changed",
                if undo { "undo" } else { "redo" }
            )));
        }
    }
    // Undo works from the innermost submodule outwards, redo the other way.
    let mut order: Vec<&(String, Option<Oid>, Option<Oid>)> = record.changes.iter().collect();
    order.sort_by_key(|(path, _, _)| path.matches('/').count());
    if undo {
        order.reverse();
    }
    for (path, before, after) in order {
        let target = if undo { before } else { after };
        let parent = parent_of(path, &record.changes);
        // A nested submodule whose parent goes away with a deinit needs nothing.
        if undo
            && parent.is_some_and(|(p, _)| {
                record
                    .changes
                    .iter()
                    .any(|(q, before, _)| q == p && before.is_none())
            })
        {
            continue;
        }
        let (dir, rel) = match parent {
            Some((parent, rel)) => (parent.to_owned(), rel.to_owned()),
            None => (".".to_owned(), path.clone()),
        };
        match target {
            Some(commit) if head_of(path).is_some() || !undo => {
                if head_of(path).is_none() {
                    git.run(
                        &[
                            "-C",
                            &dir,
                            "submodule",
                            "update",
                            "--init",
                            "--quiet",
                            "--",
                            &rel,
                        ],
                        RunOptions::default(),
                    )?;
                }
                git.run(
                    &["-C", path, "checkout", "--quiet", "--detach", commit],
                    RunOptions::default(),
                )?;
            }
            Some(_) => {}
            None => {
                git.run(
                    &[
                        "-C",
                        &dir,
                        "submodule",
                        "deinit",
                        "--quiet",
                        "--force",
                        "--",
                        &rel,
                    ],
                    RunOptions::default(),
                )?;
            }
        }
    }
    Ok(())
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
