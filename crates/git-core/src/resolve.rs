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
    /// Binary content: NUL bytes without a UTF-16 byte order mark.
    Binary,
    /// The versions are text in different encodings (M15-01), e.g. UTF-16
    /// on one side and UTF-8 on the other.
    MixedEncodings {
        first: ConflictEncoding,
        second: ConflictEncoding,
    },
    /// A version is larger than 2 MB (M4-13).
    TooLarge,
}

/// Text encoding of a conflicted file (M15-01). The versions are merged as
/// UTF-8 and the resolved file is written back in this encoding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConflictEncoding {
    /// UTF-8 without a byte order mark (also plain ASCII).
    #[default]
    Utf8,
    /// UTF-8 with a byte order mark (`EF BB BF`).
    Utf8Bom,
    /// UTF-16 little endian with a byte order mark (`FF FE`).
    Utf16Le,
    /// UTF-16 big endian with a byte order mark (`FE FF`).
    Utf16Be,
    /// 8-bit text that is not UTF-8 (no NUL bytes), mapped byte for byte
    /// as Latin-1 so that it round-trips exactly.
    EightBit,
}

impl ConflictEncoding {
    /// Name for the conflict view header and messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::EightBit => "8-bit",
        }
    }

    /// Encodes the UTF-8 `text` of a resolved file in this encoding.
    pub fn encode(self, path: &str, text: &[u8]) -> Result<Vec<u8>> {
        let as_str = || {
            std::str::from_utf8(text)
                .map_err(|_| Error::Backend(format!("{path}: the resolved text is not UTF-8")))
        };
        Ok(match self {
            Self::Utf8 => text.to_vec(),
            Self::Utf8Bom if text.starts_with(UTF8_BOM) => text.to_vec(),
            Self::Utf8Bom => [UTF8_BOM, text].concat(),
            Self::Utf16Le | Self::Utf16Be => {
                let little = self == Self::Utf16Le;
                let mut out = if little {
                    vec![0xFF, 0xFE]
                } else {
                    vec![0xFE, 0xFF]
                };
                for unit in as_str()?.encode_utf16() {
                    out.extend(if little {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    });
                }
                out
            }
            Self::EightBit => as_str()?
                .chars()
                .map(|c| u8::try_from(u32::from(c)))
                .collect::<std::result::Result<Vec<u8>, _>>()
                .map_err(|_| {
                    Error::Backend(format!(
                        "{path}: the resolved text contains characters that the file's \
                         8-bit encoding cannot store"
                    ))
                })?,
        })
    }
}

/// What one version of a conflicted file contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageClass {
    /// No bytes (also a missing stage): fits every encoding.
    Empty,
    /// ASCII only: UTF-8 or 8-bit text.
    Ascii,
    Text(ConflictEncoding),
    Binary,
}

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

fn classify(data: &[u8]) -> StageClass {
    if data.is_empty() {
        return StageClass::Empty;
    }
    match data {
        [0xFF, 0xFE, ..] => return StageClass::Text(ConflictEncoding::Utf16Le),
        [0xFE, 0xFF, ..] => return StageClass::Text(ConflictEncoding::Utf16Be),
        _ => {}
    }
    if data.iter().take(8000).any(|&b| b == 0) {
        return StageClass::Binary;
    }
    match std::str::from_utf8(data) {
        Ok(_) if data.starts_with(UTF8_BOM) => StageClass::Text(ConflictEncoding::Utf8Bom),
        Ok(_) if data.is_ascii() => StageClass::Ascii,
        Ok(_) => StageClass::Text(ConflictEncoding::Utf8),
        Err(_) => StageClass::Text(ConflictEncoding::EightBit),
    }
}

/// Common encoding of the versions of a conflicted file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Text(ConflictEncoding),
    Binary,
    Mixed(ConflictEncoding, ConflictEncoding),
}

/// UTF-8 with and without a byte order mark are one encoding; the mark of
/// ours decides the output.
fn family(encoding: ConflictEncoding) -> ConflictEncoding {
    match encoding {
        ConflictEncoding::Utf8Bom => ConflictEncoding::Utf8,
        other => other,
    }
}

fn verdict(base: &[u8], ours: &[u8], theirs: &[u8]) -> Verdict {
    let classes = [classify(ours), classify(theirs), classify(base)];
    if classes.contains(&StageClass::Binary) {
        return Verdict::Binary;
    }
    let mut families: Vec<ConflictEncoding> = Vec::new();
    for class in classes {
        if let StageClass::Text(encoding) = class {
            let encoding = family(encoding);
            if !families.contains(&encoding) {
                families.push(encoding);
            }
        }
    }
    // ASCII fits UTF-8 and 8-bit text, not UTF-16.
    let ascii = classes.contains(&StageClass::Ascii);
    if ascii && !families.is_empty() && families.iter().all(|f| is_utf16(*f)) {
        families.push(ConflictEncoding::Utf8);
    }
    match families.as_slice() {
        [] => Verdict::Text(ConflictEncoding::Utf8),
        [ConflictEncoding::Utf8] if classes[0] == StageClass::Text(ConflictEncoding::Utf8Bom) => {
            Verdict::Text(ConflictEncoding::Utf8Bom)
        }
        [only] => Verdict::Text(*only),
        [first, second, ..] => Verdict::Mixed(*first, *second),
    }
}

fn is_utf16(encoding: ConflictEncoding) -> bool {
    matches!(
        encoding,
        ConflictEncoding::Utf16Le | ConflictEncoding::Utf16Be
    )
}

/// `data` as UTF-8 text, decoded from `encoding` (byte order mark removed).
fn decode(data: &[u8], encoding: ConflictEncoding) -> Vec<u8> {
    match encoding {
        ConflictEncoding::Utf16Le | ConflictEncoding::Utf16Be => {
            crate::diff::decode_utf16(data).map_or_else(Vec::new, |(text, _)| text)
        }
        ConflictEncoding::EightBit => data
            .iter()
            .map(|&b| char::from(b))
            .collect::<String>()
            .into_bytes(),
        ConflictEncoding::Utf8 | ConflictEncoding::Utf8Bom => {
            data.strip_prefix(UTF8_BOM).unwrap_or(data).to_vec()
        }
    }
}

/// Versions larger than this are only resolved by taking a whole side.
pub const MAX_CONFLICT_SIZE: usize = 2 * 1024 * 1024;

/// A conflicted file as the conflict view needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConflict {
    pub path: String,
    pub stages: ConflictStages,
    pub content: ConflictContent,
    /// Encoding of a text conflict (UTF-8 for whole-file choices).
    pub encoding: ConflictEncoding,
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

/// Loads the conflict of `path`: hunks from `git merge-file` on the index
/// stages (as stored, before checkout filters), or whole-file choices.
pub fn load_conflict(git: &dyn GitWriter, path: &str) -> Result<LoadedConflict> {
    let stages = conflict_stages(git, path)?;
    let loaded = |content| LoadedConflict {
        path: path.to_owned(),
        stages: stages.clone(),
        content,
        encoding: ConflictEncoding::Utf8,
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
    let encoding = match verdict(&base, &ours, &theirs) {
        Verdict::Text(encoding) => encoding,
        Verdict::Binary => return Ok(loaded(ConflictContent::WholeFile(WholeFileReason::Binary))),
        Verdict::Mixed(first, second) => {
            return Ok(loaded(ConflictContent::WholeFile(
                WholeFileReason::MixedEncodings { first, second },
            )))
        }
    };
    // Merged as UTF-8 text; `resolve_conflict` encodes the result back.
    let (base, ours, theirs) = (
        decode(&base, encoding),
        decode(&ours, encoding),
        decode(&theirs, encoding),
    );

    let temp = TempDir::new(&crate::identity::temp_prefix("conflict"))?;
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
    Ok(LoadedConflict {
        encoding,
        ..loaded(ConflictContent::Text(text))
    })
}

/// Writes `content` (in index form, as produced from [`load_conflict`]) as
/// the resolved version of `path`: index stage 0 and the working tree file
/// (checkout filters such as `core.autocrlf` applied). The UTF-8 `content`
/// is encoded back to the encoding of the versions (M15-01; byte order mark
/// as in ours).
pub fn resolve_conflict(git: &dyn GitWriter, path: &str, content: &[u8]) -> Result<()> {
    let stages = conflict_stages(git, path)?;
    let encoded;
    let content = if stages.ours.is_some() && stages.theirs.is_some() {
        let base = blob(git, stages.base.as_ref())?;
        let ours = blob(git, stages.ours.as_ref())?;
        let theirs = blob(git, stages.theirs.as_ref())?;
        match verdict(&base, &ours, &theirs) {
            Verdict::Text(encoding) => {
                encoded = encoding.encode(path, content)?;
                encoded.as_slice()
            }
            Verdict::Binary | Verdict::Mixed(..) => content,
        }
    } else {
        content
    };
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
