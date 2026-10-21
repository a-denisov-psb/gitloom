# Changelog

All notable changes to this project are documented here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning: [SemVer](https://semver.org).

## [Unreleased]

## [0.2.0] - 2026-10-21

### Added

- Real repositories are read with gitoxide in the background: history, branches, tags, stashes, working tree status and commit details. The UI stays responsive while loading.
- Refresh with F5 and automatically when the window regains focus; load errors are shown in the tab with a "Retry" button.
- Open repositories from the start screen (native folder dialog, Ctrl+O, or a typed path) or the command line (`gitloom <path>...`); a subfolder opens its repository, an already open repository switches to its tab.
- Tab shortcuts: Ctrl+T, Ctrl+W, Ctrl+Tab / Ctrl+Shift+Tab, Ctrl+1…9 (Cmd on macOS).
- Session restore: open tabs, active tab and pane sizes come back on the next start; the start screen lists up to 20 recent repositories. A missing repository can be removed from the recent list.
- Ctrl+J / Ctrl+K toggle the left panel and the commit detail panel.
- The commit list renders only visible rows, so repositories with 100 000 loaded commits scroll smoothly; selecting a branch in the sidebar scrolls its commit into view. Author, date and SHA columns hide automatically when the graph needs the space.
- Keyboard navigation in the graph: ↓/J and ↑/K, Shift+↓/Shift+J to the first parent, Shift+↑/Shift+K to the child on the same branch, Ctrl+Home/Ctrl+End (Cmd+↑/↓ on macOS) to the first/last commit, Ctrl+Alt+F focuses the sidebar filter, Esc closes the diff. Shortcuts pause while a text field has focus.
- Read-only diff view for commit, staged, unstaged and untracked files: hunk headers, old/new line numbers, colored added/removed lines, clear messages for binary and very large files. Esc or × returns to the graph at the same scroll position.
- Lane graph: stable lanes with curved merge and fork lines, colored per branch, commit nodes with author initials, small merge nodes and a hollow WIP node with a dashed line to HEAD. Local and remote branches on the same commit share one pill; further references collapse into "+N" with a tooltip.

- Project documentation: roadmap, M1 plan, architecture, decisions, UX parity reference, handover.
- Tooling: workspace lints, rustfmt and clippy config, check scripts, CI workflow, Claude Code setup.

### Changed

- No sample data anymore: actions that are not available yet (Undo, Redo, Pull, Push, Branch, Stash, Pop, Terminal, pickers, staging and committing) are disabled and explain "Not available yet".
- Detached HEAD shows as `HEAD (abc1234)` in the branch picker and as a HEAD pill in the graph; local branches show commits ahead/behind their upstream (↑/↓); a truncated history ends with "History truncated at 100 000 commits".

## [0.1.0] - 2026-10-20

### Added

- Layout shell modeled on GitKraken: repository tabs, toolbar, sidebar, commit graph table with WIP row, staging panel, commit details, diff placeholder, status bar, resizable panes.
- `git-core` crate with domain types, `GitBackend` trait and sample backend.
