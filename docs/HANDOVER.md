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

Verified:

- `./scripts/check.sh` green on Linux (WSL2, Rust 1.99.1).
- UI checked by hand through WSLg/XWayland (screenshots and synthetic mouse input); keyboard input could only be injected while the window had host focus, so shortcuts are mainly covered by unit tests.

Not done / known limitations:

- CI has never run (no remote yet); Windows and macOS are untested.
- Partial (blobless) clones: diffs of blobs that were never fetched fail with "object could not be found" (gix does not fetch lazily). No fix in 0.2.0.
- Keyboard shortcuts and the J/K direction still need a check on a real display.
- Working tree files are diffed as-is (no CRLF/LFS filters).
- Icons are Unicode placeholders (M5); remote markers on pills are text.

## Follow-up tasks (for M2 planning)

| Task | Origin |
|---|---|
| Blobless/partial clones: fall back to `CliBackend` (which fetches missing blobs like plain Git) for diffs; until then show an understandable message instead of "object could not be found" | M1 release |
| LaneHints preparation for a Git-flow aware graph layout (backlog "Git flow graph layout") — not implemented in M1-03 and not part of the M1 plan in this repo; add the backlog entry and plan it | M1 release |
| Incremental refresh with the file watcher instead of rebuilding the full snapshot on every window focus | M1-15 |
| Optional: release or limit gix pack mappings; allocator retention after refresh | M1-15 |

## Environment notes

- Rust ≥ 1.88 via rustup (`rust-toolchain.toml` selects stable with rustfmt and clippy).
- Linux needs X11/Wayland dev libraries and `libxkbcommon`. Without a GPU: `ICED_BACKEND=tiny-skia cargo run -p gitloom`.
- `git` is required for tests (temp repos are built with the CLI), not for running the app in M1.
- First full debug build takes a few minutes (iced + wgpu).

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
3. Run `/next-task` → starts M1-01.

## Open questions

| # | Question | Raised | Status |
|---|---|---|---|
| 1 | Final project name ("Gitloom" is a working title) | M0 | open |
| 2 | License confirmation (MIT OR Apache-2.0 provisional) | M0 | open |
| 3 | Own ideas beyond GitKraken for the ideas backlog | M0 | open |
| 4 | Navigation direction of J/K (GitKraken docs table looks swapped) | M1 plan | M1-12 implements the natural direction (↓/J older, ↑/K newer); maintainer please verify in GitKraken |
| 5 | Release flow: create `master` (and tag `v0.1.0` on the initial commit?) before M1-16 | M1 | done: `master` on the M0 commit, annotated `v0.1.0` |
| 6 | Memory target: accept "private memory < 300 MB" or require total RSS | M1-15 | done: private memory counts (ADR-012) |
| 7 | Start screen path input as fallback for missing folder dialogs (Linux without XDG portal) — keep? | M1-08 | done: native dialog is the default on all platforms, path input stays as fallback |
