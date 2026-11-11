//! Commit template file and options (M10-06). One test: it points
//! `GIT_CONFIG_GLOBAL` at a temporary file for the whole process.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::commit_template::{read_template, save_template, set_template_option};
use git_core::{GitBackend, GixBackend};
use support::TestRepo;

#[test]
fn template_is_saved_read_and_options_follow_the_config() {
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("gitconfig");
    std::fs::write(&global, "").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &global);

    // No commit.template: saving creates the file in .git/gitloom and the
    // repository config entry.
    let repo = TestRepo::new();
    let settings = read_template(repo.path()).unwrap();
    assert_eq!(settings.file, None);
    assert!(settings.options.apply && settings.options.remove_comments);
    let path = save_template(repo.path(), "Fix: ", "Why:\n# hint", false).unwrap();
    assert!(
        path.ends_with(".git/gitloom/commit-template.txt"),
        "{path:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "Fix:\n\nWhy:\n# hint\n"
    );
    let settings = read_template(repo.path()).unwrap();
    assert_eq!(settings.file.as_ref().unwrap().scope, "local");
    assert_eq!(settings.content, "Fix:\n\nWhy:\n# hint\n");
    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(
        backend.commit_template().unwrap().as_deref(),
        Some("Fix:\n\nWhy:\n# hint\n")
    );

    // An existing local template file is edited in place.
    let other = TestRepo::new();
    let own = other.path().join("my-template.txt");
    std::fs::write(&own, "old\n").unwrap();
    other.git(&["config", "commit.template", own.to_str().unwrap()]);
    let written = save_template(other.path(), "New", "", false).unwrap();
    assert_eq!(written, own);
    assert_eq!(std::fs::read_to_string(&own).unwrap(), "New\n");

    // A global template is changed only after confirmation.
    let shared = dir.path().join("global-template.txt");
    std::fs::write(&shared, "global\n").unwrap();
    let third = TestRepo::new();
    std::process::Command::new("git")
        .args([
            "config",
            "--global",
            "commit.template",
            shared.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    let settings = read_template(third.path()).unwrap();
    assert!(settings.file.as_ref().unwrap().shared());
    assert!(save_template(third.path(), "Changed", "", false).is_err());
    assert_eq!(std::fs::read_to_string(&shared).unwrap(), "global\n");
    save_template(third.path(), "Changed", "", true).unwrap();
    assert_eq!(std::fs::read_to_string(&shared).unwrap(), "Changed\n");

    // Options and a custom comment character, read by both paths.
    set_template_option(repo.path(), true, false).unwrap();
    set_template_option(repo.path(), false, false).unwrap();
    repo.git(&["config", "core.commentChar", ";"]);
    let options = read_template(repo.path()).unwrap().options;
    assert!(!options.apply && !options.remove_comments);
    assert_eq!(options.comment_char, ";");
    let options = GixBackend::discover(repo.path())
        .unwrap()
        .commit_template_options()
        .unwrap();
    assert!(!options.apply && !options.remove_comments);
    assert_eq!(options.comment_char, ";");
}
