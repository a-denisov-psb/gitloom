//! Finishing a Git flow branch (M7-09, ADR-037): a plan of steps built from
//! the branch type, the options and the configuration, run with existing
//! operations. The plan and the index of the next step are saved in
//! `<git-dir>/gitloom/gitflow-finish.json`, so a finish stopped by a
//! conflict continues after the conflict is resolved, also after a restart.
//! Nothing is pushed.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::flow::{FlowBranch, FlowConfig, FlowKind};
use crate::merge::{merge_in_progress, merge_no_ff, pseudo_ref_exists, rebase, MergeOutcome};
use crate::{Error, GitWriter, Result, RunOptions};

/// File of a running finish, in the app's folder of the git dir.
const PLAN_FILE: &str = "gitflow-finish.json";
const PLAN_VERSION: u32 = 1;

/// Options of the finish dialog (decisions table of M7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishOptions {
    /// Feature: rebase it on the development branch first.
    pub rebase: bool,
    /// Delete the finished branch.
    pub delete: bool,
    /// Release/hotfix: tag name (`<versiontag><version>`).
    pub tag: String,
    /// Release/hotfix: message of the annotated tag.
    pub tag_message: String,
}

impl FinishOptions {
    /// Dialog defaults: no rebase, delete the branch, tag `<versiontag><name>`
    /// with "Release <name>" / "Hotfix <name>".
    pub fn defaults(config: &FlowConfig, branch: &FlowBranch) -> Self {
        let tag_message = match branch.kind {
            FlowKind::Feature => String::new(),
            FlowKind::Release => format!("Release {}", branch.name),
            FlowKind::Hotfix => format!("Hotfix {}", branch.name),
        };
        Self {
            rebase: false,
            delete: true,
            tag: format!("{}{}", config.version_tag, branch.name),
            tag_message,
        }
    }
}

/// One step of a finish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "kebab-case")]
pub enum FinishStep {
    /// Rebase `branch` onto `onto` (`git rebase <onto> <branch>`).
    Rebase {
        branch: String,
        onto: String,
    },
    Checkout {
        branch: String,
    },
    /// `git merge --no-ff <branch>` into the checked-out branch.
    Merge {
        branch: String,
        into: String,
    },
    /// Annotated tag at HEAD.
    Tag {
        name: String,
        message: String,
    },
    Delete {
        branch: String,
    },
}

impl FinishStep {
    /// What the step did, for the abort dialog ("Merged release/1.0 into main").
    pub fn done_label(&self) -> String {
        match self {
            Self::Rebase { branch, onto } => format!("Rebased {branch} onto {onto}"),
            Self::Checkout { branch } => format!("Checked out {branch}"),
            Self::Merge { branch, into } => format!("Merged {branch} into {into}"),
            Self::Tag { name, .. } => format!("Created tag {name}"),
            Self::Delete { branch } => format!("Deleted {branch}"),
        }
    }
}

/// A finish: what is finished and its steps; `next` is the first step not
/// started yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishPlan {
    pub version: u32,
    pub kind: FlowKind,
    /// Name without prefix ("login", "1.2.0").
    pub name: String,
    /// Full branch name.
    pub branch: String,
    pub steps: Vec<FinishStep>,
    pub next: usize,
    /// Refs the finish touches and their values when it started (M7-11).
    #[serde(default)]
    pub before: Vec<(String, Option<String>)>,
    /// Branch checked out when it started (`None`: detached).
    #[serde(default)]
    pub head_before: Option<String>,
}

impl FinishPlan {
    /// Steps done so far; `stopped` (a merge or rebase of the plan waits
    /// for its conclusion) means the last started step is not done yet.
    pub fn done(&self, stopped: bool) -> &[FinishStep] {
        let done = if stopped {
            self.next.saturating_sub(1)
        } else {
            self.next
        };
        &self.steps[..done.min(self.steps.len())]
    }

    /// "Finishing release 1.2.0" (operation banner).
    pub fn title(&self) -> String {
        format!("Finishing {} {}", self.kind.noun(), self.name)
    }
}

/// Steps of finishing `branch` with `options`:
/// - feature: (rebase onto development) → checkout development → merge
///   `--no-ff` → (delete);
/// - release/hotfix: checkout production → merge `--no-ff` → tag →
///   checkout development → merge `--no-ff` the branch → (delete).
///
/// A hotfix is merged into the development branch also while a release
/// branch exists (GitKraken's documented behavior; the CLI merges into the
/// release branch then).
pub fn plan(config: &FlowConfig, branch: &FlowBranch, options: &FinishOptions) -> FinishPlan {
    let full = config.branch_name(branch.kind, &branch.name);
    let merge = |into: &str| FinishStep::Merge {
        branch: full.clone(),
        into: into.to_owned(),
    };
    let checkout = |branch: &str| FinishStep::Checkout {
        branch: branch.to_owned(),
    };
    let mut steps = Vec::new();
    match branch.kind {
        FlowKind::Feature => {
            if options.rebase {
                steps.push(FinishStep::Rebase {
                    branch: full.clone(),
                    onto: config.development.clone(),
                });
            }
            steps.push(checkout(&config.development));
            steps.push(merge(&config.development));
        }
        FlowKind::Release | FlowKind::Hotfix => {
            steps.push(checkout(&config.production));
            steps.push(merge(&config.production));
            steps.push(FinishStep::Tag {
                name: options.tag.clone(),
                message: options.tag_message.clone(),
            });
            steps.push(checkout(&config.development));
            steps.push(merge(&config.development));
        }
    }
    if options.delete {
        steps.push(FinishStep::Delete {
            branch: full.clone(),
        });
    }
    FinishPlan {
        version: PLAN_VERSION,
        kind: branch.kind,
        name: branch.name.clone(),
        branch: full,
        steps,
        next: 0,
        before: Vec::new(),
        head_before: None,
    }
}

/// One ref moved by a finish: values before and after (`None`: missing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefState {
    /// Full ref name (`refs/heads/main`, `refs/tags/v1.0`).
    pub name: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// A completed finish as one undo entry (M7-11): every touched ref and the
/// checked-out branch before and after. Undo/redo only move refs and check
/// out; commits and the annotated tag object stay, so redo restores the
/// identical tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishRecord {
    /// "Finish release 1.2.0".
    pub label: String,
    pub refs: Vec<RefState>,
    pub head_before: Option<String>,
    pub head_after: Option<String>,
}

/// Full names of the refs `plan` touches.
fn touched_refs(plan: &FinishPlan) -> Vec<String> {
    let mut refs = vec![format!("refs/heads/{}", plan.branch)];
    for step in &plan.steps {
        let name = match step {
            FinishStep::Merge { into, .. } => format!("refs/heads/{into}"),
            FinishStep::Tag { name, .. } => format!("refs/tags/{name}"),
            _ => continue,
        };
        if !refs.contains(&name) {
            refs.push(name);
        }
    }
    refs
}

fn ref_value(git: &dyn GitWriter, name: &str) -> Result<Option<String>> {
    match git.run(&["rev-parse", "--verify", "--quiet", name], read_only()) {
        Ok(output) => Ok(Some(output.stdout_text().trim().to_owned())),
        Err(Error::Command { code: Some(1), .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn head_branch(git: &dyn GitWriter) -> Result<Option<String>> {
    match git.run(&["symbolic-ref", "--quiet", "--short", "HEAD"], read_only()) {
        Ok(output) => Ok(Some(output.stdout_text().trim().to_owned())),
        Err(Error::Command { code: Some(1), .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Why a finish cannot start (decisions table "Finish preconditions").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishProblem {
    /// Staged or unstaged changes to tracked files.
    LocalChanges,
    /// A merge, rebase, cherry-pick or revert is in progress.
    OperationInProgress,
    /// Another finish is still running.
    FinishRunning(String),
    /// A branch of the plan does not exist locally.
    MissingBranch(String),
    /// The tag of a release/hotfix exists already or is empty.
    TagExists(String),
}

impl std::fmt::Display for FinishProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalChanges => write!(f, "Commit or stash your changes first"),
            Self::OperationInProgress => {
                write!(f, "Finish or abort the operation in progress first")
            }
            Self::FinishRunning(title) => write!(f, "{title} is still running"),
            Self::MissingBranch(branch) => write!(f, "The branch {branch} does not exist"),
            Self::TagExists(tag) if tag.is_empty() => write!(f, "Enter a tag name"),
            Self::TagExists(tag) => write!(f, "The tag {tag} exists already"),
        }
    }
}

/// How a run of the plan ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishOutcome {
    /// All steps ran; the record undoes the whole finish.
    Done(Box<FinishRecord>),
    /// A merge or rebase stopped with conflicts; after it is concluded,
    /// [`resume`] runs the remaining steps.
    Conflicts,
}

fn read_only() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

fn plan_path(git: &dyn GitWriter) -> Result<PathBuf> {
    let dir = git.run(&["rev-parse", "--absolute-git-dir"], read_only())?;
    Ok(PathBuf::from(dir.stdout_text().trim())
        .join(crate::identity::BINARY_NAME)
        .join(PLAN_FILE))
}

/// The running finish, if any.
pub fn pending(git: &dyn GitWriter) -> Result<Option<FinishPlan>> {
    read_plan(&plan_path(git)?)
}

/// The running finish of the repository with git dir `git_dir` (read
/// side, for the snapshot).
pub fn pending_in(git_dir: &std::path::Path) -> Result<Option<FinishPlan>> {
    read_plan(&git_dir.join(crate::identity::BINARY_NAME).join(PLAN_FILE))
}

fn read_plan(path: &std::path::Path) -> Result<Option<FinishPlan>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| Error::Backend(format!("{} is damaged: {e}", path.display())))
}

fn save(git: &dyn GitWriter, plan: &FinishPlan) -> Result<()> {
    let path = plan_path(git)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| Error::Backend(format!("could not create {}: {e}", dir.display())))?;
    }
    let text = serde_json::to_string_pretty(plan)
        .map_err(|e| Error::Backend(format!("could not save the finish: {e}")))?;
    std::fs::write(&path, text)
        .map_err(|e| Error::Backend(format!("could not write {}: {e}", path.display())))
}

fn clear(git: &dyn GitWriter) -> Result<()> {
    let path = plan_path(git)?;
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::Backend(format!(
            "could not remove {}: {e}",
            path.display()
        ))),
        _ => Ok(()),
    }
}

fn operation_in_progress(git: &dyn GitWriter) -> Result<bool> {
    let dir = git.run(&["rev-parse", "--absolute-git-dir"], read_only())?;
    Ok(!crate::state::detect(std::path::Path::new(dir.stdout_text().trim())).is_clean())
}

fn branch_exists(git: &dyn GitWriter, branch: &str) -> Result<bool> {
    pseudo_ref_exists(git, &format!("refs/heads/{branch}"))
}

/// Why `plan` cannot start now, if it cannot.
pub fn problem(git: &dyn GitWriter, plan: &FinishPlan) -> Result<Option<FinishProblem>> {
    if let Some(running) = pending(git)? {
        return Ok(Some(FinishProblem::FinishRunning(running.title())));
    }
    if operation_in_progress(git)? {
        return Ok(Some(FinishProblem::OperationInProgress));
    }
    let status = git.run(
        &["status", "--porcelain", "--untracked-files=no"],
        read_only(),
    )?;
    if !status.stdout.is_empty() {
        return Ok(Some(FinishProblem::LocalChanges));
    }
    let mut branches = vec![plan.branch.clone()];
    for step in &plan.steps {
        if let FinishStep::Checkout { branch } = step {
            branches.push(branch.clone());
        }
    }
    for branch in branches {
        if !branch_exists(git, &branch)? {
            return Ok(Some(FinishProblem::MissingBranch(branch)));
        }
    }
    for step in &plan.steps {
        if let FinishStep::Tag { name, .. } = step {
            if name.is_empty() || pseudo_ref_exists(git, &format!("refs/tags/{name}"))? {
                return Ok(Some(FinishProblem::TagExists(name.clone())));
            }
        }
    }
    Ok(None)
}

/// Starts `plan`: checks the preconditions, saves it and runs its steps.
pub fn start(git: &dyn GitWriter, plan: &FinishPlan) -> Result<FinishOutcome> {
    if let Some(problem) = problem(git, plan)? {
        return Err(Error::Backend(problem.to_string()));
    }
    let mut plan = plan.clone();
    plan.before = touched_refs(&plan)
        .into_iter()
        .map(|name| ref_value(git, &name).map(|value| (name, value)))
        .collect::<Result<_>>()?;
    plan.head_before = head_branch(git)?;
    run(git, plan)
}

/// Runs the remaining steps of the saved finish once the stopped merge or
/// rebase was concluded (Continue, or after a restart).
pub fn resume(git: &dyn GitWriter) -> Result<FinishOutcome> {
    let Some(plan) = pending(git)? else {
        return Err(Error::Backend("No Git flow finish is running".into()));
    };
    if operation_in_progress(git)? {
        return Ok(FinishOutcome::Conflicts);
    }
    run(git, plan)
}

fn run(git: &dyn GitWriter, mut plan: FinishPlan) -> Result<FinishOutcome> {
    save(git, &plan)?;
    while let Some(step) = plan.steps.get(plan.next).cloned() {
        let outcome = execute(git, &step)?;
        // A stopped merge/rebase counts as started: the user concludes it,
        // then the next step follows.
        plan.next += 1;
        save(git, &plan)?;
        if outcome == MergeOutcome::Conflicts {
            return Ok(FinishOutcome::Conflicts);
        }
    }
    clear(git)?;
    let refs = plan
        .before
        .iter()
        .map(|(name, before)| {
            Ok(RefState {
                name: name.clone(),
                before: before.clone(),
                after: ref_value(git, name)?,
            })
        })
        .collect::<Result<_>>()?;
    Ok(FinishOutcome::Done(Box::new(FinishRecord {
        label: format!("Finish {} {}", plan.kind.noun(), plan.name),
        refs,
        head_before: plan.head_before.clone(),
        head_after: head_branch(git)?,
    })))
}

/// Undoes a finish: every ref back to its value before, the branch that
/// was checked out before checked out again.
pub fn undo(git: &dyn GitWriter, record: &FinishRecord) -> Result<()> {
    apply(git, record, true)
}

/// Redoes an undone finish.
pub fn redo(git: &dyn GitWriter, record: &FinishRecord) -> Result<()> {
    apply(git, record, false)
}

fn apply(git: &dyn GitWriter, record: &FinishRecord, undo: bool) -> Result<()> {
    let status = git.run(
        &["status", "--porcelain", "--untracked-files=no"],
        read_only(),
    )?;
    if !status.stdout.is_empty() {
        return Err(Error::Backend(FinishProblem::LocalChanges.to_string()));
    }
    if operation_in_progress(git)? {
        return Err(Error::Backend(
            FinishProblem::OperationInProgress.to_string(),
        ));
    }
    // Every ref must still be where the finish (or the undo) left it.
    for state in &record.refs {
        let expected = if undo { &state.after } else { &state.before };
        if ref_value(git, &state.name)? != *expected {
            return Err(Error::Backend(
                "The repository changed since then; undo and redo only work while the \
                 branches are where they were left"
                    .into(),
            ));
        }
    }
    let message = crate::identity::reflog_message(if undo { "undo" } else { "redo" });
    // Detached first: moving the checked-out branch must not touch the
    // working tree; the final checkout updates it.
    git.run(&["switch", "--quiet", "--detach"], RunOptions::default())?;
    for state in &record.refs {
        let (from, to) = if undo {
            (&state.after, &state.before)
        } else {
            (&state.before, &state.after)
        };
        match (from, to) {
            (_, Some(to)) => {
                let old = from.clone().unwrap_or_else(|| "0".repeat(to.len()));
                git.run(
                    &["update-ref", "-m", &message, &state.name, to, &old],
                    RunOptions::default(),
                )?;
            }
            (Some(from), None) => {
                git.run(
                    &["update-ref", "-m", &message, "-d", &state.name, from],
                    RunOptions::default(),
                )?;
            }
            (None, None) => {}
        }
    }
    let head = if undo {
        &record.head_before
    } else {
        &record.head_after
    };
    if let Some(branch) = head {
        git.switch_branch(branch)?;
    }
    Ok(())
}

fn execute(git: &dyn GitWriter, step: &FinishStep) -> Result<MergeOutcome> {
    match step {
        FinishStep::Rebase { branch, onto } => {
            git.switch_branch(branch)?;
            rebase(git, onto, false)
        }
        FinishStep::Checkout { branch } => git.switch_branch(branch).map(|_| MergeOutcome::Done),
        FinishStep::Merge { branch, .. } => merge_no_ff(git, branch),
        FinishStep::Tag { name, message } => git
            .create_tag(name, "HEAD", Some(message))
            .map(|_| MergeOutcome::Done),
        FinishStep::Delete { branch } => {
            git.delete_branch(branch, false).map(|_| MergeOutcome::Done)
        }
    }
}

/// Aborts the running finish: a stopped merge or rebase is aborted, the
/// plan is dropped. Returns what was done before (steps stay done).
pub fn abort(git: &dyn GitWriter) -> Result<Vec<String>> {
    let Some(plan) = pending(git)? else {
        return Ok(Vec::new());
    };
    let mut done = plan.next;
    if merge_in_progress(git)? {
        git.run(&["merge", "--abort"], RunOptions::default())?;
        done = done.saturating_sub(1);
    } else if pseudo_ref_exists(git, "REBASE_HEAD")? || rebase_dir_exists(git)? {
        git.run(&["rebase", "--abort"], RunOptions::default())?;
        done = done.saturating_sub(1);
    }
    clear(git)?;
    Ok(plan.steps[..done.min(plan.steps.len())]
        .iter()
        .map(FinishStep::done_label)
        .collect())
}

fn rebase_dir_exists(git: &dyn GitWriter) -> Result<bool> {
    let dir = git.run(&["rev-parse", "--absolute-git-dir"], read_only())?;
    let dir = PathBuf::from(dir.stdout_text().trim());
    Ok(dir.join("rebase-merge").exists() || dir.join("rebase-apply").exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> FlowConfig {
        FlowConfig {
            version_tag: "v".into(),
            ..FlowConfig::defaults(&["main"])
        }
    }

    fn branch(kind: FlowKind, name: &str) -> FlowBranch {
        FlowBranch {
            kind,
            name: name.into(),
        }
    }

    fn labels(plan: &FinishPlan) -> Vec<String> {
        plan.steps.iter().map(FinishStep::done_label).collect()
    }

    #[test]
    fn feature_plan() {
        let feature = branch(FlowKind::Feature, "login");
        let mut options = FinishOptions::defaults(&config(), &feature);
        assert!(!options.rebase && options.delete);
        assert_eq!(
            labels(&plan(&config(), &feature, &options)),
            [
                "Checked out develop",
                "Merged feature/login into develop",
                "Deleted feature/login"
            ]
        );
        options.rebase = true;
        options.delete = false;
        assert_eq!(
            labels(&plan(&config(), &feature, &options)),
            [
                "Rebased feature/login onto develop",
                "Checked out develop",
                "Merged feature/login into develop"
            ]
        );
    }

    #[test]
    fn release_and_hotfix_plans() {
        let release = branch(FlowKind::Release, "1.2.0");
        let options = FinishOptions::defaults(&config(), &release);
        assert_eq!(options.tag, "v1.2.0");
        assert_eq!(options.tag_message, "Release 1.2.0");
        let plan = plan(&config(), &release, &options);
        assert_eq!(plan.title(), "Finishing release 1.2.0");
        assert_eq!(
            labels(&plan),
            [
                "Checked out main",
                "Merged release/1.2.0 into main",
                "Created tag v1.2.0",
                "Checked out develop",
                "Merged release/1.2.0 into develop",
                "Deleted release/1.2.0"
            ]
        );
        let hotfix = branch(FlowKind::Hotfix, "1.2.1");
        assert_eq!(
            FinishOptions::defaults(&config(), &hotfix).tag_message,
            "Hotfix 1.2.1"
        );
    }

    #[test]
    fn plans_survive_a_round_trip() {
        let release = branch(FlowKind::Release, "1.0");
        let plan = plan(
            &config(),
            &release,
            &FinishOptions::defaults(&config(), &release),
        );
        let text = serde_json::to_string(&plan).unwrap();
        assert!(text.contains("\"step\":\"merge\""), "{text}");
        assert_eq!(serde_json::from_str::<FinishPlan>(&text).unwrap(), plan);
    }
}
