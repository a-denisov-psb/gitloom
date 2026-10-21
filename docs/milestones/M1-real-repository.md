# M1 – Real repository (read-only) and lane graph

Target version: **0.2.0**

## Goal

Gitloom opens any local, non-bare Git repository and shows its real state: history with a correct lane graph, branches, remotes, tags, stashes, working tree status, commit details and read-only diffs. Nothing in the app is fake anymore.

## Scope

In scope:

- Reading via gitoxide (`gix`), no `git` binary needed at runtime
- Topological ordering and lane layout in a new pure crate `git-graph`
- Background loading, error states, manual and on-focus refresh
- Open flow: start screen, folder picker, CLI argument, tabs, session restore
- Virtualized graph list, keyboard navigation, read-only diff view
- Removing all demo data and fake actions from the app

Out of scope (later milestones):

- Any write operation (stage, commit, checkout, fetch, ...) → M2/M3
- File watcher → M2
- Context menus, drag and drop, command palette, search → M3/M5
- Stashes as graph nodes → M2
- Avatars, icon set, syntax highlighting → M5
- Bare repositories, worktrees, submodule contents, LFS, sparse checkout

## Milestone acceptance

- [ ] Opening this repository and a large public repository (≥ 200k commits, e.g. `rust-lang/rust`) works without UI freeze.
- [ ] Graph matches `git log --graph --date-order --all` topologically (same parent/child connections; lane positions may differ).
- [ ] Performance targets met (see M1-15).
- [ ] No sample data, no "not implemented" status messages; unavailable actions are disabled with tooltip.
- [ ] `./scripts/check.sh` green, `CHANGELOG.md` complete, docs up to date.
- [ ] Released as `v0.2.0` via `release/0.2.0` → `master` (M1-16).

## Dependencies to add

| Crate | Version | Where | Notes |
|---|---|---|---|
| `gix` | `0.88` | git-core | `default-features = false`, features `["max-performance-safe", "basic", "status", "mailmap", "sha1"]`. `sha1` is required when default features are off. No network features. |
| `rfd` | `0.17` | app | Async folder dialog. Check its Linux backend (xdg-portal vs GTK) and adjust CI packages if needed. |
| `directories` | `6.0` | app | Config dir for session file |
| `serde` (+ `derive`), `serde_json` | `1.0` | app | Session persistence |
| `iced` feature `tokio` | – | app | For `spawn_blocking` |
| `criterion` | latest stable | git-graph (dev) | Benchmarks in M1-15 |
| `tempfile` | latest stable | git-core (dev) | Temporary test repos |

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M1-01 | Tooling baseline | – |
| M1-02 | `git-graph`: topological order | M1-01 |
| M1-03 | `git-graph`: lane layout | M1-02 |
| M1-04 | `GixBackend`: open, log, refs, stashes | M1-01 |
| M1-05 | `GixBackend`: working tree status | M1-04 |
| M1-06 | `GixBackend`: commit changes and file diffs | M1-04 |
| M1-07 | App: async loading, tab state, errors, refresh | M1-04 |
| M1-08 | App: open flow and tabs | M1-07 |
| M1-09 | App: session persistence | M1-08 |
| M1-10 | App: lane graph rendering | M1-03, M1-07 |
| M1-11 | App: virtualized graph list | M1-10 |
| M1-12 | App: keyboard navigation and panel shortcuts | M1-11 |
| M1-13 | App: read-only diff view | M1-06, M1-07 |
| M1-14 | App: remove demo, read-only states, data polish | M1-05, M1-08, M1-13 |
| M1-15 | Performance verification | M1-11 |
| M1-16 | Release v0.2.0 | all |

M1-02/03 and M1-04/05/06 can be done in either order.

---

## Tasks

### [x] M1-01 Tooling baseline

> Done: check.sh green on Linux (WSL2, Rust 1.99.1 via rustup). Fixed missing executable bit on `scripts/check.sh`. Helper: `crates/git-core/tests/support/mod.rs` (`TestRepo`), smoke test `tests/support_smoke.rs`.

Goal: a reliable quality gate before real work starts.

- Run `./scripts/check.sh` on the maintainer's machine (it passed in the planning environment with Rust 1.91 on Linux); fix any platform-specific findings.
- Verify `rust-toolchain.toml` works with rustup on the maintainer's machine.
- Add `tests/support` helper skeleton in `git-core` for building temp repos via `git` CLI (deterministic env: `GIT_AUTHOR_NAME/EMAIL/DATE`, `GIT_COMMITTER_*`, `GIT_CONFIG_NOSYSTEM=1`, `HOME` pointing into the temp dir).

Acceptance:
- `./scripts/check.sh` green on Linux or Windows (whatever the maintainer uses).
- Helper can create a repo with commits, branches and a merge in a test; one smoke test uses it.

### [x] M1-02 `git-graph`: topological order

> Done: `topo_order` returns `Result<TopoOrder, git_graph::Error>`; `Id` is the input position (mismatch → error). Extra tests: equal times, duplicate parents, cycles, merge of two roots.

Goal: new crate `crates/git-graph` that orders commits like `git log --date-order`.

- Input type independent of gix: `GraphCommit { id: Id, parents: Vec<Id>, time: i64 }` with a small `Id` type (e.g. `u32` index into the input, mapping done by the caller).
- Kahn's algorithm: children before parents; among ready commits the newest commit time first; stable for equal times (input order).
- Parents missing from the input are allowed (truncated history, shallow clones) and are reported as open ends.
- Handles multiple roots and disconnected histories.

Acceptance:
- Unit tests: linear, fork, merge, octopus (3+ parents), criss-cross merge, clock skew (parent newer than child), multiple roots, missing parents.
- No dependency besides `std`.

### [x] M1-03 `git-graph`: lane layout

> Done: `Segment::Through` got a `dashed` field (dashed WIP lane passing newer commits), approved by the maintainer. Timing (release, Intel Core Ultra 9 285H, WSL2): 100k synthetic commits, max width 31 lanes → topo ~7 ms, topo + layout ~27 ms. Rows store a segment per open lane, so memory grows with rows × width; very wide graphs need watching in M1-15.

Goal: compute per-row lane geometry as specified in `docs/ARCHITECTURE.md` → "Graph layout".

- Output per row: node lane, node color id, segments (`Through`, `IntoNode`, `FromNode`, each with lane(s) and color id), row width in lanes.
- Stable lanes: lanes never shift sideways; freed slots are reused (lowest free index).
- First parent continues in the node's lane and color; other parents reuse a lane already waiting for them or get a new lane with a new color.
- Support a virtual first row (WIP) whose edge to HEAD is marked `dashed`.
- Open ends (missing parents) produce a `FromNode` segment that ends at the bottom of the last row.

Acceptance:
- Text snapshot tests for all histories from M1-02 (e.g. `"* | |"`-style rendering per row produced by a test helper).
- Property test or loop test: every parent edge in the input appears exactly once as a connection in the output.
- Layout of 100k commits in release mode < 100 ms (benchmark added in M1-15, quick timing check here).

### [ ] M1-04 `GixBackend`: open, log, refs, stashes

Goal: real read access for history and refs in `git-core`.

- `GixBackend::discover(path)`: finds the repo root from any subfolder. Errors: not a repository, bare repository (unsupported in M1).
- Store `gix::ThreadSafeRepository`; each call uses `to_thread_local()` (gix `Repository` is not `Sync`).
- `log(limit)`: walk from all local branches, remote branches, tags and HEAD. Load at most `limit` commits (default 100 000). Order is not important here (done by `git-graph`). Mark truncation in the result.
- Commit fields: extend `Commit` with `author_time` and `commit_time` (display uses author time, ordering uses commit time); apply mailmap to author names.
- `references()`: local branches, remote branches (skip `*/HEAD` symrefs), tags (peeled to commits; annotated and lightweight), HEAD state including detached HEAD.
- `stashes()`: from `refs/stash` reflog.
- Replace the trait methods if needed; keep the trait object-safe and `Send + Sync`.
- Move `MockBackend` behind `#[cfg(any(test, feature = "mock"))]`.

Acceptance:
- Integration tests on temp repos: linear, branches, merge, annotated + lightweight tag, detached HEAD, stash, repo opened from a subfolder, not-a-repo error, bare repo error.
- Opening `rust-lang/rust` and loading 100k commits takes < 1.5 s in release mode (manual check, note the time in the task).

### [ ] M1-05 `GixBackend`: working tree status

Goal: real staged/unstaged lists.

- Use gix status (index ↔ worktree and HEAD tree ↔ index). Map to `ChangeKind` incl. renames where gix detects them, untracked files (respect `.gitignore`), deletions.
- Conflicted entries get a new `ChangeKind::Conflicted` (display only in M1).
- Submodules: show as a single modified entry, no recursion.

Acceptance:
- Tests: modified, added (staged), deleted (staged and unstaged), renamed (staged), untracked, ignored file not listed, file both staged and modified again (appears in both lists), conflict after failed merge.
- Large repo status (rust-lang/rust, clean worktree) < 1 s in release mode (manual check).

### [ ] M1-06 `GixBackend`: commit changes and file diffs

Goal: file lists and diff data for commits and the working tree.

- `commit_changes(id)`: tree diff against the first parent; root commit against the empty tree; rename detection on.
- New method `file_diff(request) -> FileDiff`. Requests: commit file (commit vs first parent), staged file (HEAD vs index), unstaged file (index vs worktree), untracked file (empty vs worktree).
- `FileDiff`: old/new path, binary flag, hunks with header and lines (`Context`, `Added`, `Removed`, old/new line numbers).
- Binary detection; files above 2 MB return `too_large` instead of hunks.
- Prefer gix's blob diff (imara-diff). Document in `DECISIONS.md` if another crate is used.

Acceptance:
- Tests for each request kind, rename, binary file, deleted file, empty file, file without trailing newline.

### [ ] M1-07 App: async loading, tab state, errors, refresh

Goal: the app works with the real backend without blocking the UI.

- Tab state: `Start`, `Loading { path }`, `Ready(RepoView)`, `Failed { path, error }`.
- One `RepoSnapshot` (commits in topo order, layout, refs, stashes, working tree, head, truncated flag) loaded in `spawn_blocking`, delivered via message.
- Selection by commit id instead of row index; it survives refreshes (fallback: WIP or first row).
- Refresh: F5 and when the window regains focus (debounce 500 ms, ignore while a load is running).
- Errors: banner inside the tab with message and "Retry"; status bar only for transient info.

Acceptance:
- Opening a large repo shows a loading state and the UI stays responsive (window can be resized, tabs switched).
- Changing a file outside the app and focusing the window updates the WIP row.
- Unit tests for selection restore logic.

### [ ] M1-08 App: open flow and tabs

Goal: open repositories like GitKraken.

- Start screen (tab state `Start`): "Open repository" button and list of recent repositories (from M1-09; empty list until then).
- Native folder dialog via `rfd` (async). Picking a subfolder of a repo opens the repo root.
- CLI: `gitloom <path>` opens that repo in a tab.
- Opening an already open repo switches to its tab.
- Tabs: `+` opens a new tab with the start screen. Shortcuts: Ctrl+T new tab, Ctrl+W close tab, Ctrl+Tab / Ctrl+Shift+Tab next/previous, Ctrl+1…9 switch tab. Ctrl+O opens the folder dialog (Gitloom addition, see UX-PARITY).
- Window title: `<repo> – Gitloom`.

Acceptance:
- All shortcuts work on Windows/Linux with Ctrl; on macOS with Cmd (use iced's platform modifier).
- Opening a non-repo folder shows the error banner, not a crash.

### [ ] M1-09 App: session persistence

Goal: Gitloom reopens where the user left off.

- Session file `session.json` in the platform config dir (`directories::ProjectDirs`), with `version` field.
- Stores: recent repositories (max 20, most recent first, path + last opened), open tabs, active tab, pane ratios.
- Written on change (debounced) and on exit; atomic write (temp file + rename).
- Corrupt or unknown-version file: rename to `session.json.bak`, start fresh, show a one-time notice.
- Missing repo paths on startup: tab shows the error banner with "Remove from recent".

Acceptance:
- Unit tests for (de)serialization, version handling and recent list ordering/capping.
- Manual: close app with 3 tabs, reopen → same tabs, same active tab, same pane sizes.

### [ ] M1-10 App: lane graph rendering

Goal: draw the real graph from `git-graph` output.

- Each row's canvas draws its segments: straight lines for `Through`, smooth curves (cubic Bézier) for lane changes in `IntoNode`/`FromNode`, nodes on top.
- Node styles: commit = filled circle with author initials (avatar placeholder), merge commit = small filled circle, WIP = hollow circle with dashed line to HEAD.
- Colors from `theme::LANE_COLORS` by color id.
- Graph column width follows the maximum lane count (min 2 lanes, max 12 visible; wider graphs are clipped).
- Ref pills: local + matching remote branch shown as one pill with both markers (like GitKraken); HEAD marked; tags as tag pills; `+N` for the rest with tooltip listing them.
- Row highlight color uses the node lane color for the selected row (subtle).

Acceptance:
- Visually checked against `git log --graph --all` on this repo and a repo with many parallel branches.
- Screenshot of the graph added to the task note (`docs/screenshots/m1-graph.png`).

### [ ] M1-11 App: virtualized graph list

Goal: smooth scrolling with 100k+ rows.

- Fixed row height; render only visible rows plus a small overscan; spacers above and below keep the scrollbar correct.
- Track scroll offset via the scrollable's viewport callback; use a scrollable `Id` to scroll programmatically.
- Row widgets must not allocate per frame beyond the visible window.

Acceptance:
- Scrolling through 100k rows is fluid (no visible stutter on a mid-range machine, release build).
- Selecting a commit outside the visible area (e.g. via sidebar) scrolls it into view.

### [ ] M1-12 App: keyboard navigation and panel shortcuts

Goal: GitKraken keyboard behavior for read-only actions (see `docs/UX-PARITY.md`).

- ↓ / J next (older) commit, ↑ / K previous (newer) commit. Verify direction against real GitKraken; its docs table looks swapped.
- Shift+↓ / Shift+J to first parent, Shift+↑ / Shift+K to child on the same branch.
- Ctrl+Home / Ctrl+End first / last commit (Cmd+↑ / Cmd+↓ on macOS).
- Ctrl+J toggle left panel, Ctrl+K toggle commit detail panel (pane layout is restored when toggled back).
- Ctrl+Alt+F focus sidebar filter, Esc closes the diff view.
- Shortcuts are ignored while a text input has focus (except Esc and Ctrl+Enter later).

Acceptance:
- Unit tests for navigation logic (parent/child jumps, bounds).
- All shortcuts listed in UX-PARITY are marked implemented for M1.

### [ ] M1-13 App: read-only diff view

Goal: real diffs in the center pane.

- Unified hunk view: hunk headers, old/new line numbers, added/removed backgrounds, monospace font.
- Binary and too-large files show a clear message.
- Hunk/Inline/Split buttons: only Hunk active in M1, others disabled with tooltip.
- Diff loading happens in the background; show a loading state for slow files.
- Esc or × returns to the graph with the previous scroll position.

Acceptance:
- Diffs for commit files, staged, unstaged and untracked files are correct (compare with `git diff`).

### [ ] M1-14 App: remove demo, read-only states, data polish

Goal: no fake behavior left; real data shown precisely.

- App no longer depends on `MockBackend` (only tests / `mock` feature).
- Toolbar write actions (Undo, Redo, Pull, Push, Branch, Stash, Pop) disabled with tooltip "Not available yet". Terminal button: disabled as well in M1.
- Staging panel read-only: no stage/unstage buttons, commit box disabled with hint.
- Branch double click: no action in M1 (no fake status message).
- Detached HEAD: toolbar branch picker shows `HEAD (abc1234)`; graph shows a HEAD marker.
- Sidebar: ahead/behind counts against upstream for local branches (read-only), count badges like GitKraken.
- Truncated history: last row shows "History truncated at 100 000 commits".
- Remove the in-memory staging code from M0.

Acceptance:
- Grep for "not implemented", `MockBackend` usage in `crates/app/src` → no hits outside tests.
- Manual walkthrough of every visible control: either works or is disabled with tooltip.

### [ ] M1-15 Performance verification

Goal: measured, documented performance.

- Criterion benchmark for topo order + layout with 100k synthetic commits (many branches).
- Manual measurements (release build) on `rust-lang/rust`: open to first graph paint, status time, scroll smoothness, memory.
- Record results in `docs/PERFORMANCE.md` with machine specs.

Targets:

| Metric | Target |
|---|---|
| Topo order + layout, 100k commits | < 100 ms |
| Open large repo → first graph paint | < 2 s |
| Status on large clean repo | < 1 s |
| Memory with 100k commits loaded | < 300 MB |

Acceptance:
- Targets met or deviations documented with a follow-up task proposal.

### [ ] M1-16 Release v0.2.0

Only when the maintainer asks.

- `git switch -c release/0.2.0 develop`, bump workspace version to `0.2.0`, finalize `CHANGELOG.md`.
- Update README status table and screenshot.
- Merge `--no-ff` into `master`, tag `v0.2.0`, merge back into `develop`.
- Update `docs/ROADMAP.md` (M1 done, current milestone → M2) and `docs/HANDOVER.md`.

Acceptance:
- `master` at `v0.2.0`, `develop` contains the release merge, checks green on both.
