# Changelog

All notable changes to this project are documented here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning: [SemVer](https://semver.org).

## [Unreleased]

### Added

- Real repositories are read with gitoxide in the background: history, branches, tags, stashes, working tree status and commit details. The UI stays responsive while loading.
- Refresh with F5 and automatically when the window regains focus; load errors are shown in the tab with a "Retry" button.
- Open repositories from the start screen (native folder dialog, Ctrl+O, or a typed path) or the command line (`gitloom <path>...`); a subfolder opens its repository, an already open repository switches to its tab.
- Tab shortcuts: Ctrl+T, Ctrl+W, Ctrl+Tab / Ctrl+Shift+Tab, Ctrl+1…9 (Cmd on macOS).

- Project documentation: roadmap, M1 plan, architecture, decisions, UX parity reference, handover.
- Tooling: workspace lints, rustfmt and clippy config, check scripts, CI workflow, Claude Code setup.

## [0.1.0] - 2026-10-20

### Added

- Layout shell modeled on GitKraken: repository tabs, toolbar, sidebar, commit graph table with WIP row, staging panel, commit details, diff placeholder, status bar, resizable panes.
- `git-core` crate with domain types, `GitBackend` trait and sample backend.
