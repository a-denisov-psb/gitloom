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

- Status: accepted (M6 plan, 2026-11-03; replaces the provisional MIT OR Apache-2.0)
- Decision: GPL-3.0-or-later. Dependencies must have GPL-3.0 compatible licenses,
  checked by `cargo-deny`; third-party notices ship with every package.
- Why: maintainer decision; forks and redistributed builds stay open source.
- Implementation (M6-03): `LICENSE` holds the GPL-3.0 text, the workspace declares
  `GPL-3.0-or-later`, our crates are `publish = false`. `deny.toml` allows only
  GPL-3.0 compatible licenses (checked against the tree); `cargo deny` runs in
  `check.sh` (skipped with a hint when missing) and in the CI job "Licenses and
  advisories". `scripts/third-party-licenses.sh` (cargo-about, `about.toml`) writes
  `THIRD-PARTY-LICENSES.md` (all crates of the release targets with license texts,
  plus Lucide icons) and `crates/app/assets/third-party.tsv` (the list in the About
  dialog); `--check` fails in CI when either is stale or misses a crate of `cargo tree`.
  Where a crate offers a choice, the most permissive license is listed.

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
- Extension (M3-12): checkout, branch create/delete/rename and tag create/delete are
  `git_core::RefChange` entries; undo/redo checks that HEAD or the ref is still where
  the change left it (otherwise "the repository changed") and replays the inverse
  Git command. Not undone: the stash made by "Stash and checkout", the remote side of
  a branch rename; restored annotated tags are new tag objects with the same message.
  Network operations are not undoable and clear the redo stack.
- Extension (M4 plan): a hard reset is undoable by snapshot: before the reset the index
  tree (`git write-tree`) and every changed worktree file (blob, `--no-filters`) are
  recorded; undo restores HEAD, index and files.

## ADR-015 Context menus and modals in M2

- Status: accepted (M2 plan)
- Decision: build context menu and modal infrastructure in M2 instead of M3,
  because discard, stash and file actions need them.

## ADR-016 Release builds

- Status: accepted (updated in the M6 plan, 2026-11-03)
- Decision: release binaries are built and published by CI only, from a version tag
  on `master` (`vX.Y.Z`, `vX.Y.Z-rc.N`, hotfixes included), as a draft GitHub release
  with `SHA256SUMS`. Targets: Linux AppImage + tar.gz, macOS universal dmg, Windows x64
  zip + msi, Windows x86 zip + msi as best effort (allowed to fail).
  Signing: Windows via SignPath Foundation (free for open source, needs the public
  repository and builds on public CI); macOS ad-hoc signature without notarization;
  Linux unsigned with checksums. The repository goes public with the first release
  candidate so that SignPath can sign 1.0.
- Why: signed Windows binaries avoid SmartScreen warnings without a paid certificate;
  macOS notarization (Apple Developer ID) is deferred to the backlog.

## ADR-017 Autonomous task merges

- Status: accepted (M2), extended by ADR-034 (no questions within a milestone, autonomous releases)
- Decision: Claude Code merges finished feature branches into `develop` itself when
  all acceptance criteria are met and `./scripts/check.sh` is green, then pushes
  `develop`. The maintainer is asked only for plan deviations, scope questions,
  maintainer-only actions and releases.

## ADR-018 Working tree diffs use Git's filter pipeline

- Status: accepted (M2-04)
- Decision: before diffing against the index, working tree content runs through gix's `convert_to_git` pipeline: eol conversion (`core.autocrlf`, `.gitattributes` `text`/`eol`), `ident`, and configured external `filter.<name>.clean`/`process` drivers (e.g. Git LFS). If the pipeline fails (driver missing or failing), the diff view shows "Filtered file – diff not available" with the reason.
- Why: the diff then equals `git diff`, so patches built from it (hunk/line staging, M2-07) apply cleanly; running the configured drivers is what Git itself does.
- Consequences: external filter programs run in the background when such a file's diff is opened.

## ADR-019 Askpass and credentials

- Status: accepted (M3 plan)
- Decision: Gitloom never stores credentials. Network commands run with
  `GIT_ASKPASS` and `SSH_ASKPASS` pointing to the Gitloom executable in askpass mode
  (`gitloom --askpass "<prompt>"`, small modal window, answer on stdout) and
  `SSH_ASKPASS_REQUIRE=force`. Credential helpers and ssh-agent are used first, as
  configured by the user.
- Why: works with every credential setup without security-sensitive storage in Gitloom.
- Implementation note (M3-02): Git and SSH start the askpass program with the prompt as
  the only argument (no extra flags possible), so Gitloom recognizes askpass mode by the
  environment variable `GITLOOM_ASKPASS=1`, which `CliBackend` sets for its Git processes;
  `gitloom --askpass "<prompt>"` works as well. Password/passphrase/PIN/token prompts are
  masked, SSH host key questions get Yes/No buttons, failures get a hint in the error dialog.
- Addendum (M4-02): the same executable also acts as Git's editor (ADR-021). Editor calls
  are recognized by their first argument (`--git-editor=sequence|message <file>`) before
  askpass mode is checked; `GITLOOM_EDITOR=1` is set for them.

## ADR-020 Network operations and auto-fetch

- Status: accepted (M3 plan)
- Decision: network operations are write operations (one at a time per repo), show
  progress parsed from `--progress` and can be cancelled (process tree kill).
  Auto-fetch is on by default every 5 minutes, yields to user operations and is
  skipped during merge/rebase states.
- Note: this is network access by default; it only contacts remotes the user
  configured, so it does not conflict with ADR-009.

## ADR-021 Gitloom as Git editor

- Status: accepted (M4 plan)
- Decision: Git commands that would open an editor run with `GIT_SEQUENCE_EDITOR` and
  `GIT_EDITOR` pointing to the Gitloom executable in editor mode (env flag like askpass,
  ADR-019, plus the path of a prepared file). Editor mode copies the prepared todo list or
  message into the file Git passes and exits 0; without preparation it exits 1, so Git
  aborts cleanly. Editor mode never starts a UI.
- Why: interactive rebase, reword, squash, merge and revert messages need editor input;
  terminal editors would hang a GUI process. Preparing the content in the UI first keeps
  every command non-interactive.
- Implementation note (M4-02): `GitWriter::run_editing(args, EditorInput)` writes the todo
  list and a queue of commit messages (`message-0001`, ...) into a private temp directory
  (`GITLOOM_EDITOR_DIR`, removed afterwards) and sets
  `GIT_SEQUENCE_EDITOR='<exe>' --git-editor=sequence` and
  `GIT_EDITOR='<exe>' --git-editor=message` (shell-quoted, since Git runs editors through
  `sh`; forward slashes on Windows). Each message editor call consumes the next message, so
  one interactive rebase with several rewords and squashes needs no round trips; an empty
  queue exits 1. All other commands keep `GIT_EDITOR=:`.

## ADR-022 Conflict resolution model

- Status: accepted (M4 plan)
- Decision: the conflict view follows GitKraken: "ours" top left, "theirs" top right with
  checkboxes per hunk and line, editable output below. Data comes from index stages 1/2/3;
  the initial output is `git merge-file -p` (with markers). A pure model in git-core maps
  hunks and lines of both sides to the output; once the user edits the output, the edited
  text wins over the checkboxes (with an indicator and "Reset output"). Line endings are
  preserved.
- Why: matches GitKraken users' expectations and keeps the logic testable outside the UI.
- Implementation note (M4-05): `git merge-file -p --diff3 --marker-size=31` runs on the
  stage blobs (written to a temp dir, as stored in the index); the long markers keep
  conflict markers committed in the file from being parsed as real ones. The output shown
  to the user uses Git's 7-character markers for open conflicts and, per conflict, the
  picked lines with ours before theirs. Lines keep their terminators, so CRLF content is
  reproduced byte for byte; text typed into the output gets the file's line ending. Saving
  writes the content as a blob, sets it as stage 0 (`update-index --cacheinfo`) and checks
  it out (`checkout-index`), so checkout filters such as `core.autocrlf` apply. Binary and
  delete/modify conflicts offer only "take ours/theirs" (`checkout --ours/--theirs` + `add`,
  or `rm` for the deleting side). `git merge-file` ends both sides of a conflict at the end
  of a file without trailing newline with a newline; the resolved file then has one.

## ADR-023 Settings file separate from the session

- Status: accepted (M5 plan)
- Decision: user preferences live in `<config dir>/gitloom/settings.json` (versioned,
  atomic writes); `session.json` keeps state (tabs, recents, panes, pull mode, hidden
  and soloed branches). `auto_fetch_minutes` moves from the session to the settings
  with a one-time migration. A corrupted settings file falls back to defaults and is
  kept as `settings.json.bak`.
- Why: preferences and state change at different rates; a broken session must not
  reset preferences and vice versa.
- Implementation note (M5-02): the file sits next to `session.json` (same config dir);
  missing and unknown fields fall back to defaults, a newer version or a broken value
  resets the whole file to defaults (backup kept). Writes are debounced (300 ms).

## ADR-024 Git CLI for file history and blame

- Status: accepted (M5 plan)
- Decision: file history (`git log --follow`) and blame (`git blame --porcelain`) are
  read through the Git CLI with `--no-optional-locks`, cancellable by killing the
  process. All other reads stay on gix (ADR-002).
- Why: rename following, `.mailmap` and `blame.ignoreRevsFile` then behave exactly like
  Git; `git` is required since M2 anyway.
- Implementation note (M5-09): both run through `GitWriter::run_network`, which already
  reads stdout in a thread and kills the process tree when its `CancelToken` is set; the
  app cancels by dropping the history view.

## ADR-025 Icon set

- Status: accepted (M5 plan)
- Decision: Lucide icons (ISC license), vendored as SVG files under
  `crates/app/assets/icons/` with the license file, embedded at compile time and
  rendered with iced's `svg` feature, tinted by theme colors. Only icons in use are
  vendored.
- Why: permissive license, consistent line style, no runtime dependency; GitKraken
  assets are not allowed (ADR-005).

## ADR-026 Avatars

- Status: accepted (M5 plan)
- Decision: avatars are opt-in (off by default, ADR-009). When on, Gitloom requests
  `https://gravatar.com/avatar/<sha256(email)>?d=404` with `ureq` (rustls, proxy from
  the environment), caches results and misses for 7 days in the cache dir, runs at most
  4 requests in parallel and only for visible commits. When off, no HTTP client exists.
- Why: privacy by default; the hash, not the email, leaves the machine, and the
  setting says so.
- Implementation note (M5-11): `ureq 3.4` (locked to 3.4.2, the newest release at least
  14 days old), `sha2 0.11`; images through iced's `image-without-codecs` feature plus
  `image 0.25` with only the `png` and `jpeg` codecs, instead of iced's `image` feature,
  which enables every codec of the `image` crate (AVIF encoder, TIFF, …) for nothing.
  Graph nodes overlay an `image` widget on the canvas, because canvas images are not
  clipped round.

## ADR-027 Theme system

- Status: accepted (M5 plan)
- Decision: all colors come from a runtime `ThemePalette`; Dark (default) and Light.
  A "System" option is added only if iced 0.14 can detect the OS theme (result recorded
  here by M5-03). Text colors reach WCAG AA contrast in both palettes.
- Why: light theme parity with GitKraken; one place for colors.
- Result (M5-03): iced 0.14 reports the OS theme (`iced::system::theme()` and the
  `system::theme_changes()` subscription, no extra feature), so "System" is offered. When
  the window system reports no preference (`Mode::None`), Dark is used. Style functions
  pick the palette from the iced theme they receive (`extended_palette().is_dark`).

## ADR-028 Product identity in one place

- Status: accepted (M6 plan)
- Decision: display name, binary name, application id, config/cache/data dir name,
  repository and issue URLs live in one module; code uses only that module. A rename
  would be one task (the name was kept, ADR-033, so no config migration exists).
- Why: "Gitloom" collides with an existing product; the name must be a one-place
  change whichever way the decision falls (kept in ADR-033).
- Implementation (M6-01): `git_core::identity` (`crates/git-core/src/identity.rs`), because
  `git-core` needs env names, temp prefixes and reflog messages and must not depend on
  `app`. `crates/git-core/tests/identity_guard.rs` fails when the name appears in code
  outside that module and test code.

## ADR-029 Crash reports

- Status: accepted (M6 plan)
- Decision: a panic hook writes local crash reports (version, commit, OS, thread,
  message, backtrace) to the data dir, keeps the newest 20 and offers them on the next
  start. Nothing is uploaded.
- Why: users can attach a report to an issue; privacy by default (ADR-009).
- Implementation (M6-05): `crates/app/src/crash.rs`. Reports are
  `<data dir>/crashes/<UTC>Z.txt` (Linux `~/.local/share/gitloom/crashes`, macOS
  `~/Library/Application Support/Gitloom/crashes`, Windows `%APPDATA%\Gitloom\data\crashes`).
  The hook is installed after the askpass/editor checks, so helper processes never
  write reports. Panics on other threads are reported too ("had an internal error"
  instead of "closed unexpectedly"). `shown.txt` lists the reports already offered; the
  next start offers the newest unseen one in the status bar ("Open report folder",
  "Copy report", "Dismiss"). The panic message is written as is (a path inside it is
  not filtered). Release builds keep the symbol table (`strip = "debuginfo"`) so
  backtraces name functions; Windows MSVC builds have no PDB in the packages, so their
  frames may stay unnamed. Debug builds panic at start when `GITLOOM_DEBUG_PANIC`
  names a directory (integration test).

## ADR-030 Update check

- Status: accepted (M6 plan)
- Decision: opt-in, off by default; at most once per day the latest GitHub release is
  requested via `ureq`; a newer version shows a status bar notice with a link. No
  download or installation; pre-releases only for pre-release builds.
- Why: ADR-009 (network features are opt-in); installers differ per platform, so
  automatic updates are a backlog item.
- Implementation (M6-06): `crates/app/src/update.rs`. Preferences → General → "Check
  for updates" (off). The check requests
  `api.github.com/repos/<owner>/<repo>/releases?per_page=30` (derived from
  `identity::REPOSITORY_URL`; the list instead of `/releases/latest`, which hides
  pre-releases), takes the version from the tag and skips drafts. The time of the last
  attempt and the dismissed version are stored in the session file (`update`); a failed
  request also waits a day and stays silent. The `ureq` agent is created only when a
  check runs; with the setting off nothing is scheduled.

## ADR-031 Packaging

- Status: accepted (M6-08)
- Decision: `cargo-packager 0.11` (installed as a tool, not a dependency) builds AppImage,
  macOS `.app`/`.dmg` and Windows `.msi` from `[package.metadata.packager]` in
  `crates/app/Cargo.toml`, so name, version and description come from Cargo.
  `scripts/package.sh` (Linux, macOS) and `scripts/package.ps1` (Windows x64/x86) build the
  local platform's packages into `dist/`; the plain archives (`.tar.gz` with binary,
  licenses, desktop entry, metainfo and icon; `.zip` with exe, licenses, README) are made
  by the scripts. macOS: arm64 and x86_64 builds joined with `lipo`, packaged as
  `universal-apple-darwin`, ad-hoc signed (`codesign -s -`) before the dmg is made.
  Every package carries `LICENSE` and `THIRD-PARTY-LICENSES.md` (AppImage
  `usr/lib/gitloom/`, msi and zip next to the exe, app bundle `Resources/`), where the
  About dialog finds them.
- Install scope: the MSI uses cargo-packager's WiX template, which installs per machine
  (elevation). A per-user MSI needs a custom WiX template with registry key paths for
  every component (ICE38/ICE64), which cannot be validated without Windows; the portable
  zip is the per-user option. AppImage, tar.gz and dmg need no installation rights.
- Linux: AppImage and tar.gz are built on the oldest Ubuntu LTS runner (22.04, glibc 2.35)
  in the release workflow; builds on newer systems need a newer glibc. linuxdeploy runs
  with `APPIMAGE_EXTRACT_AND_RUN=1` (no FUSE needed); appimagetool needs `file`.
- MSI version (1.0.0-rc.1): WiX accepts only numeric versions, so `package.ps1` passes
  cargo-packager a generated configuration (with `name` and `binariesDir`, which
  `--config` needs) whose version `1.0.0-rc.N` becomes `1.0.0-N`; cargo-packager turns that
  into the MSI version `1.0.0.N`. Windows Installer ignores the fourth field for upgrades,
  so the final 1.0.0 replaces a release candidate. File names keep `1.0.0-rc.N`.
- Why: one configuration for three installer formats; fallbacks were not needed.

## ADR-032 CI budget until the repository is public

- Status: superseded (repository public since 2026-11-06)
- Decision: Linux check and MSRV run on every push and pull request. Windows and macOS
  run only on pushes to `master` and `release/**`, on tags and via `workflow_dispatch`.
  Runs are grouped per ref with `cancel-in-progress`. Before every merge the maintainer
  workflow runs `./scripts/check.sh` and clippy for the Windows target locally.
- Why: private repositories bill Windows and macOS minutes at a multiple of Linux; the
  budget is limited until the repository is public (M6-11). Revisit after that.
- Superseded (2026-11-06): the repository is public, standard runners are free, so CI runs
  Linux, Windows and macOS on every push and pull request again.

## ADR-033 Keep the name Gitloom

- Status: accepted (maintainer, 2026-11-05; supersedes the rename decision of 2026-11-03)
- Context: the M6 plan intended a rename because "GitLoom" exists as a product (an
  LLM-memory SaaS, GitHub org `gitloom`, crate `gitloom` on crates.io) and several small
  Git tools use similar names (e.g. `AidinD/git-loom`, `pedrohperalta/gitloom`). A check on
  2026-11-05 confirmed all of this. Alternatives were brainstormed; none was clearly
  better. Trademark and domain checks were not done.
- Decision: keep the name "Gitloom" for 1.0. The repository is
  `github.com/a-denisov-psb/gitloom`; the application id is derived from the GitHub
  account (`io.github.a_denisov_psb.Gitloom`) because no own domain exists. The collision
  is a known and accepted risk (discoverability, possible trademark objection).
- Consequences: no rename migration (config dir `Gitloom` stays); the values in
  `git_core::identity` are final; the crate is not published on crates.io under this
  name (taken). A later rename stays possible as one task because of ADR-028. ADR-031 is
  reserved for M6-08 (packaging), so the numbering has a gap until then (filled by M6-08).

## ADR-034 Autonomous milestone execution

- Status: accepted (maintainer, 2026-11-06; changes the workflow of ADR-017)
- Decision: Claude Code works through a whole milestone without asking back, one Claude
  Code chat per milestone.
  - Plan gaps, contradictions and design questions are decided by Claude Code itself: the
    most conservative variant that is closest to GitKraken. The decision is recorded as an
    ADR (architecture or dependencies) or as a deviation note in the milestone file or
    `docs/UX-PARITY.md`, and mentioned in one sentence in the report.
  - Claude Code stops only for:
    (a) CI is red and still not fixed after two fix attempts;
    (b) maintainer-only actions (certificates, secrets, accounts, domains, money): skip
    these tasks, record them under "Maintainer to-dos" in `docs/HANDOVER.md`, continue;
    (c) irreversible actions outside the repository;
    (d) scope extension by new features. Reducing scope or moving items to the ideas
    backlog is allowed, with a note in the milestone file.
  - The release at the end of the milestone is fully autonomous (`/release`):
    `release/X.Y.Z`, merge into `master`, tag `vX.Y.Z`, push `master` and the tag, merge
    back into `develop`. Preconditions: `./scripts/check.sh` green, clippy for the
    Windows target green, CI on `develop` green.
- Why: the maintainer's role moves from approving every step to planning milestones and
  reviewing results; stops are reserved for things only the maintainer can do or undo.
- Consequences: `CLAUDE.md`, `.claude/commands/` and `.claude/settings.json` allow
  pushing `master` and tags as part of `/release`. Force-push, `reset --hard`, `rebase`
  and `clean` stay denied. Reports are one line per task and one report per milestone.

## ADR-035 Headless UI checks with iced_test

- Status: accepted (M6-03)
- Context: UI tasks must be checked by hand, but keyboard input never reached the
  window in the WSLg setup, and screenshots of the Windows desktop fail while it is
  locked.
- Decision: `iced_test 0.14` (iced's own simulator, MIT) is a dev-dependency of the app.
  `crates/app/src/ui_check.rs` renders the real `App::view` headless, clicks widgets by
  their text, types keys and writes PNG snapshots to `target/ui-snapshots/`. The checks
  are `#[ignore]`d (they need a renderer) and run with
  `ICED_TEST_BACKEND=tiny-skia cargo test -p gitloom ui_check -- --ignored`.
- Consequences: snapshots use iced's Fira Sans test font instead of the system font;
  the release binary is unaffected (dev-dependency only). A real window check stays
  part of the release checklist (M6-13).

## ADR-036 App icon and platform metadata

- Status: accepted (M6-04)
- Decision: the icon master is `crates/app/assets/app-icon/icon.svg` (own design: two Git
  lanes woven like loom threads). `scripts/app-icon.sh` renders the PNG sizes 16–1024,
  `icon.ico` (PNG entries) and `icon.icns` (PNG entries) with `resvg 0.48`
  (dev-dependency, newest release at least 14 days old). The window icon is the
  embedded 256 px PNG; on Linux the window's application id is `identity::APP_ID`, the
  name of the `.desktop` file. Windows executables get a version resource and the icon
  from `build.rs` via `winresource 0.1` (build-dependency, MIT; runs only for Windows
  targets, uses `rc.exe` or MinGW's `windres`). Desktop file, AppStream metainfo and the
  macOS `Info.plist` keys live in `packaging/`; a test checks them against
  `git_core::identity`. Minimum macOS version: 11.0 (first release with universal
  binaries).
- Why: one reproducible source for every icon format without image tools on the
  build machine.

## ADR-037 Native Git flow

- Status: accepted (M7 plan, 2026-11-06)
- Decision: Git flow (initialize, start, finish for feature, release and hotfix) is implemented in Gitloom on top of the existing `GitWriter` operations; the `git-flow` CLI is neither required nor called. The configuration uses git-flow's keys (`gitflow.branch.master`, `gitflow.branch.develop`, `gitflow.prefix.feature|release|hotfix|versiontag`) in the repository config, so repositories initialized by the CLI, GitKraken or Gitloom are interchangeable; unknown `gitflow.*` keys are kept. A running finish is persisted in `.git/gitloom/gitflow-finish.json` and resumed after conflicts or a restart. Finish never pushes.
- Why: GitKraken works without the CLI; the CLI is not installed everywhere (and differs between nvie and AVH editions); native steps get Gitloom's conflict handling, progress and undo.
- Undo (M7-11, extends ADR-014): a finish is one undo entry recording every touched ref and the checked-out branch before and after; undo/redo move refs only (commits and the tag object stay) and refuse when a ref moved meanwhile.
- Consequences: Gitloom's finish follows GitKraken's documented behavior, which may differ from a CLI edition in details (e.g. hotfix while a release branch exists); differences are listed in `docs/UX-PARITY.md`.

## ADR-038 Diff view modes

- Status: accepted (M7 plan, 2026-11-06)
- Decision: the diff view has three modes (Hunk, Inline, Split), one global setting. Hunk mode keeps today's 3-line-context data. Inline and Split load a full-context diff (`DiffContext::Full`, one hunk for the whole file) from the same diff pipeline. In Inline and Split, staging works by lines and by change blocks; a block action is turned into a line selection and applied through the existing line staging path (`build_patch`).
- Why: no regression risk for Hunk mode, no second patch builder; full context is needed anyway to render whole files.
- Consequences: switching modes reloads the diff in the background; the 2 MB diff limit applies to all modes.

## ADR-039 M7 on develop before 1.0.0

- Status: accepted (maintainer, 2026-11-09)
- Decision: M7 starts on `develop` while 1.0.0 is still in release candidates (rc.2 tagged, not published). `master` (`v1.0.0-rc.2`) is the 1.0 line: further release candidates and 1.0.0 itself (M6-15) are made from `master` with `hotfix/1.0.x`-style branches (`release/1.0.0` from `master` for the final version), never from `develop`; each such fix is merged back into `develop`.
- Why: the remaining M6 steps wait for the maintainer (publishing, SignPath, checklist run); development should not stall, and 1.0 must not contain M7 work.
- Consequences: `/release` for M6-15 branches from `master`, not `develop`; CHANGELOG `[Unreleased]` on `develop` collects 1.1 changes, fixes for 1.0 get their own section on the 1.0 line.

## ADR-040 M8 on develop before 1.1.0

- Status: accepted (maintainer, 2026-11-10)
- Decision: before the first M8 merge, `release/1.1.0` is branched from `develop` (M7 complete including the follow-ups M7-15 … M7-18 and the rc.3 merge-back). It is the 1.1 line: M7-13 (version, CHANGELOG date, tag) happens there after `v1.0.0`; fixes for 1.1 are made on it and merged back into `develop`. `develop` continues with M8. In `develop`'s CHANGELOG the M7 entries move from `[Unreleased]` to a section `## [1.1.0] - unreleased`; `[Unreleased]` collects M8 only.
- Why: same reasoning as ADR-039: development does not wait for the maintainer's release steps, and 1.1 must not contain M8 work.
- Consequences: three lines exist for a while (`master` 1.0, `release/1.1.0`, `develop`); a fix for 1.0 is merged from `master` into `release/1.1.0` and `develop`; `/release` for M7-13 runs on `release/1.1.0`.

## ADR-041 Identities and per-repository Git settings

- Status: accepted (M8 plan, 2026-11-10)
- Decision: Gitloom keeps a list of identities (name and email only) in `settings.json`. A repository's identity, SSH key and Git flow live in Git's own configuration (`user.name`/`user.email`, `core.sshCommand`, `gitflow.*`) and are read from it with their origin. Gitloom writes the repository config only on an explicit choice, and the global config only for the default identity while "Keep the global Git config updated with the default identity" is on. SSH keys are referenced by path (`ssh -i "<key>" -o IdentitiesOnly=yes`), never copied; passphrases are not stored. A `core.sshCommand` that Gitloom did not write is replaced only after confirmation.
- Why: the command line, other clients and Gitloom see the same settings; no hidden per-repository state that drifts from Git's; no secrets in Gitloom's files.
- Consequences: identity and SSH key per repository are Gitloom extensions (GitKraken switches a global profile and has one global key); UX-PARITY lists them as deviations. Automatic selection (`includeIf`), signing keys and a key generator stay in the backlog.
