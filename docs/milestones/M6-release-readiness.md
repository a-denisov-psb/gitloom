# M6 – Release readiness

Target version: **1.0.0** (preceded by `1.0.0-rc.N`)

## Goal

Gitloom ships as version 1.0: installers for Linux, macOS and Windows built by CI from a version tag, Windows binaries signed, the project licensed GPL-3.0-or-later, with crash reports, an opt-in update check, user documentation and a public repository. All open Windows checks and GitKraken verification items from M2–M5 are closed.

## Scope

In scope:

- Product identity in one place and its finalization (the name "Gitloom" is kept, ADR-033)
- License switch to GPL-3.0-or-later, third-party notices, About dialog
- App icon and desktop metadata
- Crash reports (local only)
- Opt-in update check
- Accessibility pass
- Packaging for all targets and a release workflow on GitHub Actions
- Windows code signing via SignPath Foundation; macOS ad-hoc signature
- Public repository preparation and user documentation
- Consolidated Windows walkthrough and GitKraken parity verification
- Release candidate(s) and release 1.0.0

Out of scope (later):

- macOS notarization (Apple Developer ID) → backlog
- Flatpak, `.deb`/`.rpm`, Homebrew, winget → backlog
- Automatic download/installation of updates → backlog
- Screen reader support (iced 0.14 exposes no accessibility tree) → backlog, documented as known limitation
- Ideas backlog items (git flow layout, diff modes, …) → after 1.0

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Name | Kept: "Gitloom" (ADR-033, maintainer 2026-11-05). It collides with an existing product (GitLoom SaaS) and the crates `gitloom`/`git-loom`; the maintainer accepted that risk. Repository `github.com/a-denisov-psb/gitloom`. All name-dependent values live in one place (ADR-028), so a later rename stays one task. |
| License | GPL-3.0-or-later (ADR-008 updated). `cargo-deny` checks that every dependency license is GPL-3.0 compatible; third-party notices are generated and shipped with every package and shown in the About dialog. |
| Code signing | Windows: SignPath Foundation (free for open source; needs a public repository and builds on public CI). macOS: ad-hoc signature (`codesign -s -`), not notarized; the user documentation explains the Gatekeeper prompt. Linux: unsigned, SHA256 checksums. (ADR-016 updated.) |
| Public repository | The repository goes public with the first release candidate. Before that: secret scan of the whole history, review of internal content, maintainer decision on commit author emails. |
| Release flow | Tag `vX.Y.Z` or `vX.Y.Z-rc.N` on `master` → workflow builds all targets, attaches artifacts and `SHA256SUMS` to a draft GitHub release (pre-release for `-rc`), release notes from the CHANGELOG section. The maintainer publishes the draft. Windows x86 may fail without blocking. |
| Packaging | `cargo-packager` for AppImage, dmg and msi from one configuration; plain archives (`.tar.gz`, `.zip`) built by the workflow. Fallbacks per target only if `cargo-packager` cannot do it (ADR-031, written by M6-08). Linux builds run on the oldest Ubuntu LTS available as a GitHub runner or container (glibc compatibility). |
| Crash reports | A panic hook writes a local report (version, OS, thread, message, backtrace; no repository paths or file contents) to the data dir; the next start offers "Open report folder" and "Copy report". Nothing is uploaded (ADR-029). |
| Update check | Opt-in, off by default: at most once per day, `GET` of the latest release from the GitHub API with `ureq`; a newer version shows a notice in the status bar with a link to the release page. No download, no installation (ADR-030). Pre-releases are ignored unless the running version is one. |
| Accessibility | Every action reachable by keyboard (palette), visible focus everywhere, contrast already AA (ADR-027), zoom to 200 %, no information by color alone. Screen readers are not supported by iced 0.14 → documented limitation. |
| About dialog | Help entry in the palette and the gear menu: name, version, commit, license (GPL-3.0-or-later with link), third-party notices, links to repository and issue tracker. |

## Maintainer decisions and verification items

| Item | Used in |
|---|---|
| ~~Final project name~~ decided: "Gitloom" kept (ADR-033); trademark and domain not checked, accepted risk | – |
| Commit author email in the history (`the old personal address`) becomes public: keep, or rewrite to a GitHub noreply address before going public (one-time exception to "never rewrite published history", maintainer decision) | M6-11 |
| SignPath Foundation application after the repository is public | M6-10 |
| GitHub Actions billing fixed (until the repository is public the release workflow runs on the private plan) | M6-09 |
| GitKraken checks from M4/M5: drag and drop onto a branch that is not checked out; interactive rebase list order; commit search dims or filters; hide the checked-out branch; file history/blame layout; palette sub-lists | M6-13 |
| macOS machine available for a smoke test of the dmg? (otherwise CI only) | M6-13 |

## Milestone acceptance

- [ ] A tag on `master` produces a draft release with AppImage, Linux `.tar.gz`, macOS universal dmg, Windows x64 `.zip` + `.msi` (signed), Windows x86 (best effort) and `SHA256SUMS`.
- [ ] Packages start on a clean machine of each platform (Windows by the maintainer, Linux AppImage on the oldest supported LTS, macOS at least in CI).
- [ ] The project carries one consistent identity everywhere (binary, window title, packages, config dirs, docs); name-dependent values come from `git_core::identity`.
- [ ] License GPL-3.0-or-later in the repository and all packages; `cargo-deny` green; third-party notices shipped.
- [ ] All Windows and keyboard maintainer to-dos from M2–M5 are checked off or explicitly moved to the backlog; GitKraken verification items answered and the code adjusted.
- [ ] With crash reports and update check, Gitloom still opens no network connection by default (update check off).
- [ ] The repository is public, CI runs on GitHub Actions for all platforms, `./scripts/check.sh` green.
- [ ] Released as `v1.0.0` (M6-15) after at least one release candidate.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M6-01 | Product identity in one place | – |
| M6-02 | Finalize the identity values (name kept) | M6-01 |
| M6-03 | License switch, third-party notices, About dialog | – |
| M6-04 | App icon and desktop metadata | M6-01 |
| M6-05 | Crash reports | M6-01 |
| M6-06 | Opt-in update check | M6-01 |
| M6-07 | Accessibility pass | – |
| M6-08 | Packaging configuration | M6-01, M6-03, M6-04 |
| M6-09 | Release workflow | M6-08 |
| M6-10 | Windows code signing (SignPath) | M6-09, M6-14 |
| M6-11 | Public repository preparation | M6-02, M6-03 |
| M6-12 | User documentation | M6-02 |
| M6-13 | Consolidated walkthrough and GitKraken verification | M6-02 … M6-09 |
| M6-14 | Release candidate v1.0.0-rc.1 | M6-02 … M6-09, M6-11, M6-12 |
| M6-15 | Release v1.0.0 | all |

---

## Tasks

### [x] M6-01 Product identity in one place

- One module (e.g. `crates/app/src/identity.rs`) with display name, binary name, application id (reverse DNS), config/cache/data dir name, repository URL, issue URL; Cargo metadata (`description`, `repository`, `homepage`) in the workspace.
- Every use of the name in code (window title, dirs, askpass/editor env names such as `GITLOOM_ASKPASS`/`GITLOOM_EDITOR`, temp file prefixes, user agent of avatars/update check) goes through it.
- Env names keep a compatibility note: they are internal between Gitloom processes, so they simply follow the new name.
- ADR-028.

Acceptance: a test (or `grep` in `check.sh`) fails if the literal name appears in `crates/` outside the identity module, tests and the identity module; behavior unchanged.

Note: the module lives in `crates/git-core/src/identity.rs` (not in `app`) because `git-core` needs the env names, temp prefixes and reflog messages and must not depend on `app`; `app` uses `git_core::identity`. The guard is `crates/git-core/tests/identity_guard.rs` (skips comments and test code; only the identity module may spell the name).

### [x] M6-02 Finalize the identity values (name kept)

Decision 2026-11-05 (ADR-033): the name "Gitloom" is kept, so there is no rename and no config migration. The task confirms the identity values.

- `git_core::identity`: repository and issue URLs point to `github.com/a-denisov-psb/gitloom`; the application id is `io.github.a_denisov_psb.Gitloom` (derived from the GitHub account, no own domain); placeholder comments removed. `[workspace.package]` carries the same URL.
- README no longer calls the name a working title; the guard test allows the name only in the identity module.
- A later rename remains possible as one task (ADR-028).

Acceptance: `./scripts/check.sh` green including the guard test; docs no longer describe the name as pending.

Note: ADR-033 documents the accepted collision risk; trademark and domain checks were not done. The crate `gitloom` on crates.io is taken, so Gitloom is not published there under this name.

### [x] M6-03 License switch, third-party notices, About dialog

- Replace `LICENSE-MIT` and `LICENSE-APACHE` by `LICENSE` (GPL-3.0 text), workspace `license = "GPL-3.0-or-later"`, README and CONTRIBUTING license sections, SPDX note in CONTRIBUTING for contributions.
- `cargo-deny` (`deny.toml`): license allow list compatible with GPL-3.0 (e.g. MIT, Apache-2.0, ISC, BSD-2/3-Clause, Zlib, Unicode-3.0, MPL-2.0, CDLA-Permissive-2.0 — verify each against the actual dependency tree), advisories and bans; run in `check.sh` (skipped with a hint if the tool is missing) and in CI.
- Third-party notices generated (e.g. `cargo-about`) into `THIRD-PARTY-LICENSES.md`/`.html` incl. Lucide icons and bundled fonts; regenerated by a script, checked in CI for being current.
- About dialog per planning decision (palette "About", gear menu).
- ADR-008 status → accepted.

Acceptance: `cargo deny check` green; notices file contains every crate of `cargo tree` and the icons; About dialog checked by hand.

Note: there is no gear menu (the gear opens the Preferences), so "About" sits in the palette and at the bottom of the Preferences categories (UX-PARITY deviation). Notices: `THIRD-PARTY-LICENSES.md` only (no HTML); "Third-party licenses" in the dialog opens the shipped file, or the repository copy of the version. `cargo deny` required tokio 1.53.3 (RUSTSEC-2026-0386) and replacing yanked `clipboard-win`/`async-executor` versions. Checked headless with `iced_test` (ADR-035; the Windows desktop was locked): Preferences → "About" → dialog → Close.

### [x] M6-04 App icon and desktop metadata

- Own app icon (SVG master, no GitKraken resemblance) and generated sizes: PNG set, `.ico`, `.icns`; window icon at runtime.
- Linux `.desktop` file (name, comment, categories `Development;RevisionControl;`, MimeType none), AppStream metainfo optional.
- macOS `Info.plist` values (bundle id from identity, minimum macOS version, high-DPI), Windows version resource (product name, version, copyright).

Acceptance: icon visible in the window, taskbar/dock and package; generation reproducible by a script.

Note: ADR-036. Checked: Windows exe (cross-built) shows the icon and version info (extracted with PowerShell), the Linux app starts with the window icon set; taskbar/dock and package icons are verified with the packages (M6-08/M6-09) and in the release checklist (M6-13), because the Windows desktop was locked. Minimum macOS version 11.0.

### [x] M6-05 Crash reports

- Panic hook (main process only, not in askpass/editor mode): writes `<data dir>/<name>/crashes/<UTC timestamp>.txt` with version, commit, OS/arch, thread name, panic message, backtrace (`std::backtrace`, forced capture); no repository paths, file contents or emails. Keeps the newest 20 reports.
- Next start: status bar notice "Gitloom closed unexpectedly" with "Open report folder" and "Copy report"; shown once per report.
- ADR-029.

Acceptance: unit tests for formatting (no path leak from a panic message containing a path is not filtered — document that panic messages are written as is), rotation and "shown once"; integration test with a debug-only panic trigger (env var, compiled only in tests/debug).

Note: panics on background threads also get a report, offered as "Gitloom had an internal error" (they do not end the app). Release profile changed to `strip = "debuginfo"` so backtraces carry function names. Checked: debug trigger wrote a report, the next real start marked it shown; the status bar notice checked headless (ADR-035).

### [x] M6-06 Opt-in update check

- Settings → General: "Check for updates" (off), note that it contacts api.github.com once a day.
- Check at start and every 24 h while running (stored last check time in the session file): latest release from the GitHub API via `ureq` (timeout 10 s, proxy from env), semver comparison, pre-releases only when running a pre-release. Newer → status bar notice with "Release notes" (opens the release page in the browser) and "Dismiss" (per version).
- Off → no request, no client.
- ADR-030.

Acceptance: unit tests (semver comparison incl. rc, pre-release rule, 24 h scheduling, dismiss per version, "off means no request" with a fake fetcher); checked by hand against a real release once one exists (M6-14).

Note: the release list endpoint is used (not `/releases/latest`, which hides pre-releases); the last attempt counts, so a failed request also waits a day and stays silent. The HTTP part was checked against a public repository (ignored test `live_github_release_list`), the setting and the notice headless (ADR-035); the check against our own release follows in M6-14.

### [x] M6-07 Accessibility pass

- Keyboard: every action reachable (palette), Tab order in dialogs, settings and forms; Esc closes every overlay; focus ring visible in both themes.
- Information not by color alone: check conflict view, search dimming, hidden branches, file status (icons exist).
- Tooltips on all icon-only controls (re-check after M5-05).
- Document the screen reader limitation in the user docs and README.

Acceptance: checklist in the task note with every view; fixes for every gap found; app tests for new keyboard paths.

Note (checklist; "fixed" = gap found and fixed here, checked with tests and headless snapshots, ADR-035):

| View | Keyboard | Esc | Focus visible | Not by color alone | Icon tooltips |
|---|---|---|---|---|---|
| Toolbar | palette: Pull, Push, Stash, Pop stash added (fixed); Undo/Redo, Branch, Fetch, Terminal had shortcuts | – | – | disabled buttons are dimmed and say why | fixed: disabled Stash/Pop in compact mode had none |
| Graph | ↑/↓/J/K, Shift+J/K, Ctrl+Home/End, Shift+↑/↓; commit menu by Shift+F10 / Menu key with ↑/↓/Enter (fixed) | closes search / diff | selection bar | refs labelled, search counter "n of m", dimming is a luminance change | search, prev/next, close: ok |
| Sidebar | Ctrl+Alt+F filter, Ctrl+J toggle; branch actions via palette (checkout) and graph menu | – | filter ring (fixed, 2 px) | hidden branches: eye-off icon; HEAD: check icon | eye, "+" remote: ok |
| Staging / commit panel | S/U, Ctrl+Shift+S/U, Ctrl+Enter, Ctrl+Shift+M, Tab between summary and description (fixed) | – | summary and description ring (fixed) | file status icons, conflict section title | – |
| Diff | Esc closes; S/U for the shown file | ok | – | +/− markers | close: ok |
| Conflict view | Take all/Take file buttons; editor | ok (second Esc after the editor) | output editor ring (fixed) | checkboxes and "ours"/"theirs" labels | – |
| Interactive rebase | P/R/S/F/D, ↑/↓, Alt+↑/↓ | ok | message editors ring (fixed) | action names as text | grip is part of a labelled row |
| File history / blame | ↑/↓, Esc | ok | – | text labels | close: ok |
| Palette, shortcut list | Ctrl+P, Ctrl+/, arrows, Enter | ok | query ring (fixed) | – | – |
| Dialogs, forms, message dialogs | Enter submits, Tab between fields (fixed) | fixed: Esc in a focused field only unfocused it | ring (fixed) | errors as text | – |
| Preferences, About | Ctrl+, ; palette "About" | ok (fixed for the focused input) | ring (fixed) | toggles say On/Off | zoom −/+ (fixed) |
| Start screen, askpass | Ctrl+O, path input; askpass Enter/Esc | ok | ring (fixed) | – | – |
| Status bar notices | buttons (mouse); actions also in palette where they exist | – | – | icon plus text | – |

Zoom up to 200 % exists since M5-04. Screen readers: limitation documented in README and UX-PARITY (user docs in M6-12). iced offers no keyboard focus for buttons, so buttons stay mouse-only where no shortcut or palette entry exists (status bar notice buttons, About links); their actions are not needed for Git work.

### [x] M6-08 Packaging configuration

- `cargo-packager` configuration (or per-target fallback, ADR-031): AppImage, macOS `.app` + `.dmg` (universal binary via `lipo` of arm64 and x86_64), Windows `.msi` (x64, x86); plus `.tar.gz` and `.zip` of the plain binary with LICENSE and notices.
- Packages contain LICENSE, THIRD-PARTY-LICENSES, icon, desktop metadata; installers install per user where the format allows.
- Script `scripts/package.sh`/`.ps1` to build the local platform's packages.

Acceptance: Linux packages built and started locally (AppImage on Ubuntu 22.04 or the oldest supported LTS in a container); Windows/macOS packaging verified in M6-09.

Note: ADR-031. AppImage and tar.gz built and started locally (Debian 13, WSL); no container runtime is available here, so the Ubuntu 22.04 start moves to the release workflow (M6-09 builds the Linux packages on `ubuntu-22.04` and starts the AppImage there under Xvfb). The MSI installs per machine (cargo-packager's WiX template); the zip is the per-user option.

### [ ] M6-09 Release workflow

- `.github/workflows/release.yml` on tags `v*.*.*` and `v*.*.*-rc.*`: matrix Linux x64, macOS universal, Windows x64, Windows x86 (`continue-on-error`); runs `check` first; ad-hoc `codesign -s -` for macOS; `SHA256SUMS`; draft release (pre-release for `-rc`) with notes from the CHANGELOG section of that version.
- A Windows signing step prepared for M6-10 (skipped while not configured).
- Manual dry run via `workflow_dispatch` that uploads artifacts without creating a release.

Acceptance: dry run green on all required targets; artifacts downloaded and started (Linux by Claude Code, Windows by the maintainer); no secrets needed for the unsigned path.

### [ ] M6-10 Windows code signing (SignPath)

- Maintainer: apply at SignPath Foundation once the repository is public (M6-14); set up the project, signing policy and the GitHub integration.
- Workflow: Windows artifacts (`.exe` inside zip and msi) are submitted to SignPath from the release workflow and replaced by the signed versions; signature verified (`signtool verify` or `Get-AuthenticodeSignature`) before upload.
- Code signing policy page required by SignPath (`docs/CODE-SIGNING.md`, linked from README).

Acceptance: an rc tag produces signed Windows artifacts; verification step green; SmartScreen behavior noted by the maintainer.

### [ ] M6-11 Public repository preparation

- Secret scan of the full history (e.g. `gitleaks`), findings fixed or confirmed harmless with the maintainer.
- Review of internal content: `CLAUDE.md`, `.claude/`, `docs/PLANNING-HANDOVER.md`, HANDOVER environment notes, scripts (local paths, SSH host aliases); keep or move with the maintainer's OK.
- Commit author email: apply the maintainer decision before the first public push.
- Add `SECURITY.md` (GitHub private vulnerability reporting), issue templates (bug with version/OS/crash report, feature request), PR template, `CODE_OF_CONDUCT.md` (Contributor Covenant), CONTRIBUTING update (DCO or not: maintainer decision, default none).
- README for the public: what it is, screenshots, install per platform incl. the macOS Gatekeeper note, GitKraken shortcut compatibility, privacy statement, license.

Acceptance: scan report in the task note; maintainer approves the repository content before it goes public.

Note (Claude Code part done, 2026-11-06; the checkbox waits for the maintainer's approval):

- Secret scan: `gitleaks` could not be installed in this environment (downloading its binary was blocked), so the full history (335 commits, all branches, `git log --all -p`, without `Cargo.lock`, notices and images) was searched for the gitleaks core patterns: AWS keys, GitHub/GitLab tokens (`ghp_`, `github_pat_`, `gho_`, `glpat-`), Slack tokens, private key blocks, OpenAI/Google API keys, JWTs and `password/secret/token/api_key = "…"` assignments. Result: no findings; the only hits are the askpass test prompts (`"Password:"`). Recommended before going public: one `gitleaks git` run by the maintainer.
- Personal data: every commit has the author and committer email `the old personal address` (maintainer decision: keep or rewrite to the GitHub noreply address); it also appears once in this milestone file. Test emails are `@example.com`; test paths are `/home/me`.
- Internal content reviewed: `CLAUDE.md`, `.claude/` (commands, permission settings), `docs/PLANNING-HANDOVER.md`, `docs/HANDOVER.md` (environment notes mention WSL, no hosts or paths) and `scripts/` contain no local paths, host names or SSH aliases. Recommendation: keep them public (they document how the project is developed); the maintainer decides.
- Added: `SECURITY.md` (GitHub private vulnerability reporting), issue forms (bug with version/OS/Git/crash report, feature request with "how does GitKraken do it"), PR template, `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1, reports via the private reporting channel), CONTRIBUTING (contact, no CLA/DCO), README for the public (features, install per platform with the Gatekeeper and SmartScreen notes, coming from GitKraken, privacy, accessibility, build).

### [ ] M6-12 User documentation

- `docs/user/` (Markdown, rendered on GitHub): getting started, coming from GitKraken (shortcuts and deviations from UX-PARITY in user language), daily workflow, branches and remotes, merge/rebase/conflicts, undo, settings, privacy (what contacts the network and when), troubleshooting (credentials, askpass, external tools, crash reports), known limitations.
- Linked from README and the About dialog.

Acceptance: every feature in the CHANGELOG since 0.1.0 is covered; screenshots current.

### [ ] M6-13 Consolidated walkthrough and GitKraken verification

- Claude Code merges all open maintainer to-dos (Windows, keyboard, credentials) from HANDOVER into one checklist `docs/RELEASE-CHECKLIST.md`, grouped by area, run against the packaged build (not `cargo run`).
- The maintainer runs it on Windows (and macOS if available) and answers the GitKraken verification items; Claude Code fixes findings, adjusts behavior/UX-PARITY per the answers and re-runs affected tests.
- Old to-dos in HANDOVER are ticked off or moved to the backlog explicitly.

Acceptance: checklist complete; HANDOVER "Maintainer to-dos" contains no M2–M5 items.

### [ ] M6-14 Release candidate v1.0.0-rc.1

- Release branch `release/1.0.0`, version `1.0.0-rc.1`, CHANGELOG section, tag on `master` by the maintainer, draft pre-release by the workflow.
- Maintainer: make the repository public, apply at SignPath (M6-10), announce where desired.
- Further release candidates (`rc.2`, …) for findings from M6-10/M6-13.

Acceptance: pre-release published with all artifacts; update check (M6-06) of a `rc.1` build sees a later `rc.2`.

### [ ] M6-15 Release v1.0.0

Same procedure as before (`/release`), only when the maintainer asks; Windows artifacts signed.
