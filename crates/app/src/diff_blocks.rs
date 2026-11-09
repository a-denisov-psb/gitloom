//! Change blocks of the Inline and Split views (M7-03, ADR-038): changed
//! lines whose 3-line contexts touch form one block, the same rule that
//! groups changes into hunks in the Hunk view. A block action is a line
//! selection of the block applied through the line staging path.

use git_core::{DiffLineKind, FileDiff};

/// Unchanged lines `text_diff` keeps around a change (Git's default).
const CONTEXT: u32 = 3;

/// Changed lines `first..=last` of hunk `hunk` (line indices in the hunk).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub hunk: u32,
    pub first: u32,
    pub last: u32,
}

impl Block {
    /// Line `l` of hunk `h` lies in the block (context lines inside it too;
    /// callers pick changed lines only).
    pub fn contains(&self, h: usize, l: usize) -> bool {
        h == self.hunk as usize && (self.first as usize..=self.last as usize).contains(&l)
    }
}

/// The change blocks of `file`, in order.
pub fn blocks(file: &FileDiff) -> Vec<Block> {
    let mut out = Vec::new();
    for (h, hunk) in file.hunks.iter().enumerate() {
        let mut current: Option<Block> = None;
        for (l, line) in hunk.lines.iter().enumerate() {
            if line.kind == DiffLineKind::Context {
                continue;
            }
            let (h, l) = (h as u32, l as u32);
            current = match current {
                // Context lines between the two changes: l - last - 1.
                Some(block) if l - block.last - 1 <= 2 * CONTEXT => {
                    Some(Block { last: l, ..block })
                }
                Some(block) => {
                    out.push(block);
                    Some(Block {
                        hunk: h,
                        first: l,
                        last: l,
                    })
                }
                None => Some(Block {
                    hunk: h,
                    first: l,
                    last: l,
                }),
            };
        }
        out.extend(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full(old: &str, new: &str) -> FileDiff {
        let text =
            git_core::text_diff_with(old.as_bytes(), new.as_bytes(), git_core::DiffContext::Full);
        FileDiff {
            old_path: Some("f".into()),
            new_path: Some("f".into()),
            binary: text.binary,
            too_large: text.too_large,
            hunks: text.hunks,
        }
    }

    fn numbered(n: u32) -> String {
        (1..=n).map(|i| format!("line {i}\n")).collect()
    }

    #[test]
    fn distant_changes_are_separate_blocks() {
        let old = numbered(30);
        let new = old
            .replace("line 3\n", "three\n")
            .replace("line 25\n", "twenty-five\n");
        let blocks = blocks(&full(&old, &new));
        // Lines: 0 1 [2 -] [3 +] ... line 25 removed at index 25, added 26.
        assert_eq!(
            blocks,
            [
                Block {
                    hunk: 0,
                    first: 2,
                    last: 3
                },
                Block {
                    hunk: 0,
                    first: 25,
                    last: 26
                }
            ]
        );
    }

    #[test]
    fn changes_with_touching_contexts_share_a_block() {
        let old = numbered(30);
        // Six unchanged lines between: contexts of 3 touch.
        let near = old
            .replace("line 5\n", "five\n")
            .replace("line 12\n", "twelve\n");
        assert_eq!(blocks(&full(&old, &near)).len(), 1);
        // Seven unchanged lines between: separate.
        let far = old
            .replace("line 5\n", "five\n")
            .replace("line 13\n", "thirteen\n");
        assert_eq!(blocks(&full(&old, &far)).len(), 2);
    }

    #[test]
    fn blocks_equal_the_hunks_of_the_hunk_view() {
        let old = numbered(60);
        let new = old
            .replace("line 2\n", "two\n")
            .replace("line 10\n", "")
            .replace("line 30\n", "thirty\nthirty b\n")
            .replace("line 36\n", "36\n")
            .replace("line 60\n", "sixty");
        let hunks =
            git_core::text_diff_with(old.as_bytes(), new.as_bytes(), git_core::DiffContext::Hunks)
                .hunks;
        let full = full(&old, &new);
        let blocks = blocks(&full);
        assert_eq!(blocks.len(), hunks.len());
        // Same changed lines per block and hunk.
        for (block, hunk) in blocks.iter().zip(&hunks) {
            let in_block: Vec<_> = full.hunks[0].lines[block.first as usize..=block.last as usize]
                .iter()
                .filter(|l| l.kind != DiffLineKind::Context)
                .map(|l| (l.kind, l.content.clone()))
                .collect();
            let in_hunk: Vec<_> = hunk
                .lines
                .iter()
                .filter(|l| l.kind != DiffLineKind::Context)
                .map(|l| (l.kind, l.content.clone()))
                .collect();
            assert_eq!(in_block, in_hunk);
        }
    }

    #[test]
    fn no_changes_no_blocks() {
        assert!(blocks(&full("a\n", "a\n")).is_empty());
    }

    /// Real repositories (M7-03 acceptance): a block action gives the same
    /// index or working tree as the same change's hunk action in Hunk mode,
    /// and a line selection the same as in Hunk mode.
    mod against_hunk_mode {
        use std::path::Path;
        use std::process::Command;

        use git_core::{
            build_patch, CliBackend, DiffContext, DiffKind, DiffRequest, FileDiff, GitBackend,
            GitWriter, GixBackend, PatchDirection,
        };

        use super::super::blocks;

        const PATH: &str = "f.txt";

        fn git(dir: &Path, args: &[&str]) {
            let status = Command::new("git")
                .args(args)
                .current_dir(dir)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("HOME", dir)
                .env("XDG_CONFIG_HOME", dir)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .output()
                .unwrap();
            assert!(status.status.success(), "{args:?}: {status:?}");
        }

        /// Repository with `old` committed and `new` in the working tree
        /// (also staged when `staged`).
        fn repo(old: &str, new: &str, staged: bool) -> tempfile::TempDir {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            git(root, &["init", "-q", "-b", "main"]);
            git(root, &["config", "core.autocrlf", "false"]);
            std::fs::write(root.join(PATH), old).unwrap();
            git(root, &["add", PATH]);
            git(root, &["commit", "-q", "-m", "base"]);
            std::fs::write(root.join(PATH), new).unwrap();
            if staged {
                git(root, &["add", PATH]);
            }
            dir
        }

        fn diff(root: &Path, kind: DiffKind, context: DiffContext) -> FileDiff {
            GixBackend::discover(root)
                .unwrap()
                .file_diff_with(
                    &DiffRequest {
                        kind,
                        path: PATH.into(),
                        old_path: None,
                    },
                    context,
                )
                .unwrap()
        }

        fn index(root: &Path) -> Vec<u8> {
            Command::new("git")
                .args(["show", &format!(":{PATH}")])
                .current_dir(root)
                .output()
                .unwrap()
                .stdout
        }

        fn worktree(root: &Path) -> Vec<u8> {
            std::fs::read(root.join(PATH)).unwrap()
        }

        /// Old/new contents: LF, CRLF and without trailing newline.
        fn cases() -> Vec<(String, String)> {
            let lf: String = (1..=40).map(|i| format!("line {i}\n")).collect();
            let lf_new = lf
                .replace("line 3\n", "three\n")
                .replace("line 20\n", "")
                .replace("line 21\n", "twenty-one\nextra\n")
                .replace("line 38\n", "38\n");
            let crlf = lf.replace('\n', "\r\n");
            let crlf_new = lf_new.replace('\n', "\r\n");
            let no_eol = lf.trim_end().to_owned();
            let no_eol_new = format!("{}\nappended", lf_new.trim_end());
            vec![(lf, lf_new), (crlf, crlf_new), (no_eol, no_eol_new)]
        }

        /// Changed lines of `file` as `(hunk, line)`, in order.
        fn changed(file: &FileDiff) -> Vec<(usize, usize)> {
            file.hunks
                .iter()
                .enumerate()
                .flat_map(|(h, hunk)| {
                    hunk.lines
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.kind != git_core::DiffLineKind::Context)
                        .map(move |(l, _)| (h, l))
                })
                .collect()
        }

        #[test]
        fn stage_and_unstage_by_block_equal_hunks() {
            for (old, new) in cases() {
                for staged in [false, true] {
                    let (kind, direction) = if staged {
                        (DiffKind::Staged, PatchDirection::Unstage)
                    } else {
                        (DiffKind::Unstaged, PatchDirection::Stage)
                    };
                    let hunks = diff(
                        repo(&old, &new, staged).path(),
                        kind.clone(),
                        DiffContext::Hunks,
                    );
                    for (index_of_block, _) in hunks.hunks.iter().enumerate() {
                        let (hunk_repo, block_repo) =
                            (repo(&old, &new, staged), repo(&old, &new, staged));
                        let hunk_diff = diff(hunk_repo.path(), kind.clone(), DiffContext::Hunks);
                        let full = diff(block_repo.path(), kind.clone(), DiffContext::Full);
                        let block = blocks(&full)[index_of_block];
                        let by_hunk =
                            build_patch(&hunk_diff, direction, |h, _| h == index_of_block).unwrap();
                        let by_block =
                            build_patch(&full, direction, |h, l| block.contains(h, l)).unwrap();
                        CliBackend::new(hunk_repo.path())
                            .apply_cached(&by_hunk, staged)
                            .unwrap();
                        CliBackend::new(block_repo.path())
                            .apply_cached(&by_block, staged)
                            .unwrap();
                        assert_eq!(
                            index(block_repo.path()),
                            index(hunk_repo.path()),
                            "block {index_of_block} staged {staged} old {old:?}"
                        );
                    }
                }
            }
        }

        #[test]
        fn discard_by_block_and_by_line_equal_hunk_mode() {
            for (old, new) in cases() {
                let count = diff(
                    repo(&old, &new, false).path(),
                    DiffKind::Unstaged,
                    DiffContext::Hunks,
                )
                .hunks
                .len();
                for b in 0..count {
                    let (hunk_repo, block_repo) =
                        (repo(&old, &new, false), repo(&old, &new, false));
                    let hunk_diff = diff(hunk_repo.path(), DiffKind::Unstaged, DiffContext::Hunks);
                    let full = diff(block_repo.path(), DiffKind::Unstaged, DiffContext::Full);
                    let block = blocks(&full)[b];
                    let content = worktree(hunk_repo.path());
                    let writer = CliBackend::new(hunk_repo.path());
                    git_core::discard_lines(
                        &writer,
                        hunk_repo.path(),
                        &hunk_diff,
                        &content,
                        |h, _| h == b,
                    )
                    .unwrap();
                    let writer = CliBackend::new(block_repo.path());
                    git_core::discard_lines(&writer, block_repo.path(), &full, &content, |h, l| {
                        block.contains(h, l)
                    })
                    .unwrap();
                    assert_eq!(
                        worktree(block_repo.path()),
                        worktree(hunk_repo.path()),
                        "{old:?}"
                    );
                }

                // Lines: every second changed line, in both modes.
                let (hunk_repo, full_repo) = (repo(&old, &new, false), repo(&old, &new, false));
                let hunk_diff = diff(hunk_repo.path(), DiffKind::Unstaged, DiffContext::Hunks);
                let full = diff(full_repo.path(), DiffKind::Unstaged, DiffContext::Full);
                let (in_hunks, in_full) = (changed(&hunk_diff), changed(&full));
                assert_eq!(in_hunks.len(), in_full.len());
                let pick = |lines: &[(usize, usize)]| -> Vec<(usize, usize)> {
                    lines.iter().copied().step_by(2).collect()
                };
                let (pick_hunks, pick_full) = (pick(&in_hunks), pick(&in_full));
                for (root, file, picked) in [
                    (hunk_repo.path(), &hunk_diff, &pick_hunks),
                    (full_repo.path(), &full, &pick_full),
                ] {
                    let patch =
                        build_patch(file, PatchDirection::Stage, |h, l| picked.contains(&(h, l)))
                            .unwrap();
                    CliBackend::new(root).apply_cached(&patch, false).unwrap();
                }
                assert_eq!(index(full_repo.path()), index(hunk_repo.path()), "{old:?}");
            }
        }
    }
}
