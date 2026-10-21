# Decisions

Architecture decision records in short form. New entries at the bottom. Change a decision by adding a new entry that supersedes the old one.

## ADR-001 Rust and iced, no webview

- Status: accepted (M0)
- Context: cross-platform is essential; webview-based stacks (Electron, Tauri) were rejected for size and overhead.
- Decision: Rust with iced 0.14 (Elm architecture, wgpu with tiny-skia fallback). egui and Slint were considered; iced won for architecture clarity and license.
- Consequences: custom widgets (graph, diff) are drawn with `canvas`; some widgets (context menus, tooltips with rich content) must be built ourselves.

## ADR-002 Hybrid Git backend

- Status: accepted (M0)
- Decision: reads via gitoxide (`gix`), writes via the `git` executable (from M2), both behind traits in `git-core`.
- Why: gix is fast and needs no external binary for reading. The CLI gives exact Git behavior for writes (hooks, signing, credential helpers, SSH config, rebase edge cases).
- Consequences: without a `git` binary the app is fully usable for browsing, but write actions are disabled with an explanation.

## ADR-003 Threading model

- Status: accepted (M1 plan)
- Decision: `gix::ThreadSafeRepository` in the backend, thread-local conversion per call; all backend calls in `tokio::task::spawn_blocking` via `Task::perform` (iced feature `tokio`). Loads carry a generation counter to drop stale results.
- Why: the UI must never block; gix `Repository` is not `Sync`.

## ADR-004 Graph layout in a pure crate

- Status: accepted (M1 plan)
- Decision: `git-graph` crate without gix/iced; Kahn ordering by commit time; stable lanes without sideways shifting; per-row segments.
- Why: testable in isolation, deterministic, rendering can be virtualized per row.

## ADR-005 GitKraken parity as acceptance criterion

- Status: accepted (M0)
- Decision: layout, interactions and shortcuts follow GitKraken. Intentional deviations are listed in `docs/UX-PARITY.md`. Colors and icons are our own.
- Note: no GitKraken name, logo, icons or other assets are used.

## ADR-006 No demo data in the app

- Status: accepted (M1 plan)
- Decision: `MockBackend` only for tests and the opt-in `mock` feature. Unimplemented actions are disabled with tooltip "Not available yet".

## ADR-007 Git flow

- Status: accepted (M1 plan)
- Decision: `master` (releases, tags), `develop` (integration), `feature/<task-id>-<slug>`, `release/X.Y.Z`, `hotfix/X.Y.Z`. Merges with `--no-ff`. Details in `CONTRIBUTING.md`.

## ADR-008 License

- Status: provisional
- Decision: MIT OR Apache-2.0 (Rust ecosystem default). To be confirmed by the maintainer before the first public push.

## ADR-009 Privacy

- Status: accepted (M0)
- Decision: no telemetry, no accounts, offline by default. Features that contact the network (avatars, update check) are opt-in.

## ADR-010 Native folder dialog via rfd

- Status: accepted (M1)
- Decision: `rfd` 0.17 (`AsyncFileDialog`) with its default backends: Win32 on Windows, AppKit on macOS, XDG desktop portal over D-Bus on Linux (no GTK build dependency).
- Consequences: Linux needs a running `xdg-desktop-portal`; without it the dialog silently returns nothing. The start screen therefore also accepts a typed path (see UX-PARITY deviations).

## ADR-011 Lane layout stored as runs

- Status: accepted (M1-15)
- Context: rust-lang/rust keeps up to 254 lanes open; one `Through` segment per lane and row meant 17.8 million segments (142 MB) for 100 000 commits.
- Decision: `git_graph::Layout` stores per row only the node and its curve segments (`IntoNode`, `FromNode`), plus vertical runs of `Through` segments with a checkpoint index every 64 rows. `Layout::row(i)` rebuilds the same `Row` as before; `Layout::rows()` iterates all rows.
- Consequences: memory is proportional to edges instead of rows × lanes; layout got faster (27 ms → 8 ms for 100k synthetic commits). Callers use `row(i)` / `len()` instead of a public `rows` vector.

## ADR-012 Memory target counts private memory

- Status: accepted (M1-16, maintainer)
- Context: with rust-lang/rust loaded, total RSS was 396 MB, of which 208 MB were pack files memory-mapped by gix. These pages are file-backed page cache that the OS can drop at any time.
- Decision: memory targets apply to private memory (heap and anonymous mappings, i.e. `RssAnon` on Linux). Memory-mapped pack files are reported separately and not counted.
- Consequences: M1 meets its 300 MB target (146 MB private). `docs/PERFORMANCE.md` lists both numbers.

## ADR-013 CliBackend for writes

- Status: accepted (M2 plan)
- Decision: all write operations run the `git` executable through `CliBackend`
  (argument lists, no shell, `GIT_TERMINAL_PROMPT=0`, `LC_ALL=C`, no interactive editor,
  messages via stdin, no console window on Windows). Minimum Git version 2.34.
  One write operation at a time per repository; reads continue in parallel.
- Why: exact Git behavior for hooks, signing, config and credential helpers.
- Consequences: without a suitable `git`, the app stays read-only with a clear reason.

## ADR-014 Undo model

- Status: accepted (M2 plan)
- Decision: per-tab in-memory undo/redo stacks. Commit and amend are undone with
  `git reset --soft` to the recorded previous HEAD, only if HEAD still equals the
  recorded value. Discards are made undoable by writing the affected worktree files
  as blobs (`git hash-object -w`) before discarding and restoring them on undo.
- Why: no extra storage, works for tracked and untracked files, safe preconditions.
- Consequences: undo history is lost on restart; snapshot blobs are unreferenced and
  pruned by `git gc` after the usual grace period.

## ADR-015 Context menus and modals in M2

- Status: accepted (M2 plan)
- Decision: build context menu and modal infrastructure in M2 instead of M3,
  because discard, stash and file actions need them.

## ADR-016 Release builds

- Status: accepted (planned for M6)
- Decision: release binaries are built and published by CI only from M6 on, signed
  from the start. Every version tag triggers a build, including hotfixes.
  Targets: Linux AppImage + tar.gz, macOS universal dmg, Windows x64 zip + msi,
  Windows x86 zip + msi as best effort (allowed to fail without blocking the release).
- Why: unsigned builds cause Gatekeeper/SmartScreen warnings; the maintainer prefers
  to ship installers only once they are signed.

## ADR-017 Autonomous task merges

- Status: accepted (M2)
- Decision: Claude Code merges finished feature branches into `develop` itself when
  all acceptance criteria are met and `./scripts/check.sh` is green, then pushes
  `develop`. The maintainer is asked only for plan deviations, scope questions,
  maintainer-only actions and releases.
