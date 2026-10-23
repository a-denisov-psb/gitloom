//! Write access through the `git` executable (ADR-013).
//!
//! Every command runs with an argument list (never through a shell), in the
//! repository root, without prompts or editors, so hooks, signing and config
//! behave exactly like plain Git.

use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{CancelToken, Error, Progress, Result};

/// Oldest supported Git (Ubuntu 22.04 LTS ships 2.34).
pub const MIN_GIT_VERSION: GitVersion = GitVersion {
    major: 2,
    minor: 34,
    patch: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl GitVersion {
    /// Parses `git --version` output, e.g. `git version 2.45.1.windows.1`
    /// or `git version 2.39.5 (Apple Git-154)`.
    pub fn parse(output: &str) -> Option<Self> {
        let rest = output.trim().strip_prefix("git version ")?;
        let number = rest.split_whitespace().next()?;
        let mut parts = number.split('.').map(|p| {
            // Release candidates look like `2.48.0-rc1`.
            let digits: String = p.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().ok()
        });
        let major = parts.next()??;
        let minor = parts.next()??;
        let patch = parts.next().flatten().unwrap_or(0);
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

/// Why write actions are unavailable, if they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitAvailability {
    Available(GitVersion),
    Missing,
    TooOld(GitVersion),
}

impl GitAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available(_))
    }

    /// User-facing reason why writing is disabled.
    pub fn reason(&self) -> Option<String> {
        match self {
            Self::Available(_) => None,
            Self::Missing => Some("Git was not found on PATH; Gitloom is read-only".into()),
            Self::TooOld(version) => Some(format!(
                "Git {version} is too old (needs {MIN_GIT_VERSION} or newer); Gitloom is read-only"
            )),
        }
    }
}

/// Looks for `git` on `PATH` and checks its version. Blocking.
pub fn detect_git() -> GitAvailability {
    let output = match command(Path::new("git")).arg("--version").output() {
        Ok(output) if output.status.success() => output,
        _ => return GitAvailability::Missing,
    };
    match GitVersion::parse(&String::from_utf8_lossy(&output.stdout)) {
        Some(version) if version >= MIN_GIT_VERSION => GitAvailability::Available(version),
        Some(version) => GitAvailability::TooOld(version),
        None => GitAvailability::Missing,
    }
}

/// Output of a successful command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl GitOutput {
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

/// How a command is run.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions<'a> {
    /// Data written to the command's stdin.
    pub stdin: Option<&'a [u8]>,
    /// Read-like helper call: do not take optional locks (`GIT_OPTIONAL_LOCKS=0`).
    pub read_only: bool,
}

/// Write side of a repository. Implemented by [`CliBackend`]; operations are
/// added per milestone task.
pub trait GitWriter: Send + Sync {
    /// Runs `git` with `args` in the repository root.
    fn run(&self, args: &[&str], options: RunOptions<'_>) -> Result<GitOutput>;

    /// Runs a long (network) command: reports progress parsed from stderr
    /// (pass `--progress` in `args`) and stops the process tree when `cancel`
    /// is set (then fails with [`Error::Cancelled`]). The default ignores both.
    fn run_network(
        &self,
        args: &[&str],
        cancel: &CancelToken,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<GitOutput> {
        let _ = (cancel, progress);
        self.run(args, RunOptions::default())
    }

    /// True if HEAD points to a commit (false before the first commit).
    fn has_head(&self) -> Result<bool> {
        let read = RunOptions {
            read_only: true,
            ..RunOptions::default()
        };
        match self.run(&["rev-parse", "--verify", "--quiet", "HEAD"], read) {
            Ok(_) => Ok(true),
            Err(Error::Command { code: Some(1), .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Stages `paths` including deletions; staging a conflicted file marks it resolved.
    fn stage(&self, paths: &[String]) -> Result<()> {
        with_pathspecs(self, &["add", "--all"], paths)
    }

    /// Removes `paths` from the index (back to HEAD, or untracked before the first commit).
    fn unstage(&self, paths: &[String]) -> Result<()> {
        if self.has_head()? {
            with_pathspecs(self, &["restore", "--staged"], paths)
        } else {
            with_pathspecs(self, &["rm", "--cached", "-r", "--quiet"], paths)
        }
    }

    /// Applies `patch` to the index (`reverse` for unstaging).
    fn apply_cached(&self, patch: &str, reverse: bool) -> Result<()> {
        let mut args = vec!["apply", "--cached", "--whitespace=nowarn"];
        if reverse {
            args.push("--reverse");
        }
        args.push("-");
        self.run(
            &args,
            RunOptions {
                stdin: Some(patch.as_bytes()),
                read_only: false,
            },
        )
        .map(drop)
    }

    /// Writes `data` as a blob without filters and returns its id.
    fn write_blob(&self, data: &[u8]) -> Result<String> {
        let output = self.run(
            &["hash-object", "-w", "--no-filters", "--stdin"],
            RunOptions {
                stdin: Some(data),
                read_only: false,
            },
        )?;
        Ok(output.stdout_text().trim().to_owned())
    }

    /// Writes the working tree files `paths` as blobs (filters applied like
    /// `git add`) and returns their ids in order.
    fn write_worktree_blobs(&self, paths: &[String]) -> Result<Vec<String>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let input: String = paths.iter().map(|p| format!("{p}\n")).collect();
        let output = self.run(
            &["hash-object", "-w", "--stdin-paths"],
            RunOptions {
                stdin: Some(input.as_bytes()),
                read_only: false,
            },
        )?;
        Ok(output.stdout_text().lines().map(str::to_owned).collect())
    }

    /// Blob `id` converted to working tree form for `path` (eol, smudge filters).
    fn blob_for_worktree(&self, path: &str, id: &str) -> Result<Vec<u8>> {
        let path_arg = format!("--path={path}");
        Ok(self
            .run(
                &["cat-file", "--filters", &path_arg, id],
                RunOptions {
                    read_only: true,
                    ..RunOptions::default()
                },
            )?
            .stdout)
    }

    /// Makes object `id` available locally. In a partial clone Git fetches
    /// a missing object from the promisor remote (network access).
    fn fetch_object(&self, id: &str) -> Result<()> {
        self.run(
            &["cat-file", "blob", id],
            RunOptions {
                read_only: true,
                ..RunOptions::default()
            },
        )
        .map(drop)
    }

    /// Resets index and working tree of `paths` to HEAD; paths not in HEAD
    /// are removed from both.
    fn restore_from_head(&self, paths: &[String]) -> Result<()> {
        with_pathspecs(
            self,
            &["restore", "--source=HEAD", "--staged", "--worktree"],
            paths,
        )
    }

    /// Stage-0 index entries as `(path, mode, object id)`.
    fn index_entries(&self) -> Result<Vec<(String, String, String)>> {
        let output = self.run(
            &["ls-files", "--stage", "-z"],
            RunOptions {
                read_only: true,
                ..RunOptions::default()
            },
        )?;
        Ok(output
            .stdout
            .split(|&b| b == 0)
            .filter_map(|record| {
                let record = std::str::from_utf8(record).ok()?;
                let (meta, path) = record.split_once('\t')?;
                let mut fields = meta.split(' ');
                let (mode, id, stage) = (fields.next()?, fields.next()?, fields.next()?);
                (stage == "0").then(|| (path.to_owned(), mode.to_owned(), id.to_owned()))
            })
            .collect())
    }

    /// Sets index entries: `Some((mode, id))` adds or replaces, `None` removes.
    fn set_index_entries(&self, entries: &[(String, Option<(String, String)>)]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let null = if entries.iter().any(|(_, e)| e.is_none()) {
            let read = RunOptions {
                read_only: true,
                ..RunOptions::default()
            };
            let format = self.run(&["rev-parse", "--show-object-format"], read)?;
            let len = if format.stdout_text().trim() == "sha256" {
                64
            } else {
                40
            };
            "0".repeat(len)
        } else {
            String::new()
        };
        let mut input = Vec::new();
        for (path, entry) in entries {
            let line = match entry {
                Some((mode, id)) => format!("{mode} {id}\t{path}"),
                None => format!("0 {null}\t{path}"),
            };
            input.extend_from_slice(line.as_bytes());
            input.push(0);
        }
        self.run(
            &["update-index", "-z", "--index-info"],
            RunOptions {
                stdin: Some(&input),
                read_only: false,
            },
        )
        .map(drop)
    }

    /// Creates branch `name` at commit `start` and switches to it; Git
    /// validates the name first (`check-ref-format --branch`).
    fn create_branch(&self, name: &str, start: &str) -> Result<()> {
        let read = RunOptions {
            read_only: true,
            ..RunOptions::default()
        };
        self.run(&["check-ref-format", "--branch", name], read)
            .map_err(|_| Error::Backend(format!("\"{name}\" is not a valid branch name")))?;
        self.run(
            &["switch", "--no-guess", "--create", name, start, "--"],
            RunOptions::default(),
        )
        .map(drop)
    }

    /// Renames local branch `old` to `new` (config and upstream move along).
    fn rename_branch(&self, old: &str, new: &str) -> Result<()> {
        let read = RunOptions {
            read_only: true,
            ..RunOptions::default()
        };
        self.run(&["check-ref-format", "--branch", new], read)
            .map_err(|_| Error::Backend(format!("\"{new}\" is not a valid branch name")))?;
        self.run(&["branch", "--move", old, new], RunOptions::default())
            .map(drop)
    }

    /// Deletes local branch `name`; without `force` Git refuses unmerged
    /// branches (see [`branch_not_merged`]).
    fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        let flag = if force { "-D" } else { "-d" };
        self.run(&["branch", flag, name], RunOptions::default())
            .map(drop)
    }

    /// Switches to local branch `branch`.
    fn switch_branch(&self, branch: &str) -> Result<()> {
        self.run(
            &["switch", "--no-guess", branch, "--"],
            RunOptions::default(),
        )
        .map(drop)
    }

    /// Creates local branch `local` tracking remote branch `remote_ref`
    /// (e.g. `origin/topic`) and switches to it.
    fn switch_tracking(&self, local: &str, remote_ref: &str) -> Result<()> {
        self.run(
            &["switch", "--create", local, "--track", remote_ref, "--"],
            RunOptions::default(),
        )
        .map(drop)
    }

    /// Checks out commit `id` as detached HEAD.
    fn switch_detached(&self, id: &str) -> Result<()> {
        self.run(&["switch", "--detach", id, "--"], RunOptions::default())
            .map(drop)
    }

    /// Aborts a merge in progress (`git merge --abort`).
    fn abort_merge(&self) -> Result<()> {
        self.run(&["merge", "--abort"], RunOptions::default())
            .map(drop)
    }

    /// Aborts a rebase in progress (`git rebase --abort`).
    fn abort_rebase(&self) -> Result<()> {
        self.run(&["rebase", "--abort"], RunOptions::default())
            .map(drop)
    }

    /// Resets the working tree files `paths` to their index content.
    fn restore_worktree(&self, paths: &[String]) -> Result<()> {
        with_pathspecs(self, &["restore", "--worktree"], paths)
    }

    /// Commits the index with `message` (passed via stdin, hooks run as usual).
    /// `amend` replaces the HEAD commit.
    fn commit(&self, message: &str, amend: bool) -> Result<()> {
        let mut args = vec!["commit", "--file=-"];
        if amend {
            args.push("--amend");
        }
        self.run(
            &args,
            RunOptions {
                stdin: Some(message.as_bytes()),
                read_only: false,
            },
        )
        .map(drop)
    }

    /// Stashes all changes including untracked files (`git stash push -u`).
    fn stash_push(&self, message: Option<&str>) -> Result<()> {
        let mut args = vec!["stash", "push", "--include-untracked", "--quiet"];
        if let Some(message) = message {
            args.extend(["--message", message]);
        }
        self.run(&args, RunOptions::default()).map(drop)
    }

    /// Current `stash@{n}` name of the stash with commit `id`.
    fn stash_name(&self, id: &str) -> Result<String> {
        let output = self.run(
            &["stash", "list", "--format=%H"],
            RunOptions {
                read_only: true,
                ..RunOptions::default()
            },
        )?;
        output
            .stdout_text()
            .lines()
            .position(|line| line.trim() == id)
            .map(|index| format!("stash@{{{index}}}"))
            .ok_or_else(|| Error::NotFound(format!("stash {id}")))
    }

    /// Applies and removes the stash `id`; on conflicts Git keeps the stash.
    fn stash_pop(&self, id: &str) -> Result<()> {
        let name = self.stash_name(id)?;
        self.run(&["stash", "pop", "--quiet", &name], RunOptions::default())
            .map(drop)
    }

    fn stash_apply(&self, id: &str) -> Result<()> {
        let name = self.stash_name(id)?;
        self.run(&["stash", "apply", "--quiet", &name], RunOptions::default())
            .map(drop)
    }

    fn stash_drop(&self, id: &str) -> Result<()> {
        let name = self.stash_name(id)?;
        self.run(&["stash", "drop", "--quiet", &name], RunOptions::default())
            .map(drop)
    }

    fn stage_all(&self) -> Result<()> {
        self.run(&["add", "--all"], RunOptions::default()).map(drop)
    }

    fn unstage_all(&self) -> Result<()> {
        if self.has_head()? {
            self.run(&["reset", "--quiet"], RunOptions::default())
        } else {
            self.run(
                &["rm", "--cached", "-r", "--quiet", "."],
                RunOptions::default(),
            )
        }
        .map(drop)
    }
}

/// Runs `args` with `paths` passed NUL-separated on stdin, which avoids
/// command line length limits; no-op without paths.
fn with_pathspecs<W: GitWriter + ?Sized>(
    writer: &W,
    args: &[&str],
    paths: &[String],
) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    // Paths we pass are file names, never glob patterns. Only for these
    // commands: Git itself relies on pathspec magic internally (e.g. stash -u).
    let mut full = vec!["--literal-pathspecs"];
    full.extend_from_slice(args);
    full.extend(["--pathspec-from-file=-", "--pathspec-file-nul"]);
    writer
        .run(
            &full,
            RunOptions {
                stdin: Some(&input),
                read_only: false,
            },
        )
        .map(drop)
}

/// True if a checkout failed because local changes would be overwritten
/// (they can be stashed first).
pub fn checkout_blocked(stderr: &str) -> bool {
    stderr.contains("would be overwritten by checkout")
        || stderr.contains("untracked working tree files would be overwritten")
        || stderr.contains("Please commit your changes or stash them")
}

/// True if `git branch -d` refused because the branch is not merged.
pub fn branch_not_merged(stderr: &str) -> bool {
    stderr.contains("is not fully merged")
}

/// Environment variable that tells a program started as `GIT_ASKPASS` /
/// `SSH_ASKPASS` that it runs in askpass mode (Git passes only the prompt).
pub const ASKPASS_ENV: &str = "GITLOOM_ASKPASS";

/// Runs the `git` executable in one repository.
#[derive(Debug, Clone)]
pub struct CliBackend {
    git: PathBuf,
    root: PathBuf,
    /// Program answering credential and host key prompts (ADR-019).
    askpass: Option<PathBuf>,
}

impl CliBackend {
    /// Backend for the repository whose working tree root is `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            git: PathBuf::from("git"),
            root: root.into(),
            askpass: None,
        }
    }

    /// Prompts for credentials (when no credential helper or ssh-agent can
    /// answer) and host key confirmations go to `program`: it receives the
    /// prompt as only argument, with [`ASKPASS_ENV`] set, and prints the answer.
    pub fn with_askpass(mut self, program: impl Into<PathBuf>) -> Self {
        self.askpass = Some(program.into());
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl GitWriter for CliBackend {
    fn run_network(
        &self,
        args: &[&str],
        cancel: &CancelToken,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<GitOutput> {
        self.network(args, cancel, progress)
    }

    fn run(&self, args: &[&str], options: RunOptions<'_>) -> Result<GitOutput> {
        let mut cmd = self.command();
        cmd.args(args).current_dir(&self.root);
        if options.read_only {
            cmd.env("GIT_OPTIONAL_LOCKS", "0");
        }
        cmd.stdin(if options.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let display = format!("git {}", args.join(" "));
        let mut child = cmd
            .spawn()
            .map_err(|e| Error::Backend(format!("could not run {display}: {e}")))?;
        if let (Some(data), Some(mut stdin)) = (options.stdin, child.stdin.take()) {
            // Write on a separate thread so a large stdin cannot deadlock against full stdout.
            let data = data.to_vec();
            let writer = std::thread::spawn(move || stdin.write_all(&data));
            let output = child.wait_with_output();
            let written = writer.join();
            let output =
                output.map_err(|e| Error::Backend(format!("could not run {display}: {e}")))?;
            if let Ok(Err(e)) = written {
                // A command that exits early closes stdin; report its own error first.
                if output.status.success() {
                    return Err(Error::Backend(format!("could not write to {display}: {e}")));
                }
            }
            return finish(display, output);
        }
        let output = child
            .wait_with_output()
            .map_err(|e| Error::Backend(format!("could not run {display}: {e}")))?;
        finish(display, output)
    }
}

impl CliBackend {
    /// `git` command with the fixed environment and the askpass program.
    fn command(&self) -> Command {
        let mut cmd = command(&self.git);
        if let Some(askpass) = &self.askpass {
            cmd.env("GIT_ASKPASS", askpass)
                .env("SSH_ASKPASS", askpass)
                // OpenSSH >= 8.4: use askpass even without a display.
                .env("SSH_ASKPASS_REQUIRE", "force")
                .env(ASKPASS_ENV, "1");
        }
        cmd
    }

    fn spawn_network(&self, args: &[&str]) -> Result<std::process::Child> {
        let mut cmd = self.command();
        cmd.args(args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Own process group, so cancelling also stops remote helpers and ssh.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            cmd.process_group(0);
        }
        cmd.spawn()
            .map_err(|e| Error::Backend(format!("could not run git {}: {e}", args.join(" "))))
    }

    /// See [`GitWriter::run_network`].
    fn network(
        &self,
        args: &[&str],
        cancel: &CancelToken,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<GitOutput> {
        use std::io::Read as _;
        use std::sync::mpsc;

        let display = format!("git {}", args.join(" "));
        let mut child = self.spawn_network(args)?;
        let mut stdout = child.stdout.take();
        let stdout_reader = std::thread::spawn(move || {
            let mut data = Vec::new();
            if let Some(out) = stdout.as_mut() {
                let _ = out.read_to_end(&mut data);
            }
            data
        });
        // Stderr is read in chunks; progress segments end with `\r` or `\n`.
        let (segments, received) = mpsc::channel::<String>();
        let mut stderr = child.stderr.take();
        let stderr_reader = std::thread::spawn(move || {
            let mut all = Vec::new();
            let mut pending = Vec::new();
            let mut buf = [0u8; 4096];
            while let Some(Ok(n @ 1..)) = stderr.as_mut().map(|e| e.read(&mut buf)) {
                all.extend_from_slice(&buf[..n]);
                for &byte in &buf[..n] {
                    if byte == b'\r' || byte == b'\n' {
                        let _ = segments.send(String::from_utf8_lossy(&pending).into_owned());
                        pending.clear();
                    } else {
                        pending.push(byte);
                    }
                }
            }
            all
        });

        let mut report = |segment: &str| {
            if let Some(p) = crate::network::parse_progress(segment) {
                progress(p);
            }
        };
        let status = loop {
            if cancel.is_cancelled() {
                kill_tree(&mut child);
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(Error::Cancelled);
            }
            let closed = match received.recv_timeout(std::time::Duration::from_millis(50)) {
                Ok(segment) => {
                    report(&segment);
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => false,
                Err(mpsc::RecvTimeoutError::Disconnected) => true,
            };
            match child.try_wait() {
                Ok(Some(status)) => break status,
                // Stderr closed but the process still runs: do not spin.
                Ok(None) if closed => std::thread::sleep(std::time::Duration::from_millis(20)),
                Ok(None) => {}
                Err(e) => return Err(Error::Backend(format!("could not run {display}: {e}"))),
            }
        };
        let stdout = stdout_reader.join().unwrap_or_default();
        let stderr = stderr_reader.join().unwrap_or_default();
        for segment in received.try_iter() {
            report(&segment);
        }
        if status.success() {
            Ok(GitOutput { stdout, stderr })
        } else {
            Err(Error::Command {
                command: display,
                code: status.code(),
                stderr: crate::network::strip_progress(&String::from_utf8_lossy(&stderr)),
            })
        }
    }
}

/// Stops `child` and every process it started.
fn kill_tree(child: &mut std::process::Child) {
    let pid = child.id().to_string();
    #[cfg(unix)]
    {
        // The child leads its own process group (see `spawn_network`).
        let group = format!("-{pid}");
        let _ = Command::new("kill")
            .args(["-KILL", "--", &group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    // Fallback (and the only way on other platforms): the direct child.
    let _ = child.kill();
}

fn finish(command: String, output: std::process::Output) -> Result<GitOutput> {
    if output.status.success() {
        Ok(GitOutput {
            stdout: output.stdout,
            stderr: output.stderr,
        })
    } else {
        Err(Error::Command {
            command,
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr)
                .trim_end()
                .to_owned(),
        })
    }
}

/// `git` command with the fixed, non-interactive environment.
fn command(git: &Path) -> Command {
    let mut cmd = Command::new(git);
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        // Never open an editor; messages are always passed via `-F -`.
        .env("GIT_EDITOR", ":")
        .env("GIT_SEQUENCE_EDITOR", ":")
        .env("GIT_MERGE_AUTOEDIT", "no");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        /// Keeps a console window from flashing up for every command.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u32, minor: u32, patch: u32) -> GitVersion {
        GitVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn parses_version_formats() {
        assert_eq!(
            GitVersion::parse("git version 2.47.3\n"),
            Some(version(2, 47, 3))
        );
        assert_eq!(
            GitVersion::parse("git version 2.45.1.windows.1"),
            Some(version(2, 45, 1))
        );
        assert_eq!(
            GitVersion::parse("git version 2.39.5 (Apple Git-154)"),
            Some(version(2, 39, 5))
        );
        assert_eq!(
            GitVersion::parse("git version 2.48.0-rc1"),
            Some(version(2, 48, 0))
        );
        assert_eq!(GitVersion::parse("git version 3.0"), Some(version(3, 0, 0)));
        assert_eq!(GitVersion::parse("hg version 6"), None);
        assert_eq!(GitVersion::parse("git version x.y"), None);
    }

    #[test]
    fn minimum_version_ordering() {
        assert!(version(2, 34, 0) >= MIN_GIT_VERSION);
        assert!(version(2, 33, 9) < MIN_GIT_VERSION);
        assert!(version(3, 0, 0) > MIN_GIT_VERSION);
    }

    #[test]
    fn availability_reasons() {
        assert_eq!(GitAvailability::Available(version(2, 40, 0)).reason(), None);
        assert!(GitAvailability::Missing
            .reason()
            .unwrap()
            .contains("not found"));
        assert!(GitAvailability::TooOld(version(2, 20, 1))
            .reason()
            .unwrap()
            .contains("2.20.1 is too old"));
    }
}
