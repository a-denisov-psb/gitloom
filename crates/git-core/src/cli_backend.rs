//! Write access through the `git` executable (ADR-013).
//!
//! Every command runs with an argument list (never through a shell), in the
//! repository root, without prompts or editors, so hooks, signing and config
//! behave exactly like plain Git.

use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{Error, Result};

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
    let mut full = args.to_vec();
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

/// Runs the `git` executable in one repository.
#[derive(Debug, Clone)]
pub struct CliBackend {
    git: PathBuf,
    root: PathBuf,
}

impl CliBackend {
    /// Backend for the repository whose working tree root is `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            git: PathBuf::from("git"),
            root: root.into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl GitWriter for CliBackend {
    fn run(&self, args: &[&str], options: RunOptions<'_>) -> Result<GitOutput> {
        let mut cmd = command(&self.git);
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
        .env("GIT_MERGE_AUTOEDIT", "no")
        // Paths we pass are file names, never glob patterns.
        .env("GIT_LITERAL_PATHSPECS", "1");
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
