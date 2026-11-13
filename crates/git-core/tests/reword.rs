// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Editing the message of unpushed commits (ADR-051, M14-04).

mod support;

use git_core::reword::{edit_block, edit_facts, reword_unpushed, EditBlock};
use git_core::CliBackend;
use support::TestRepo;

fn repo() -> (TestRepo, CliBackend) {
    let repo = TestRepo::new();
    // The CLI backend commits as this identity (the committer of rebuilt commits).
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    let cli = CliBackend::new(repo.path());
    (repo, cli)
}

fn message(repo: &TestRepo, rev: &str) -> String {
    repo.git(&["log", "-1", "--format=%B", rev])
}

fn author(repo: &TestRepo, rev: &str) -> String {
    repo.git(&["log", "-1", "--format=%an <%ae> %at", rev])
}

fn subjects(repo: &TestRepo) -> Vec<String> {
    repo.git(&["log", "--format=%s", "--topo-order"])
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn reword_head() {
    let (repo, cli) = repo();
    repo.commit("a.txt", "a\n", "One");
    let head = repo.commit("a.txt", "b\n", "Typo");
    let before_author = author(&repo, "HEAD");
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    assert_eq!(edit_block(&cli, &head).unwrap(), None);

    let done = reword_unpushed(&cli, &head, "Fixed summary\n\nWith a body").unwrap();
    assert_eq!(done.branch, "main");
    assert_eq!(done.old_head, head);
    assert_eq!(repo.head(), done.new_head);
    assert_eq!(done.new_commit, done.new_head);
    assert_eq!(message(&repo, "HEAD"), "Fixed summary\n\nWith a body");
    assert_eq!(repo.git(&["rev-parse", "HEAD^{tree}"]), tree);
    // Author kept, committer is the current identity.
    assert_eq!(author(&repo, "HEAD"), before_author);
    assert_eq!(
        repo.git(&["log", "-1", "--format=%cn <%ce>"]),
        "Gitloom Test <test@example.com>"
    );
    // HEAD stays on the branch.
    assert_eq!(repo.git(&["symbolic-ref", "--short", "HEAD"]), "main");
}

#[test]
fn reword_three_back_keeps_everything_else() {
    let (repo, cli) = repo();
    repo.commit("a.txt", "1\n", "One");
    let target = repo.commit("a.txt", "2\n", "Two");
    repo.commit("b.txt", "3\n", "Three");
    repo.commit("c.txt", "4\n", "Four");
    let authors: Vec<String> = ["HEAD", "HEAD~1", "HEAD~2", "HEAD~3"]
        .iter()
        .map(|r| author(&repo, r))
        .collect();
    let trees: Vec<String> = ["HEAD", "HEAD~1", "HEAD~2", "HEAD~3"]
        .iter()
        .map(|r| repo.git(&["rev-parse", &format!("{r}^{{tree}}")]))
        .collect();
    let root = repo.git(&["rev-parse", "HEAD~3"]);
    // Staged, unstaged and untracked changes stay as they are.
    repo.write("a.txt", "staged\n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "staged and edited\n");
    repo.write("u.txt", "untracked\n");
    let index = repo.git(&["ls-files", "-s"]);
    let status = repo.git(&["status", "--porcelain", "--untracked-files=all"]);

    let done = reword_unpushed(&cli, &target, "Two, better").unwrap();
    assert_eq!(subjects(&repo), ["Four", "Three", "Two, better", "One"]);
    assert_eq!(repo.git(&["rev-parse", "HEAD~2"]), done.new_commit);
    assert_eq!(
        repo.git(&["rev-parse", "HEAD~3"]),
        root,
        "older commits keep their ids"
    );
    for (i, rev) in ["HEAD", "HEAD~1", "HEAD~2", "HEAD~3"].iter().enumerate() {
        assert_eq!(author(&repo, rev), authors[i]);
        assert_eq!(
            repo.git(&["rev-parse", &format!("{rev}^{{tree}}")]),
            trees[i]
        );
    }
    assert_eq!(repo.git(&["ls-files", "-s"]), index);
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        status
    );
    assert_eq!(
        std::fs::read(repo.path().join("a.txt")).unwrap(),
        b"staged and edited\n"
    );
    assert_eq!(
        std::fs::read(repo.path().join("u.txt")).unwrap(),
        b"untracked\n"
    );

    // Undo and redo move the branch exactly.
    done.apply(&cli, true).unwrap();
    assert_eq!(repo.head(), done.old_head);
    done.apply(&cli, false).unwrap();
    assert_eq!(repo.head(), done.new_head);
    // The branch moved meanwhile: refused.
    repo.commit("d.txt", "5\n", "Five");
    let error = done.apply(&cli, true).unwrap_err().to_string();
    assert!(error.contains("the branch main changed"), "{error}");
}

#[test]
fn reword_before_a_merge_rebuilds_the_merge() {
    let (repo, cli) = repo();
    repo.commit("a.txt", "base\n", "Base");
    let target = repo.commit("a.txt", "main 1\n", "Main one");
    repo.git(&["switch", "--quiet", "-c", "topic", "HEAD~1"]);
    // Conflicting change, resolved in the merge: the rebuild must not replay it.
    let side = repo.commit("a.txt", "topic\n", "Topic");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "--no-ff", "-m", "Merge topic", "topic"]));
    repo.write("a.txt", "resolved\n");
    repo.git(&["add", "a.txt"]);
    repo.git(&["commit", "--quiet", "--no-edit"]);
    repo.commit("b.txt", "after\n", "After");
    // topic keeps the old commits; it does not contain the target.
    let facts = edit_facts(&cli, &target).unwrap();
    assert_eq!(facts.branch, "main");
    assert!(facts.keep_old.is_empty(), "{:?}", facts.keep_old);

    let done = reword_unpushed(&cli, &target, "Main one, reworded").unwrap();
    let merge = repo.git(&["rev-parse", "HEAD~1"]);
    let parents = repo.parents(&merge);
    assert_eq!(parents.len(), 2);
    assert_eq!(parents[0], done.new_commit);
    assert_eq!(parents[1], side, "second parent unchanged");
    assert_eq!(
        std::fs::read(repo.path().join("a.txt")).unwrap(),
        b"resolved\n"
    );
    assert_eq!(
        repo.git(&["rev-parse", "HEAD:a.txt"]),
        repo.git(&["rev-parse", &format!("{}:a.txt", done.old_head)])
    );
    assert_eq!(repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn pushed_other_branch_and_running_operation_are_refused() {
    let (repo, cli) = repo();
    let first = repo.commit("a.txt", "1\n", "One");
    let second = repo.commit("a.txt", "2\n", "Two");
    repo.fake_upstream("origin", "main", &first);
    // Pushed: named with the remote branch.
    assert_eq!(
        edit_block(&cli, &first).unwrap(),
        Some(EditBlock::Pushed("origin/main".into()))
    );
    let error = reword_unpushed(&cli, &first, "x").unwrap_err().to_string();
    assert!(error.contains("Already pushed to origin/main"), "{error}");
    assert_eq!(edit_block(&cli, &second).unwrap(), None);

    // A commit of another branch.
    repo.git(&["switch", "--quiet", "-c", "side", &first]);
    let side = repo.commit("s.txt", "s\n", "Side");
    repo.checkout("main");
    assert_eq!(
        edit_block(&cli, &side).unwrap(),
        Some(EditBlock::NotOnBranch)
    );
    // Other refs that contain the commit keep the old one.
    repo.git(&["tag", "v1", &second]);
    repo.git(&["branch", "copy", &second]);
    let mut keep = edit_facts(&cli, &second).unwrap().keep_old;
    keep.sort();
    assert_eq!(keep, ["copy", "v1"]);

    // Detached HEAD.
    repo.git(&["switch", "--quiet", "--detach", &second]);
    assert_eq!(
        edit_block(&cli, &second).unwrap(),
        Some(EditBlock::NotOnBranch)
    );
    repo.checkout("main");

    // During a merge.
    repo.git(&["switch", "--quiet", "-c", "clash", &first]);
    repo.commit("a.txt", "clash\n", "Clash");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "clash"]));
    assert_eq!(
        edit_block(&cli, &second).unwrap(),
        Some(EditBlock::OperationRunning)
    );
    assert!(reword_unpushed(&cli, &second, "x").is_err());
    assert_eq!(repo.head(), second);
}

#[test]
fn signed_rebuild_with_an_ssh_key() {
    if !cfg!(unix) {
        return;
    }
    let (repo, cli) = repo();
    let dir = repo.path().join("..").join("keys");
    std::fs::create_dir_all(&dir).unwrap();
    let keygen = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test", "-f"])
        .arg(dir.join("id"))
        .status();
    if !keygen.is_ok_and(|s| s.success()) {
        eprintln!("ssh-keygen not available: signed rebuild not tested");
        return;
    }
    let target = repo.commit("a.txt", "1\n", "One");
    repo.commit("a.txt", "2\n", "Two");
    // Signing off: a signed commit in the chain would lose its signature.
    assert!(!edit_facts(&cli, &target).unwrap().drops_signature);
    repo.git(&["config", "gpg.format", "ssh"]);
    repo.git(&[
        "config",
        "user.signingkey",
        dir.join("id.pub").to_str().unwrap(),
    ]);
    repo.git(&["config", "commit.gpgSign", "true"]);
    let done = reword_unpushed(&cli, &target, "One, signed").unwrap();
    for rev in [done.new_commit.as_str(), done.new_head.as_str()] {
        let raw = repo.git(&["cat-file", "commit", rev]);
        assert!(
            raw.contains("gpgsig -----BEGIN SSH SIGNATURE-----"),
            "{raw}"
        );
    }
    // With signing off again, the note warns about dropped signatures.
    repo.git(&["config", "commit.gpgSign", "false"]);
    assert!(edit_facts(&cli, &done.new_commit).unwrap().drops_signature);
}

// M15-11: changing the author (ADR-051 amendment).

use git_core::reword::{edit_facts_many, rewrite_unpushed, CommitEdit};
use git_core::Identity;

fn bo() -> Identity {
    Identity {
        name: "Bo Kim".into(),
        email: "bo@example.com".into(),
    }
}

fn author_edit(commit: &str) -> CommitEdit {
    CommitEdit {
        commit: commit.to_owned(),
        message: None,
        author: Some(bo()),
    }
}

fn author_date(repo: &TestRepo, rev: &str) -> String {
    repo.git(&["log", "-1", "--format=%ad", "--date=raw", rev])
}

#[test]
fn author_of_head() {
    let (repo, cli) = repo();
    repo.commit("a.txt", "a\n", "One");
    let head = repo.commit(
        "a.txt",
        "b\n",
        "Two\n\nBody\n\nCo-authored-by: Cy <cy@example.com>",
    );
    let date = author_date(&repo, "HEAD");
    // Index and working tree changes stay.
    repo.write("a.txt", "dirty\n");
    repo.write("staged.txt", "s\n");
    repo.git(&["add", "staged.txt"]);
    let status = repo.git(&["status", "--porcelain"]);

    let done = rewrite_unpushed(&cli, &[author_edit(&head)]).unwrap();
    assert_eq!(repo.head(), done.new_head);
    assert_eq!(
        repo.git(&["log", "-1", "--format=%an <%ae>"]),
        "Bo Kim <bo@example.com>"
    );
    assert_eq!(author_date(&repo, "HEAD"), date, "author date kept");
    assert_eq!(
        repo.git(&["log", "-1", "--format=%cn <%ce>"]),
        "Gitloom Test <test@example.com>"
    );
    // The message (with its co-author trailer) is untouched.
    assert_eq!(
        message(&repo, "HEAD"),
        "Two\n\nBody\n\nCo-authored-by: Cy <cy@example.com>"
    );
    assert_eq!(repo.git(&["status", "--porcelain"]), status);
    // One undo step back to the exact oid, redo forward.
    done.apply(&cli, true).unwrap();
    assert_eq!(repo.head(), head);
    done.apply(&cli, false).unwrap();
    assert_eq!(repo.head(), done.new_head);
}

#[test]
fn authors_of_two_commits_across_a_merge() {
    let (repo, cli) = repo();
    repo.commit("a.txt", "1\n", "c6");
    // The side branch forks before the edited commits: it stays.
    repo.branch("topic");
    let five = repo.commit("a.txt", "2\n", "c5 back");
    repo.commit("a.txt", "3\n", "c4");
    repo.checkout("topic");
    let side = repo.commit("t.txt", "t\n", "side");
    repo.checkout("main");
    repo.merge("topic", "Merge topic");
    let three = repo.commit("a.txt", "4\n", "c3 back");
    repo.commit("a.txt", "5\n", "c2");
    let head = repo.commit("a.txt", "6\n", "c1");
    let before = subjects(&repo);
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);

    let done = rewrite_unpushed(&cli, &[author_edit(&three), author_edit(&five)]).unwrap();
    assert_eq!(subjects(&repo), before, "same shape");
    assert_eq!(repo.git(&["rev-parse", "HEAD^{tree}"]), tree);
    let author_of = |subject: &str| {
        repo.git(&[
            "log",
            "--format=%an",
            "-1",
            "--grep",
            &format!("^{subject}$"),
        ])
    };
    assert_eq!(author_of("c5 back"), "Bo Kim");
    assert_eq!(author_of("c3 back"), "Bo Kim");
    assert_ne!(author_of("c4"), "Bo Kim");
    assert_ne!(author_of("c1"), "Bo Kim");
    // The merge was rebuilt; its second parent is the unchanged side.
    let merge = repo.git(&["log", "--format=%H", "-1", "--grep", "^Merge topic$"]);
    let parents = repo.parents(&merge);
    assert_eq!(parents.len(), 2);
    assert_eq!(parents[1], side);
    done.apply(&cli, true).unwrap();
    assert_eq!(repo.head(), head);
}

#[test]
fn message_and_author_in_one_step() {
    let (repo, cli) = repo();
    repo.commit("a.txt", "a\n", "One");
    let target = repo.commit("a.txt", "b\n", "Tpyo");
    let head = repo.commit("a.txt", "c\n", "Three");
    let done = rewrite_unpushed(
        &cli,
        &[CommitEdit {
            commit: target.clone(),
            message: Some("Typo fixed".into()),
            author: Some(bo()),
        }],
    )
    .unwrap();
    assert_ne!(repo.head(), head);
    assert_eq!(message(&repo, "HEAD~1"), "Typo fixed");
    assert_eq!(
        repo.git(&["log", "-1", "--format=%an <%ae>", "HEAD~1"]),
        "Bo Kim <bo@example.com>"
    );
    assert_eq!(done.new_commit, repo.git(&["rev-parse", "HEAD~1"]));
    assert_eq!(subjects(&repo), ["Three", "Typo fixed", "One"]);
}

#[test]
fn one_blocked_commit_refuses_the_whole_change() {
    let (repo, cli) = repo();
    let pushed = repo.commit("a.txt", "a\n", "One");
    repo.fake_upstream("origin", "main", &pushed);
    let free = repo.commit("a.txt", "b\n", "Two");
    let head = repo.head();
    let error = rewrite_unpushed(&cli, &[author_edit(&free), author_edit(&pushed)])
        .unwrap_err()
        .to_string();
    assert!(error.contains(&format!("{}: ", &pushed[..7])), "{error}");
    assert!(error.contains("Already pushed to origin/main"), "{error}");
    assert_eq!(repo.head(), head, "nothing changed");
    // Facts of several commits: refs that keep any old commit.
    repo.git(&["tag", "v1", &free]);
    let facts = edit_facts_many(&cli, &[free.clone(), head.clone()]).unwrap();
    assert_eq!(facts.keep_old, ["v1"]);
}
