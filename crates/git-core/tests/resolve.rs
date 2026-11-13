// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    load_conflict, resolve_conflict, resolve_with_side, CliBackend, ConflictContent, Picks, Side,
    WholeFileReason,
};
use support::TestRepo;

/// `main` and `topic` change `path` from `base` to `ours` / `theirs`; the
/// merge of `topic` into `main` stops with the conflict.
fn conflict(path: &str, base: &str, ours: &str, theirs: &str) -> TestRepo {
    let repo = TestRepo::new();
    repo.commit(path, base, "base");
    repo.branch("topic");
    repo.commit(path, ours, "ours");
    repo.checkout("topic");
    repo.commit(path, theirs, "theirs");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "topic"]));
    repo
}

fn text(repo: &TestRepo, path: &str) -> git_core::ConflictText {
    match load_conflict(&CliBackend::new(repo.path()), path)
        .unwrap()
        .content
    {
        ConflictContent::Text(text) => text,
        other => panic!("unexpected {other:?}"),
    }
}

fn unmerged(repo: &TestRepo) -> String {
    repo.git(&["diff", "--name-only", "--diff-filter=U"])
}

#[test]
fn text_conflict_resolves_into_index_and_working_tree() {
    let repo = conflict(
        "a.txt",
        "1\n2\n3\n4\n5\n",
        "1\nours\n3\n4\n5\n",
        "1\ntheirs\n3\n4\n5\n",
    );
    let text = text(&repo, "a.txt");
    assert_eq!(text.hunks().count(), 1);
    let mut picks = Picks::new(&text);
    assert_eq!(
        text.output(&picks, ("HEAD", "topic")),
        "1\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> topic\n3\n4\n5\n"
    );
    picks.take_all(Side::Theirs);
    let output = text.output(&picks, ("HEAD", "topic"));
    resolve_conflict(&CliBackend::new(repo.path()), "a.txt", output.as_bytes()).unwrap();
    assert_eq!(unmerged(&repo), "");
    assert_eq!(repo.git(&["show", ":a.txt"]), "1\ntheirs\n3\n4\n5");
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "1\ntheirs\n3\n4\n5\n"
    );
    repo.git(&["commit", "--no-edit"]);
    assert_eq!(repo.parents("HEAD").len(), 2);
}

#[test]
fn committed_markers_stay_content() {
    let base = "<<<<<<< old\nx\n=======\ny\n>>>>>>> old\nz\n";
    let repo = conflict(
        "a.txt",
        base,
        &base.replace("z\n", "ours\n"),
        &base.replace("z\n", "theirs\n"),
    );
    let text = text(&repo, "a.txt");
    assert_eq!(text.hunks().count(), 1);
    let mut picks = Picks::new(&text);
    picks.take_all(Side::Ours);
    assert_eq!(
        text.output(&picks, ("HEAD", "topic")),
        base.replace("z\n", "ours\n")
    );
}

#[test]
fn crlf_content_keeps_crlf() {
    let repo = conflict(
        "a.txt",
        "1\r\n2\r\n3\r\n",
        "1\r\nours\r\n3\r\n",
        "1\r\ntheirs\r\n3\r\n",
    );
    let text = text(&repo, "a.txt");
    assert_eq!(text.eol, "\r\n");
    let mut picks = Picks::new(&text);
    picks.toggle_line(0, Side::Ours, 0);
    picks.toggle_line(0, Side::Theirs, 0);
    let output = text.output(&picks, ("HEAD", "topic"));
    resolve_conflict(&CliBackend::new(repo.path()), "a.txt", output.as_bytes()).unwrap();
    assert_eq!(
        std::fs::read(repo.path().join("a.txt")).unwrap(),
        b"1\r\nours\r\ntheirs\r\n3\r\n"
    );
}

#[test]
fn autocrlf_converts_the_resolved_file_on_checkout() {
    let repo = TestRepo::new();
    repo.git(&["config", "core.autocrlf", "true"]);
    repo.commit("a.txt", "1\r\n2\r\n", "base");
    repo.branch("topic");
    repo.commit("a.txt", "1\r\nours\r\n", "ours");
    repo.checkout("topic");
    repo.commit("a.txt", "1\r\ntheirs\r\n", "theirs");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "topic"]));

    // The index stores LF; the model works on that form.
    let text = text(&repo, "a.txt");
    assert_eq!(text.eol, "\n");
    let mut picks = Picks::new(&text);
    picks.take_all(Side::Ours);
    let output = text.output(&picks, ("HEAD", "topic"));
    resolve_conflict(&CliBackend::new(repo.path()), "a.txt", output.as_bytes()).unwrap();
    assert_eq!(
        std::fs::read(repo.path().join("a.txt")).unwrap(),
        b"1\r\nours\r\n"
    );
    assert_eq!(repo.git_bytes(&["show", ":a.txt"]), b"1\nours\n");
}

#[test]
fn add_add_conflict_has_no_base() {
    let repo = TestRepo::new();
    repo.commit("other.txt", "x\n", "base");
    repo.branch("topic");
    repo.commit("a.txt", "ours\n", "ours");
    repo.checkout("topic");
    repo.commit("a.txt", "theirs\n", "theirs");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "topic"]));
    let loaded = load_conflict(&CliBackend::new(repo.path()), "a.txt").unwrap();
    assert!(loaded.stages.base.is_none());
    let ConflictContent::Text(text) = loaded.content else {
        panic!("expected text");
    };
    let mut picks = Picks::new(&text);
    picks.take_all(Side::Theirs);
    assert_eq!(text.output(&picks, ("HEAD", "topic")), "theirs\n");
}

#[test]
fn delete_modify_conflict_takes_a_whole_side() {
    for (side, exists) in [(Side::Ours, true), (Side::Theirs, false)] {
        let repo = TestRepo::new();
        repo.commit("a.txt", "base\n", "base");
        repo.branch("topic");
        repo.commit("a.txt", "changed\n", "ours");
        repo.checkout("topic");
        repo.git(&["rm", "--quiet", "a.txt"]);
        repo.git(&["commit", "--quiet", "-m", "delete"]);
        repo.checkout("main");
        assert!(!repo.try_git(&["merge", "topic"]));
        let cli = CliBackend::new(repo.path());
        let loaded = load_conflict(&cli, "a.txt").unwrap();
        assert_eq!(
            loaded.content,
            ConflictContent::WholeFile(WholeFileReason::DeletedOnOneSide {
                ours_deleted: false
            })
        );
        resolve_with_side(&cli, "a.txt", side).unwrap();
        assert_eq!(unmerged(&repo), "", "{side:?}");
        assert_eq!(repo.path().join("a.txt").exists(), exists, "{side:?}");
    }
}

#[test]
fn binary_conflict_takes_a_whole_side() {
    let repo = TestRepo::new();
    repo.commit("a.bin", "base\0\n", "base");
    repo.branch("topic");
    repo.commit("a.bin", "ours\0\n", "ours");
    repo.checkout("topic");
    repo.commit("a.bin", "theirs\0\n", "theirs");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "topic"]));
    let cli = CliBackend::new(repo.path());
    assert_eq!(
        load_conflict(&cli, "a.bin").unwrap().content,
        ConflictContent::WholeFile(WholeFileReason::Binary)
    );
    resolve_with_side(&cli, "a.bin", Side::Theirs).unwrap();
    assert_eq!(unmerged(&repo), "");
    assert_eq!(
        std::fs::read(repo.path().join("a.bin")).unwrap(),
        b"theirs\0\n"
    );
}

#[test]
fn unknown_path_is_not_found() {
    let repo = conflict("a.txt", "1\n", "2\n", "3\n");
    assert!(matches!(
        load_conflict(&CliBackend::new(repo.path()), "nope.txt"),
        Err(git_core::Error::NotFound(_))
    ));
}

#[test]
fn large_conflicts_take_a_whole_side() {
    let big = |line: &str| line.repeat(git_core::MAX_CONFLICT_SIZE / line.len() + 1);
    let repo = conflict("big.txt", &big("base\n"), &big("ours\n"), &big("theirs\n"));
    let cli = CliBackend::new(repo.path());
    assert_eq!(
        load_conflict(&cli, "big.txt").unwrap().content,
        ConflictContent::WholeFile(WholeFileReason::TooLarge)
    );
    resolve_with_side(&cli, "big.txt", Side::Ours).unwrap();
    assert_eq!(unmerged(&repo), "");
}

// M15-01: conflicts in UTF-16, UTF-8 with a byte order mark and 8-bit text.

fn utf16(text: &str, little: bool) -> Vec<u8> {
    let mut out = if little {
        vec![0xFF, 0xFE]
    } else {
        vec![0xFE, 0xFF]
    };
    for unit in text.encode_utf16() {
        out.extend(if little {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        });
    }
    out
}

/// Like [`conflict`] with raw bytes per version.
fn byte_conflict(path: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> TestRepo {
    let repo = TestRepo::new();
    let commit = |data: &[u8], message: &str| {
        std::fs::write(repo.path().join(path), data).unwrap();
        repo.git(&["add", "--", path]);
        repo.git(&["commit", "--quiet", "-m", message]);
    };
    commit(base, "base");
    repo.branch("topic");
    commit(ours, "ours");
    repo.checkout("topic");
    commit(theirs, "theirs");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "topic"]));
    repo
}

/// Loads the text conflict, takes theirs and resolves; returns the
/// encoding, the working tree file and stage 0.
fn take_theirs(repo: &TestRepo) -> (git_core::ConflictEncoding, Vec<u8>, Vec<u8>) {
    let cli = CliBackend::new(repo.path());
    let loaded = load_conflict(&cli, "a.txt").unwrap();
    let ConflictContent::Text(text) = loaded.content else {
        panic!("not a text conflict: {:?}", loaded.content);
    };
    let mut picks = Picks::new(&text);
    picks.take_all(Side::Theirs);
    let output = text.output(&picks, ("HEAD", "topic"));
    resolve_conflict(&cli, "a.txt", output.as_bytes()).unwrap();
    assert_eq!(unmerged(repo), "");
    (
        loaded.encoding,
        std::fs::read(repo.path().join("a.txt")).unwrap(),
        repo.git_bytes(&["cat-file", "blob", ":a.txt"]),
    )
}

#[test]
fn utf16_le_with_bom_and_crlf_is_a_text_conflict() {
    // The maintainer's PowerShell-written readme.md starts like this.
    let base = utf16("\r\nasdf\r\n", true);
    assert_eq!(
        base,
        [
            0xFF, 0xFE, 0x0D, 0x00, 0x0A, 0x00, 0x61, 0x00, 0x73, 0x00, 0x64, 0x00, 0x66, 0x00,
            0x0D, 0x00, 0x0A, 0x00
        ]
    );
    let repo = byte_conflict(
        "a.txt",
        &base,
        &utf16("\r\nasdf ours\r\nä\r\n", true),
        &utf16("\r\nasdf theirs\r\nä\r\n", true),
    );
    let (encoding, file, stage) = take_theirs(&repo);
    assert_eq!(encoding, git_core::ConflictEncoding::Utf16Le);
    let expected = utf16("\r\nasdf theirs\r\nä\r\n", true);
    assert_eq!(file, expected);
    assert_eq!(stage, expected);
}

#[test]
fn utf16_be_is_a_text_conflict() {
    let repo = byte_conflict(
        "a.txt",
        &utf16("1\n2\n3\n", false),
        &utf16("1\nours\n3\n", false),
        &utf16("1\ntheirs ü\n3\n", false),
    );
    let (encoding, file, stage) = take_theirs(&repo);
    assert_eq!(encoding, git_core::ConflictEncoding::Utf16Be);
    let expected = utf16("1\ntheirs ü\n3\n", false);
    assert_eq!(file, expected);
    assert_eq!(stage, expected);
}

#[test]
fn utf8_with_bom_keeps_the_mark_of_ours() {
    let bom = |text: &str| [&[0xEF, 0xBB, 0xBF][..], text.as_bytes()].concat();
    let repo = byte_conflict(
        "a.txt",
        &bom("1\n2\n3\n"),
        &bom("1\nours\n3\n"),
        // Theirs dropped the mark: still UTF-8, the output follows ours.
        b"1\ntheirs\n3\n",
    );
    let (encoding, file, stage) = take_theirs(&repo);
    assert_eq!(encoding, git_core::ConflictEncoding::Utf8Bom);
    assert_eq!(file, bom("1\ntheirs\n3\n"));
    assert_eq!(stage, bom("1\ntheirs\n3\n"));
}

#[test]
fn eight_bit_text_round_trips_byte_for_byte() {
    let repo = byte_conflict(
        "a.txt",
        b"caf\xe9\n2\n3\n",
        b"caf\xe9\nours \xfc\n3\n",
        b"caf\xe9\ntheirs \xe4\xff\x80\n3\n",
    );
    let (encoding, file, stage) = take_theirs(&repo);
    assert_eq!(encoding, git_core::ConflictEncoding::EightBit);
    assert_eq!(file, b"caf\xe9\ntheirs \xe4\xff\x80\n3\n");
    assert_eq!(stage, b"caf\xe9\ntheirs \xe4\xff\x80\n3\n");
}

#[test]
fn eight_bit_output_rejects_characters_it_cannot_store() {
    let repo = byte_conflict("a.txt", b"\xe9\n", b"\xe9 ours\n", b"\xe9 theirs\n");
    let error = resolve_conflict(&CliBackend::new(repo.path()), "a.txt", "€\n".as_bytes())
        .unwrap_err()
        .to_string();
    assert!(error.contains("8-bit"), "{error}");
    assert_eq!(unmerged(&repo), "a.txt");
}

#[test]
fn mixed_encodings_take_a_whole_side() {
    let repo = byte_conflict(
        "a.txt",
        &utf16("1\n", true),
        &utf16("ours\n", true),
        "theirs ä\n".as_bytes(),
    );
    let cli = CliBackend::new(repo.path());
    assert_eq!(
        load_conflict(&cli, "a.txt").unwrap().content,
        ConflictContent::WholeFile(WholeFileReason::MixedEncodings {
            first: git_core::ConflictEncoding::Utf16Le,
            second: git_core::ConflictEncoding::Utf8,
        })
    );
    // ASCII on one side does not fit UTF-16 either.
    let repo = byte_conflict("a.txt", &utf16("1\n", true), &utf16("2\n", true), b"3\n");
    assert!(matches!(
        load_conflict(&CliBackend::new(repo.path()), "a.txt")
            .unwrap()
            .content,
        ConflictContent::WholeFile(WholeFileReason::MixedEncodings { .. })
    ));
}

#[test]
fn ascii_fits_eight_bit_text() {
    let repo = byte_conflict("a.txt", b"1\n2\n", b"1\nours\n", b"1\ntheirs \xe9\n");
    let (encoding, file, _) = take_theirs(&repo);
    assert_eq!(encoding, git_core::ConflictEncoding::EightBit);
    assert_eq!(file, b"1\ntheirs \xe9\n");
}
