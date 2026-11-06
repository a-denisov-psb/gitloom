//! M6-05: the debug-only panic trigger runs the real binary's crash hook and
//! leaves a report in the given directory.

#![allow(clippy::unwrap_used)]

use std::process::Command;

#[test]
fn panic_trigger_writes_a_crash_report() {
    let dir = tempfile::tempdir().unwrap();
    let env_name = env_name();
    let output = Command::new(env!("CARGO_BIN_EXE_gitloom"))
        .env(&env_name, dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success(), "the trigger must panic");
    let reports: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect();
    assert_eq!(reports.len(), 1, "{reports:?}");
    let text = std::fs::read_to_string(&reports[0]).unwrap();
    assert!(text.contains("Thread:    main"), "{text}");
    assert!(text.contains("debug panic trigger"), "{text}");
    assert!(text.contains(&format!("Version:   {}", env!("CARGO_PKG_VERSION"))));
    assert!(text.contains("Backtrace:"));
}

/// `git_core::identity::DEBUG_PANIC_ENV` (the binary crate exports nothing).
fn env_name() -> String {
    let source = include_str!("../../git-core/src/identity.rs");
    source
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("pub const DEBUG_PANIC_ENV: &str = \"")?
                .strip_suffix("\";")
                .map(str::to_owned)
        })
        .unwrap()
}
