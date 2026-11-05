//! ADR-028: the product name is spelled only in `identity.rs`, in tests and in
//! the config migration code (M6-02). Everything else goes through
//! `git_core::identity`.

use std::fs;
use std::path::{Path, PathBuf};

/// Files that may spell the name: the identity module and the migration code.
const ALLOWED: &[&str] = &["git-core/src/identity.rs", "app/src/migrate.rs"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Integration tests and build output are exempt.
            if path
                .file_name()
                .is_some_and(|n| n == "tests" || n == "target")
            {
                continue;
            }
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Offending lines of one source file: code before the first `#[cfg(test)]`,
/// comments excluded.
fn offences(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg(test)]") {
            break;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        if line.to_lowercase().contains("gitloom") {
            found.push((index + 1, line.trim().to_string()));
        }
    }
    found
}

#[test]
fn product_name_only_in_identity_module() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    rust_files(&crates, &mut files);
    assert!(
        files.len() > 20,
        "found too few source files: {}",
        files.len()
    );

    let mut problems = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&crates)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let relative = relative.trim_start_matches("./").to_string();
        if ALLOWED.iter().any(|allowed| relative.ends_with(allowed)) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };
        for (line, text) in offences(&source) {
            problems.push(format!("{relative}:{line}: {text}"));
        }
    }
    assert!(
        problems.is_empty(),
        "the product name must come from git_core::identity:\n{}",
        problems.join("\n")
    );
}

#[test]
fn guard_detects_code_but_not_comments_or_tests() {
    assert_eq!(offences("let a = \"Gitloom\";").len(), 1);
    assert!(offences("/// Gitloom docs\n// gitloom note").is_empty());
    assert!(offences("fn x() {}\n#[cfg(test)]\nmod t { \"gitloom\" }").is_empty());
}
