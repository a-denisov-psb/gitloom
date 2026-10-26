//! Loading and resolving conflicted files (M4-05, ADR-022). Blocking: runs
//! `git` and writes temporary files.

use crate::conflict::{ConflictText, Side, PARSE_MARKER_SIZE};
use crate::editor::TempDir;
use crate::{Error, GitWriter, Result, RunOptions};

/// One index stage of a conflicted path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageEntry {
    pub mode: String,
    pub id: String,
}

/// Stages 1 (base), 2 (ours) and 3 (theirs) of a conflicted path; a missing
/// stage means the file does not exist on that side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConflictStages {
    pub base: Option<StageEntry>,
    pub ours: Option<StageEntry>,
    pub theirs: Option<StageEntry>,
}

/// What the conflict view can offer for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictContent {
    /// Text on both sides: hunks and lines can be picked.
    Text(ConflictText),
    /// Only a whole side can be taken.
    WholeFile(WholeFileReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFileReason {
    /// One side deleted the file, the other changed it.
    DeletedOnOneSide { ours_deleted: bool },
    /// Binary content or not valid UTF-8.
    Binary,
    /// A version is larger than 2 MB (M4-13).
    TooLarge,
}

/// Versions larger than this are only resolved by taking a whole side.
pub const MAX_CONFLICT_SIZE: usize = 2 * 1024 * 1024;

/// A conflicted file as the conflict view needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConflict {
    pub path: String,
    pub stages: ConflictStages,
    pub content: ConflictContent,
}

fn read_only() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

/// Index stages of `path` (`git ls-files -u`).
pub fn conflict_stages(git: &dyn GitWriter, path: &str) -> Result<ConflictStages> {
    let output = git.run(
        &["--literal-pathspecs", "ls-files", "-u", "-z", "--", path],
        read_only(),
    )?;
    let mut stages = ConflictStages::default();
    for record in output.stdout.split(|&b| b == 0) {
        let record = String::from_utf8_lossy(record);
        // "<mode> <id> <stage>\t<path>"
        let Some((info, entry_path)) = record.split_once('\t') else {
            continue;
        };
        if entry_path != path {
            continue;
        }
        let mut fields = info.split_whitespace();
        let (Some(mode), Some(id), Some(stage)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let entry = Some(StageEntry {
            mode: mode.to_owned(),
            id: id.to_owned(),
        });
        match stage {
            "1" => stages.base = entry,
            "2" => stages.ours = entry,
            "3" => stages.theirs = entry,
            _ => {}
        }
    }
    if stages.ours.is_none() && stages.theirs.is_none() {
        return Err(Error::NotFound(format!("conflict in {path}")));
    }
    Ok(stages)
}

fn blob(git: &dyn GitWriter, entry: Option<&StageEntry>) -> Result<Vec<u8>> {
    match entry {
        Some(entry) => Ok(git
            .run(&["cat-file", "blob", &entry.id], read_only())?
            .stdout),
        None => Ok(Vec::new()),
    }
}

fn is_binary(data: &[u8]) -> bool {
    data.iter().take(8000).any(|&b| b == 0) || std::str::from_utf8(data).is_err()
}

/// Loads the conflict of `path`: hunks from `git merge-file` on the index
/// stages (as stored, before checkout filters), or whole-file choices.
pub fn load_conflict(git: &dyn GitWriter, path: &str) -> Result<LoadedConflict> {
    let stages = conflict_stages(git, path)?;
    let loaded = |content| LoadedConflict {
        path: path.to_owned(),
        stages: stages.clone(),
        content,
    };
    if stages.ours.is_none() || stages.theirs.is_none() {
        return Ok(loaded(ConflictContent::WholeFile(
            WholeFileReason::DeletedOnOneSide {
                ours_deleted: stages.ours.is_none(),
            },
        )));
    }
    let base = blob(git, stages.base.as_ref())?;
    let ours = blob(git, stages.ours.as_ref())?;
    let theirs = blob(git, stages.theirs.as_ref())?;
    if [&base, &ours, &theirs]
        .iter()
        .any(|data| data.len() > MAX_CONFLICT_SIZE)
    {
        return Ok(loaded(ConflictContent::WholeFile(
            WholeFileReason::TooLarge,
        )));
    }
    if [&base, &ours, &theirs].iter().any(|data| is_binary(data)) {
        return Ok(loaded(ConflictContent::WholeFile(WholeFileReason::Binary)));
    }

    let temp = TempDir::new("gitloom-conflict")?;
    let write = |name: &str, data: &[u8]| {
        let file = temp.path().join(name);
        std::fs::write(&file, data)
            .map_err(|e| Error::Backend(format!("could not write {}: {e}", file.display())))?;
        Ok::<_, Error>(file.to_string_lossy().into_owned())
    };
    let (ours_file, base_file, theirs_file) = (
        write("ours", &ours)?,
        write("base", &base)?,
        write("theirs", &theirs)?,
    );
    let marker = format!("--marker-size={PARSE_MARKER_SIZE}");
    let (code, output) = git.run_status(
        &[
            "merge-file",
            "-p",
            "--diff3",
            &marker,
            "-L",
            "ours",
            "-L",
            "base",
            "-L",
            "theirs",
            &ours_file,
            &base_file,
            &theirs_file,
        ],
        read_only(),
    )?;
    // The exit code is the number of conflicts; negative means failure.
    if !code.is_some_and(|c| c >= 0) {
        return Err(Error::Command {
            command: "git merge-file".into(),
            code,
            stderr: String::from_utf8_lossy(&output.stderr)
                .trim_end()
                .to_owned(),
        });
    }
    let merged = String::from_utf8(output.stdout)
        .map_err(|_| Error::Backend(format!("{path}: merge result is not UTF-8")))?;
    let text = ConflictText::parse(&merged)
        .ok_or_else(|| Error::Backend(format!("{path}: could not read the conflict markers")))?;
    Ok(loaded(ConflictContent::Text(text)))
}

/// Writes `content` (in index form, as produced from [`load_conflict`]) as
/// the resolved version of `path`: index stage 0 and the working tree file
/// (checkout filters such as `core.autocrlf` applied).
pub fn resolve_conflict(git: &dyn GitWriter, path: &str, content: &[u8]) -> Result<()> {
    let stages = conflict_stages(git, path)?;
    let mode = stages
        .ours
        .as_ref()
        .or(stages.theirs.as_ref())
        .map_or("100644", |entry| entry.mode.as_str());
    let id = git.write_blob(content)?;
    let info = format!("{mode},{id},{path}");
    // A stage 0 entry replaces the conflict stages.
    git.run(
        &["update-index", "--add", "--cacheinfo", &info],
        RunOptions::default(),
    )?;
    git.run(
        &["checkout-index", "--force", "--", path],
        RunOptions::default(),
    )
    .map(drop)
}

/// Takes the whole file from `side` (deletes it if that side deleted it) and
/// marks it resolved.
pub fn resolve_with_side(git: &dyn GitWriter, path: &str, side: Side) -> Result<()> {
    let stages = conflict_stages(git, path)?;
    let (entry, flag) = match side {
        Side::Ours => (&stages.ours, "--ours"),
        Side::Theirs => (&stages.theirs, "--theirs"),
    };
    let literal = "--literal-pathspecs";
    if entry.is_some() {
        git.run(
            &[literal, "checkout", flag, "--", path],
            RunOptions::default(),
        )?;
        git.run(&[literal, "add", "--", path], RunOptions::default())
            .map(drop)
    } else {
        git.run(
            &[literal, "rm", "--quiet", "--force", "--", path],
            RunOptions::default(),
        )
        .map(drop)
    }
}
