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
