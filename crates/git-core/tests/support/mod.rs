//! Test support: deterministic temporary repositories built with the `git` CLI.
//!
//! Include from an integration test with `mod support;`.

// Each test binary uses a different subset of the helpers.
#![allow(dead_code)]
// Test-only code: failing loudly is the desired behavior. Clippy's
// `allow-*-in-tests` only covers `#[test]` functions, not shared helpers.
#![allow(clippy::expect_used, clippy::panic)]

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// Commit timestamp of the first commit (2023-11-14 22:13:20 UTC).
const START_TIME: i64 = 1_700_000_000;
/// Seconds between two consecutive commits.
const TIME_STEP: i64 = 60;

/// A throwaway repository whose commits are fully deterministic:
/// fixed identity, fixed dates and no system or user configuration.
pub struct TestRepo {
    // Owns the temp directory; deleted on drop.
    _dir: TempDir,
    home: PathBuf,
    root: PathBuf,
    clock: Cell<i64>,
}

impl TestRepo {
    /// Creates an empty repository with `main` as initial branch.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        let home = dir.path().join("home");
        let root = dir.path().join("repo");
        fs::create_dir_all(&home).expect("create home dir");
        fs::create_dir_all(&root).expect("create repo dir");
        let repo = Self {
            _dir: dir,
            home,
            root,
            clock: Cell::new(START_TIME),
        };
        repo.git(&["init", "--quiet", "--initial-branch=main"]);
        repo
    }

    /// Working tree root of the repository.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Runs `git` in the repository root and returns trimmed stdout.
    /// Panics with stderr if the command fails.
    pub fn git(&self, args: &[&str]) -> String {
        self.git_in(&self.root, args)
    }

    /// Runs `git` in `dir` with the deterministic environment.
    pub fn git_in(&self, dir: &Path, args: &[&str]) -> String {
        let output = self.run_git(dir, args);
        assert!(
            output.status.success(),
            "git {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// Runs `git` in the repository root and returns raw stdout (not trimmed).
    pub fn git_bytes(&self, args: &[&str]) -> Vec<u8> {
        let output = self.run_git(&self.root, args);
        assert!(
            output.status.success(),
            "git {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    /// Runs `git` in the repository root and returns whether it succeeded,
    /// for commands that are expected to fail (e.g. conflicting merges).
    pub fn try_git(&self, args: &[&str]) -> bool {
        self.run_git(&self.root, args).status.success()
    }

    fn run_git(&self, dir: &Path, args: &[&str]) -> Output {
        let date = format!("{} +0000", self.clock.get());
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_OBJECT_DIRECTORY")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.home)
            .env("LC_ALL", "C")
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_NAME", "Test Committer")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .env("GIT_COMMITTER_DATE", &date)
            .output()
            .expect("run git (is it installed?)")
    }

    /// Writes `content` to `rel` (parent directories are created).
    pub fn write(&self, rel: &str, content: &str) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, content).expect("write file");
    }

    /// Writes a file, stages it and commits. Returns the new commit id.
    pub fn commit(&self, rel: &str, content: &str, message: &str) -> String {
        self.write(rel, content);
        self.git(&["add", "--", rel]);
        self.git(&["commit", "--quiet", "-m", message]);
        self.tick();
        self.head()
    }

    /// Creates a branch at HEAD without switching to it.
    pub fn branch(&self, name: &str) {
        self.git(&["branch", name]);
    }

    /// Switches to an existing branch.
    pub fn checkout(&self, name: &str) {
        self.git(&["switch", "--quiet", name]);
    }

    /// Merges `branch` into the current branch, always creating a merge commit.
    /// Returns the merge commit id.
    pub fn merge(&self, branch: &str, message: &str) -> String {
        self.git(&["merge", "--quiet", "--no-ff", "-m", message, branch]);
        self.tick();
        self.head()
    }

    /// Full id of the commit HEAD points to.
    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    /// Parent ids of `rev`, in order.
    pub fn parents(&self, rev: &str) -> Vec<String> {
        self.git(&["rev-list", "--parents", "-n", "1", rev])
            .split_whitespace()
            .skip(1)
            .map(str::to_owned)
            .collect()
    }

    fn tick(&self) {
        self.clock.set(self.clock.get() + TIME_STEP);
    }
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRepo {
    /// Creates a bare repository (no working tree).
    pub fn new_bare() -> Self {
        let repo = Self::new();
        repo.git(&["config", "core.bare", "true"]);
        repo
    }

    /// Creates an annotated tag at HEAD.
    pub fn tag_annotated(&self, name: &str, message: &str) {
        self.git(&["tag", "-a", name, "-m", message]);
    }

    /// Creates a lightweight tag at HEAD.
    pub fn tag(&self, name: &str) {
        self.git(&["tag", name]);
    }

    /// Fakes a remote-tracking branch `refs/remotes/<remote>/<branch>` at `rev`
    /// and configures `branch` to track it.
    pub fn fake_upstream(&self, remote: &str, branch: &str, rev: &str) {
        let fetch = format!("+refs/heads/*:refs/remotes/{remote}/*");
        self.git(&["config", &format!("remote.{remote}.url"), "."]);
        self.git(&["config", &format!("remote.{remote}.fetch"), &fetch]);
        self.git(&["config", &format!("branch.{branch}.remote"), remote]);
        self.git(&[
            "config",
            &format!("branch.{branch}.merge"),
            &format!("refs/heads/{branch}"),
        ]);
        self.git(&[
            "update-ref",
            &format!("refs/remotes/{remote}/{branch}"),
            rev,
        ]);
    }
}
