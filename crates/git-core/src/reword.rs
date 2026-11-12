//! Edit the message of an unpushed commit without a rebase (ADR-051,
//! M14-04): the commits from the target to HEAD are rebuilt with
//! `git commit-tree` (same trees, authors and other parents) and the
//! checked-out branch is moved with `git update-ref`. HEAD stays symbolic,
//! so the index and the working tree are untouched.

use std::collections::HashMap;

use crate::{Error, GitWriter, Oid, Result, RunOptions};

/// Why a commit's message cannot be edited by a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditBlock {
    /// A remote-tracking branch (short name) contains the commit.
    Pushed(String),
    /// Not on the checked-out branch, or HEAD is detached.
    NotOnBranch,
    /// A merge, rebase, cherry-pick or revert runs.
    OperationRunning,
}

impl EditBlock {
    /// Tooltip text of the message box.
    pub fn reason(&self) -> String {
        match self {
            Self::Pushed(remote) => format!(
                "Already pushed to {remote}. Use interactive rebase to change pushed history."
            ),
            Self::NotOnBranch => "Check out a branch that contains this commit".to_owned(),
            Self::OperationRunning => "Finish the running operation first".to_owned(),
        }
    }
}

/// What an edit changes, for the note before saving.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditFacts {
    /// The checked-out branch that is moved.
    pub branch: String,
    /// Other local branches and tags that contain the commit; they keep
    /// the old commits.
    pub keep_old: Vec<String>,
    /// A rebuilt commit is signed but signing is off: its signature is lost.
    pub drops_signature: bool,
}

/// The result of an edit, also the undo record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reworded {
    pub branch: String,
    pub old_head: Oid,
    pub new_head: Oid,
    /// The rebuilt target commit.
    pub new_commit: Oid,
}

impl Reworded {
    /// Moves the branch back (`undo`) or forward again; fails with "the
    /// branch changed" when it is not where the edit left it.
    pub fn apply(&self, git: &dyn GitWriter, undo: bool) -> Result<()> {
        let (to, from) = if undo {
            (&self.old_head, &self.new_head)
        } else {
            (&self.new_head, &self.old_head)
        };
        let reason = crate::identity::reflog_message(if undo {
            "undo edit message"
        } else {
            "redo edit message"
        });
        move_branch(git, &self.branch, to, from, &reason).map_err(|_| {
            Error::Backend(format!(
                "Cannot {}: the branch {} changed",
                if undo { "undo" } else { "redo" },
                self.branch
            ))
        })
    }
}

fn read() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

fn text(git: &dyn GitWriter, args: &[&str]) -> Result<String> {
    Ok(git.run(args, read())?.stdout_text().trim().to_owned())
}

/// The checked-out branch, `None` when HEAD is detached.
fn head_branch(git: &dyn GitWriter) -> Result<Option<String>> {
    let (code, output) = git.run_status(&["symbolic-ref", "-q", "--short", "HEAD"], read())?;
    Ok((code == Some(0))
        .then(|| output.stdout_text().trim().to_owned())
        .filter(|b| !b.is_empty()))
}

fn refs_containing(git: &dyn GitWriter, commit: &str, patterns: &[&str]) -> Result<Vec<String>> {
    let mut args = vec!["for-each-ref", "--format=%(refname)", "--contains", commit];
    args.extend_from_slice(patterns);
    Ok(git
        .run(&args, read())?
        .stdout_text()
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Why `commit` cannot be edited, `None` when it can (ADR-051).
pub fn edit_block(git: &dyn GitWriter, commit: &str) -> Result<Option<EditBlock>> {
    let git_dir = text(git, &["rev-parse", "--absolute-git-dir"])?;
    if !crate::state::detect(std::path::Path::new(&git_dir)).is_clean() {
        return Ok(Some(EditBlock::OperationRunning));
    }
    let Some(branch) = head_branch(git)? else {
        return Ok(Some(EditBlock::NotOnBranch));
    };
    let (code, _) = git.run_status(&["merge-base", "--is-ancestor", commit, "HEAD"], read())?;
    if code != Some(0) {
        return Ok(Some(EditBlock::NotOnBranch));
    }
    let remotes: Vec<String> = refs_containing(git, commit, &["refs/remotes"])?
        .into_iter()
        .filter(|r| !r.ends_with("/HEAD"))
        .filter_map(|r| r.strip_prefix("refs/remotes/").map(str::to_owned))
        .collect();
    if remotes.is_empty() {
        return Ok(None);
    }
    // Name the branch's own upstream when it contains the commit.
    let upstream = text(
        git,
        &[
            "for-each-ref",
            "--format=%(upstream:short)",
            &format!("refs/heads/{branch}"),
        ],
    )?;
    let remote = if remotes.contains(&upstream) {
        upstream
    } else {
        remotes[0].clone()
    };
    Ok(Some(EditBlock::Pushed(remote)))
}

/// Commits to rebuild, oldest first: the target, then every commit on the
/// ancestry path from it to HEAD.
fn chain(git: &dyn GitWriter, commit: &str) -> Result<Vec<Oid>> {
    let range = format!("{commit}..HEAD");
    let mut chain = vec![text(
        git,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?];
    chain.extend(
        git.run(
            &[
                "rev-list",
                "--topo-order",
                "--reverse",
                "--ancestry-path",
                &range,
            ],
            read(),
        )?
        .stdout_text()
        .lines()
        .map(str::to_owned),
    );
    Ok(chain)
}

/// A commit object split into what the rebuild needs.
struct RawCommit {
    tree: String,
    parents: Vec<String>,
    author: Ident,
    encoding: Option<String>,
    signed: bool,
    message: Vec<u8>,
}

struct Ident {
    name: String,
    email: String,
    /// `@<seconds> <zone>`, Git's raw date format.
    date: String,
}

fn parse_ident(line: &str) -> Option<Ident> {
    let open = line.find('<')?;
    let close = line[open..].find('>')? + open;
    let name = line[..open].trim().to_owned();
    let email = line[open + 1..close].to_owned();
    let date = line[close + 1..].trim();
    let (seconds, zone) = date.split_once(' ')?;
    Some(Ident {
        name,
        email,
        date: format!("@{seconds} {zone}"),
    })
}

fn parse_commit(raw: &[u8]) -> Result<RawCommit> {
    let malformed = || Error::Backend("unexpected commit object".into());
    let split = raw
        .windows(2)
        .position(|w| w == b"\n\n")
        .ok_or_else(malformed)?;
    let headers = String::from_utf8_lossy(&raw[..split]);
    let message = raw[split + 2..].to_vec();
    let (mut tree, mut parents, mut author, mut encoding, mut signed) =
        (None, Vec::new(), None, None, false);
    for line in headers.lines() {
        // Continuation lines of multi-line headers (signatures, mergetags).
        if line.starts_with(' ') {
            continue;
        }
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "tree" => tree = Some(value.to_owned()),
            "parent" => parents.push(value.to_owned()),
            "author" => author = parse_ident(value),
            "encoding" => encoding = Some(value.to_owned()),
            "gpgsig" | "gpgsig-sha256" => signed = true,
            _ => {}
        }
    }
    Ok(RawCommit {
        tree: tree.ok_or_else(malformed)?,
        parents,
        author: author.ok_or_else(malformed)?,
        encoding,
        signed,
        message,
    })
}

fn raw_commit(git: &dyn GitWriter, id: &str) -> Result<RawCommit> {
    parse_commit(&git.run(&["cat-file", "commit", id], read())?.stdout)
}

fn sign_commits(git: &dyn GitWriter) -> Result<bool> {
    let (code, output) = git.run_status(&["config", "--bool", "commit.gpgSign"], read())?;
    Ok(code == Some(0) && output.stdout_text().trim() == "true")
}

/// The note before saving: the moved branch, refs that keep the old
/// commits and whether a signature would be dropped.
pub fn edit_facts(git: &dyn GitWriter, commit: &str) -> Result<EditFacts> {
    let branch = head_branch(git)?.unwrap_or_default();
    let own = format!("refs/heads/{branch}");
    let keep_old = refs_containing(git, commit, &["refs/heads", "refs/tags"])?
        .into_iter()
        .filter(|r| *r != own)
        .map(|r| {
            r.strip_prefix("refs/heads/")
                .or_else(|| r.strip_prefix("refs/tags/"))
                .unwrap_or(&r)
                .to_owned()
        })
        .collect();
    let drops_signature = !sign_commits(git)?
        && chain(git, commit)?
            .iter()
            .map(|id| raw_commit(git, id).map(|c| c.signed))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .any(|signed| signed);
    Ok(EditFacts {
        branch,
        keep_old,
        drops_signature,
    })
}

fn move_branch(
    git: &dyn GitWriter,
    branch: &str,
    to: &str,
    from: &str,
    reason: &str,
) -> Result<()> {
    git.run(
        &[
            "update-ref",
            "-m",
            reason,
            &format!("refs/heads/{branch}"),
            to,
            from,
        ],
        RunOptions::default(),
    )
    .map(drop)
}

/// Gives `commit` the message `message` (ADR-051). Refused when
/// [`edit_block`] names a reason or the branch moved meanwhile.
pub fn reword_unpushed(git: &dyn GitWriter, commit: &str, message: &str) -> Result<Reworded> {
    if let Some(block) = edit_block(git, commit)? {
        return Err(Error::Backend(block.reason()));
    }
    let branch =
        head_branch(git)?.ok_or_else(|| Error::Backend(EditBlock::NotOnBranch.reason()))?;
    let old_head = text(git, &["rev-parse", "HEAD"])?;
    let chain = chain(git, commit)?;
    let sign = sign_commits(git)?;
    let mut message = message.trim_end().to_owned();
    message.push('\n');
    let mut rebuilt: HashMap<String, String> = HashMap::new();
    for (index, id) in chain.iter().enumerate() {
        let raw = raw_commit(git, id)?;
        let target = index == 0;
        let parents: Vec<String> = raw
            .parents
            .iter()
            .map(|p| rebuilt.get(p).cloned().unwrap_or_else(|| p.clone()))
            .collect();
        let encoding = (!target)
            .then_some(raw.encoding.as_deref())
            .flatten()
            .map(|e| format!("i18n.commitEncoding={e}"));
        let mut args: Vec<&str> = Vec::new();
        if let Some(encoding) = &encoding {
            args.extend(["-c", encoding.as_str()]);
        }
        args.extend(["commit-tree", raw.tree.as_str()]);
        for parent in &parents {
            args.extend(["-p", parent.as_str()]);
        }
        if sign {
            args.push("-S");
        }
        let env = [
            ("GIT_AUTHOR_NAME", raw.author.name.as_str()),
            ("GIT_AUTHOR_EMAIL", raw.author.email.as_str()),
            ("GIT_AUTHOR_DATE", raw.author.date.as_str()),
        ];
        let body: &[u8] = if target {
            message.as_bytes()
        } else {
            &raw.message
        };
        let output = git.run_with_env(
            &args,
            &env,
            RunOptions {
                stdin: Some(body),
                ..RunOptions::default()
            },
        )?;
        rebuilt.insert(id.clone(), output.stdout_text().trim().to_owned());
    }
    let new_commit = rebuilt
        .get(&chain[0])
        .cloned()
        .ok_or_else(|| Error::Backend("nothing rebuilt".into()))?;
    let new_head = rebuilt
        .get(&old_head)
        .cloned()
        .ok_or_else(|| Error::Backend("HEAD is not on the ancestry path".into()))?;
    let short = &commit[..commit.len().min(7)];
    move_branch(
        git,
        &branch,
        &new_head,
        &old_head,
        &crate::identity::reflog_message(&format!("edit message of {short}")),
    )
    .map_err(|_| {
        Error::Backend(format!(
            "Cannot edit the message: the branch {branch} changed"
        ))
    })?;
    Ok(Reworded {
        branch,
        old_head,
        new_head,
        new_commit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_identities_and_headers() {
        let ident = parse_ident("Ana Lyst <ana@example.com> 1600000000 +0200").unwrap();
        assert_eq!(
            (
                ident.name.as_str(),
                ident.email.as_str(),
                ident.date.as_str()
            ),
            ("Ana Lyst", "ana@example.com", "@1600000000 +0200")
        );
        let raw = b"tree t1\nparent p1\nparent p2\nauthor A <a@x> 1 +0000\ncommitter C <c@x> 2 +0000\nencoding ISO-8859-1\ngpgsig -----BEGIN SSH SIGNATURE-----\n abc\n -----END SSH SIGNATURE-----\n\nSubject\n\nBody\n";
        let commit = parse_commit(raw).unwrap();
        assert_eq!(commit.tree, "t1");
        assert_eq!(commit.parents, ["p1", "p2"]);
        assert_eq!(commit.encoding.as_deref(), Some("ISO-8859-1"));
        assert!(commit.signed);
        assert_eq!(commit.message, b"Subject\n\nBody\n");
        assert!(parse_commit(b"no headers").is_err());
    }

    #[test]
    fn reasons() {
        assert_eq!(
            EditBlock::Pushed("origin/main".into()).reason(),
            "Already pushed to origin/main. Use interactive rebase to change pushed history."
        );
        assert_eq!(
            EditBlock::NotOnBranch.reason(),
            "Check out a branch that contains this commit"
        );
    }
}
