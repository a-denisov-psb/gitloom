# Gitloom

A fast, open-source Git client with the workflow GitKraken users already know. Native Rust, no webview, no subscription.

> Not affiliated with GitKraken or Axosoft.

![Gitloom showing a repository with its lane graph](docs/screenshots/m5-main-dark-100.png)

## Status

Early development. Version 0.6.0 (M5) adds productivity features: command palette, keyboard shortcut list, commit search, hide and solo branches, file history and blame, light and dark theme, zoom, opt-in avatars, external diff/merge tools, terminal and file manager. Version 0.5.0 (M4) added history editing: merge and fast-forward, rebase and interactive rebase, cherry-pick, revert, reset, a conflict resolution view, drag and drop of branches, multi-selection in the graph and undo/redo for all of them. Gitloom acts as Git's editor, so no command waits for a terminal. Version 0.4.0 (M3) brought branches, tags and remotes, version 0.3.0 (M2) the daily loop of staging, commit and stash. Writes go through the `git` executable, so hooks, signing and credential helpers work as usual. Release readiness (installers, signing, update check) follows in M6. See [docs/ROADMAP.md](docs/ROADMAP.md).

| Milestone | Version | State |
|---|---|---|
| M0 Layout shell | 0.1.0 | Done |
| M1 Real repository (read-only) + lane graph | 0.2.0 | Done |
| M2 Working copy | 0.3.0 | Done |
| M3 Branches and remotes | 0.4.0 | Done |
| M4 Merge, rebase, conflicts | 0.5.0 | Done |
| M5 Productivity parity | 0.6.0 | Done |
| M6 Release readiness | 1.0.0 | Next |

## Build

Requires Rust 1.88+ (rustup picks the toolchain from `rust-toolchain.toml`).

```sh
cargo run -p gitloom
cargo run -p gitloom -- <path-to-repository>
```

Linux needs X11/Wayland libraries and `libxkbcommon`. Without a GPU, use the software renderer:

```sh
ICED_BACKEND=tiny-skia cargo run -p gitloom
```

## Documentation

| Document | Content |
|---|---|
| [ROADMAP](docs/ROADMAP.md) | Milestones, non-goals, ideas |
| [M1 plan](docs/milestones/M1-real-repository.md) | M1 tasks and notes (done) |
| [M2 plan](docs/milestones/M2-working-copy.md) | M2 tasks and notes (done) |
| [M3 plan](docs/milestones/M3-branches-remotes.md) | M3 tasks and notes (done) |
| [M4 plan](docs/milestones/M4-merge-rebase.md) | M4 tasks and notes (done) |
| [M5 plan](docs/milestones/M5-productivity.md) | M5 tasks and notes (done) |
| [M6 plan](docs/milestones/M6-release-readiness.md) | M6 tasks (current) |
| [PERFORMANCE](docs/PERFORMANCE.md) | Measurements |
| [ARCHITECTURE](docs/ARCHITECTURE.md) | Crates, threading, graph layout |
| [DECISIONS](docs/DECISIONS.md) | Architecture decision records |
| [UX-PARITY](docs/UX-PARITY.md) | GitKraken layout, interactions, shortcuts |
| [HANDOVER](docs/HANDOVER.md) | Current state, open questions |
| [CONTRIBUTING](CONTRIBUTING.md) | Setup, git flow, commit style |
| [CLAUDE.md](CLAUDE.md) | Instructions for Claude Code |

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
