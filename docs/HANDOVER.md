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

## Current state (end of M5, version 0.6.0)

Done (M4-01 … M4-15, see the notes in `docs/milestones/M4-merge-rebase.md`; M1–M3 in their milestone files):

- `git-core`: reads via `GixBackend`; writes via `CliBackend` / `GitWriter`, now also merge and fast-forward, rebase and interactive rebase, cherry-pick, revert, reset, continue/skip/abort, conflict model and resolution (ADR-022); Gitloom as Git's editor (ADR-021); undo/redo entries for all history operations.
- `git-graph`: topological order, lane layout as vertical runs (ADR-011), `LaneHints` lane groups.
- `app`: everything from M3 plus multi-selection in the graph, merge/rebase/cherry-pick/revert/reset from the menus, interactive rebase panel, reword and squash shortcuts, conflict resolution view, operation banner with Continue/Skip/Abort, drag and drop of branches.
- M5 (see `docs/milestones/M5-productivity.md`): action registry and shortcut list, settings, light/dark theme, zoom, icons, command palette, commit search, hide/solo, file history, blame, avatars, external tools, terminal and file manager.
- Planning: M6 is planned in task-level detail (`docs/milestones/M6-release-readiness.md`, ADR-028 … ADR-030). The name "Gitloom" is kept (ADR-033); M6-01 is done, M6-02 finalizes the identity values.
- Measurements: `docs/PERFORMANCE.md` (M1 … M4 sections).
- Branches: `master` with `v0.1.0` … `v0.5.0` (`v0.6.0` after the maintainer pushes it); `develop` (default branch on GitHub).

Verified:

- `./scripts/check.sh` green on Linux, clippy for the Windows target; CI green on all platforms for the last code commit (e461b48). The CI run of the planning merge e048618 did not start (GitHub account billing), see Maintainer to-dos.
- M4 walkthrough on Linux by mouse on a clone of this repo (see the M4-14 note).
- UI checked by hand through WSLg (synthetic mouse; keyboard input and modifiers did not reach the window, gaps are maintainer to-dos).

Not done / known limitations:

- M4 known limitations (rebase conflict loses remaining prepared messages, revert without changes, autostash conflict notice): ROADMAP ideas backlog (open question 9).
- Real credential prompts (GCM on Windows, SSH passphrase on Linux), keyboard parts of the M3/M4 walkthroughs and the Windows runs of M2–M4 are open maintainer checks.
- Partial clones: commit file lists show renames as deletion plus addition when blobs are missing (UX-PARITY).
- Follow-up proposals from M1-15 (pack mappings, allocator retention) stay open (`docs/PERFORMANCE.md`).

## Maintainer to-dos

Tasks only the maintainer can do. Claude Code adds new items here and ticks them off when done.

- [ ] M6-13: run `docs/RELEASE-CHECKLIST.md` against the packaged release candidate (Windows required, macOS if available) and answer its GitKraken verification items. The open Windows and keyboard checks of M2–M5 were moved there (M6-13).

- [x] GitHub → Settings → General → Default branch: set to `develop`.
- [x] Push `master` (tags `v0.1.0` and `v0.2.0` are already on origin): `git push -u origin master --tags`
- [x] Delete the accidental `main` branch on GitHub (only after the default branch is `develop`): `git push origin --delete main`
- [x] M2-01: give Claude Code read access to CI results (the repository is private): install `gh` in WSL and run `gh auth login`, or export a fine-grained token with "Actions: read" as `GH_TOKEN`. Until then, please report failing CI jobs (Windows, macOS, Linux, MSRV) with their log.
- [x] Check the CI run of `develop` 7d9b15e (Windows fix): all jobs should be green. Locally verified with the Windows test binaries (GNU target, built in WSL, run on Windows with Git for Windows): 275 passed. CI builds MSVC.
- [x] M2-02: on Windows, check that no console window flashes up when Gitloom runs `git` (first visible with staging, M2-06).
- [x] M2-09: commit once through the commit box with the keyboard (type a summary, Ctrl+Enter); keyboard input cannot be injected in my WSLg test setup.
- [x] M2-01: run the app natively on Windows (`cargo run -p gitloom -- <repo>`) and check the shortcuts (checklist sent 2026-10-23, based on `docs/UX-PARITY.md`); report findings. First run: only shortcuts allowed in text fields worked; second run: all but Shift+J/K; both fixed on `develop`, please re-check Shift+J/K.

- [x] M3-08: rename a branch with the keyboard (verified in the M3-14 walkthrough against GitHub): right click a local branch → Rename… → edit the name → Enter; for a branch with an upstream choose "Rename remote too" and check the remote. (Menu, input, delete, force delete and remote delete verified by mouse; keyboard input did not reach the window in my session.)
- [x] Check the CI runs of `master` v0.4.0 (d4f9f4a) and `develop` (2d31663): both green (checked with `gh run list`, 2026-10-25). From now on Claude Code checks CI itself after each push to `develop`.
- [x] GitHub Actions: CI of `develop` checked on 2026-11-04 (billing fixed), green on Linux, Windows, macOS and MSRV.
- [ ] M6-11: approve the repository content before it goes public (task note in the M6 milestone): keep `CLAUDE.md`, `.claude/` and the planning docs public (recommended) or name what to move; decide the commit author email (old personal address) (keep, or rewrite the history once to the GitHub noreply address before the first public push); optionally run `gitleaks git` once (the pattern scan found nothing).
- [ ] M6-14: make the repository public, then enable Settings → Code security → "Private vulnerability reporting" (SECURITY.md and the Code of Conduct point to it).
- [ ] M6-10: apply at SignPath Foundation once the repository is public; create the project and signing policy, connect GitHub, and set the repository variable `SIGNPATH_ORGANIZATION_ID` (the release workflow has the step prepared).
- [ ] ADR-033 (name kept): optional trademark/domain check for "Gitloom"; decide before the repository goes public (M6-11) whether it stays under `a-denisov-psb` or moves to an own organization (then update `git_core::identity` and the remote).
- [x] GitHub Actions: jobs are not started ("recent account payments have failed or your spending limit needs to be increased", run 40330435175 for `develop` e048618, 2026-10-26). Please check Billing & plans; afterwards re-run the job (`gh run rerun 40330435175`) and check the CI of `master` v0.5.0. Resolved: CI runs again since 2026-11-04.

## Environment notes

- Rust ≥ 1.88 via rustup (`rust-toolchain.toml` selects stable with rustfmt and clippy).
- Linux needs X11/Wayland dev libraries and `libxkbcommon`. Without a GPU: `ICED_BACKEND=tiny-skia cargo run -p gitloom`.
- `git` is required for tests (temp repos are built with the CLI), not for running the app in M1.
- First full debug build takes a few minutes (iced + wgpu).
- License tooling (M6-03): `cargo install --locked cargo-deny` and `cargo install --locked cargo-about --features cli`; `check.sh` runs both when installed. After dependency changes run `./scripts/third-party-licenses.sh`.
- App icon (ADR-036): edit `crates/app/assets/app-icon/icon.svg`, then `./scripts/app-icon.sh`.
- Packages (ADR-031): `cargo install --locked cargo-packager`, then `./scripts/package.sh` (Linux/macOS) or `scripts\package.ps1` (Windows) → `dist/`. Linux needs `file` (here a local copy from `apt-get download file libmagic1t64 libmagic-mgc`, since there is no sudo).
- Headless UI checks (ADR-035): `ICED_TEST_BACKEND=tiny-skia cargo test -p gitloom ui_check -- --ignored` writes PNGs to `target/ui-snapshots/`.
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
| 1 | Final project name ("Gitloom" is a working title) | M0 | decided (maintainer, 2026-11-05): keep "Gitloom" despite the collision with the GitLoom product (ADR-033); repository `github.com/a-denisov-psb/gitloom`; trademark/domain not checked |
| 2 | License confirmation (MIT OR Apache-2.0 provisional) | M0 | decided (maintainer, 2026-11-03): GPL-3.0-or-later (ADR-008), switch in M6-03 |
| 3 | Own ideas beyond GitKraken for the ideas backlog | M0 | open |
| 4 | Navigation direction of J/K (GitKraken docs table looks swapped) | M1 plan | done: confirmed by maintainer (J down/older, K up/newer) |
| 5 | Release flow: create `master` (and tag `v0.1.0` on the initial commit?) before M1-16 | M1 | done: `master` on the M0 commit, annotated `v0.1.0` |
| 6 | Memory target: accept "private memory < 300 MB" or require total RSS | M1-15 | done: private memory counts (ADR-012) |
| 8 | "Discard all": should it also reset staged changes like GitKraken might (reset --hard + clean)? M2-08 discards only unstaged changes and untracked files | M2-08 | decided (maintainer, 2026-10-23): if changes are staged, ask whether to discard them too; implemented |
| 9 | M4 known limitations, candidates for the backlog: (a) an interactive rebase that stops for a conflict loses the remaining prepared reword/squash messages (Git then stops at those steps; Continue uses the prefilled original message); (b) reverting a commit without changes fails in Git itself ("nothing to commit"), Gitloom shows Git's error; (c) an autostash that conflicts after a merge/rebase stays in the stash list without a dedicated notice (the conflicted files and the stash entry are visible) | M4-13 | decided (maintainer, 2026-10-26): all three go to the ROADMAP ideas backlog, no follow-up in M5 |
| 7 | Start screen path input as fallback for missing folder dialogs (Linux without XDG portal) — keep? | M1-08 | done: native dialog is the default on all platforms, path input stays as fallback |
