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
