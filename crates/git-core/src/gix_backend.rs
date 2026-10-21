//! Read access through gitoxide.

use std::collections::HashSet;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;
use gix::ObjectId;

use crate::{
    Commit, Error, FileChange, GitBackend, Head, Log, RefKind, Reference, Result, Stash,
    WorkingTree,
};

/// Object cache per call; avoids decoding commits twice during date-ordered walks.
const OBJECT_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// Repository opened with gitoxide. Cheap to share between threads; every
/// call works on its own thread-local [`gix::Repository`].
pub struct GixBackend {
    repo: gix::ThreadSafeRepository,
    root: PathBuf,
    name: String,
}

impl GixBackend {
    /// Opens the repository containing `path` (the path may be any folder inside it).
    pub fn discover(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::NotFound(path.display().to_string()));
        }
        let repo = gix::ThreadSafeRepository::discover(path)
            .map_err(|_| Error::NotARepository(path.display().to_string()))?;
        let Some(root) = repo.work_dir().map(Path::to_path_buf) else {
            return Err(Error::BareRepository(repo.git_dir().display().to_string()));
        };
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        Ok(Self { repo, root, name })
    }

    fn local(&self) -> gix::Repository {
        let mut repo = self.repo.to_thread_local();
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
        repo
    }
}

impl GitBackend for GixBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn log(&self, limit: usize) -> Result<Log> {
        let repo = self.local();
        let mut tips: HashSet<ObjectId> = ref_targets(&repo)?
            .into_iter()
            .map(|(_, _, id)| id)
            .collect();
        if let Ok(head) = repo.head_id() {
            tips.insert(head.detach());
        }
        if tips.is_empty() {
            return Ok(Log::default());
        }

        let mailmap = repo.open_mailmap();
        let walk = repo
            .rev_walk(tips)
            .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
            .all()
            .map_err(backend)?;

        let mut log = Log::default();
        for info in walk {
            let info = info.map_err(backend)?;
            if log.commits.len() == limit {
                log.truncated = true;
                break;
            }
            let commit = info.object().map_err(backend)?;
            log.commits.push(to_commit(&commit, &mailmap)?);
        }
        Ok(log)
    }

    fn head(&self) -> Result<Head> {
        let repo = self.local();
        let head = repo.head().map_err(backend)?;
        let branch = head
            .referent_name()
            .map(|name| name.shorten().to_str_lossy().into_owned());
        if head.is_unborn() {
            return Ok(Head::Unborn {
                name: branch.unwrap_or_default(),
            });
        }
        let target = repo.head_id().map_err(backend)?.to_string();
        Ok(match branch {
            Some(name) if !head.is_detached() => Head::Branch { name, target },
            _ => Head::Detached { target },
        })
    }

    fn references(&self) -> Result<Vec<Reference>> {
        let repo = self.local();
        let head = self.head()?;
        let targets = ref_targets(&repo)?;

        let mut refs = Vec::with_capacity(targets.len());
        for (kind, full_name, id) in &targets {
            let name = full_name.as_ref().shorten().to_str_lossy().into_owned();
            let mut reference = Reference {
                is_head: *kind == RefKind::LocalBranch && head.branch() == Some(name.as_str()),
                name,
                kind: *kind,
                target: id.to_string(),
                upstream: None,
                ahead_behind: None,
            };
            if *kind == RefKind::LocalBranch {
                let upstream = repo
                    .branch_remote_tracking_ref_name(
                        full_name.as_ref(),
                        gix::remote::Direction::Fetch,
                    )
                    .and_then(std::result::Result::ok);
                if let Some(upstream) = upstream {
                    reference.ahead_behind = targets
                        .iter()
                        .find(|(_, name, _)| *name == upstream)
                        .map(|(_, _, upstream_id)| ahead_behind(&repo, *id, *upstream_id))
                        .transpose()?;
                    reference.upstream =
                        Some(upstream.as_ref().shorten().to_str_lossy().into_owned());
                }
            }
            refs.push(reference);
        }
        Ok(refs)
    }

    fn stashes(&self) -> Result<Vec<Stash>> {
        let repo = self.local();
        let Some(stash) = repo.try_find_reference("refs/stash").map_err(backend)? else {
            return Ok(Vec::new());
        };
        let mut log = stash.log_iter();
        let Some(entries) = log.rev().map_err(backend)? else {
            return Ok(Vec::new());
        };
        entries
            .enumerate()
            .map(|(index, line)| {
                let line = line.map_err(backend)?;
                Ok(Stash {
                    index,
                    message: line.message.to_str_lossy().into_owned(),
                    id: line.new_oid.to_string(),
                })
            })
            .collect()
    }

    fn working_tree(&self) -> Result<WorkingTree> {
        Err(Error::Backend(
            "working tree status is not available yet".into(),
        ))
    }

    fn commit_changes(&self, _id: &str) -> Result<Vec<FileChange>> {
        Err(Error::Backend(
            "commit changes are not available yet".into(),
        ))
    }
}

/// Local branches, remote branches (without symbolic `*/HEAD`) and tags with
/// the commit they point to. Tags pointing to non-commits are skipped.
fn ref_targets(repo: &gix::Repository) -> Result<Vec<(RefKind, gix::refs::FullName, ObjectId)>> {
    let platform = repo.references().map_err(backend)?;
    let groups = [
        (
            RefKind::LocalBranch,
            platform.local_branches().map_err(backend)?,
        ),
        (
            RefKind::RemoteBranch,
            platform.remote_branches().map_err(backend)?,
        ),
        (RefKind::Tag, platform.tags().map_err(backend)?),
    ];

    let mut targets = Vec::new();
    for (kind, iter) in groups {
        for reference in iter {
            let mut reference = reference.map_err(backend)?;
            if kind == RefKind::RemoteBranch
                && matches!(reference.target(), gix::refs::TargetRef::Symbolic(_))
            {
                continue;
            }
            let Ok(commit) = reference.peel_to_commit() else {
                continue;
            };
            targets.push((kind, reference.name().to_owned(), commit.id));
        }
    }
    Ok(targets)
}

/// Commits reachable from `local` but not `upstream`, and vice versa.
fn ahead_behind(
    repo: &gix::Repository,
    local: ObjectId,
    upstream: ObjectId,
) -> Result<(usize, usize)> {
    if local == upstream {
        return Ok((0, 0));
    }
    let count = |from: ObjectId, hidden: ObjectId| -> Result<usize> {
        let walk = repo
            .rev_walk([from])
            .with_hidden([hidden])
            .all()
            .map_err(backend)?;
        let mut count = 0;
        for info in walk {
            info.map_err(backend)?;
            count += 1;
        }
        Ok(count)
    };
    Ok((count(local, upstream)?, count(upstream, local)?))
}

fn to_commit(commit: &gix::Commit<'_>, mailmap: &gix::mailmap::Snapshot) -> Result<Commit> {
    let decoded = commit.decode().map_err(backend)?;
    let author = decoded.author().map_err(backend)?;
    let committer = decoded.committer().map_err(backend)?;
    let identity = mailmap.resolve_cow(author);
    let message = decoded.message();
    Ok(Commit {
        id: commit.id.to_string(),
        parents: decoded.parents().map(|id| id.to_string()).collect(),
        summary: message.summary().to_str_lossy().into_owned(),
        body: message
            .body
            .map(|body| body.to_str_lossy().trim_end().to_owned())
            .unwrap_or_default(),
        author: identity.name.to_str_lossy().into_owned(),
        email: identity.email.to_str_lossy().into_owned(),
        author_time: author.seconds(),
        commit_time: committer.seconds(),
    })
}

fn backend(error: impl Display) -> Error {
    Error::Backend(error.to_string())
}
