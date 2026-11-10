# M8 – Repository settings and tools

Target version: **1.2.0**

Starts on `develop` once `release/1.1.0` is branched off (ADR-040); released after `v1.1.0`.

## Goal

Gitloom shows and changes the Git identity and the SSH key of each repository, keeps a list of identities with a global default, and opens the repository or a file in an external editor. Repository settings (Identity, SSH key, Gitflow) sit in one group of the Preferences like GitKraken's repo-specific preferences.

## Scope

In scope:

- Preferences: group "Repository · <name>" with the categories Identity, SSH key and Gitflow (moved from M7) for the active tab's repository
- Identities (name and email) in Preferences → Identities; a default identity kept in the global Git config
- Identity per repository: "Committing as …" in the commit panel, command palette, repository category; writes `user.name`/`user.email` to the repository config
- SSH key per repository through `core.sshCommand`
- External editor: Preferences → External tools; open the repository (palette, Alt+E, toolbar) and single files (file context menus)
- User guide, UX parity and release 1.2.0

Out of scope (later):

- Workspace profiles like GitKraken's (own tabs and settings per profile); decided 2026-11-09: identities only
- Automatic identity by remote URL or path (`includeIf`), signing keys per identity, SSH key generator, toolbar picker dropdowns (ideas backlog)
- GitKraken's other repo-specific categories (Encoding, Git Hooks, LFS, Commit template, Submodules)
- Storing passphrases or credentials (never)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Preferences structure | Category list: global categories General, Appearance, Identities, External tools; below them a group header "Repository · <name>" (the active tab's repository, truncated with tooltip) with Identity, SSH key, Gitflow, like GitKraken's "Repo-Specific Preferences". On the start screen the group header reads "Repository" and its categories are disabled with "Open a repository first". "About" stays at the bottom. When the active tab changes, the repository categories follow it. Replaces the single "Repository" category of the ROADMAP outline: one page per topic keeps the Gitflow page unchanged. |
| Source of truth | Git's configuration (ADR-041). Gitloom stores only the list of identities in `settings.json`. A repository's identity is what `user.name`/`user.email` resolve to (local over global over system), read through gix with the origin of each value; the SSH key is `core.sshCommand`. Changes made outside Gitloom appear like other config changes (config fingerprint of M6-18). |
| Identities | Preferences → Identities: list (name, email, "Default" marker), Add, Edit, Remove (confirmation), "Set as default". Validation: name not empty; email not empty, contains `@`, no spaces, no `<` or `>`; the same name and email twice is refused. When the list is empty and the global config has `user.name` and `user.email`, they become the first identity and the default, without asking. |
| Default identity | Checkbox "Keep the global Git config updated with the default identity" (on by default; GitKraken's profile option "Keep my .gitconfig updated with my profile info"). While on, setting or editing the default writes `git config --global user.name/user.email`. Off: Gitloom never writes the global config. Removing the default identity leaves the global config untouched. |
| Repository identity | Commit panel, above the summary: muted line "Committing as <name> <email>", tooltip with the origin ("from the repository config" / "from the global config" / "from the system config"). A click opens a menu: the identities (check on the current one), "Use global identity (<name>)" when the repository has its own values, "Save as identity" when the current values are not in the list, "Manage identities…". Choosing an identity writes `git config --local user.name/user.email`; "Use global identity" unsets both local keys (a missing key is not an error). The same choices are in the palette ("Choose identity…" sub-list, like "Checkout branch…") and in Preferences → Repository → Identity. Not undoable (configuration, not history); the status bar confirms "Committing as <name> in <repository>". |
| No identity | If name or email does not resolve: the line reads "No Git identity set" in the warning color, Commit, Amend and Continue are disabled with "Choose an identity first", and the menu offers the identities and "Add identity…". |
| SSH key | Preferences → Repository → SSH key: "System default (ssh-agent, ~/.ssh/config)" or a key: the keys found in `~/.ssh` (a file `X` with a sibling `X.pub`; `known_hosts*`, `config` and `authorized_keys` excluded) plus "Browse…". A key writes `core.sshCommand = ssh -i "<key>" -o IdentitiesOnly=yes` to the repository config (forward slashes on Windows, path in double quotes, a path containing `"` is refused); "System default" unsets it. A local `core.sshCommand` in another form is shown read-only ("Custom SSH command: …") and replaced only after "Replace…" with confirmation. A value from the global or system config is shown as inherited with its origin; choosing a key overrides it locally. Without any SSH remote the page says "This repository has no SSH remotes" (choices stay available). Passphrases go through the existing askpass prompt and are never stored. |
| External editor | Preferences → External tools → "External editor": presets None (default), VS Code (`code {path}`), VSCodium (`codium {path}`), Sublime Text (`subl {path}`), IntelliJ IDEA (`idea {path}`), Zed (`zed {path}`), Custom. A preset fills an editable command template with `{path}` (same rules as the terminal template of M5-13; without `{path}` the path is appended as the last argument). Windows: launchers like `code.cmd` are found through `PATHEXT` and started without a console window. |
| Editor entry points | "Open repository in editor": palette, Alt+E, toolbar button "Editor" to the right of "Terminal", shown only while an editor is set; `{path}` = repository root. "Open in external editor": file context menus of the unstaged and staged lists, the commit file list and the file history (only when the file exists in the working tree); `{path}` = absolute file path (the helper of M6-21). Disabled reasons: "Set an external editor in Preferences → External tools", "The file does not exist in the working tree". |

## Maintainer decisions and GitKraken verification items

Answer what you can in GitKraken before or during M8. Claude Code decides the rest conservatively (ADR-034) and records it in UX-PARITY.

| Item | Used in |
|---|---|
| Does GitKraken show the committing identity in the commit panel? Where, and is it clickable? | M8-04 |
| Order and labels of the repo-specific categories in a current GitKraken | M8-01 |
| External editor: setting label, presets, text of the file context menu entry, shortcut (if any) | M8-07, M8-08 |
| SSH: GitKraken has one global key (Preferences → SSH); confirm there is no per-repository key | M8-06 |

## Milestone acceptance

- [ ] Preferences show the group "Repository · <name>" with Identity, SSH key and Gitflow for the active tab; Gitflow works as in 1.1.
- [ ] Identities can be added, edited, removed and set as default; the default updates the global Git config only while the option is on.
- [ ] The commit panel shows the identity the next commit uses; choosing another writes the repository config and the next commit uses it as author and committer; "Use global identity" removes the local keys.
- [ ] Without an identity, Commit, Amend and Continue are disabled with the reason.
- [ ] A key chosen for a repository is used by fetch, pull and push of that repository (scripted test with a fake `ssh`); "System default" restores the previous behavior; a foreign `core.sshCommand` is never replaced without confirmation.
- [ ] The repository and single files open in the configured editor from every entry point, also with `code.cmd` on Windows.
- [ ] Changes to `user.*` or `core.sshCommand` made outside Gitloom appear without a restart.
- [ ] User guide, UX-PARITY and CHANGELOG updated; `./scripts/check.sh` green; CI green on all platforms.
- [ ] Released as `v1.2.0` after `v1.1.0`.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M8-01 | Repository group in the Preferences | – |
| M8-02 | Git identity model (git-core) | – |
| M8-03 | Identities in the Preferences | M8-02 |
| M8-04 | Identity per repository | M8-01, M8-03 |
| M8-05 | SSH key model (git-core) | – |
| M8-06 | SSH key per repository (UI) | M8-01, M8-05 |
| M8-07 | External editor setting and launcher | – |
| M8-08 | Editor entry points | M8-07 |
| M8-09 | Documentation and walkthrough | M8-01 … M8-08 |
| M8-10 | Release v1.2.0 | all, `v1.1.0` |

---

## Tasks

### [x] M8-01 Repository group in the Preferences

- `settings::Category` distinguishes global and repository categories; the category list renders the group header "Repository · <name>" (truncated with tooltip) above the repository categories.
- Gitflow moves into the group unchanged (form, validation, live refresh); "Initialize Gitflow" in the palette still opens it.
- Start screen: header "Repository", categories disabled with "Open a repository first".
- The repository categories follow the active tab (title and form reload).
- Identity and SSH key are added by M8-04 and M8-06; this task ships only the group with Gitflow.

Acceptance: app tests for the group with and without a repository, a tab switch and the palette entry; `iced_test` snapshot of the category list in both themes.

Note (2026-11-10): done. `Category::GLOBAL` / `Category::REPOSITORY` (Identities is added to the global list by M8-03); the group header is a muted section label like the left panel's, names longer than 22 characters end in "…" with the full name as tooltip. `Page::repo` remembers the tab and repository a repository form was built for; the central `App::update` reloads the form when the active tab shows another repository and returns to General when it shows none (also covers tabs that finish loading or close). `SettingsCategory` ignores repository categories without a repository; disabled list items are now drawn muted (`theme::list_item`). The Gitflow form title stays "Gitflow · <name>" (unchanged). Snapshot `ui_check_preferences_repository_group` checked by eye in both themes.

### [x] M8-02 Git identity model (git-core)

- `Identity { name, email }` with the validation from the decisions table (shared with the app).
- Reading: resolved `user.name` and `user.email`, each with its origin (local, global, system, other), from the gix config the backend already holds; part of the repository snapshot, compared by the refs/config check so outside changes refresh the tab.
- Writing through `CliBackend`: set local, set global, unset local (exit code 5 for a missing key is not an error). Unsetting removes the `[user]` section when it ends up empty, so no empty section is left behind (see M6-20).
- Tests never touch the user's global config (`GIT_CONFIG_GLOBAL` and `GIT_CONFIG_NOSYSTEM` set to temporary values).

Acceptance: integration tests: local over global, unset local falls back to global and leaves no empty `[user]` section, global write, nothing set = no identity, an outside `git config` change refreshes the snapshot; unit tests for the validation.

Note (2026-11-10): done. `git_core::git_identity`: `Identity` with `identity_problem` (also refuses `<`/`>` in the name, which Git would reject), `ConfigOrigin`, `ResolvedIdentity` (`GitBackend::identity`, read by `GixBackend` from the `user` sections in load order, the last non-empty value wins, empty values count as unset), `set_identity` (local/global) and `unset_local_identity` (`--unset-all`, exit code 5 ignored, `--remove-section user` only when no `user.*` key is left, so e.g. `user.signingkey` stays). The identity is part of the config part of the snapshot (`snapshot::Remotes`), so the refs/config check refreshes the tab after an outside `git config` change. Tests: `tests/git_identity.rs` (own binary, temporary `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM`), unit tests in the module, `a_changed_gitflow_config_counts_as_changed_refs` extended.

### [ ] M8-03 Identities in the Preferences

- `settings.json` gains `identities` (list of name and email, default, `sync_global` = true); older files load unchanged (missing = empty list, sync on), settings version unchanged.
- Import of the global identity on first use (decisions table).
- Preferences → Identities: list, Add/Edit (form dialog with two fields and live validation), Remove (confirmation), "Set as default", the sync checkbox.
- Writes to the global config run like other writes (busy, Git missing: buttons disabled with the reason).
- Palette: "Manage identities" opens the category.

Acceptance: unit tests for validation, settings migration and import; app tests: setting the default writes the global config while sync is on and writes nothing while it is off; snapshot of the category in both themes.

### [ ] M8-04 Identity per repository

- Commit panel line "Committing as …" with origin tooltip and menu (decisions table); no-identity state with disabled Commit, Amend and Continue.
- Palette "Choose identity…" sub-list with the same entries.
- Preferences → Repository → Identity: effective name and email with origin, the identities as a choice list, "Use global identity", "Save as identity".
- Status bar confirmation after a change.

Acceptance: integration test: choose an identity → commit → author and committer of `HEAD` match; "Use global identity" removes both local keys; without an identity Commit, Amend and Continue are disabled with the reason; a change in a terminal updates the line without restart; snapshots of the commit panel with and without identity in both themes.

### [ ] M8-05 SSH key model (git-core)

- `SshSetting`: `SystemDefault`, `Key(path)` (the form Gitloom writes), `Custom(command)`, each with the origin of `core.sshCommand`.
- `ssh_command(path)`: forward slashes, double quotes, refuses `"`; parse and format round-trip.
- Key discovery in a given directory (decisions table rules), sorted by file name; the home `.ssh` directory comes from the existing directories helper.
- Write and unset in the repository config (no empty `[core]` section is possible, but unset must not fail when the key is missing); helper "has SSH remotes" (`ssh://`, `git+ssh://` and scp-like `user@host:path`).
- `CliBackend` sets neither `GIT_SSH` nor `GIT_SSH_COMMAND`, so the configuration applies to every network operation.

Acceptance: unit tests for parse/format (Windows paths, spaces, custom commands) and key discovery in a temporary directory; integration test on Linux and macOS: a fake `ssh` in `PATH` records its arguments, a fetch through Gitloom's network path against `ssh://example.invalid/repo` sees `-i <key>` and `IdentitiesOnly=yes` with a key set and neither with the system default (Windows: release checklist).

### [ ] M8-06 SSH key per repository (UI)

- Preferences → Repository → SSH key with the choices from the decisions table, "Browse…" (file dialog, path input as fallback like the start screen), custom and inherited display, "Replace…" with confirmation, the no-SSH-remotes hint.
- Status bar confirmation ("SSH key for <repository>: <file name>" / "system default").
- An authentication error while a key is set adds "Uses SSH key <path> (Preferences → Repository → SSH key)" to the error dialog's hint.

Acceptance: app tests: choosing a key writes the config, "System default" unsets it, a custom command is not replaced without confirmation, an inherited value is shown with its origin; snapshot of the category in both themes.

### [ ] M8-07 External editor setting and launcher

- `ExternalTools` gains `editor_command` (empty = none); preset choice and template field in Preferences → External tools.
- `launch::editor(template, path)` reuses the template rules of the terminal (`split_words`, `{path}`); without `{path}` the path is appended.
- Windows: resolve the program through `PATHEXT` (`code` → `code.cmd`), start `.cmd`/`.bat` without a console window; a failed start shows "Could not start the editor: …" in the status bar.

Acceptance: unit tests for template expansion (spaces, quotes, missing `{path}`) and Windows resolution with a fake lookup (like the terminal tests); settings migration test.

### [ ] M8-08 Editor entry points

- Action "Open repository in editor" (Alt+E, palette, shortcut list); toolbar button "Editor" right of "Terminal" while an editor is set (icon vendored like the M5 icons, license kept).
- "Open in external editor" in the file context menus (unstaged, staged, commit file list, file history) with the disabled reasons from the decisions table.

Acceptance: app tests for every entry point with a recording launcher, including the disabled states; toolbar snapshot with and without an editor.

### [ ] M8-09 Documentation and walkthrough

- User guide: `settings.md` (Identities, Repository group, External editor), "Committing as" in `daily-workflow.md`, SSH key in `branches-and-remotes.md` and `troubleshooting.md`, "Coming from GitKraken" (identities instead of profiles, SSH key per repository).
- `docs/UX-PARITY.md`: M8 rows set to done with the verification results; deviations confirmed or adjusted.
- CHANGELOG `[Unreleased]`; `docs/RELEASE-CHECKLIST.md` section "7b. New in 1.2" with the Windows checks (identity switch and commit, default identity and global config, SSH key with a passphrase via the prompt, `code.cmd` from all entry points, no console window).

Acceptance: docs reviewed against the implemented behavior; checklist section present.

### [ ] M8-10 Release v1.2.0

`/release` (ADR-034), only after `v1.1.0`.
