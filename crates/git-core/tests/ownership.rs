//! Git's "dubious ownership" refusal and "Trust this repository…" (M12-14).
//! Its own test binary: it changes the process environment.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[cfg(unix)]
mod support;

/// Unix: Git for Windows checks ownership by SID and ignores the test
/// variable.
#[cfg(unix)]
#[test]
fn trusting_a_repository_owned_by_another_user() {
    let repo = support::TestRepo::new();
    repo.commit("a.txt", "a\n", "base");
    let global = tempfile::NamedTempFile::new().unwrap();
    // Git ≥ 2.35.2 treats the repository as owned by someone else; the
    // global config is a temporary file.
    std::env::set_var("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1");
    std::env::set_var("GIT_CONFIG_GLOBAL", global.path());
    let error = git_core::check_repository_access(repo.path())
        .unwrap_err()
        .to_string();
    let path = git_core::dubious_ownership(&error)
        .unwrap_or_else(|| panic!("{error}"))
        .to_owned();
    assert_eq!(
        git_core::explain::explain(&error).map(|e| e.title),
        Some("Repository not trusted")
    );
    git_core::trust_directory(&path).unwrap();
    let written = std::fs::read_to_string(global.path()).unwrap();
    assert!(
        written.contains("safe") && written.contains(&path),
        "{written}"
    );
    git_core::check_repository_access(repo.path()).unwrap();
}
