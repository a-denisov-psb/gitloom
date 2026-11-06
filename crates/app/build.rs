//! Embeds the Git commit of the build as `BUILD_GIT_COMMIT` (About dialog,
//! crash reports). `BUILD_GIT_COMMIT` in the environment overrides it (CI,
//! source archives without `.git`); without either the value is "unknown".

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=BUILD_GIT_COMMIT");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    if let Some(git_dir) = git_dir(&manifest) {
        watch_head(&git_dir);
    }
    let commit = std::env::var("BUILD_GIT_COMMIT")
        .ok()
        .filter(|c| !c.trim().is_empty())
        .or_else(|| head_commit(&manifest))
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=BUILD_GIT_COMMIT={}", commit.trim());
}

fn head_commit(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=10", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|c| !c.is_empty())
}

fn git_dir(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

/// Re-run when HEAD moves: HEAD itself, the branch it points to and
/// packed refs.
fn watch_head(git_dir: &Path) {
    let head = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );
    if let Ok(content) = std::fs::read_to_string(&head) {
        if let Some(reference) = content.trim().strip_prefix("ref: ") {
            println!(
                "cargo:rerun-if-changed={}",
                git_dir.join(reference).display()
            );
        }
    }
}
