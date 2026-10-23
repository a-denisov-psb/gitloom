# Handover

Hand-off from the planning phase (claude.ai chat with the maintainer) to implementation (Claude Code). Update the "Current state" and "Open questions" sections whenever a milestone ends or something important changes.

## How we work

| Who | Where | What |
|---|---|---|
| Maintainer + Claude (chat) | claude.ai | Product decisions, roadmap, detailed milestone plans |
| Claude Code | this repo | Implementation of milestone tasks, tests, docs updates |

- The milestone files under `docs/milestones/` are the contract. Claude Code implements them task by task and does not change scope on its own.
- Questions or plan problems found during implementation go to "Open questions" below and are raised with the maintainer.
- Maintainer communication: German, short, bullet points. Repository content: English only.

## Product summary

- Open-source GitKraken alternative: same layout, same interactions, same shortcuts, without subscription and without the bloat of recent versions.
- Cross-platform (Windows, macOS, Linux), native Rust, no webview.
- Read via gitoxide; write via the `git` CLI (from M2).
- Details: `docs/ROADMAP.md`, `docs/DECISIONS.md`, `docs/UX-PARITY.md`.

## Current state (end of M2, version 0.3.0)

Done (M2-01 … M2-17, see the notes in `docs/milestones/M2-working-copy.md`; M1 in `docs/milestones/M1-real-repository.md`):

- `git-core`: `GixBackend` for all reads (now with filter-aware working tree diffs, operation state detection, partial clone detection); `CliBackend` / `GitWriter` for all writes through the `git` executable (stage, unstage, apply patches, discard with snapshots, commit, amend, stash, undo primitives, lazy fetch of missing blobs).
- `git-graph`: topological order, lane layout as vertical runs (ADR-011), dashed edges for stashes and WIP, optional `LaneHints` lane groups (prepared for the Git flow layout).
- `app`: file watcher with incremental status refresh, staging by file/hunk/line, discard (all, file, hunk, lines; "Discard all" asks about staged changes), commit box with template and merge conclusion, amend, stash/pop/apply/delete with stashes in the graph, undo/redo, operation banner (merge/rebase/cherry-pick/revert), lock retry dialog, modal and context menu infrastructure.
- Measurements: `docs/PERFORMANCE.md` (M1 and M2 sections).
- Branches: `master` with `v0.1.0`, `v0.2.0`, `v0.3.0`; `develop` (default branch on GitHub).

Verified:

- `./scripts/check.sh` green on Linux; CI green on Windows, macOS, Linux and MSRV 1.88 (maintainer).
- Maintainer on Windows: all shortcuts, keyboard commit, no console window flashing.
- UI checked by hand through WSLg (screenshots, synthetic mouse and, since M2-01, keyboard input).

Not done / known limitations:

- Checkout, branches, tags, fetch/pull/push: M3. Merge/rebase actions and conflict resolution: M4 (the operation banner shows Abort/Continue disabled).
- Partial clones: commit file lists show renames as deletion plus addition when blobs are missing (UX-PARITY).
- Rebase state was only verified by integration tests (`git rebase` is blocked in Claude Code's sandbox).
- Icons are Unicode placeholders (M5); remote markers on pills are text.
- Follow-up proposals from M1-15 (pack mappings, allocator retention) stay open (`docs/PERFORMANCE.md`).

## Maintainer to-dos

Tasks only the maintainer can do. Claude Code adds new items here and ticks them off when done.

- [x] GitHub → Settings → General → Default branch: set to `develop`.
- [x] Push `master` (tags `v0.1.0` and `v0.2.0` are already on origin): `git push -u origin master --tags`
- [x] Delete the accidental `main` branch on GitHub (only after the default branch is `develop`): `git push origin --delete main`
- [x] M2-01: give Claude Code read access to CI results (the repository is private): install `gh` in WSL and run `gh auth login`, or export a fine-grained token with "Actions: read" as `GH_TOKEN`. Until then, please report failing CI jobs (Windows, macOS, Linux, MSRV) with their log.
- [x] Check the CI run of `develop` 7d9b15e (Windows fix): all jobs should be green. Locally verified with the Windows test binaries (GNU target, built in WSL, run on Windows with Git for Windows): 275 passed. CI builds MSVC.
- [x] M2-02: on Windows, check that no console window flashes up when Gitloom runs `git` (first visible with staging, M2-06).
- [x] M2-09: commit once through the commit box with the keyboard (type a summary, Ctrl+Enter); keyboard input cannot be injected in my WSLg test setup.
- [x] M2-01: run the app natively on Windows (`cargo run -p gitloom -- <repo>`) and check the shortcuts (checklist sent 2026-10-23, based on `docs/UX-PARITY.md`); report findings. First run: only shortcuts allowed in text fields worked; second run: all but Shift+J/K; both fixed on `develop`, please re-check Shift+J/K.

- [ ] M3-02 (once fetch/push exist, M3-03/M3-05): HTTPS with Git Credential Manager on Windows and SSH with a passphrase-protected key on Linux; a missing credential must show Gitloom's prompt window. Also check that Esc closes the prompt window (keyboard input did not reach it in my WSLg session; Cancel/OK/Yes by mouse verified).
- [ ] Manual end-to-end walkthrough of M2 on Windows (line staging in a CRLF repo, commit, amend, undo, stash with untracked, pop).

## Environment notes

- Rust ≥ 1.88 via rustup (`rust-toolchain.toml` selects stable with rustfmt and clippy).
- Linux needs X11/Wayland dev libraries and `libxkbcommon`. Without a GPU: `ICED_BACKEND=tiny-skia cargo run -p gitloom`.
- `git` is required for tests (temp repos are built with the CLI), not for running the app in M1.
- First full debug build takes a few minutes (iced + wgpu).
- Windows check from WSL: `cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings`; `cargo test --workspace --no-run --target x86_64-pc-windows-gnu` builds `.exe` test binaries (needs `mingw-w64`) that run on Windows through WSL interop when Git for Windows is installed. Copy them to a Windows folder first.

## Repository setup (once, by the maintainer)

```sh
cd gitloom
git init -b master
git add .
git commit -m "chore: layout shell (M0)"
git tag v0.1.0
git switch -c develop
```

Later, when moving to GitHub: create an empty repo, `git remote add origin <url>`, push `master`, `develop` and tags. Then remove the `git push` deny rule from `.claude/settings.json` if Claude Code should push feature branches and open PRs.

## First Claude Code session

1. Read `CLAUDE.md` and this file.
2. Run `./scripts/check.sh`.
3. `/next-task` starts the current milestone.

## Open questions

| # | Question | Raised | Status |
|---|---|---|---|
| 1 | Final project name ("Gitloom" is a working title) | M0 | open |
| 2 | License confirmation (MIT OR Apache-2.0 provisional) | M0 | open |
| 3 | Own ideas beyond GitKraken for the ideas backlog | M0 | open |
| 4 | Navigation direction of J/K (GitKraken docs table looks swapped) | M1 plan | done: confirmed by maintainer (J down/older, K up/newer) |
| 5 | Release flow: create `master` (and tag `v0.1.0` on the initial commit?) before M1-16 | M1 | done: `master` on the M0 commit, annotated `v0.1.0` |
| 6 | Memory target: accept "private memory < 300 MB" or require total RSS | M1-15 | done: private memory counts (ADR-012) |
| 8 | "Discard all": should it also reset staged changes like GitKraken might (reset --hard + clean)? M2-08 discards only unstaged changes and untracked files | M2-08 | decided (maintainer, 2026-10-23): if changes are staged, ask whether to discard them too; implemented |
| 7 | Start screen path input as fallback for missing folder dialogs (Linux without XDG portal) — keep? | M1-08 | done: native dialog is the default on all platforms, path input stays as fallback |
