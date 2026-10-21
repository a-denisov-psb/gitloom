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

## Current state (end of M1, version 0.2.0)

Done (M1-01 … M1-16, see the notes in `docs/milestones/M1-real-repository.md`):

- `git-core`: `GixBackend` (gix 0.88) for log, refs with upstream ahead/behind, HEAD, stashes, working tree status, commit changes and file diffs; `MockBackend` only behind the `mock` feature.
- `git-graph`: topological order and stable lane layout, stored as vertical runs (ADR-011); criterion benchmark.
- `app`: background loading per tab, refresh (F5, window focus), open flow (start screen with native folder dialog as the default way, typed path as fallback, CLI), session restore, lane graph rendering, virtualized list, keyboard navigation, read-only diff view, disabled write actions with tooltips.
- Measurements: `docs/PERFORMANCE.md` (memory target counts private memory, ADR-012).
- Branches: `master` with `v0.1.0` (M0) and `v0.2.0` (M1), `develop`.
- GitHub repository exists (default branch `develop`); CI runs from M2-01 on.

Verified:

- `./scripts/check.sh` green on Linux (WSL2, Rust 1.99.1).
- UI checked by hand through WSLg/XWayland (screenshots and synthetic mouse input); keyboard input could only be injected while the window had host focus, so shortcuts are mainly covered by unit tests.

Not done / known limitations:

- CI has not run yet (starts with M2-01); Windows and macOS are untested.
- Partial (blobless) clones: diffs of blobs that were never fetched fail with "object could not be found" (gix does not fetch lazily). No fix in 0.2.0. Fixed in M2-14: missing blobs are fetched through `git cat-file blob`.
- Keyboard shortcuts still need a check on a real display (M2-01, maintainer).
- Working tree files are diffed as-is (no CRLF/LFS filters).
- Icons are Unicode placeholders (M5); remote markers on pills are text.

## Maintainer to-dos

Tasks only the maintainer can do. Claude Code adds new items here and ticks them off when done.

- [ ] GitHub → Settings → General → Default branch: set to `develop`.
- [ ] Push `master` (tags `v0.1.0` and `v0.2.0` are already on origin): `git push -u origin master --tags`
- [ ] Delete the accidental `main` branch on GitHub (only after the default branch is `develop`): `git push origin --delete main`
- [ ] M2-01: give Claude Code read access to CI results (the repository is private): install `gh` in WSL and run `gh auth login`, or export a fine-grained token with "Actions: read" as `GH_TOKEN`. Until then, please report failing CI jobs (Windows, macOS, Linux, MSRV) with their log.
- [ ] Check the CI run of `develop` 7d9b15e (Windows fix): all jobs should be green. Locally verified with the Windows test binaries (GNU target, built in WSL, run on Windows with Git for Windows): 275 passed. CI builds MSVC.
- [ ] M2-02: on Windows, check that no console window flashes up when Gitloom runs `git` (first visible with staging, M2-06).
- [ ] M2-09: commit once through the commit box with the keyboard (type a summary, Ctrl+Enter); keyboard input cannot be injected in my WSLg test setup.
- [ ] M2-01: run the app natively on Windows (`cargo run -p gitloom -- <repo>`) and check the M1 shortcuts from `docs/UX-PARITY.md`; report findings.

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
| 8 | "Discard all": should it also reset staged changes like GitKraken might (reset --hard + clean)? M2-08 discards only unstaged changes and untracked files | M2-08 | open |
| 7 | Start screen path input as fallback for missing folder dialogs (Linux without XDG portal) — keep? | M1-08 | done: native dialog is the default on all platforms, path input stays as fallback |
