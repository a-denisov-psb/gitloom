//! Embeds the Git commit of the build as `BUILD_GIT_COMMIT` (About dialog,
//! crash reports). `BUILD_GIT_COMMIT` in the environment overrides it (CI,
//! source archives without `.git`); without either the value is "unknown".
//!
//! For Windows targets it also compiles the version resource with the app
//! icon (M6-04). The product name is read from `git_core::identity` (ADR-028).

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resource(&manifest)?;
    }
    Ok(())
}

/// Version resource (product name, description, version, copyright) and the
/// exe icon. The version comes from `CARGO_PKG_VERSION`.
fn windows_resource(manifest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let identity = manifest.join("../git-core/src/identity.rs");
    let icon = manifest.join("assets/app-icon/icon.ico");
    println!("cargo:rerun-if-changed={}", identity.display());
    println!("cargo:rerun-if-changed={}", icon.display());
    let source = std::fs::read_to_string(&identity)?;
    let name = string_const(&source, "DISPLAY_NAME").ok_or("DISPLAY_NAME not found")?;
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(&icon.to_string_lossy())
        .set("ProductName", &name)
        .set("FileDescription", &name)
        .set("InternalName", &name)
        .set(
            "LegalCopyright",
            &format!("Copyright © 2026 the {name} contributors. GPL-3.0-or-later."),
        );
    resource.compile()?;
    Ok(())
}

/// Value of `pub const <NAME>: &str = "...";` in `source`.
fn string_const(source: &str, name: &str) -> Option<String> {
    let prefix = format!("pub const {name}: &str = \"");
    source.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix("\";"))
            .map(str::to_owned)
    })
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
/// packed refs. Only existing files are watched; a missing one would make
/// Cargo re-run the script on every build.
fn watch_head(git_dir: &Path) {
    let head = git_dir.join("HEAD");
    let mut watched = vec![head.clone(), git_dir.join("packed-refs")];
    if let Ok(content) = std::fs::read_to_string(&head) {
        if let Some(reference) = content.trim().strip_prefix("ref: ") {
            watched.push(git_dir.join(reference));
        }
    }
    for path in watched.iter().filter(|p| p.exists()) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
