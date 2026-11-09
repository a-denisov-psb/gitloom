//! Read access through gitoxide.

use std::collections::HashSet;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;
use gix::ObjectId;

use crate::{
    diff, ChangeKind, Commit, DiffKind, DiffRequest, Error, FileChange, FileDiff, GitBackend, Head,
    Log, RefKind, Reference, RepoState, Result, Stash, StashChanges, StashPart, WorkingTree,
};

/// Object cache per call; avoids decoding commits twice during date-ordered walks.
const OBJECT_CACHE_BYTES: usize = 16 * 1024 * 1024;

/// Repository opened with gitoxide. Cheap to share between threads; every
/// call works on its own thread-local [`gix::Repository`].
pub struct GixBackend {
    /// Reopened when a config file changes (gix reads them only on open).
    repo: std::sync::RwLock<gix::ThreadSafeRepository>,
    /// Config files the open repository depends on and their content.
    config: std::sync::Mutex<ConfigFiles>,
    root: PathBuf,
    name: String,
    /// Objects may be missing locally and fetched on demand (blobless clone).
    partial_clone: bool,
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
        // Absolute (not canonical) so paths compare well and stay readable on Windows.
        let root = std::path::absolute(&root).unwrap_or(root);
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        // Read the config files before the final open: a change in between
        // is then seen as a change (and not lost).
        let files = config_paths(&repo.to_thread_local());
        let fingerprint = fingerprint(&files);
        let repo = gix::ThreadSafeRepository::open(repo.git_dir())
            .map_err(|_| Error::NotARepository(path.display().to_string()))?;
        let mut config = ConfigFiles {
            files,
            fingerprint,
            stale: false,
        };
        config.extend(&repo.to_thread_local());
        let partial_clone = is_partial_clone(&repo.to_thread_local());
        Ok(Self {
            repo: std::sync::RwLock::new(repo),
            config: std::sync::Mutex::new(config),
            root,
            name,
            partial_clone,
        })
    }

    /// The repository is a partial clone (`extensions.partialClone` or a
    /// promisor remote); missing blobs can be fetched with `git`.
    pub fn is_partial_clone(&self) -> bool {
        self.partial_clone
    }

    /// What a batch of changed absolute paths (from a file watcher) requires.
    /// Ignored working tree paths and `.git` internals other than the index,
    /// refs and operation state are dropped.
    pub fn classify_changes(&self, paths: &[PathBuf]) -> crate::ChangeSet {
        use gix::index::entry::Mode;

        let repo = self.local();
        let git_dir = std::path::absolute(repo.git_dir()).unwrap_or_else(|_| repo.git_dir().into());
        // Watchers may report canonical paths (e.g. /private/var on macOS).
        let roots: Vec<PathBuf> = [Some(self.root.clone()), self.root.canonicalize().ok()]
            .into_iter()
            .flatten()
            .collect();
        let git_dirs: Vec<PathBuf> = [Some(git_dir.clone()), git_dir.canonicalize().ok()]
            .into_iter()
            .flatten()
            .collect();

        let index = repo.index_or_empty().ok();
        let mut stack = index
            .as_ref()
            .and_then(|index| repo.excludes(index, None, Default::default()).ok());
        let mut ignored = crate::watch::IgnoreCache::new(|path: &Path, is_dir: bool| {
            let mode = if is_dir { Mode::DIR } else { Mode::FILE };
            stack.as_mut().is_some_and(|stack| {
                stack
                    .at_path(path, Some(mode))
                    .is_ok_and(|platform| platform.is_excluded())
            })
        });

        let mut changes = crate::ChangeSet::default();
        for path in paths {
            if let Some(relative) = git_dirs.iter().find_map(|dir| path.strip_prefix(dir).ok()) {
                let change = crate::classify_git_path(relative);
                changes.status |= change.status;
                changes.refs |= change.refs;
            } else if let Some(relative) =
                roots.iter().find_map(|root| path.strip_prefix(root).ok())
            {
                if !relative.as_os_str().is_empty() && !ignored.is_ignored(relative) {
                    changes.status = true;
                }
            }
            if changes.status && changes.refs {
                break;
            }
        }
        changes
    }

    /// Files changed between two trees, with rename detection, sorted by path.
    fn tree_changes(
        &self,
        repo: &gix::Repository,
        old_tree: &gix::Tree<'_>,
        new_tree: &gix::Tree<'_>,
    ) -> Result<Vec<FileChange>> {
        use gix::object::tree::diff::ChangeDetached;

        let options = gix::diff::Options::default().with_rewrites(Some(Default::default()));
        let changes = match repo.diff_tree_to_tree(old_tree, new_tree, options) {
            Ok(changes) => changes,
            // Similarity rename detection reads blob contents, which a partial
            // clone may not have; renames then show as deletion plus addition.
            Err(_) if self.partial_clone => repo
                .diff_tree_to_tree(
                    old_tree,
                    new_tree,
                    gix::diff::Options::default().with_rewrites(None),
                )
                .map_err(backend)?,
            Err(error) => return Err(backend(error)),
        };

        let mut files: Vec<FileChange> = changes
            .iter()
            .filter_map(|change| {
                let (kind, mode, old_path) = match change {
                    ChangeDetached::Addition { entry_mode, .. } => {
                        (ChangeKind::Added, entry_mode, None)
                    }
                    ChangeDetached::Deletion { entry_mode, .. } => {
                        (ChangeKind::Deleted, entry_mode, None)
                    }
                    ChangeDetached::Modification { entry_mode, .. } => {
                        (ChangeKind::Modified, entry_mode, None)
                    }
                    ChangeDetached::Rewrite {
                        entry_mode,
                        copy: true,
                        ..
                    } => (ChangeKind::Added, entry_mode, None),
                    ChangeDetached::Rewrite {
                        entry_mode,
                        source_location,
                        ..
                    } => (
                        ChangeKind::Renamed,
                        entry_mode,
                        Some(source_location.to_str_lossy().into_owned()),
                    ),
                };
                // Directories are reported as well; only files are of interest.
                (!mode.is_tree()).then(|| FileChange {
                    path: change.location().to_str_lossy().into_owned(),
                    kind,
                    old_path,
                })
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    fn local(&self) -> gix::Repository {
        self.reload_if_config_changed();
        let shared = self.repo.read().unwrap_or_else(|e| e.into_inner());
        let mut repo = shared.to_thread_local();
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
        repo
    }

    /// gix reads the config only when opening: reopen after `git` or any
    /// other program changed a config file (new remote, upstream set by
    /// `push -u`, a rewrite by `git filter-repo`, the global config, ...).
    /// Compares contents, not modification times: several writes within one
    /// timestamp tick (about 16 ms on Windows) would otherwise go unnoticed
    /// and leave a stale state until a restart (M6-18).
    fn reload_if_config_changed(&self) {
        let mut config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        let now = fingerprint(&config.files);
        if now == config.fingerprint && !config.stale {
            return;
        }
        let git_dir = self
            .repo
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .git_dir()
            .to_path_buf();
        // On failure (e.g. a half-written config) keep the old state and retry later.
        if let Ok(fresh) = gix::ThreadSafeRepository::open(git_dir) {
            config.fingerprint = now;
            config.stale = false;
            config.extend(&fresh.to_thread_local());
            *self.repo.write().unwrap_or_else(|e| e.into_inner()) = fresh;
        }
    }
}

/// The config files an open repository was built from.
struct ConfigFiles {
    files: Vec<PathBuf>,
    /// Content hash per file (`None`: missing or unreadable).
    fingerprint: Vec<Option<u64>>,
    /// Reopen on the next access regardless of the fingerprint.
    stale: bool,
}

impl ConfigFiles {
    /// Adds files that `repo` loaded and that are not tracked yet
    /// (includes), with their current content.
    fn extend(&mut self, repo: &gix::Repository) {
        for path in config_paths(repo) {
            if !self.files.contains(&path) {
                self.fingerprint.push(file_hash(&path));
                self.files.push(path);
            }
        }
    }
}

/// Config files `repo` depends on: repository, worktree, global and system
/// config (also when they do not exist yet) and every included file.
fn config_paths(repo: &gix::Repository) -> Vec<PathBuf> {
    use gix::config::Source;

    let mut paths = vec![
        repo.git_dir().join("config"),
        repo.common_dir().join("config"),
        repo.git_dir().join("config.worktree"),
    ];
    let mut env = |name: &str| std::env::var_os(name);
    for source in [
        Source::GitInstallation,
        Source::System,
        Source::Git,
        Source::User,
    ] {
        if let Some(path) = source.storage_location(&mut env) {
            paths.push(path);
        }
    }
    let config = repo.config_snapshot();
    paths.extend(
        config
            .plumbing()
            .sections()
            .filter_map(|section| section.meta().path.clone()),
    );
    let mut unique = Vec::with_capacity(paths.len());
    for path in paths {
        if !unique.contains(&path) {
            unique.push(path);
        }
    }
    unique
}

fn fingerprint(files: &[PathBuf]) -> Vec<Option<u64>> {
    files.iter().map(|path| file_hash(path)).collect()
}

/// Hash of the file content (`None` if it does not exist or cannot be read).
fn file_hash(path: &Path) -> Option<u64> {
    use std::hash::{Hash as _, Hasher as _};
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(hasher.finish())
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
                let commit = repo.find_commit(line.new_oid).map_err(backend)?;
                let decoded = commit.decode().map_err(backend)?;
                let base = decoded.parents().next().map(|id| id.to_string());
                let committer = decoded.committer().map_err(backend)?;
                let author = decoded.author().map_err(backend)?;
                Ok(Stash {
                    index,
                    message: line.message.to_str_lossy().into_owned(),
                    id: line.new_oid.to_string(),
                    base,
                    time: committer.seconds(),
                    author: author.name.to_str_lossy().into_owned(),
                })
            })
            .collect()
    }

    fn working_tree(&self) -> Result<WorkingTree> {
        use gix::status::index_worktree::Item as WorktreeItem;
        use gix::status::plumbing::index_as_worktree_with_renames::Summary;
        use gix::status::Item;

        let repo = self.local();
        let items = repo
            .status(gix::progress::Discard)
            .map_err(backend)?
            .untracked_files(gix::status::UntrackedFiles::Files)
            .into_iter(None)
            .map_err(backend)?;

        let mut tree = WorkingTree::default();
        let mut conflicted = HashSet::new();
        for item in items {
            match item.map_err(backend)? {
                Item::IndexWorktree(item) => {
                    let Some(summary) = item.summary() else {
                        continue;
                    };
                    let path = item.rela_path().to_str_lossy().into_owned();
                    let kind = match summary {
                        Summary::Removed => ChangeKind::Deleted,
                        Summary::Added => ChangeKind::Untracked,
                        Summary::Modified | Summary::TypeChange => ChangeKind::Modified,
                        Summary::Renamed => ChangeKind::Renamed,
                        Summary::Copied | Summary::IntentToAdd => ChangeKind::Added,
                        Summary::Conflict => ChangeKind::Conflicted,
                    };
                    // Conflicts are reported once per stage; list the path once.
                    if kind == ChangeKind::Conflicted && !conflicted.insert(path.clone()) {
                        continue;
                    }
                    let old_path = match &item {
                        WorktreeItem::Rewrite { source, .. } => {
                            Some(source.rela_path().to_str_lossy().into_owned())
                        }
                        _ => None,
                    };
                    tree.unstaged.push(FileChange {
                        path,
                        kind,
                        old_path,
                    });
                }
                Item::TreeIndex(change) => tree.staged.push(index_change(&change)),
            }
        }
        // Unmerged paths only show up as conflicts, not as staged changes.
        tree.staged.retain(|f| !conflicted.contains(&f.path));
        tree.unstaged.sort_by(|a, b| a.path.cmp(&b.path));
        tree.staged.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(tree)
    }

    fn commit_changes(&self, id: &str) -> Result<Vec<FileChange>> {
        let repo = self.local();
        let commit = find_commit(&repo, id)?;
        let new_tree = commit.tree().map_err(backend)?;
        let old_tree = match commit.parent_ids().next() {
            Some(parent) => parent
                .object()
                .map_err(backend)?
                .peel_to_tree()
                .map_err(backend)?,
            None => repo.empty_tree(),
        };
        self.tree_changes(&repo, &old_tree, &new_tree)
    }

    fn remotes(&self) -> Result<Vec<String>> {
        Ok(self
            .local()
            .remote_names()
            .into_iter()
            .map(|name| name.to_string())
            .collect())
    }

    fn external_remotes(&self) -> Result<Vec<(String, crate::ConfigScope)>> {
        use gix::config::Source;

        let repo = self.local();
        let names = repo.remote_names();
        let config = repo.config_snapshot();
        let Some(sections) = config.plumbing().sections_by_name("remote") else {
            return Ok(Vec::new());
        };
        // Sources per remote name (only remotes gix lists, i.e. trusted ones).
        let mut sources: Vec<(String, Vec<Source>)> = Vec::new();
        for section in sections {
            let Some(name) = section.header().subsection_name() else {
                continue;
            };
            if !names.contains(name) {
                continue;
            }
            let name = name.to_string();
            let source = section.meta().source;
            match sources.iter_mut().find(|(n, _)| *n == name) {
                Some((_, list)) => list.push(source),
                None => sources.push((name, vec![source])),
            }
        }
        Ok(sources
            .into_iter()
            .filter(|(_, list)| {
                !list
                    .iter()
                    .any(|s| matches!(s, Source::Local | Source::Worktree))
            })
            .map(|(name, list)| {
                let scope = match list.first() {
                    Some(Source::User | Source::Git) => crate::ConfigScope::Global,
                    Some(Source::System | Source::GitInstallation) => crate::ConfigScope::System,
                    _ => crate::ConfigScope::Other,
                };
                (name, scope)
            })
            .collect())
    }

    fn reload(&self) {
        self.config.lock().unwrap_or_else(|e| e.into_inner()).stale = true;
    }

    fn remote_url(&self, name: &str) -> Result<Option<String>> {
        let repo = self.local();
        let Ok(remote) = repo.find_remote(name) else {
            return Ok(None);
        };
        Ok(remote
            .url(gix::remote::Direction::Fetch)
            .map(|url| url.to_bstring().to_string()))
    }

    fn stash_changes(&self, id: &str) -> Result<StashChanges> {
        let repo = self.local();
        let stash = find_commit(&repo, id)?;
        let parents = stash_parent_trees(&stash)?;
        let (Some(head), Some(index)) = (parents.first(), parents.get(1)) else {
            return Err(Error::Backend(format!("{id} is not a stash commit")));
        };
        let worktree = stash.tree().map_err(backend)?;
        let untracked = match parents.get(2) {
            Some(tree) => self
                .tree_changes(&repo, &repo.empty_tree(), tree)?
                .into_iter()
                .map(|file| FileChange {
                    kind: ChangeKind::Untracked,
                    ..file
                })
                .collect(),
            None => Vec::new(),
        };
        Ok(StashChanges {
            staged: self.tree_changes(&repo, head, index)?,
            unstaged: self.tree_changes(&repo, index, &worktree)?,
            untracked,
        })
    }

    fn commit_template(&self) -> Result<Option<String>> {
        let repo = self.local();
        let config = repo.config_snapshot();
        let Some(path) = config.trusted_path("commit.template").map_err(backend)? else {
            return Ok(None);
        };
        // A missing template file is ignored like Git warns and continues.
        Ok(std::fs::read_to_string(path).ok())
    }

    fn state(&self) -> Result<RepoState> {
        Ok(crate::state::detect(self.local().git_dir()))
    }

    fn merge_message(&self) -> Result<Option<String>> {
        let repo = self.local();
        let git_dir = repo.git_dir();
        let read = |name: &str| std::fs::read_to_string(git_dir.join(name)).ok();
        Ok(match crate::state::detect(git_dir) {
            RepoState::Clean => None,
            // A stopped rebase step has MERGE_MSG after a conflict, otherwise
            // only the message of the commit being applied.
            RepoState::Rebasing { .. } => {
                read("MERGE_MSG").or_else(|| read("rebase-merge/message"))
            }
            _ => read("MERGE_MSG"),
        })
    }

    fn worktree_content(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let repo = self.local();
        match worktree_file(&repo, &self.root, path)? {
            Some(Blob::Data(data)) => Ok(Some(data)),
            Some(Blob::TooLarge) => Err(Error::Backend(format!("{path} is too large"))),
            None => Ok(None),
        }
    }

    fn file_diff_with(
        &self,
        request: &DiffRequest,
        context: crate::DiffContext,
    ) -> Result<FileDiff> {
        let repo = self.local();
        let old_path = request.old_path.as_deref().unwrap_or(&request.path);
        let (old, new) = match &request.kind {
            DiffKind::Commit(id) => {
                let commit = find_commit(&repo, id)?;
                let new_tree = commit.tree().map_err(backend)?;
                let old_tree = match commit.parent_ids().next() {
                    Some(parent) => Some(
                        parent
                            .object()
                            .map_err(backend)?
                            .peel_to_tree()
                            .map_err(backend)?,
                    ),
                    None => None,
                };
                let old = match &old_tree {
                    Some(tree) => tree_blob(&repo, tree, old_path)?,
                    None => None,
                };
                (old, tree_blob(&repo, &new_tree, &request.path)?)
            }
            DiffKind::Stash { id, part } => {
                let stash = find_commit(&repo, id)?;
                let parents = stash_parent_trees(&stash)?;
                let worktree = stash.tree().map_err(backend)?;
                let (old_tree, new_tree) = match part {
                    StashPart::Staged => (parents.first(), parents.get(1)),
                    StashPart::Unstaged => (parents.get(1), Some(&worktree)),
                    StashPart::Untracked => (None, parents.get(2)),
                };
                let old = match old_tree {
                    Some(tree) => tree_blob(&repo, tree, old_path)?,
                    None => None,
                };
                let new = match new_tree {
                    Some(tree) => tree_blob(&repo, tree, &request.path)?,
                    None => None,
                };
                (old, new)
            }
            DiffKind::Staged => {
                let old = match repo.head_tree() {
                    Ok(tree) => tree_blob(&repo, &tree, old_path)?,
                    // Unborn HEAD: everything in the index is new.
                    Err(_) => None,
                };
                (old, index_blob(&repo, &request.path)?)
            }
            DiffKind::Unstaged => (
                index_blob(&repo, old_path)?,
                worktree_file(&repo, &self.root, &request.path)?,
            ),
            DiffKind::Untracked => (None, worktree_file(&repo, &self.root, &request.path)?),
        };

        let text = match (&old, &new) {
            (Some(Blob::TooLarge), _) | (_, Some(Blob::TooLarge)) => diff::TextDiff {
                too_large: true,
                ..diff::TextDiff::default()
            },
            _ => diff::text_diff_with(
                old.as_ref().map_or(&[][..], Blob::bytes),
                new.as_ref().map_or(&[][..], Blob::bytes),
                context,
            ),
        };
        Ok(FileDiff {
            old_path: old.is_some().then(|| old_path.to_owned()),
            new_path: new.is_some().then(|| request.path.clone()),
            binary: text.binary,
            too_large: text.too_large,
            hunks: text.hunks,
        })
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

/// Content of one side of a diff.
enum Blob {
    Data(Vec<u8>),
    /// Larger than [`diff::MAX_DIFF_BYTES`]; not loaded.
    TooLarge,
}

impl Blob {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Data(data) => data,
            Self::TooLarge => &[],
        }
    }
}

/// Trees of a stash commit's parents: HEAD, index and (with `-u`) untracked files.
fn stash_parent_trees<'r>(stash: &gix::Commit<'r>) -> Result<Vec<gix::Tree<'r>>> {
    stash
        .parent_ids()
        .map(|parent| {
            parent
                .object()
                .map_err(backend)?
                .peel_to_tree()
                .map_err(backend)
        })
        .collect()
}

fn find_commit<'r>(repo: &'r gix::Repository, id: &str) -> Result<gix::Commit<'r>> {
    let oid =
        ObjectId::from_hex(id.as_bytes()).map_err(|_| Error::NotFound(format!("commit {id}")))?;
    repo.find_commit(oid)
        .map_err(|_| Error::NotFound(format!("commit {id}")))
}

/// Loads an object as diff side; submodule entries render like Git does.
fn object_blob(
    repo: &gix::Repository,
    id: ObjectId,
    mode: gix::objs::tree::EntryMode,
) -> Result<Blob> {
    if mode.is_commit() {
        return Ok(Blob::Data(format!("Subproject commit {id}\n").into_bytes()));
    }
    if !repo.has_object(id) {
        return Err(Error::MissingObject {
            id: id.to_string(),
            partial_clone: is_partial_clone(repo),
        });
    }
    let header = repo.find_header(id).map_err(backend)?;
    if header.size() > diff::MAX_DIFF_BYTES as u64 {
        return Ok(Blob::TooLarge);
    }
    let object = repo.find_object(id).map_err(backend)?;
    Ok(Blob::Data(object.detach().data))
}

fn tree_blob(repo: &gix::Repository, tree: &gix::Tree<'_>, path: &str) -> Result<Option<Blob>> {
    match tree.lookup_entry_by_path(path).map_err(backend)? {
        Some(entry) if !entry.mode().is_tree() => {
            object_blob(repo, entry.object_id(), entry.mode()).map(Some)
        }
        _ => Ok(None),
    }
}

fn index_blob(repo: &gix::Repository, path: &str) -> Result<Option<Blob>> {
    let index = repo.index_or_empty().map_err(backend)?;
    match index.entry_by_path(path.into()) {
        Some(entry) => {
            let mode = entry
                .mode
                .to_tree_entry_mode()
                .unwrap_or_else(|| gix::objs::tree::EntryKind::Blob.into());
            object_blob(repo, entry.id, mode).map(Some)
        }
        None => Ok(None),
    }
}

/// Working tree content as Git would store it: run through the filter
/// pipeline (eol/`core.autocrlf`, `ident`, configured `filter.*` drivers),
/// so the diff against the index matches `git diff` (ADR-018).
fn worktree_file(repo: &gix::Repository, root: &Path, path: &str) -> Result<Option<Blob>> {
    let full = root.join(path);
    let Ok(metadata) = std::fs::symlink_metadata(&full) else {
        return Ok(None);
    };
    if metadata.is_dir() {
        // Submodule or nested repository: no line diff.
        return Ok(Some(Blob::Data(Vec::new())));
    }
    if metadata.len() > diff::MAX_DIFF_BYTES as u64 {
        return Ok(Some(Blob::TooLarge));
    }
    let data = if metadata.file_type().is_symlink() {
        std::fs::read_link(&full)
            .map_err(backend)?
            .to_string_lossy()
            .into_owned()
            .into_bytes()
    } else {
        to_git(repo, &full, path).map_err(|reason| {
            Error::Backend(format!("Filtered file – diff not available ({reason})"))
        })?
    };
    Ok(Some(Blob::Data(data)))
}

fn to_git(repo: &gix::Repository, full: &Path, path: &str) -> std::result::Result<Vec<u8>, String> {
    use gix::filter::plumbing::pipeline::convert::ToGitOutcome;
    use std::io::Read as _;

    let (mut pipeline, index) = repo.filter_pipeline(None).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(full).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    match pipeline
        .convert_to_git(file, Path::new(path), &index)
        .map_err(|e| e.to_string())?
    {
        ToGitOutcome::Unchanged(mut reader) => {
            reader.read_to_end(&mut data).map_err(|e| e.to_string())?;
        }
        ToGitOutcome::Process(mut reader) => {
            reader.read_to_end(&mut data).map_err(|e| e.to_string())?;
        }
        ToGitOutcome::Buffer(buffer) => data.extend_from_slice(buffer),
    }
    Ok(data)
}

fn index_change(change: &gix::diff::index::Change) -> FileChange {
    use gix::diff::index::ChangeRef;

    let (kind, old_path) = match change {
        ChangeRef::Addition { .. } => (ChangeKind::Added, None),
        ChangeRef::Deletion { .. } => (ChangeKind::Deleted, None),
        ChangeRef::Modification { .. } => (ChangeKind::Modified, None),
        ChangeRef::Rewrite { copy: true, .. } => (ChangeKind::Added, None),
        ChangeRef::Rewrite {
            source_location, ..
        } => (
            ChangeKind::Renamed,
            Some(source_location.to_str_lossy().into_owned()),
        ),
    };
    FileChange {
        path: change.location().to_str_lossy().into_owned(),
        kind,
        old_path,
    }
}

fn is_partial_clone(repo: &gix::Repository) -> bool {
    let config = repo.config_snapshot();
    if config.string("extensions.partialClone").is_some() {
        return true;
    }
    let truthy = |v: &[u8]| {
        let v = String::from_utf8_lossy(v).to_ascii_lowercase();
        matches!(v.as_str(), "true" | "yes" | "on" | "1")
    };
    let promisor = config
        .plumbing()
        .sections_by_name("remote")
        .into_iter()
        .flatten()
        .any(|section| section.value("promisor").is_some_and(|v| truthy(&v)));
    promisor
}

fn backend(error: impl Display) -> Error {
    Error::Backend(error.to_string())
}
