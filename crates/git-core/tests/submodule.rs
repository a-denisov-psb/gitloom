//! Submodule status and update (M12-06) against local repositories.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::submodule::{submodules, update_args, SubmoduleState};
use git_core::{CliBackend, GitWriter as _, RunOptions};
use support::TestRepo;

/// Local clones need the file protocol, which Git restricts for submodules.
const ALLOW_FILE: [&str; 2] = ["-c", "protocol.file.allow=always"];

fn url(repo: &TestRepo) -> String {
    repo.path().to_string_lossy().replace('\\', "/")
}

/// A superproject with `libs/lib`, which has its own submodule `inner`.
fn superproject() -> (TestRepo, TestRepo, TestRepo) {
    let inner = TestRepo::new();
    inner.commit("inner.txt", "inner\n", "inner");
    let lib = TestRepo::new();
    lib.commit("lib.txt", "lib\n", "lib");
    lib.git(&[
        ALLOW_FILE[0],
        ALLOW_FILE[1],
        "submodule",
        "add",
        "--quiet",
        &url(&inner),
        "inner",
    ]);
    lib.git(&["commit", "--quiet", "-m", "add inner"]);
    let top = TestRepo::new();
    top.commit("README.md", "top\n", "top");
    top.git(&[
        ALLOW_FILE[0],
        ALLOW_FILE[1],
        "submodule",
        "add",
        "--quiet",
        &url(&lib),
        "libs/lib",
    ]);
    top.git(&["commit", "--quiet", "-m", "add lib"]);
    top.git(&[
        ALLOW_FILE[0],
        ALLOW_FILE[1],
        "submodule",
        "update",
        "--init",
        "--recursive",
        "--quiet",
    ]);
    (top, lib, inner)
}

fn states(repo: &TestRepo) -> Vec<(String, SubmoduleState)> {
    submodules(&CliBackend::new(repo.path()), repo.path())
        .unwrap()
        .into_iter()
        .map(|s| (s.path, s.state))
        .collect()
}

#[test]
fn nested_submodules_modified_and_out_of_date() {
    let (top, _lib, _inner) = superproject();
    assert_eq!(
        states(&top),
        [
            ("libs/lib".to_owned(), SubmoduleState::Current),
            ("libs/lib/inner".to_owned(), SubmoduleState::Current),
        ]
    );
    // Local changes in the nested submodule.
    std::fs::write(top.path().join("libs/lib/inner/inner.txt"), "changed\n").unwrap();
    assert_eq!(states(&top)[1].1, SubmoduleState::Modified);
    // A new commit checked out in lib: the superproject records another one.
    let lib_dir = top.path().join("libs/lib");
    std::fs::write(lib_dir.join("lib.txt"), "newer\n").unwrap();
    top.git_in(&lib_dir, &["commit", "--quiet", "-am", "newer"]);
    assert_eq!(states(&top)[0].1, SubmoduleState::OutOfDate);
}

#[test]
fn not_initialized_until_updated() {
    let (top, _lib, _inner) = superproject();
    let clone = TestRepo::new();
    let target = clone.path().join("clone");
    clone.git(&["clone", "--quiet", &url(&top), &target.to_string_lossy()]);
    let git = CliBackend::new(&target);
    let list = submodules(&git, &target).unwrap();
    assert_eq!(
        list.len(),
        1,
        "nested ones appear once the parent is cloned"
    );
    assert_eq!(list[0].state, SubmoduleState::NotInitialized);
    let args = update_args(&["libs/lib".into()]);
    let mut all: Vec<&str> = ALLOW_FILE.to_vec();
    all.extend(args.iter().map(String::as_str));
    git.run(&all, RunOptions::default()).unwrap();
    let list = submodules(&git, &target).unwrap();
    let states: Vec<SubmoduleState> = list.iter().map(|s| s.state).collect();
    assert_eq!(states, [SubmoduleState::Current, SubmoduleState::Current]);
}

#[test]
fn no_gitmodules_no_submodules() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "a");
    assert!(states(&repo).is_empty());
}
