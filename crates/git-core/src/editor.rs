//! Gitloom as Git editor (ADR-021, addendum to ADR-019).
//!
//! Commands that need a todo list or a commit message (interactive rebase,
//! reword, squash) run with `GIT_SEQUENCE_EDITOR` and `GIT_EDITOR` pointing to
//! the Gitloom executable. Before the command starts, the content is written
//! to a private directory; Gitloom started as editor copies it into the file
//! Git passes and exits. Without prepared content it exits with code 1, so Git
//! aborts instead of waiting for a terminal editor.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Error, Result};

/// Set (to `1`) for every editor call started by [`crate::CliBackend`].
pub const EDITOR_ENV: &str = "GITLOOM_EDITOR";
/// Directory with the prepared content of one command.
pub const EDITOR_DIR_ENV: &str = "GITLOOM_EDITOR_DIR";

/// First argument of an editor call; the file to edit follows.
const SEQUENCE_FLAG: &str = "--git-editor=sequence";
const MESSAGE_FLAG: &str = "--git-editor=message";

/// File in the editor directory holding the todo list.
const SEQUENCE_FILE: &str = "sequence";
/// Prefix of the numbered message files (`message-0001`, ...).
const MESSAGE_PREFIX: &str = "message-";

/// Content Git asks for while a command runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorInput {
    /// Todo list for the sequence editor (interactive rebase).
    pub sequence: Option<String>,
    /// Commit messages, handed out in order: one per message editor call
    /// (e.g. each reword and each squash of an interactive rebase).
    pub messages: Vec<String>,
}

/// Which editor Git started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKind {
    Sequence,
    Message,
}

impl EditorKind {
    fn flag(self) -> &'static str {
        match self {
            Self::Sequence => SEQUENCE_FLAG,
            Self::Message => MESSAGE_FLAG,
        }
    }
}

/// Editor call parsed from the command line: `--git-editor=<kind> <file>`.
pub fn editor_call(args: &[String]) -> Option<(EditorKind, PathBuf)> {
    let kind = match args.first()?.as_str() {
        SEQUENCE_FLAG => EditorKind::Sequence,
        MESSAGE_FLAG => EditorKind::Message,
        _ => return None,
    };
    match &args[1..] {
        [file] => Some((kind, PathBuf::from(file))),
        _ => Some((kind, PathBuf::new())),
    }
}

/// Runs one editor call and returns the process exit code. `dir` is the
/// value of [`EDITOR_DIR_ENV`]. Never starts a UI.
pub fn editor_main(kind: EditorKind, file: &Path, dir: Option<&Path>) -> i32 {
    match fill(kind, file, dir) {
        Ok(()) => 0,
        Err(error) => {
            // Git shows the editor's stderr above its own error.
            eprintln!("gitloom: {error}");
            1
        }
    }
}

fn fill(kind: EditorKind, file: &Path, dir: Option<&Path>) -> Result<()> {
    let dir = dir.ok_or_else(|| Error::Backend("no prepared editor content".into()))?;
    if file.as_os_str().is_empty() {
        return Err(Error::Backend("Git passed no file to edit".into()));
    }
    let source = match kind {
        EditorKind::Sequence => {
            let path = dir.join(SEQUENCE_FILE);
            if !path.is_file() {
                return Err(Error::Backend("no prepared todo list".into()));
            }
            path
        }
        EditorKind::Message => {
            next_message(dir)?.ok_or_else(|| Error::Backend("no prepared commit message".into()))?
        }
    };
    let content = fs::read(&source).map_err(|e| io_error("read prepared content", e))?;
    fs::write(file, content).map_err(|e| io_error("write the file Git passed", e))?;
    if kind == EditorKind::Message {
        // Consumed: the next call gets the next message.
        fs::remove_file(&source).map_err(|e| io_error("remove used message", e))?;
    }
    Ok(())
}

/// Lowest-numbered message file left in `dir`.
fn next_message(dir: &Path) -> Result<Option<PathBuf>> {
    let entries = fs::read_dir(dir).map_err(|e| io_error("read editor directory", e))?;
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(MESSAGE_PREFIX))
        .collect();
    names.sort();
    Ok(names.first().map(|name| dir.join(name)))
}

fn io_error(what: &str, error: io::Error) -> Error {
    Error::Backend(format!("could not {what}: {error}"))
}

/// Private directory with the prepared content of one command; removed on drop.
#[derive(Debug)]
pub(crate) struct PreparedEditor {
    dir: PathBuf,
}

impl PreparedEditor {
    /// Writes `input` into a new directory below the system temp dir.
    pub(crate) fn new(input: &EditorInput) -> Result<Self> {
        let prepared = Self {
            dir: create_unique_dir()?,
        };
        if let Some(sequence) = &input.sequence {
            fs::write(prepared.dir.join(SEQUENCE_FILE), sequence)
                .map_err(|e| io_error("prepare todo list", e))?;
        }
        for (index, message) in input.messages.iter().enumerate() {
            let name = format!("{MESSAGE_PREFIX}{:04}", index + 1);
            fs::write(prepared.dir.join(name), message)
                .map_err(|e| io_error("prepare commit message", e))?;
        }
        Ok(prepared)
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for PreparedEditor {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn create_unique_dir() -> Result<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    for _ in 0..100 {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = base.join(format!("gitloom-editor-{}-{n}-{nanos}", std::process::id()));
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_error("create editor directory", e)),
        }
    }
    Err(Error::Backend(
        "could not create editor directory: too many attempts".into(),
    ))
}

/// Editor command for Git, which runs it through a shell and appends the
/// file name: `'<program>' --git-editor=<kind>`.
pub(crate) fn editor_command(program: &Path, kind: EditorKind) -> OsString {
    let mut path = program.to_string_lossy().into_owned();
    if cfg!(windows) {
        // Git for Windows runs editors through its POSIX shell.
        path = path.replace('\\', "/");
    }
    let quoted = format!("'{}'", path.replace('\'', r"'\''"));
    OsString::from(format!("{quoted} {}", kind.flag()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_editor_calls() {
        assert_eq!(
            editor_call(&args(&["--git-editor=sequence", "/r/.git/todo"])),
            Some((EditorKind::Sequence, PathBuf::from("/r/.git/todo")))
        );
        assert_eq!(
            editor_call(&args(&["--git-editor=message", "MSG"])),
            Some((EditorKind::Message, PathBuf::from("MSG")))
        );
        // Still editor mode (so no UI starts), but without a file.
        assert_eq!(
            editor_call(&args(&["--git-editor=message"])),
            Some((EditorKind::Message, PathBuf::new()))
        );
        assert_eq!(editor_call(&args(&["/some/repo"])), None);
        assert_eq!(editor_call(&args(&[])), None);
    }

    #[test]
    fn quotes_the_program_for_the_shell() {
        let command = editor_command(Path::new("/opt/it's here/gitloom"), EditorKind::Message);
        assert_eq!(
            command,
            OsString::from(r"'/opt/it'\''s here/gitloom' --git-editor=message")
        );
    }

    #[test]
    fn copies_the_todo_list() {
        let input = EditorInput {
            sequence: Some("pick abc one\n".into()),
            messages: Vec::new(),
        };
        let prepared = PreparedEditor::new(&input).unwrap();
        let target = prepared.dir().join("git-rebase-todo");
        fs::write(&target, "pick abc one\n# help\n").unwrap();
        let code = editor_main(EditorKind::Sequence, &target, Some(prepared.dir()));
        assert_eq!(code, 0);
        assert_eq!(fs::read_to_string(&target).unwrap(), "pick abc one\n");
        // The todo list can be requested again (e.g. `--edit-todo`).
        assert_eq!(
            editor_main(EditorKind::Sequence, &target, Some(prepared.dir())),
            0
        );
    }

    #[test]
    fn hands_out_messages_in_order() {
        let input = EditorInput {
            sequence: None,
            messages: (1..=11).map(|n| format!("message {n}\n")).collect(),
        };
        let prepared = PreparedEditor::new(&input).unwrap();
        let target = prepared.dir().join("COMMIT_EDITMSG");
        for n in 1..=11 {
            assert_eq!(
                editor_main(EditorKind::Message, &target, Some(prepared.dir())),
                0
            );
            assert_eq!(
                fs::read_to_string(&target).unwrap(),
                format!("message {n}\n")
            );
        }
        // Queue empty: Git must abort.
        assert_eq!(
            editor_main(EditorKind::Message, &target, Some(prepared.dir())),
            1
        );
    }

    #[test]
    fn missing_preparation_fails() {
        let prepared = PreparedEditor::new(&EditorInput::default()).unwrap();
        let target = prepared.dir().join("file");
        fs::write(&target, "original").unwrap();
        assert_eq!(
            editor_main(EditorKind::Sequence, &target, Some(prepared.dir())),
            1
        );
        assert_eq!(editor_main(EditorKind::Message, &target, None), 1);
        assert_eq!(
            editor_main(EditorKind::Message, Path::new(""), Some(prepared.dir())),
            1
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "original");
    }

    #[test]
    fn prepared_directory_is_removed() {
        let prepared = PreparedEditor::new(&EditorInput {
            sequence: Some("x".into()),
            messages: vec!["m".into()],
        })
        .unwrap();
        let dir = prepared.dir().to_path_buf();
        assert!(dir.join(SEQUENCE_FILE).is_file());
        drop(prepared);
        assert!(!dir.exists());
    }
}
