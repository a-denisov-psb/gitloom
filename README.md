# Gitloom

A fast, open-source Git client with the workflow GitKraken users already know. Native Rust, no webview, no subscription.

> Working title. Not affiliated with GitKraken or Axosoft.

![Gitloom layout shell](docs/screenshot.png)

## Status

Early development. The layout shell (M0) is done with sample data; M1 brings real repositories and the lane graph. See [docs/ROADMAP.md](docs/ROADMAP.md).

| Milestone | Version | State |
|---|---|---|
| M0 Layout shell | 0.1.0 | Done |
| M1 Real repository (read-only) + lane graph | 0.2.0 | In progress |
| M2 Working copy | 0.3.0 | Planned |
| M3 Branches and remotes | 0.4.0 | Planned |
| M4 Merge, rebase, conflicts | 0.5.0 | Planned |
| M5 Productivity parity | 0.6.0 | Planned |
| M6 Release readiness | 1.0.0 | Planned |

## Build

Requires Rust 1.88+ (rustup picks the toolchain from `rust-toolchain.toml`).

```sh
cargo run -p gitloom
```

Linux needs X11/Wayland libraries and `libxkbcommon`. Without a GPU, use the software renderer:

```sh
ICED_BACKEND=tiny-skia cargo run -p gitloom
```

## Documentation

| Document | Content |
|---|---|
| [ROADMAP](docs/ROADMAP.md) | Milestones, non-goals, ideas |
| [M1 plan](docs/milestones/M1-real-repository.md) | Current milestone, task by task |
| [ARCHITECTURE](docs/ARCHITECTURE.md) | Crates, threading, graph layout |
| [DECISIONS](docs/DECISIONS.md) | Architecture decision records |
| [UX-PARITY](docs/UX-PARITY.md) | GitKraken layout, interactions, shortcuts |
| [HANDOVER](docs/HANDOVER.md) | Current state, open questions |
| [CONTRIBUTING](CONTRIBUTING.md) | Setup, git flow, commit style |
| [CLAUDE.md](CLAUDE.md) | Instructions for Claude Code |

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
