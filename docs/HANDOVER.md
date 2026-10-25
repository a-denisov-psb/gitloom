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

## Current state (end of M3, version 0.4.0)

Done (M3-01 … M3-15, see the notes in `docs/milestones/M3-branches-remotes.md`; M1/M2 in their milestone files):

- `git-core`: reads via `GixBackend` (reopens when `.git/config` changes; remotes, remote URLs, stash parts, operation state); writes via `CliBackend` / `GitWriter`: staging, discard, commit, stash, checkout, branches, tags, upstream and remote management; `run_network` with progress parsing and process tree cancel; askpass (`GITLOOM_ASKPASS`, ADR-019); undoable `RefChange`s.
- `git-graph`: topological order, lane layout as vertical runs (ADR-011), `LaneHints` lane groups.
- `app`: everything from M2 plus fetch/auto-fetch (one timer, ADR-020), pull modes, push with upstream setup and force-with-lease, checkout by double click, inline branch create/rename, tags, remotes, complete context menus (`menus.rs`, M4 actions disabled), undo for ref changes, detached HEAD guard, askpass window, generic form overlay.
- Measurements: `docs/PERFORMANCE.md` (M1, M2, M3 sections).
- Branches: `master` with `v0.1.0` … `v0.4.0`; `develop` (default branch on GitHub).

Verified:

- `./scripts/check.sh` green on Linux, clippy for the Windows target; Windows test binaries run locally where noted.
- M3 walkthrough on Linux against the GitHub remote of this repo (see the M3-14 note); GitHub left clean.
- UI checked by hand through WSLg (synthetic mouse and keyboard; keyboard input was unreliable at times, gaps are maintainer to-dos).

Not done / known limitations:

- Merge/rebase/cherry-pick/revert/reset actions, conflict resolution, drag and drop of branches: M4 (menus show them disabled).
- Real credential prompts (GCM on Windows, SSH passphrase on Linux) and the Windows run of the M2/M3 walkthroughs are open maintainer checks.
- Partial clones: commit file lists show renames as deletion plus addition when blobs are missing (UX-PARITY).
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
- [ ] M3-07: create a branch with the keyboard: select a commit, Ctrl+B, type a name (an invalid one shows a red border with the reason), Enter creates and checks it out, Esc cancels. Keyboard input was unreliable in my WSLg session (input, validation and typing verified; Enter only by tests).
- [x] M3-08: rename a branch with the keyboard (verified in the M3-14 walkthrough against GitHub): right click a local branch → Rename… → edit the name → Enter; for a branch with an upstream choose "Rename remote too" and check the remote. (Menu, input, delete, force delete and remote delete verified by mouse; keyboard input did not reach the window in my session.)
- [ ] M3-10: add a remote with "+" in the REMOTE section (type name and URL) and rename a remote from its context menu (type the new name). Set/unset upstream, Edit URL (prefilled), Fetch and Remove were verified by mouse; typing did not reach the window reliably in my session.
- [ ] M3-14: repeat the M3 walkthrough on Windows against the GitHub remote, on a throwaway branch (e.g. `e2e-windows`) and a throwaway tag: Fetch all, create branch (Ctrl+B), push with upstream, pull (fast-forward), push rejected → Force push (once without and once after a fetch), create/push/delete tag, rename branch incl. remote, delete remote and local branch, checkout of a remote branch; finally check that GitHub has no leftovers.
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
