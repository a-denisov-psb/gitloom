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

## Current state (end of M0, version 0.1.0)

Done:

- Cargo workspace: `git-core` (types, `GitBackend` trait, `MockBackend`), `app` (iced 0.14 UI, binary `gitloom`).
- Full layout shell: tabs, toolbar, sidebar, graph table with WIP row and single-lane graph, staging panel, commit details, diff placeholder, status bar, resizable panes.
- Theme module with all colors and styles.
- Tooling: workspace lints, `clippy.toml`, `rustfmt.toml`, `rust-toolchain.toml`, `scripts/check.sh` + `.ps1`, CI workflow prepared (`.github/workflows/ci.yml`, active once pushed to GitHub).
- Claude Code setup: `CLAUDE.md`, `.claude/settings.json`, slash commands.

Verified:

- Builds with Rust 1.91 (MSRV set to 1.88 because iced 0.14 requires it).
- `cargo fmt`, `cargo clippy -D warnings`, tests pass.
- App ran under a virtual display (software renderer); layout, commit selection, diff opening and branch double click detection were checked by hand.

Known limitations (all addressed in M1 unless noted):

- All data is sample data from `MockBackend`.
- Graph uses a single lane.
- Staging is in-memory only; toolbar actions only write a status message.
- Selection uses row indices instead of commit ids.
- Rows are not virtualized.
- Icons are Unicode placeholders (M5).
- Double click is detected in `App::update` with a 400 ms timer, because iced buttons swallow the events a `mouse_area` would need. Keep this approach for sidebar items.

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
| 4 | Navigation direction of J/K (GitKraken docs table looks swapped) | M1 plan | verify in M1-12 |
