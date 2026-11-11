# CLAUDE.md

Instructions for Claude Code working in this repository. Read this file completely at the start of every session.

## Project

Gitloom is an open-source, native Git client in Rust (iced 0.14, no webview). Its layout and interaction model must match GitKraken so that long-time GitKraken users feel at home immediately. Planning happens with the maintainer outside this repo; the milestone files under `docs/milestones/` are the contract for implementation.

## Start of a session

1. Read `docs/ROADMAP.md` → section "Current milestone" names the active milestone file.
2. Open that milestone file. Work on the first unchecked task whose dependencies are checked.
3. First session ever: also read `docs/HANDOVER.md`, `docs/ARCHITECTURE.md`, `docs/DECISIONS.md`, `docs/UX-PARITY.md`.

Use `/next-task` and `/finish-task` (see `.claude/commands/`) for the standard flow.

## Commands

| Purpose | Command |
|---|---|
| Quality gate (fmt, clippy, tests) | `./scripts/check.sh` (Windows: `scripts\check.ps1`) |
| Run the app | `cargo run -p gitloom` |
| Run on a repo (from M1-08) | `cargo run -p gitloom -- <path>` |
| Tests of one crate | `cargo test -p git-graph` |
| Format | `cargo fmt --all` |

## Architecture map

```text
crates/
  git-core/    domain types + GitBackend trait + backends (gix; CLI from M2). No UI deps.
  git-graph/   (M1) pure graph layout: topological order + lanes. No gix, no iced.
  app/         iced UI, binary "gitloom". Never calls gix directly.
docs/          roadmap, milestones, architecture, decisions, UX parity
```

Details: `docs/ARCHITECTURE.md`.

## Hard rules

- No webview, Electron, Tauri or web UI tech.
- The UI thread never performs Git or file I/O. Backend calls run via `Task::perform` + `tokio::task::spawn_blocking`.
- Dependency direction: `app → git-core, git-graph`. `git-core` must not depend on iced. `git-graph` must not depend on gix or iced.
- No `unwrap`, `expect`, `panic!` or `todo!` in non-test code. Return `git_core::Error`, show errors in the UI.
- No demo or fake data in the app. Features not yet implemented are shown disabled with the tooltip "Not available yet". `MockBackend` exists only for tests and the `mock` feature.
- Layout and behavior follow GitKraken. Check `docs/UX-PARITY.md` before building UI. Every intentional deviation gets an entry there.
- Everything in the repo is English: code, comments, docs, commit messages.
- Do not guess library APIs. iced 0.14 and gix 0.88 changed a lot compared to older versions: read the source in `~/.cargo/registry/src/` or docs.rs for the exact pinned version.
- New dependencies: latest stable version that is at least 14 days old, pinned to minor (`"0.88"`). Significant ones get an entry in `docs/DECISIONS.md`.
- Platform-specific code (`#[cfg(unix)]`, `#[cfg(windows)]`) keeps its imports and helpers inside the same cfg scope. Before pushing, run clippy for the Windows target if available.
- Do not extend milestone scope with new features. Decide plan gaps, contradictions and design questions yourself and document them (ADR-034: most conservative, GitKraken-closest variant; ADR, milestone note or `docs/UX-PARITY.md` deviation). Stop only for the stop reasons listed in ADR-034.
- Every write action is undoable, or listed under "Cannot be undone" in `docs/user/undo.md` with the reason (ADR-043). No state may leave the user without a visible way back (ADR-044).

## Git workflow (git flow)

| Branch | Purpose |
|---|---|
| `master` | Released versions only, each tagged `vX.Y.Z` |
| `develop` | Integration branch, always green |
| `feature/<task-id>-<slug>` | One per task, e.g. `feature/M1-03-lane-layout`, branched from `develop` |
| `release/X.Y.Z` | Release branch, only when the maintainer asks for a release (`/release`, ADR-042) |
| `hotfix/X.Y.Z` | Fixes on `master`, only when the maintainer asks or a release CI run is red |

- Commits: Conventional Commits with the crate as scope, e.g. `feat(git-graph): assign lanes to merge parents`, `fix(app): keep selection after refresh`. Small, focused commits.
- Merging a feature: when all acceptance criteria are met and `./scripts/check.sh` is green, merge it yourself (`git switch develop && git merge --no-ff feature/...`), push `develop`, delete the feature branch. Do not ask; decide open points yourself and document them (ADR-017, ADR-034).
- Tasks blocked by the maintainer: do everything else, record the open items under "Maintainer to-dos" in `docs/HANDOVER.md`, continue with the next unblocked task.
- Never commit directly to `master` or `develop`. Never rewrite published history.
- CI (ADR-032 superseded, repository public): every push runs Linux, Windows and macOS plus MSRV. Still run `./scripts/check.sh` and clippy for the Windows target (`cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings`) before every merge.
- After pushing `develop`, check the CI run with `gh run watch`; a red run takes priority over the next task. Superseded runs of the same branch are cancelled automatically.
- Push feature branches (`git push -u origin feature/...`) and `develop`. Push `master` and tags only as part of `/release`. Never force-push.

## Definition of done (per task)

- All acceptance criteria of the task are met.
- `./scripts/check.sh` passes.
- New logic has tests (see Testing). Bugs get a regression test.
- Task checkbox ticked in the milestone file, with a one-line note if something deviated.
- `CHANGELOG.md` → `[Unreleased]` updated for user-visible changes.
- Docs updated when architecture, decisions or UX deviations changed.
- For UI tasks: the app was started and the change was checked by hand; describe what you checked.

## Testing

- `git-graph`: exhaustive unit tests with small hand-built histories (linear, fork, merge, octopus, criss-cross, multiple roots, missing parents). Snapshot the lane layout as compact text.
- `git-core`: integration tests against temporary repositories created with the `git` CLI in `tests/support`. Set `GIT_AUTHOR_*`, `GIT_COMMITTER_*` and fixed dates via env so tests are deterministic.
- `app`: keep logic out of view functions; test `update` and helper functions on plain state.
- No network access in tests.

## Communication

- Report to the maintainer in German, as short as possible. Code and repo content stay English.
- Per task: exactly one line after merging: `M6-03 ✔ – <result>; Abweichung: <deviation>` (omit the deviation part if there is none).
- At milestone end: one detailed report as a table (tasks, deviations with reasons, maintainer to-dos, open risks), then continue with the next planned milestone without waiting (ADR-042).
- On problems: the problem plus what you decided.
