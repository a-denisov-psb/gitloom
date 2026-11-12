//! Operation in progress shown like GitKraken (M12-05): a row at the top of
//! the graph, a header in the commit details, the panel title, Conflicted
//! and Resolved Files and the commit/abort buttons. Replaces the tab banner
//! of M2-13 and M9; ADR-044 is met by the row, the header and the buttons.

use git_core::{Operation, RepoState};

use crate::repo_view::{ConflictBanner, RepoView};

/// What is in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpKind {
    /// A Git operation (merge, rebase, cherry-pick, revert).
    Git(Operation),
    /// A stash pop/apply that stopped in conflicts (M9-01):
    /// `ConflictBanner::StashConflicts` or `StashResolved`.
    Stash(ConflictBanner),
    /// Conflicted files that no operation explains.
    Orphaned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationStatus {
    pub kind: OpKind,
    /// Files with unresolved conflicts.
    pub conflicts: usize,
    /// Text of the graph row.
    pub row: String,
    /// Panel title, e.g. "Merging topic into main".
    pub title: String,
    /// The operation belongs to a Git flow finish: Abort aborts the finish
    /// (M7-10).
    pub finishing: bool,
}

impl OperationStatus {
    /// "Merge conflicts detected" above the panel title, while conflicts
    /// are open.
    pub fn conflicts_heading(&self) -> Option<String> {
        if self.conflicts == 0 {
            return None;
        }
        Some(match &self.kind {
            OpKind::Git(operation) => format!("{} conflicts detected", capitalized(*operation)),
            OpKind::Stash(_) => "Stash conflicts detected".to_owned(),
            OpKind::Orphaned => "Conflicts detected".to_owned(),
        })
    }

    /// "1 file conflict in working directory" (details header).
    pub fn conflicts_text(&self) -> String {
        match self.conflicts {
            1 => "1 file conflict in working directory".to_owned(),
            n => format!("{n} file conflicts in working directory"),
        }
    }

    /// Label of the commit button: "Commit and merge" / "Continue rebase" …
    pub fn commit_label(&self) -> Option<String> {
        match &self.kind {
            OpKind::Git(Operation::Merge) => Some("Commit and merge".to_owned()),
            OpKind::Git(operation) => Some(format!("Continue {}", operation.command())),
            _ => None,
        }
    }

    /// Label of the abort button.
    pub fn abort_label(&self) -> Option<String> {
        match &self.kind {
            OpKind::Git(_) if self.finishing => Some("Abort finish".to_owned()),
            OpKind::Git(operation) => Some(format!("Abort {}", operation.command())),
            _ => None,
        }
    }
}

/// "Merge", "Rebase", "Cherry-pick", "Revert".
pub fn capitalized(operation: Operation) -> String {
    let command = operation.command();
    let mut chars = command.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect::<String>()
    })
}

fn short(id: &str) -> &str {
    &id[..id.len().min(7)]
}

/// The operation in progress in `view`, if any.
pub fn status(view: &RepoView) -> Option<OperationStatus> {
    let snapshot = &view.snapshot;
    let conflicts = view.conflicted_files().len();
    if let Some(operation) = snapshot.state.kind() {
        let mut title = snapshot.state_banner().unwrap_or_default();
        let finishing = snapshot.flow_finish.is_some();
        if let Some(plan) = &snapshot.flow_finish {
            title = format!("{} · {title}", plan.title());
        }
        let head = match &snapshot.head {
            git_core::Head::Branch { name, .. } | git_core::Head::Unborn { name } => name.as_str(),
            git_core::Head::Detached { .. } => "HEAD",
        };
        let target = match &snapshot.state {
            RepoState::Merging { .. } => format!("merge into {head}"),
            RepoState::Rebasing { branch, .. } => {
                format!("rebase {}", branch.as_deref().unwrap_or("HEAD"))
            }
            RepoState::CherryPicking { commit } | RepoState::Reverting { commit } => match commit {
                Some(id) => format!("{} {}", operation.command(), short(id)),
                None => operation.command().to_owned(),
            },
            RepoState::Clean => String::new(),
        };
        let row = if conflicts > 0 {
            format!("A file conflict was found when attempting to {target}")
        } else {
            format!("{} in progress", capitalized(operation))
        };
        return Some(OperationStatus {
            kind: OpKind::Git(operation),
            conflicts,
            row,
            title,
            finishing,
        });
    }
    let banner = view.conflict_banner()?;
    let (row, title) = match &banner {
        ConflictBanner::StashConflicts { label, .. } => (
            format!("A file conflict was found when attempting to apply stash '{label}'"),
            format!("Applying stash '{label}'"),
        ),
        ConflictBanner::StashResolved { label, .. } => {
            (banner.text(), format!("Applying stash '{label}'"))
        }
        ConflictBanner::Orphaned => (banner.text(), "Conflicted files".to_owned()),
    };
    let kind = match banner {
        ConflictBanner::Orphaned => OpKind::Orphaned,
        stash => OpKind::Stash(stash),
    };
    Some(OperationStatus {
        kind,
        conflicts,
        row,
        title,
        finishing: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalized_commands() {
        assert_eq!(capitalized(Operation::CherryPick), "Cherry-pick");
        assert_eq!(capitalized(Operation::Merge), "Merge");
    }
}
