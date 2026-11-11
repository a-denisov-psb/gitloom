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

## Current state (single release line to 1.0.0, ADR-042; M10 done, M11 current, 2026-11-11)

- M10 (2026-11-11, on `develop`): done. Start screen like GitKraken's New Tab; clone with progress and cancel; init with default branch, README and bundled `.gitignore` templates; Preferences → Signing (GPG/SSH) and the signature badge; commit template (Repository → Commit); "Copy commit message prompt…" and Preferences → Templates with the stash message template; prune on fetch, `.orig` files, long paths, toolbar labels; external editor per repository; docs and release checklist 7d. Planning update 1 (2026-11-11) added M10-10/M10-11, M11-11 … M11-15, M12-10 … M12-12, M14 and ADR-051.

- M9 (2026-11-11, on `develop`): done. Stash pop/apply that conflicts is a normal conflict with a banner, Undo back to the exact state before and Drop/Keep stash (M9-01, M9-02; also undo/redo of clean pops and applies); conflicted files can be discarded, undoably (M9-03); reset on the HEAD commit, resets record unmerged index entries (M9-04); icons snap to whole physical pixels (M9-05); left panel order and GITFLOW tree like GitKraken (M9-06); summary counter inside the field (M9-07); docs and release checklist section 7c (M9-08). Also: the fetch-cancel test counts zombie helpers as stopped (containers whose PID 1 does not reap). Windows checks of M9 are in section 7c.

- 2026-11-11 (ADR-042): no releases until 1.0.0. M7 and M8 ship with 1.0.0; `release/1.1.0` merged into `develop` (deleting the remote branch is a maintainer to-do: the session gets HTTP 403); `master` stays at `v1.0.0-rc.4`. M9 … M13 run on `develop` one after another; 1.0.0 (M6-15) only when the maintainer asks, after the full release-checklist run. New principles: undo everything local (ADR-043), no dead ends (ADR-044).

M6 status (details in the task notes of `docs/milestones/M6-release-readiness.md`):

- Done: M6-01 identity, M6-02 identity values, M6-03 GPL-3.0-or-later + cargo-deny + third-party notices + About dialog, M6-04 app icon and metadata, M6-05 crash reports, M6-06 opt-in update check, M6-07 accessibility pass, M6-08 packaging (cargo-packager, ADR-031), M6-09 release workflow (dry run green on all targets), M6-12 user guide (`docs/user/`).
- Done with the maintainer: M6-11 (content approved, history rewritten to the GitHub noreply address). Claude Code part done, waiting for the maintainer: M6-13 (run `docs/RELEASE-CHECKLIST.md` on the packaged rc).
- M6-14: `v1.0.0-rc.1` tagged on `master`; the release workflow creates the draft pre-release. Repository public since 2026-11-06. Open for the maintainer: publish the draft, apply at SignPath (M6-10). Further rcs for findings of M6-10/M6-13; M6-15 (1.0.0) only when the maintainer asks.
- M7 started on `develop` on the maintainer's request (ADR-039): 1.0 fixes and 1.0.0 come from `master`, never from `develop`. Done: M7-01 … M7-12 and M7-14 (left panel tree, Hunk/Inline/Split diffs, Git flow with finish across conflicts and undo); also a fix for context menus that were too narrow for long branch names. Open: M7-13 (releases dropped, ADR-042); the GitKraken verification items of the M7 file and section 7a of `docs/RELEASE-CHECKLIST.md`.
- rc.2 (2026-11-09, `v1.0.0-rc.2` on `master`): M6-16 no console window on Windows, M6-17 toolbar pickers open the palette lists, M6-18 remotes in sync with the Git configuration (root cause: config reload keyed on the mtime of `.git/config` only). Open for the maintainer: publish the draft, confirm M6-16 and M6-18 on rc.2, check that the update check of an rc.1 build sees rc.2 (M6-14).
- rc.3 (2026-11-10, `v1.0.0-rc.3` on `master`, ADR-039: made from `master` on `release/1.0.0`, merged back into `develop`): M6-19 context menu width (backport), M6-20 empty `[remote "…"]` sections are not remotes (root cause of `github-fc`), M6-21 "Open file location" and "Copy file path" with absolute native paths, M6-22 scrollbars reserve their room, M6-23 no gaps between graph rows at fractional scales. Open for the maintainer: publish the draft, run the new items of section 7 of the release checklist.
- rc.4 (2026-11-10, `v1.0.0-rc.4` on `master` 6646605, maintainer request "Ja, rc.4 vorbereiten"): toolbar buttons center icon and label (fix requested by the maintainer, also on `release/1.1.0` and `develop`). Tagged and published by the maintainer on 2026-11-11 (the Claude Code session cannot push tags); the release workflow built the pre-release.
- 1.1 line (ADR-040, 2026-11-10): `release/1.1.0` branched from `develop` at 8327da8 (M7 complete including M7-15 … M7-18 and the rc.3 merge-back); M7-13 (releases dropped, ADR-042). M8 (repository settings and tools) is planned on `develop`: `docs/milestones/M8-repository-settings.md`.
- M8 (2026-11-10, on `develop`): M8-01 … M8-09 done (Repository group in the Preferences, identities with a global default, identity per repository, SSH key per repository, external editor). M8-10 (releases dropped, ADR-042).
- Hotfix v0.6.1 (2026-11-06): the v0.6.0 Windows CI hung in `cancel_before_start_stops_immediately`; network cancel no longer joins pipe readers after the kill and never starts Git for a cancelled token.
- Tooling added in M6: `cargo-deny`, `cargo-about`, `cargo-packager` (see Environment notes), headless UI checks with `iced_test` (ADR-035).
- Branches: `master` with `v0.1.0` … `v0.6.1`, `v1.0.0-rc.1` … `v1.0.0-rc.4`; `develop` (default branch on GitHub).

Earlier milestones (M1–M5) are described in their milestone files; the summary of M5 is in `docs/PLANNING-HANDOVER.md`.

Verified:

- `./scripts/check.sh` green on Linux (incl. cargo-deny and the notices check), clippy for the Windows target; CI green on `develop`; `master` v0.6.1 green on Linux, Windows and macOS.
- Release workflow: dry run green on Linux (Ubuntu 22.04, AppImage started under Xvfb), macOS universal (signature, lipo, dmg, start) and Windows x64/x86 (msi, zip, start). Linux artifacts downloaded and started locally.
- M6 UI changes checked headless with `iced_test` snapshots (ADR-035): the Windows desktop was locked, and keyboard input never reached WSLg windows.

Not done / known limitations:

- M4 known limitations (rebase conflict loses remaining prepared messages, revert without changes, autostash conflict notice): ROADMAP ideas backlog (open question 9).
- Real credential prompts, keyboard walkthroughs and the Windows runs of M2–M5 are collected in `docs/RELEASE-CHECKLIST.md` (M6-13).
- Partial clones: commit file lists show renames as deletion plus addition when blobs are missing (UX-PARITY).
- Follow-up proposals from M1-15 (pack mappings, allocator retention) stay open (`docs/PERFORMANCE.md`).

## Maintainer to-dos

Tasks only the maintainer can do. Claude Code adds new items here and ticks them off when done.

- [ ] Final check before 1.0.0 (ADR-042): after M14, run the whole `docs/RELEASE-CHECKLIST.md` on Windows (all sections 1 … 8, including 7 … 7h; release dry-run artifacts or a local build), answer the GitKraken verification items still open in the milestone files, then ask Claude Code for 1.0.0.
- [ ] Install `just` in WSL (`cargo install --locked just`) and on Windows (`winget install Casey.Just`), then try `just`, `just portable` (WSL, needs `sudo apt install mingw-w64`) and `just build win-x64` (Windows); see CONTRIBUTING → Building (ADR-052).
- [x] M6-18: restart Gitloom and report whether the empty remote `github-fc` is still shown. (root cause: empty section, fixed in M6-20)
- [x] rc.3: tag `v1.0.0-rc.3` pushed by the maintainer (the draft pre-release stays unpublished; rc.4 supersedes it).
- [x] rc.4: tag `v1.0.0-rc.4` pushed and the pre-release published (2026-11-11).

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
- [x] M6-11: repository content approved; history rewritten to the GitHub noreply address (2026-11-06).
- [x] M6-14: repository public and private vulnerability reporting enabled (2026-11-06, by Claude Code on request).
- [x] M6-14: check and publish the draft pre-release `v1.0.0-rc.1` (Releases page). (obsolete: rc.4 published)
- [ ] ADR-042: delete the remote branch `release/1.1.0` (already merged into `develop`; the Claude Code session gets HTTP 403 on branch deletion): `git push origin --delete release/1.1.0`. The same applies to merged feature branches the session could not delete (`feature/M7-*`, `feature/menu-width`, `feature/plan-m8`, `feature/tooling-just`, `feature/ci-ubuntu-runner` and later ones): list them with `git branch -r --merged origin/develop` and delete all but `develop`, `master` and `release/1.0.0`.
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
- CI Linux jobs run on `ubuntu-24.04` instead of `ubuntu-latest` (26.04), and their apt step may fail (`continue-on-error`, 4 min): on 2026-11-11 apt could not resolve the Ubuntu mirrors for hours on both images. The build needs none of the packages (X11/Wayland/xkbcommon are loaded at run time). Switch back to `ubuntu-latest` once apt works reliably again.
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
