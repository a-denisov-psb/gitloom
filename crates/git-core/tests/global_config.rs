//! Global Git config helpers (M10-03): `init.defaultBranch` is written to
//! and read from the global config. One test per binary: it points
//! `GIT_CONFIG_GLOBAL` at a temporary file for the whole process.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[test]
fn default_branch_is_written_read_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("gitconfig");
    std::fs::write(&file, "").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &file);

    assert_eq!(git_core::global_config("init.defaultBranch").unwrap(), None);
    git_core::set_global_config("init.defaultBranch", Some("trunk")).unwrap();
    assert_eq!(
        git_core::global_config("init.defaultBranch")
            .unwrap()
            .as_deref(),
        Some("trunk")
    );
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("defaultBranch = trunk"));
    git_core::set_global_config("init.defaultBranch", None).unwrap();
    assert_eq!(git_core::global_config("init.defaultBranch").unwrap(), None);
    // Removing a key that is not set is fine.
    git_core::set_global_config("init.defaultBranch", None).unwrap();
}
