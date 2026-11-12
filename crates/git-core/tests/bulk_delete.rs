//! Bulk delete of branches and tags (M12-11): merged and unmerged local
//! branches, tags, undo in one step, remote branches with one push per
//! remote, partial failures.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{delete_refs, delete_remote_branches_args, CliBackend, GitWriter as _, RunOptions};
use support::TestRepo;

fn refs(repo: &TestRepo) -> (String, String) {
    (
        repo.git(&[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/heads",
            "refs/tags",
        ]),
        repo.git(&["config", "--local", "--get-regexp", "^branch\\."]),
    )
}

#[test]
fn local_branches_and_tags_with_undo_in_one_step() {
    let repo = TestRepo::new();
    let base = repo.commit("a.txt", "a\n", "base");
    for name in ["merged", "also-merged"] {
        repo.branch(name);
    }
    repo.branch("unmerged");
    repo.checkout("unmerged");
    repo.commit("b.txt", "b\n", "only here");
    repo.checkout("main");
    repo.fake_upstream("origin", "merged", &base);
    repo.tag_annotated("v1", "release one");
    repo.tag("light");
    let git = CliBackend::new(repo.path());
    let before = refs(&repo);
    let branches = vec![
        "merged".to_owned(),
        "unmerged".into(),
        "also-merged".into(),
        "missing".into(),
    ];
    let tags = vec!["v1".to_owned(), "light".into()];
    let outcome = delete_refs(&git, &branches, &tags, false).unwrap();
    assert_eq!(outcome.unmerged, ["unmerged"]);
    assert_eq!(
        outcome.failed.len(),
        1,
        "missing: the others are still deleted"
    );
    assert_eq!(outcome.changes.len(), 4);
    let heads = repo.git(&[
        "for-each-ref",
        "--format=%(refname:short)",
        "refs/heads",
        "refs/tags",
    ]);
    assert_eq!(heads, "main\nunmerged");
    // Forced part, then undo of both parts in reverse order.
    let forced = delete_refs(&git, &["unmerged".to_owned()], &[], true).unwrap();
    let all: Vec<_> = outcome
        .changes
        .iter()
        .chain(&forced.changes)
        .cloned()
        .collect();
    for change in all.iter().rev() {
        change.apply(&git, true).unwrap();
    }
    let (now_refs, now_config) = refs(&repo);
    assert_eq!(now_config, before.1, "upstream config restored");
    // Annotated tags come back as new tag objects with the same message.
    assert_eq!(
        now_refs
            .lines()
            .filter(|l| !l.contains("refs/tags/v1"))
            .collect::<Vec<_>>(),
        before
            .0
            .lines()
            .filter(|l| !l.contains("refs/tags/v1"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        repo.git(&["tag", "-l", "--format=%(contents:subject)", "v1"]),
        "release one"
    );
    // Redo deletes them again.
    for change in &all {
        change.apply(&git, false).unwrap();
    }
    assert_eq!(repo.git(&["branch", "--format=%(refname:short)"]), "main");
}

#[test]
fn remote_branches_with_one_push_per_remote() {
    let origin = TestRepo::new_bare();
    let backup = TestRepo::new_bare();
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "base");
    for name in ["x", "y", "z"] {
        repo.branch(name);
    }
    for (remote, bare) in [("origin", &origin), ("backup", &backup)] {
        let url = bare.path().to_string_lossy().replace('\\', "/");
        repo.git(&["remote", "add", remote, &url]);
        repo.git(&["push", "--quiet", remote, "x", "y", "z"]);
    }
    let git = CliBackend::new(repo.path());
    for (remote, branches) in [("origin", vec!["x", "y"]), ("backup", vec!["z", "missing"])] {
        let branches: Vec<String> = branches.into_iter().map(str::to_owned).collect();
        let args = delete_remote_branches_args(remote, &branches);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let result = git.run(&args, RunOptions::default());
        // "missing" does not exist on backup: Git refuses that remote's
        // push as a whole; origin's deletions stay.
        assert_eq!(result.is_ok(), remote == "origin", "{remote}");
    }
    assert_eq!(origin.git(&["branch", "--format=%(refname:short)"]), "z");
    assert_eq!(
        backup.git(&["branch", "--format=%(refname:short)"]),
        "x\ny\nz"
    );
}
