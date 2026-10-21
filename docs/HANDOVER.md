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

## Current state (M1 implemented on `develop`, not released)

Done (M1-01 … M1-15, see the notes in `docs/milestones/M1-real-repository.md`):

- `git-core`: `GixBackend` (gix 0.88) for log, refs with upstream ahead/behind, HEAD, stashes, working tree status, commit changes and file diffs; `MockBackend` only behind the `mock` feature.
- `git-graph`: topological order and stable lane layout, stored as vertical runs (ADR-011); criterion benchmark.
- `app`: background loading per tab, refresh (F5, window focus), open flow (start screen, folder dialog, typed path, CLI), session restore, lane graph rendering, virtualized list, keyboard navigation, read-only diff view, disabled write actions with tooltips.
- Measurements: `docs/PERFORMANCE.md`.

Verified:

- `./scripts/check.sh` green on Linux (WSL2, Rust 1.99.1).
- UI checked by hand through WSLg/XWayland (screenshots and synthetic mouse input); keyboard input could only be injected while the window had host focus, so shortcuts are mainly covered by unit tests.

Not done / known limitations:

- M1-16 (release v0.2.0) is waiting for the maintainer. There is no `master` branch and no `v0.1.0` tag yet (the repository started with an initial commit on `develop`).
- CI has never run (no remote yet); Windows and macOS are untested.
- Total RSS with rust-lang/rust is 396 MB (target 300 MB) because of memory-mapped pack files; private memory ≈ 190 MB. Follow-ups in `docs/PERFORMANCE.md`.
- Partial (blobless) clones: diffs of blobs that were never fetched show "object could not be found" (gix does not fetch lazily).
- Working tree files are diffed as-is (no CRLF/LFS filters).
- Icons are Unicode placeholders (M5); remote markers on pills are text.

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
| 5 | Release flow: create `master` (and tag `v0.1.0` on the initial commit?) before M1-16 | M1 | open |
| 6 | Memory target: accept "private memory < 300 MB" or require total RSS (needs the pack-mapping follow-up) | M1-15 | open |
| 7 | Start screen path input as fallback for missing folder dialogs (Linux without XDG portal) — keep? | M1-08 | open |
