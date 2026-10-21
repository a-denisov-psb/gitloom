//! Classification of file system events for incremental refreshes.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// What a batch of file system events requires.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChangeSet {
    /// Working tree or index changed: reload the status.
    pub status: bool,
    /// HEAD, a ref or an operation state changed: check whether any ref target moved.
    pub refs: bool,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        !self.status && !self.refs
    }
}

/// Effect of a path inside the `.git` directory, given relative to it.
pub fn classify_git_path(relative: &Path) -> ChangeSet {
    let mut components = relative.components().filter_map(|c| match c {
        Component::Normal(name) => name.to_str(),
        _ => None,
    });
    let Some(first) = components.next() else {
        return ChangeSet::default();
    };
    let is_lock = relative.extension().is_some_and(|ext| ext == "lock");
    if is_lock {
        // Lock files are renamed into place; the final name produces its own event.
        return ChangeSet::default();
    }
    match first {
        "index" => ChangeSet {
            status: true,
            refs: false,
        },
        "HEAD" | "packed-refs" | "refs" | "MERGE_HEAD" | "CHERRY_PICK_HEAD" | "REVERT_HEAD"
        | "rebase-merge" | "rebase-apply" => ChangeSet {
            status: false,
            refs: true,
        },
        _ => ChangeSet::default(),
    }
}

/// Caches ignore decisions per directory while classifying one batch.
pub(crate) struct IgnoreCache<F> {
    is_excluded: F,
    dirs: HashMap<PathBuf, bool>,
}

impl<F: FnMut(&Path, bool) -> bool> IgnoreCache<F> {
    /// `is_excluded(path, is_dir)` answers for a single repo-relative path.
    pub(crate) fn new(is_excluded: F) -> Self {
        Self {
            is_excluded,
            dirs: HashMap::new(),
        }
    }

    /// A path is ignored if it or any parent directory is excluded.
    pub(crate) fn is_ignored(&mut self, relative: &Path) -> bool {
        let mut prefix = PathBuf::new();
        let components: Vec<_> = relative.components().collect();
        let Some((last, dirs)) = components.split_last() else {
            return false;
        };
        for dir in dirs {
            prefix.push(dir);
            let excluded = match self.dirs.get(&prefix) {
                Some(&excluded) => excluded,
                None => {
                    let excluded = (self.is_excluded)(&prefix, true);
                    self.dirs.insert(prefix.clone(), excluded);
                    excluded
                }
            };
            if excluded {
                return true;
            }
        }
        prefix.push(last);
        (self.is_excluded)(&prefix, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(path: &str) -> ChangeSet {
        classify_git_path(Path::new(path))
    }

    const STATUS: ChangeSet = ChangeSet {
        status: true,
        refs: false,
    };
    const REFS: ChangeSet = ChangeSet {
        status: false,
        refs: true,
    };

    #[test]
    fn git_internals() {
        assert_eq!(git("index"), STATUS);
        assert_eq!(git("HEAD"), REFS);
        assert_eq!(git("refs/heads/main"), REFS);
        assert_eq!(git("refs/remotes/origin/feature/x"), REFS);
        assert_eq!(git("packed-refs"), REFS);
        assert_eq!(git("MERGE_HEAD"), REFS);
        assert_eq!(git("rebase-merge/done"), REFS);
        assert!(git("objects/ab/cdef").is_empty());
        assert!(git("logs/HEAD").is_empty());
        assert!(git("index.lock").is_empty());
        assert!(git("refs/heads/main.lock").is_empty());
        assert!(git("COMMIT_EDITMSG").is_empty());
        assert!(git("FETCH_HEAD").is_empty());
        assert!(git("").is_empty());
    }

    #[test]
    fn ignored_parent_directory_ignores_children() {
        let mut calls = Vec::new();
        let mut cache = IgnoreCache::new(|path: &Path, is_dir: bool| {
            calls.push((path.to_path_buf(), is_dir));
            is_dir && path == Path::new("target")
        });
        assert!(cache.is_ignored(Path::new("target/debug/deps/x.o")));
        assert!(cache.is_ignored(Path::new("target/release/y")));
        assert!(!cache.is_ignored(Path::new("src/main.rs")));
        drop(cache);
        // "target" was looked up once and then cached.
        let target_lookups = calls
            .iter()
            .filter(|(p, _)| p == Path::new("target"))
            .count();
        assert_eq!(target_lookups, 1);
    }

    #[test]
    fn file_patterns_apply_to_the_file_itself() {
        let mut cache = IgnoreCache::new(|path: &Path, is_dir: bool| {
            !is_dir && path.extension().is_some_and(|e| e == "log")
        });
        assert!(cache.is_ignored(Path::new("logs/today.log")));
        assert!(!cache.is_ignored(Path::new("logs/today.txt")));
    }
}
