mod support;

use support::TestRepo;

/// Builds `main: a - b - d - M` with `feature: b - c` merged into `M`.
fn build_merge_history(repo: &TestRepo) -> [String; 5] {
    let a = repo.commit("README.md", "hello\n", "Initial commit");
    let b = repo.commit("README.md", "hello world\n", "Extend readme");
    repo.branch("feature");
    repo.checkout("feature");
    let c = repo.commit("src/feature.txt", "feature\n", "Add feature");
    repo.checkout("main");
    let d = repo.commit("main.txt", "main\n", "Work on main");
    let m = repo.merge("feature", "Merge branch 'feature'");
    [a, b, c, d, m]
}

#[test]
fn builds_history_with_branch_and_merge() {
    let repo = TestRepo::new();
    let [a, b, c, d, m] = build_merge_history(&repo);

    assert_eq!(repo.parents(&a), Vec::<String>::new());
    assert_eq!(repo.parents(&b), vec![a]);
    assert_eq!(repo.parents(&c), vec![b.clone()]);
    assert_eq!(repo.parents(&d), vec![b]);
    assert_eq!(repo.parents(&m), vec![d, c]);

    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "5");
    assert_eq!(
        repo.git(&["branch", "--format=%(refname:short)"]),
        "feature\nmain"
    );
    assert_eq!(repo.git(&["branch", "--show-current"]), "main");
}

#[test]
fn commit_ids_are_deterministic() {
    let first = build_merge_history(&TestRepo::new());
    let second = build_merge_history(&TestRepo::new());
    assert_eq!(first, second);
}

#[test]
fn uses_fixed_identity_and_dates() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "First");
    repo.commit("b.txt", "b\n", "Second");

    assert_eq!(
        repo.git(&["log", "--format=%an <%ae> %at|%cn <%ce> %ct"]),
        "Test Author <author@example.com> 1700000060|Test Committer <committer@example.com> 1700000060\n\
         Test Author <author@example.com> 1700000000|Test Committer <committer@example.com> 1700000000"
    );
}
