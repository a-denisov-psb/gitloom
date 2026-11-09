// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::collections::HashSet;

use git_core::{
    build_patch, CliBackend, DiffContext, DiffKind, DiffLineKind, DiffRequest, FileDiff,
    GitBackend, GitWriter, GixBackend, PatchDirection,
};
use support::TestRepo;

const PATH: &str = "f.txt";

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x % n
    }
}

fn random_lines(rng: &mut Rng, max: u64) -> Vec<String> {
    let words = [
        "alpha", "beta", "gamma", "delta", "eps", "zeta", "eta", "theta",
    ];
    (0..rng.below(max) + 1)
        .map(|_| words[rng.below(words.len() as u64) as usize].to_owned())
        .collect()
}

/// Mutates `lines`: replacements, insertions and deletions at random places.
fn mutate(rng: &mut Rng, lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        match rng.below(10) {
            0 => {}
            1 => out.push(format!("{line}-changed")),
            2 => {
                out.push(line.clone());
                out.push(format!("inserted-{}", rng.below(100)));
            }
            _ => out.push(line.clone()),
        }
    }
    if rng.below(3) == 0 {
        out.push("appended".into());
    }
    out
}

fn join(lines: &[String], trailing_newline: bool) -> String {
    let mut text = lines.join("\n");
    if trailing_newline && !lines.is_empty() {
        text.push('\n');
    }
    text
}

fn load_diff(repo: &TestRepo, kind: DiffKind) -> FileDiff {
    GixBackend::discover(repo.path())
        .unwrap()
        .file_diff(&DiffRequest {
            kind,
            path: PATH.into(),
            old_path: None,
        })
        .unwrap()
}

/// Random subset of the changed lines.
fn random_selection(rng: &mut Rng, diff: &FileDiff) -> HashSet<(usize, usize)> {
    let mut selection = HashSet::new();
    for (h, hunk) in diff.hunks.iter().enumerate() {
        for (l, line) in hunk.lines.iter().enumerate() {
            if line.kind != DiffLineKind::Context && rng.below(2) == 0 {
                selection.insert((h, l));
            }
        }
    }
    selection
}

fn line_text(line: &git_core::DiffLine) -> String {
    if line.missing_newline {
        line.content.clone()
    } else {
        format!("{}\n", line.content)
    }
}

/// Expected index after applying the selection: `base` is the side the patch
/// is anchored on (index for staging, index = new side for unstaging).
fn expected(
    base: &str,
    diff: &FileDiff,
    direction: PatchDirection,
    selection: &HashSet<(usize, usize)>,
) -> String {
    let base_lines: Vec<String> = base.split_inclusive('\n').map(str::to_owned).collect();
    let mut out = String::new();
    let mut cursor = 0usize;
    for (h, hunk) in diff.hunks.iter().enumerate() {
        let (start, count) = match direction {
            PatchDirection::Stage => (hunk.old_start, hunk.old_count),
            PatchDirection::Unstage => (hunk.new_start, hunk.new_count),
        };
        let first = if count == 0 {
            start as usize
        } else {
            start as usize - 1
        };
        while cursor < first {
            out.push_str(&base_lines[cursor]);
            cursor += 1;
        }
        for (l, line) in hunk.lines.iter().enumerate() {
            let pick = selection.contains(&(h, l));
            let (keep, consumes) = match (line.kind, direction) {
                (DiffLineKind::Context, _) => (true, true),
                (DiffLineKind::Removed, PatchDirection::Stage) => (!pick, true),
                (DiffLineKind::Added, PatchDirection::Stage) => (pick, false),
                (DiffLineKind::Added, PatchDirection::Unstage) => (!pick, true),
                (DiffLineKind::Removed, PatchDirection::Unstage) => (pick, false),
            };
            if keep {
                out.push_str(&line_text(line));
            }
            if consumes {
                cursor += 1;
            }
        }
    }
    while cursor < base_lines.len() {
        out.push_str(&base_lines[cursor]);
        cursor += 1;
    }
    out
}

fn index_content(repo: &TestRepo) -> String {
    String::from_utf8(repo.git_bytes(&["show", &format!(":{PATH}")])).unwrap()
}

fn apply(
    repo: &TestRepo,
    diff: &FileDiff,
    direction: PatchDirection,
    selection: &HashSet<(usize, usize)>,
) {
    let patch = build_patch(diff, direction, |h, l| selection.contains(&(h, l))).unwrap();
    CliBackend::new(repo.path())
        .apply_cached(&patch, direction == PatchDirection::Unstage)
        .unwrap_or_else(|e| panic!("{e}\npatch:\n{patch}"));
}

#[test]
fn random_line_selections_stage_exactly() {
    let mut rng = Rng(0x5eed_cafe);
    let mut cases = 0;
    while cases < 40 {
        let old = random_lines(&mut rng, 25);
        let new = mutate(&mut rng, &old);
        let old_text = join(&old, rng.below(4) != 0);
        let new_text = join(&new, rng.below(4) != 0);
        if old_text == new_text {
            continue;
        }
        let repo = TestRepo::new();
        repo.commit(PATH, &old_text, "Base");
        repo.write(PATH, &new_text);
        let diff = load_diff(&repo, DiffKind::Unstaged);
        let selection = random_selection(&mut rng, &diff);
        if selection.is_empty() {
            continue;
        }
        let want = expected(&old_text, &diff, PatchDirection::Stage, &selection);
        apply(&repo, &diff, PatchDirection::Stage, &selection);
        assert_eq!(
            index_content(&repo),
            want,
            "old {old_text:?} new {new_text:?}"
        );
        cases += 1;
    }
}

#[test]
fn random_line_selections_unstage_exactly() {
    let mut rng = Rng(0xfeed_beef);
    let mut cases = 0;
    while cases < 40 {
        let old = random_lines(&mut rng, 25);
        let new = mutate(&mut rng, &old);
        let old_text = join(&old, rng.below(4) != 0);
        let new_text = join(&new, rng.below(4) != 0);
        if old_text == new_text {
            continue;
        }
        let repo = TestRepo::new();
        repo.commit(PATH, &old_text, "Base");
        repo.write(PATH, &new_text);
        repo.git(&["add", PATH]);
        let diff = load_diff(&repo, DiffKind::Staged);
        let selection = random_selection(&mut rng, &diff);
        if selection.is_empty() {
            continue;
        }
        let want = expected(&new_text, &diff, PatchDirection::Unstage, &selection);
        apply(&repo, &diff, PatchDirection::Unstage, &selection);
        assert_eq!(
            index_content(&repo),
            want,
            "old {old_text:?} new {new_text:?}"
        );
        cases += 1;
    }
}

#[test]
fn whole_hunk_of_a_multi_hunk_file() {
    let old: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    let new = old
        .replace("line 3\n", "three\n")
        .replace("line 25\n", "twenty-five\n");
    let repo = TestRepo::new();
    repo.commit(PATH, &old, "Base");
    repo.write(PATH, &new);
    let diff = load_diff(&repo, DiffKind::Unstaged);
    assert_eq!(diff.hunks.len(), 2);
    let selection: HashSet<_> = diff.hunks[1]
        .lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind != DiffLineKind::Context)
        .map(|(l, _)| (1, l))
        .collect();
    apply(&repo, &diff, PatchDirection::Stage, &selection);
    assert_eq!(
        index_content(&repo),
        old.replace("line 25\n", "twenty-five\n")
    );
}

#[test]
fn partial_new_file() {
    let repo = TestRepo::new();
    repo.commit("other.txt", "x\n", "Base");
    repo.write(PATH, "one\ntwo\nthree\n");
    let diff = load_diff(&repo, DiffKind::Untracked);
    // Select "one" and "three".
    let selection: HashSet<_> = [(0, 0), (0, 2)].into_iter().collect();
    apply(&repo, &diff, PatchDirection::Stage, &selection);
    assert_eq!(index_content(&repo), "one\nthree\n");
}

#[test]
fn deleted_file_whole_and_partial() {
    let repo = TestRepo::new();
    repo.commit(PATH, "a\nb\nc\n", "Base");
    std::fs::remove_file(repo.path().join(PATH)).unwrap();
    let diff = load_diff(&repo, DiffKind::Unstaged);

    // Partial: removing only "b" keeps the file.
    let partial: HashSet<_> = [(0, 1)].into_iter().collect();
    apply(&repo, &diff, PatchDirection::Stage, &partial);
    assert_eq!(index_content(&repo), "a\nc\n");

    // Whole: the file is deleted from the index.
    let repo = TestRepo::new();
    repo.commit(PATH, "a\nb\nc\n", "Base");
    std::fs::remove_file(repo.path().join(PATH)).unwrap();
    let diff = load_diff(&repo, DiffKind::Unstaged);
    let all: HashSet<_> = (0..3).map(|l| (0, l)).collect();
    apply(&repo, &diff, PatchDirection::Stage, &all);
    assert!(repo.git(&["ls-files", PATH]).is_empty());
}

#[test]
fn crlf_file_without_conversion() {
    let repo = TestRepo::new();
    repo.commit(PATH, "a\r\nb\r\nc\r\n", "Base");
    repo.write(PATH, "a\r\nB\r\nc\r\nd\r\n");
    let diff = load_diff(&repo, DiffKind::Unstaged);
    let all: HashSet<_> = diff.hunks[0]
        .lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind != DiffLineKind::Context)
        .map(|(l, _)| (0, l))
        .collect();
    apply(&repo, &diff, PatchDirection::Stage, &all);
    assert_eq!(index_content(&repo), "a\r\nB\r\nc\r\nd\r\n");
}

fn load_diff_with(repo: &TestRepo, kind: DiffKind, context: DiffContext) -> FileDiff {
    GixBackend::discover(repo.path())
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

/// Positions `(hunk, line)` of the changed lines, in order.
fn changed_lines(diff: &FileDiff) -> Vec<(usize, usize)> {
    diff.hunks
        .iter()
        .enumerate()
        .flat_map(|(h, hunk)| {
            hunk.lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.kind != DiffLineKind::Context)
                .map(move |(l, _)| (h, l))
        })
        .collect()
}

/// M7-02: a selection made in a full-context diff (Inline/Split) gives the
/// same index as the same selection in the 3-line-context diff (Hunk view).
#[test]
fn full_context_selections_equal_hunk_selections() {
    let mut rng = Rng(0x0f00_d1ff);
    let mut cases = 0;
    while cases < 30 {
        let old = random_lines(&mut rng, 40);
        let new = mutate(&mut rng, &old);
        let old_text = join(&old, rng.below(4) != 0);
        let new_text = join(&new, rng.below(4) != 0);
        if old_text == new_text {
            continue;
        }
        let unstage = rng.below(2) == 0;
        let (kind, direction) = if unstage {
            (DiffKind::Staged, PatchDirection::Unstage)
        } else {
            (DiffKind::Unstaged, PatchDirection::Stage)
        };
        let setup = || {
            let repo = TestRepo::new();
            repo.commit(PATH, &old_text, "Base");
            repo.write(PATH, &new_text);
            if unstage {
                repo.git(&["add", PATH]);
            }
            repo
        };
        let (hunk_repo, full_repo) = (setup(), setup());
        let hunk_diff = load_diff_with(&hunk_repo, kind.clone(), DiffContext::Hunks);
        let full_diff = load_diff_with(&full_repo, kind, DiffContext::Full);
        assert_eq!(full_diff.hunks.len(), 1);
        let (hunk_lines, full_lines) = (changed_lines(&hunk_diff), changed_lines(&full_diff));
        assert_eq!(hunk_lines.len(), full_lines.len());
        // The same changed lines, picked by their position in the change list.
        let picks: Vec<usize> = (0..hunk_lines.len())
            .filter(|_| rng.below(2) == 0)
            .collect();
        if picks.is_empty() {
            continue;
        }
        let selection = |lines: &[(usize, usize)]| -> HashSet<(usize, usize)> {
            picks.iter().map(|&i| lines[i]).collect()
        };
        apply(&hunk_repo, &hunk_diff, direction, &selection(&hunk_lines));
        apply(&full_repo, &full_diff, direction, &selection(&full_lines));
        assert_eq!(
            index_content(&full_repo),
            index_content(&hunk_repo),
            "old {old_text:?} new {new_text:?} unstage {unstage}"
        );
        cases += 1;
    }
}
