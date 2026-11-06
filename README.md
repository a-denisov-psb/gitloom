# Gitloom

A fast, open-source Git client with the workflow GitKraken users already know. Native Rust, no webview, no subscription.

> Not affiliated with GitKraken or Axosoft.

![Gitloom showing a repository with its lane graph](docs/screenshots/m5-main-dark-100.png)

**[User guide](docs/user/README.md)** · [Releases](https://github.com/a-denisov-psb/gitloom/releases) · [Keyboard shortcuts](docs/UX-PARITY.md#keyboard-shortcuts)

Status: version 1.0 is in preparation (release candidates first). See the [roadmap](docs/ROADMAP.md).

## Features

- **Graph**: lane graph of the whole history with branches, tags, stashes and the working copy; commit search, hide and solo branches, file history and blame.
- **Daily work**: stage and unstage by file, hunk and line; commit and amend; stash; discard with undo.
- **Branches and remotes**: checkout, create, rename, delete; fetch (also automatically), pull with mode selection, push with upstream setup and force-with-lease; tags; drag and drop of branches.
- **History editing**: merge, rebase and interactive rebase, cherry-pick, revert, reset, a conflict resolution view, and undo/redo for all of them.
- **GitKraken's keyboard shortcuts**, a command palette (Ctrl+P) and a shortcut list (Ctrl+/). Differences are listed in [docs/UX-PARITY.md](docs/UX-PARITY.md).
- **Uses your Git**: writes go through the `git` executable, so hooks, commit signing, credential helpers and SSH configuration work as usual. Requires Git 2.34 or newer.
- Light and dark theme, zoom up to 200 %, external diff and merge tools, terminal and file manager integration.

## Install

Download the package for your system from the [releases page](https://github.com/a-denisov-psb/gitloom/releases) and compare its checksum with `SHA256SUMS`.

| System | Package | Notes |
|---|---|---|
| Windows 10/11 (x64, x86) | `.msi` installer or portable `.zip` | The installer asks for administrator rights; the zip needs none. |
| macOS 11+ (Apple silicon and Intel) | `.dmg` | Drag Gitloom to Applications. |
| Linux x64 (glibc 2.35+, e.g. Ubuntu 22.04) | `.AppImage` or `.tar.gz` | `chmod +x Gitloom-*.AppImage` and run it. The archive contains the binary, a desktop entry and an icon. |

**macOS:** the app is signed ad hoc but not notarized by Apple, so Gatekeeper blocks the first start. Open it once with right click → **Open** → **Open**, or allow it under System Settings → Privacy & Security → **Open Anyway**.

**Windows:** until the binaries are signed, SmartScreen may show "Windows protected your PC"; choose **More info** → **Run anyway**.

## Coming from GitKraken

The layout (repository tabs, toolbar, left panel, graph, commit panel) and the shortcuts are the same, so you can start right away. Gitloom has no accounts, no cloud workspaces, no integrations, no AI features and no subscription. Intentional differences are listed in [docs/UX-PARITY.md](docs/UX-PARITY.md).

## Privacy

Gitloom sends no telemetry and needs no account. It only connects to the network when

- Git talks to the remotes you configured (fetch, pull, push; auto-fetch every 5 minutes, adjustable or off in Preferences → General),
- you turn on avatars (Preferences → Appearance): a hash of each author's email address is sent to gravatar.com,
- you turn on the update check (Preferences → General): api.github.com is asked for the latest release once a day.

Avatars and the update check are off by default. Credentials are never stored by Gitloom; Git's credential helpers and ssh-agent are used. Crash reports are written to your data folder only and never uploaded.

## Accessibility

Every action is reachable from the keyboard (command palette Ctrl+P, context menus with Shift+F10), focus is visible in both themes, and the interface zooms up to 200 %. Screen readers are not supported yet: the UI toolkit (iced 0.14) does not expose an accessibility tree.

## Build from source

Requires Rust 1.88+ (rustup picks the toolchain from `rust-toolchain.toml`) and Git.

```sh
cargo run -p gitloom
cargo run -p gitloom -- <path-to-repository>
```

Linux needs X11/Wayland libraries and `libxkbcommon`. Without a GPU, use the software renderer: `ICED_BACKEND=tiny-skia cargo run -p gitloom`. Packages are built with `./scripts/package.sh` (Linux, macOS) or `scripts\package.ps1` (Windows); see [CONTRIBUTING](CONTRIBUTING.md).

## Development documentation

Gitloom is developed milestone by milestone; the plans and decisions are part of the repository.

| Document | Content |
|---|---|
| [User guide](docs/user/README.md) | Using Gitloom |
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

Gitloom is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version (`GPL-3.0-or-later`).

Third-party components and their licenses are listed in [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md); the same list ships with every package and is shown in the About dialog.
