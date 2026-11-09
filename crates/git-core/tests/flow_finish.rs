//! Finishing Git flow branches (M7-09) in real repositories. The `git-flow`
//! CLI is not available in CI, so the results are compared with the
//! history `git flow <type> finish` produces, written out by hand.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::flow_finish::{self, FinishOptions, FinishOutcome, FinishProblem};
use git_core::{flow, CliBackend, FlowBranch, FlowConfig, FlowKind};
use support::TestRepo;

fn config() -> FlowConfig {
    FlowConfig {
        version_tag: "v".into(),
        ..FlowConfig::defaults(&["main"])
    }
}

/// main with one commit, develop with one more, Git flow initialized.
fn repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    repo.commit("a.txt", "base\n", "base");
    flow::init(&CliBackend::new(repo.path()), &config()).unwrap();
    repo.checkout("develop");
    repo.commit("d.txt", "dev\n", "develop work");
    repo
}

fn rev(repo: &TestRepo, name: &str) -> String {
    repo.git(&["rev-parse", name])
}

fn exists(repo: &TestRepo, reference: &str) -> bool {
    repo.try_git(&["rev-parse", "--verify", "--quiet", reference])
}

fn head_branch(repo: &TestRepo) -> String {
    repo.git(&["rev-parse", "--abbrev-ref", "HEAD"])
}

fn finish(
    repo: &TestRepo,
    kind: FlowKind,
    name: &str,
    edit: impl FnOnce(&mut FinishOptions),
) -> FinishOutcome {
    let branch = FlowBranch {
        kind,
        name: name.into(),
    };
    let mut options = FinishOptions::defaults(&config(), &branch);
    edit(&mut options);
    let plan = flow_finish::plan(&config(), &branch, &options);
    flow_finish::start(&CliBackend::new(repo.path()), &plan).unwrap()
}

/// Starts `<prefix><name>` from `base` with one commit touching `file`.
fn start(repo: &TestRepo, branch: &str, base: &str, file: &str, content: &str) -> String {
    repo.git(&["switch", "-c", branch, base]);
    repo.commit(file, content, &format!("work on {branch}"))
}

#[test]
fn feature_finish_merges_without_fast_forward_and_deletes() {
    let repo = repo();
    let develop = rev(&repo, "develop");
    let tip = start(&repo, "feature/login", "develop", "f.txt", "f\n");
    assert!(matches!(
        finish(&repo, FlowKind::Feature, "login", |_| {}),
        FinishOutcome::Done(_)
    ));

    // Like `git flow feature finish`: a merge commit on develop, branch gone.
    assert_eq!(head_branch(&repo), "develop");
    assert_eq!(repo.parents("develop"), [develop, tip]);
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s", "develop"]),
        "Merge branch 'feature/login' into develop"
    );
    assert!(!exists(&repo, "refs/heads/feature/login"));
    assert!(flow_finish::pending(&CliBackend::new(repo.path()))
        .unwrap()
        .is_none());
}

#[test]
fn feature_finish_keeps_the_branch_and_can_rebase() {
    let repo = repo();
    let tip = start(&repo, "feature/a", "develop", "f.txt", "f\n");
    repo.checkout("develop");
    let develop = repo.commit("d2.txt", "more\n", "more develop work");
    assert!(matches!(
        finish(&repo, FlowKind::Feature, "a", |o| {
            o.rebase = true;
            o.delete = false;
        }),
        FinishOutcome::Done(_)
    ));
    // Rebased: the feature commit now sits on top of develop's work.
    let rebased = rev(&repo, "feature/a");
    assert_ne!(rebased, tip);
    assert_eq!(repo.parents(&rebased), std::slice::from_ref(&develop));
    assert_eq!(repo.parents("develop"), [develop, rebased]);
    assert!(exists(&repo, "refs/heads/feature/a"));
}

#[test]
fn release_finish_merges_tags_and_back_merges() {
    let repo = repo();
    let main = rev(&repo, "main");
    let develop = rev(&repo, "develop");
    let tip = start(&repo, "release/1.0", "develop", "version.txt", "1.0\n");
    assert!(matches!(
        finish(&repo, FlowKind::Release, "1.0", |_| {}),
        FinishOutcome::Done(_)
    ));

    // Like `git flow release finish 1.0`: merge into main, annotated tag
    // there, merge of the release branch into develop, branch deleted,
    // develop checked out.
    let merged_main = rev(&repo, "main");
    assert_eq!(repo.parents("main"), [main, tip.clone()]);
    assert_eq!(rev(&repo, "v1.0^{commit}"), merged_main);
    assert_eq!(repo.git(&["cat-file", "-t", "v1.0"]), "tag");
    assert_eq!(
        repo.git(&["tag", "-l", "--format=%(contents:subject)", "v1.0"]),
        "Release 1.0"
    );
    assert_eq!(repo.parents("develop"), [develop, tip]);
    assert!(!exists(&repo, "refs/heads/release/1.0"));
    assert_eq!(head_branch(&repo), "develop");
}

#[test]
fn hotfix_finish_goes_to_main_and_develop() {
    let repo = repo();
    let tip = start(&repo, "hotfix/1.0.1", "main", "fix.txt", "fix\n");
    assert!(matches!(
        finish(&repo, FlowKind::Hotfix, "1.0.1", |o| o.tag = "1.0.1".into()),
        FinishOutcome::Done(_)
    ));
    assert!(repo.parents("main").contains(&tip));
    assert!(repo.parents("develop").contains(&tip));
    assert!(exists(&repo, "refs/tags/1.0.1"));
    assert_eq!(
        repo.git(&["tag", "-l", "--format=%(contents:subject)", "1.0.1"]),
        "Hotfix 1.0.1"
    );
}

/// Release whose changes conflict with main (`main_conflict`) or with
/// develop (otherwise).
fn conflicting_release(main_conflict: bool) -> TestRepo {
    let repo = repo();
    let other = if main_conflict { "main" } else { "develop" };
    repo.checkout(other);
    repo.commit("c.txt", &format!("{other}\n"), &format!("{other} side"));
    let base = if main_conflict { "develop" } else { "main" };
    start(&repo, "release/2.0", base, "c.txt", "release\n");
    repo
}

/// Resolves the conflict in c.txt and commits the merge like Continue.
fn resolve(repo: &TestRepo) {
    repo.write("c.txt", "resolved\n");
    repo.git(&["add", "c.txt"]);
    repo.git(&["commit", "--no-edit"]);
}

#[test]
fn conflict_in_the_first_merge_resumes_after_a_restart() {
    let repo = conflicting_release(true);
    assert_eq!(
        finish(&repo, FlowKind::Release, "2.0", |_| {}),
        FinishOutcome::Conflicts
    );
    assert!(exists(&repo, "MERGE_HEAD"));
    // A new backend (as after a restart) still knows the finish.
    let cli = CliBackend::new(repo.path());
    let plan = flow_finish::pending(&cli).unwrap().unwrap();
    assert_eq!(plan.title(), "Finishing release 2.0");
    assert_eq!(plan.next, 2, "checkout and merge started");
    // Resume while the merge still waits: nothing happens.
    assert_eq!(flow_finish::resume(&cli).unwrap(), FinishOutcome::Conflicts);

    resolve(&repo);
    assert!(matches!(
        flow_finish::resume(&cli).unwrap(),
        FinishOutcome::Done(_)
    ));
    assert!(exists(&repo, "refs/tags/v2.0"));
    assert_eq!(rev(&repo, "v2.0^{commit}"), rev(&repo, "main"));
    assert!(!exists(&repo, "refs/heads/release/2.0"));
    assert!(flow_finish::pending(&cli).unwrap().is_none());
}

#[test]
fn conflict_in_the_second_merge_resumes() {
    let repo = conflicting_release(false);
    assert_eq!(
        finish(&repo, FlowKind::Release, "2.0", |_| {}),
        FinishOutcome::Conflicts
    );
    let cli = CliBackend::new(repo.path());
    assert_eq!(flow_finish::pending(&cli).unwrap().unwrap().next, 5);
    assert_eq!(head_branch(&repo), "develop");
    assert!(exists(&repo, "refs/tags/v2.0"), "tag done before");
    resolve(&repo);
    assert!(matches!(
        flow_finish::resume(&cli).unwrap(),
        FinishOutcome::Done(_)
    ));
    assert!(!exists(&repo, "refs/heads/release/2.0"));
}

#[test]
fn abort_stops_the_merge_and_reports_what_was_done() {
    let repo = conflicting_release(false);
    assert_eq!(
        finish(&repo, FlowKind::Release, "2.0", |_| {}),
        FinishOutcome::Conflicts
    );
    let develop = rev(&repo, "develop");
    let cli = CliBackend::new(repo.path());
    let done = flow_finish::abort(&cli).unwrap();
    assert_eq!(
        done,
        [
            "Checked out main",
            "Merged release/2.0 into main",
            "Created tag v2.0",
            "Checked out develop"
        ]
    );
    assert!(!exists(&repo, "MERGE_HEAD"));
    assert_eq!(rev(&repo, "develop"), develop, "the merge is undone");
    assert!(
        exists(&repo, "refs/heads/release/2.0"),
        "steps after stay undone"
    );
    assert!(flow_finish::pending(&cli).unwrap().is_none());
}

#[test]
fn preconditions_are_typed_problems() {
    let repo = repo();
    start(&repo, "feature/x", "develop", "f.txt", "f\n");
    let cli = CliBackend::new(repo.path());
    let branch = FlowBranch {
        kind: FlowKind::Feature,
        name: "x".into(),
    };
    let plan = flow_finish::plan(
        &config(),
        &branch,
        &FinishOptions::defaults(&config(), &branch),
    );
    repo.write("f.txt", "changed\n");
    assert_eq!(
        flow_finish::problem(&cli, &plan).unwrap(),
        Some(FinishProblem::LocalChanges)
    );
    repo.git(&["checkout", "--", "f.txt"]);
    // Untracked files do not block.
    repo.write("new.txt", "n\n");
    assert_eq!(flow_finish::problem(&cli, &plan).unwrap(), None);

    let missing = FlowBranch {
        kind: FlowKind::Feature,
        name: "gone".into(),
    };
    let plan = flow_finish::plan(
        &config(),
        &missing,
        &FinishOptions::defaults(&config(), &missing),
    );
    assert_eq!(
        flow_finish::problem(&cli, &plan).unwrap(),
        Some(FinishProblem::MissingBranch("feature/gone".into()))
    );

    repo.tag("v3.0");
    start(&repo, "release/3.0", "develop", "r.txt", "r\n");
    let release = FlowBranch {
        kind: FlowKind::Release,
        name: "3.0".into(),
    };
    let plan = flow_finish::plan(
        &config(),
        &release,
        &FinishOptions::defaults(&config(), &release),
    );
    assert_eq!(
        flow_finish::problem(&cli, &plan).unwrap(),
        Some(FinishProblem::TagExists("v3.0".into()))
    );
    let error = flow_finish::start(&cli, &plan).unwrap_err();
    assert!(
        error.to_string().contains("The tag v3.0 exists already"),
        "{error}"
    );
}

/// Full ref names and their values, sorted.
fn refs(repo: &TestRepo) -> String {
    repo.git(&["for-each-ref", "--format=%(refname) %(objectname)"])
}

fn finished_record(outcome: FinishOutcome) -> flow_finish::FinishRecord {
    match outcome {
        FinishOutcome::Done(record) => *record,
        FinishOutcome::Conflicts => panic!("stopped with conflicts"),
    }
}

#[test]
fn undo_restores_every_ref_and_head_and_redo_the_identical_tag() {
    let repo = repo();
    start(&repo, "release/1.0", "develop", "version.txt", "1.0\n");
    let before = refs(&repo);
    let record = finished_record(finish(&repo, FlowKind::Release, "1.0", |_| {}));
    assert_eq!(record.label, "Finish release 1.0");
    assert_eq!(record.head_before.as_deref(), Some("release/1.0"));
    assert_eq!(record.head_after.as_deref(), Some("develop"));
    let after = refs(&repo);
    let tag_object = rev(&repo, "refs/tags/v1.0");
    let cli = CliBackend::new(repo.path());

    flow_finish::undo(&cli, &record).unwrap();
    assert_eq!(
        refs(&repo),
        before,
        "branches back, tag and nothing else gone"
    );
    assert_eq!(head_branch(&repo), "release/1.0");
    assert!(repo.git(&["status", "--porcelain"]).is_empty());

    flow_finish::redo(&cli, &record).unwrap();
    assert_eq!(refs(&repo), after);
    assert_eq!(rev(&repo, "refs/tags/v1.0"), tag_object, "same tag object");
    assert_eq!(head_branch(&repo), "develop");
}

#[test]
fn undo_is_refused_after_a_commit_on_develop() {
    let repo = repo();
    start(&repo, "feature/x", "develop", "f.txt", "f\n");
    let record = finished_record(finish(&repo, FlowKind::Feature, "x", |_| {}));
    repo.commit("more.txt", "more\n", "after the finish");
    let error = flow_finish::undo(&CliBackend::new(repo.path()), &record).unwrap_err();
    assert!(
        error.to_string().contains("The repository changed"),
        "{error}"
    );
    assert!(!exists(&repo, "refs/heads/feature/x"), "nothing changed");
}

#[test]
fn a_finish_resumed_after_conflicts_is_undone_as_a_whole() {
    let repo = conflicting_release(false);
    let before = refs(&repo);
    assert_eq!(
        finish(&repo, FlowKind::Release, "2.0", |_| {}),
        FinishOutcome::Conflicts
    );
    resolve(&repo);
    let cli = CliBackend::new(repo.path());
    let record = finished_record(flow_finish::resume(&cli).unwrap());
    flow_finish::undo(&cli, &record).unwrap();
    assert_eq!(refs(&repo), before);
    assert_eq!(head_branch(&repo), "release/2.0");
}
