# M10 – Clone, init and Git settings

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M9.

## Goal

A GitKraken user can start with Gitloom from scratch: clone or create a repository from the start screen, sign commits, prefill commit messages from a template and copy a ready-made prompt for writing a commit message with a tool of their choice. The small GitKraken options that were missing (prune, `.orig` files, long paths, toolbar labels) are there.

## Scope

In scope:

- Start screen like GitKraken's New Tab
- Clone with progress, cancel and askpass
- Init ("Start a local repo") with default branch, README and `.gitignore`
- Preferences → General: default branch name, auto-prune, delete `.orig` files, long paths (Windows); Appearance: toolbar labels
- Commit signing (GPG and SSH): Preferences → Signing, signature status in the commit details
- Preferences → Repository → Commit: commit template and message prompt
- "Copy commit message prompt…"

Out of scope (later or never):

- Creating repositories on a hosting provider, cloning from a provider account list (non-goal: provider APIs)
- License templates for init; key generators; signing key per identity (ideas backlog)
- AI inside Gitloom (non-goal)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Start screen | Layout like GitKraken's New Tab: a left column of large buttons "Open a repo", "Clone a repo", "Start a local repo"; next to it "Recent repositories" (name as link, path muted, up to 20, missing ones removable as today), "Customize" (Preferences, Manage identities) and "Resources" (User guide, Report an issue, Release notes – open the browser at the GitHub repository's `docs/user/README.md`, new-issue page and `CHANGELOG.md`). The path input (ADR-010) stays as fallback under "Open a repo". No "Start a hosted repo", no boards. Ctrl+O and Ctrl+Shift+O unchanged. |
| Clone dialog | Modal "Clone a repository": URL (any URL Git accepts, also a local path), "Where to clone to" (parent folder, Browse… and text field, default = parent folder of the last clone or of the most recent repository, else the home folder), Name (derived from the URL without `.git`, editable), "Full path" preview, checkbox "Clone submodules" (on, `--recurse-submodules`). Validation: URL not empty; the target must not exist or must be an empty folder. Button "Clone". |
| Clone run | `git clone --progress [--recurse-submodules] <url> <path>` through the network path of M3 (progress, Cancel, askpass, no credentials stored). Progress inside the dialog. On success the repository opens in the tab the dialog was started from (start screen tab) and is added to the recent list. Failure or cancel: Git removes a target it created; Gitloom checks that no partial folder is left that did not exist before and removes it only then. Not undoable (ADR-043, new folder). |
| Init dialog | Modal "Start a local repository": Name, "Initialize in" (parent folder), "Full path" preview, Default branch (prefilled from `init.defaultBranch`, else `main`), checkbox "Create README.md" (on; content `# <name>`), `.gitignore` template (None plus a bundled set from github/gitignore, CC0: Rust, Node, Python, Java, Go, C++, VisualStudio, macOS, Windows, JetBrains; listed in the third-party notices). The target must not exist or must be empty. |
| Init run | `git init -b <branch> <path>`, write the chosen files, then an initial commit "Initial commit" with these files if any were chosen and an identity resolves; without identity the files stay uncommitted and a notice explains why. Opens in the tab, added to recent. An empty repository (unborn HEAD) must display without errors (verify, fix if needed). Not undoable (ADR-043). |
| Default branch name | Preferences → General "Default branch name" (GitKraken label): writes `git config --global init.defaultBranch`; empty unsets it. Hint: "Used when creating new repositories." |
| Auto-prune | Preferences → General "Prune remote branches on fetch" (GitKraken "Auto-Prune"), default on: manual fetch, auto-fetch and the fetch of pull get `--prune`. Off: no flag, so the user's `fetch.prune` config still applies. |
| `.orig` files | Preferences → External tools "Delete .orig files after merging" (default off, like GitKraken): `git mergetool` runs with `-c mergetool.keepBackup=false` while on. |
| Long paths | Windows only, Preferences → General "Support long paths" (GitKraken "Longpaths"): reflects and writes `git config --global core.longpaths` (true / unset). |
| Toolbar labels | Preferences → Appearance "Show toolbar labels" (default on); off shows icons only with tooltips. |
| Signing settings | New global category Preferences → Signing (after Identities). Format: "GPG (OpenPGP)" or "SSH" (`gpg.format`). Key: GPG = secret keys from `<gpg program> --list-secret-keys --with-colons --keyid-format=long` (ID, user id, expiry; expired keys marked), with a refresh button; SSH = public keys from `~/.ssh` (`*.pub` with a private key next to it, M8 discovery) plus Browse…. Writes `user.signingkey` (GPG key id; SSH: path of the `.pub` file). "GPG program" (`gpg.program`, empty = Git's default, Browse…), "Sign commits by default" (`commit.gpgSign`), "Sign tags by default" (`tag.gpgSign`). All in the global Git config (ADR-046); a repository-level value is shown as "overridden in this repository" with its value, read-only. Gitloom never handles passphrases: GPG uses its pinentry, SSH uses the agent. |
| Signing in operations | Commit, amend, merge commits, annotated tags and the rebase/cherry-pick steps sign through Git's config automatically (Git CLI). A signing failure shows "Signing failed" with Git's message as details and the hint "Check Preferences → Signing" (ADR-044). |
| Signature status | Commit details show a badge after the author line when the commit is signed, from `git log -1 --format=%G?%x00%GS%x00%GK <oid>` run in the background when the commit is selected (cached per commit): G "Signed by <signer>" (success color); U "Signed by <signer> (unverified trust)"; X/Y "Signed, key expired"; R "Signed, key revoked"; E "Signed (key not available)" – for SSH also when `gpg.ssh.allowedSignersFile` is missing; B "Bad signature" (error color); N no badge. Tooltip: key id. |
| Commit template page | Preferences → Repository → Commit (after SSH key): like GitKraken's "Commit Template": "Apply this template to commit messages", "Remove comments from commit messages", Summary (with counter), Description, "Save commit template". Storage: Git's `commit.template` file (resolved local over global). Without one, saving writes `.git/gitloom/commit-template.txt` and sets local `commit.template` to it; an existing template file is edited in place (a global one only after the confirmation "This template is used by all repositories. Change it?"). The two checkboxes are stored as `gitloom.applyCommitTemplate` / `gitloom.removeComments` in the repository config. File format: summary line, blank line, description. |
| Template use | With "Apply" on, an empty commit message (WIP selected, after a commit, after discarding the message) is prefilled from the template. "Remove comments": lines starting with `core.commentChar` (default `#`) are removed from the prefill and from the message when committing. Amend, reword and operation messages are not prefilled. |
| Message prompt | Same page, section "Commit message prompt (all repositories)": the instruction text, stored in `settings.json` (global), with "Reset to default". Default: "Write a Git commit message for the staged changes below. Follow the commit template if one is given and the style of the recent commits. Summary: imperative mood, at most 72 characters. Then a blank line and a short description of what changed and why. Reply with the commit message only." |
| Copy prompt | "Copy commit message prompt…" in the context menu of the Staged Files header and the staged file list, and in the palette; disabled with "Stage changes first" when nothing is staged. Modal "Commit message prompt": read-only preview, size ("12,345 characters"), notes for omitted parts, the line "Check the diff for secrets before sharing it.", buttons Copy (also Ctrl+C) and Close; Copy closes the modal and confirms in the status bar. Content in this order: instruction; commit template (if any); "Branch: <name>"; "Recent commits:" with the last 10 summaries of HEAD; "Files:" with status and +/- counts; the staged diff (`git diff --cached`, 3 lines context). Limits: 60,000 characters for the diff; binary files, files over 256 KB and files that do not fit are listed as "(diff omitted)". No network, no AI (non-goal); a Gitloom extension (UX-PARITY). |
| Undo | Clone and init: not undoable (ADR-043). Preference and Git config changes are settings, not undo entries; `docs/user/undo.md` lists them under "Cannot be undone" with the reason "change them back in the Preferences". |
| Templates page | New global category Preferences → Templates (after Signing). Sections: "Commit message prompt" (moved here from Repository → Commit; if M10-07 is not done yet, build it here directly) and "Stash message". Each section links to the other template place: "Commit template: set per repository in Repository · <name> → Commit". The commit template itself stays repository-specific like GitKraken. Maintainer decision 2026-11-11. |
| Stash message template | Section "Stash message" on the Templates page, like the external editor setting: checkbox "Use stash message template" (default off = Git writes its default message); while off the dropdown is disabled. Dropdown with presets and "Custom": `WIP on {branch} · {datetime}`, `{branch} · {datetime}` (first entry), `{datetime} · {branch}`, `{branch}: {summary}`, `{branch}/{datetime}` (hint: "groups stashes by branch in the STASHES section"). Custom shows a text field. Placeholders: `{branch}` (detached HEAD: "detached"), `{date}` (`2026-11-11`), `{time}` (`10:42`), `{datetime}` (`2026-11-11 10:42`), `{sha}` (short HEAD), `{summary}` (HEAD summary). Unknown placeholder: inline hint "Unknown placeholder {x}"; empty custom: hint "Git's default message is used". A preview line shows the result for the active repository ("Open a repository to see a preview" on the start screen). A message the user entered for a stash always wins; the template applies whenever a stash would otherwise get no message. Stored in `settings.json`. Gitloom extension (UX-PARITY). Settings are not undo entries (ADR-043). |
| External editor per repository | Preferences → Repository · <name> → "External editor": "Use global setting (<name of the global editor>)" or the same presets plus custom command as the global setting. Stored as `gitloom.externalEditor` in the repository config (pattern of the SSH key per repository, M8-05). All entry points (Alt+E, toolbar button, palette, file context menus) use the repository value when set. The toolbar button "Editor" shows when either the global or the repository editor is set. |

## Maintainer answers and GitKraken verification items

Answered (GitKraken 7.5.2 screenshots, 2026-11-11): New Tab layout (Open/Clone/Start a local repo, Recent Repos, Customize, Additional Resources); General has Auto-Fetch Interval, Auto-Prune, Default Branch Name, External Merge/Diff Tool, External Editor, Delete ".orig" files, Default Terminal, Remember tabs, Longpaths; GPG page: Signing Key, GPG Program, Sign Commits/Tags by Default, Generate; repo-specific Commit Template: Apply, Remove comments, Summary, Description, Save; UI Customization has "Show toolbar icon labels".

Open:

| Item | Used in |
|---|---|
| GitKraken's clone dialog: labels, default folder, does it open the repository automatically? | M10-02 |
| GitKraken's init dialog: fields, does it create an initial commit? | M10-03 |
| How does GitKraken show a signed commit in the commit details? | M10-05 |
| GitKraken: is there any stash message template or prompt? (expected: no → extension) | M10-10 |

## Milestone acceptance

- [ ] The start screen looks and works like GitKraken's New Tab (without hosted repos and boards).
- [ ] Clone over HTTPS (askpass) and SSH works with progress and cancel; the repository opens; a failed or cancelled clone leaves no new folder.
- [ ] Init creates the repository with the chosen branch, README and `.gitignore` and an initial commit; an empty repository displays without errors.
- [ ] Signing settings write the Git config; signed commits and tags work with GPG and SSH; the commit details show the signature status.
- [ ] The commit template prefills empty messages and is stored as Git's `commit.template`; comments are removed when chosen.
- [ ] "Copy commit message prompt…" copies the documented content within the limits.
- [ ] Prune, `.orig`, long paths and toolbar labels work as described.
- [ ] Preferences → Templates holds the commit message prompt and the stash message template; quick stashes use the template when enabled.
- [ ] A repository can override the external editor; all entry points use it.
- [ ] User guide, UX-PARITY, `docs/user/undo.md`, release checklist and CHANGELOG updated; third-party notices list the `.gitignore` templates; `./scripts/check.sh` green; CI green.

No release (ADR-042): when all tasks are done, continue with M11.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M10-01 | Start screen like GitKraken's New Tab | – |
| M10-02 | Clone | M10-01 |
| M10-03 | Init and default branch name | M10-01 |
| M10-04 | Signing settings | – |
| M10-05 | Signature status in the commit details | M10-04 |
| M10-06 | Commit template | – |
| M10-07 | Copy commit message prompt | M10-06 |
| M10-08 | Small options | – |
| M10-09 | Documentation | M10-01 … M10-08, M10-10, M10-11 |
| M10-10 | Templates page and stash message template | M10-07 |
| M10-11 | External editor per repository | – |

---

## Tasks

### [x] M10-01 Start screen like GitKraken's New Tab

- New layout per the decisions table; Clone and Start buttons open the dialogs of M10-02/M10-03 (until then disabled with "Not available yet").
- Resource links open the browser (existing `open_target`).

Acceptance: app tests for every entry (open, recent, remove missing, links, Preferences, Manage identities); snapshots in both themes at 100 and 125 %.

Note (2026-11-11): done. `ui/start.rs` rebuilt: big buttons with icon and shortcut line, the path input (ADR-010) under "Open a repo", Clone/Start disabled with "Not available yet"; right column Recent repositories (name in the link color, path muted), Customize, Resources. Links: `<repo>/blob/master/docs/user/README.md`, `<repo>/issues/new`, `<repo>/blob/master/CHANGELOG.md` (`master` = the released docs). "Remove missing" is unchanged (a recent repository that fails to open offers "Remove from recent"). `start_screen_entries` clicks every entry in the simulator (part of every run); snapshots `ui_check_start_screen` (dark/light, 100/125 %) checked.

### [x] M10-02 Clone

- git-core: clone through the network path (progress parsing, cancel, askpass), name derivation from URLs (HTTPS, SSH scp-like, `ssh://`, local paths, trailing `.git` and `/`), target validation, cleanup rule.
- App: dialog, progress, cancel, open in the tab, recent list.

Acceptance: integration tests: clone from a local bare repository (also with a submodule and `--recurse-submodules`), cancel removes the new folder, an existing non-empty target is refused; unit tests for name derivation; app tests for validation and the flow; snapshot of the dialog.

Note (2026-11-11): done. git-core `clone` module: `repo_name` (HTTPS, scp-like, `ssh://`, `file://`, local and Windows paths; a bare host gives no name), `target_problem`, `clone` through `run_network`; on failure or cancel a folder that did not exist before is removed. New `CliBackend::with_config` passes `git -c` settings (used by the tests: `protocol.file.allow` for the local submodule, `protocol.ext.allow` for a remote that never answers to test Cancel). App: `clone_dialog.rs` (pure state and validation; the target folder is checked by the clone in the background, no I/O on the UI thread), dialog in `overlays.rs`, progress stream like network operations, Esc cancels a running clone. The default folder is the parent of the most recent repository, else the home folder (the last clone becomes the most recent repository, so this covers "last clone" without extra state). Integration tests use a local non-bare source (the clone behaves the same; a bare source adds nothing). Tests: `crates/git-core/tests/clone.rs`, unit tests in `clone.rs` and `clone_dialog.rs`, app `clone_dialog_flow`, `failed_clone_shows_gits_error_in_the_dialog`; snapshots `ui_check_clone_dialog` checked.

### [x] M10-03 Init and default branch name

- git-core: init with branch, files, initial commit; bundled `.gitignore` templates (CC0, notices).
- App: dialog; Preferences → General "Default branch name"; empty repository display checked and fixed if needed.

Acceptance: integration tests: init with and without files, with and without identity, branch name honored; empty repository loads (graph empty, WIP row, commit works); settings test for `init.defaultBranch`; snapshot of the dialog.

Note (2026-11-11): done. git-core `init` module (`init`, `init_with` for a prepared backend, `GITIGNORE_TEMPLATES` via `include_str!` from `crates/git-core/assets/gitignore/` with the CC0 license; `scripts/third-party-licenses.sh` appends them to the notices and the About list). The identity check reads `user.name`/`user.email` like the commit panel (`identity_problem`), so Git's guessed identities do not count. New git-core helpers `global_config` / `set_global_config` (also used by `global_identity`). App: `InitDialog` next to `CloneDialog` (`clone_dialog.rs`), dialog in `overlays.rs`, the default branch is loaded in the background when the dialog opens; Preferences → General gets "Default branch name" with Save (validated with `check_branch_name`, empty unsets). The empty repository was already handled (`snapshot::tests::empty_repository`); `crates/git-core/tests/init.rs` checks that it loads and takes a first commit. Tests: `init.rs`, `global_config.rs`, app `init_dialog_flow`, `default_branch_name_preference`, `clone_dialog::init_tests`; snapshots `ui_check_init_dialog` checked.

### [x] M10-04 Signing settings

- git-core: read and write the signing keys of the global config with origin; GPG key listing (colon format parser, expiry); SSH key discovery reused.
- Preferences → Signing per the decisions table; signing failure message with the hint.

Acceptance: unit tests for the colon parser (fixtures with several keys, expired, no keys, gpg missing); integration tests with a temporary `GIT_CONFIG_GLOBAL`: format, key, default flags written and read back; a repository override shown read-only; SSH signing integration test where `ssh-keygen` is available (commit, then `%G?` is not `N`); snapshot of the category.

Note (2026-11-11): done. git-core `signing` module (settings of the global config, `set_global_signing` limited to the five signing keys, `local_signing_overrides`, colon parser and `list_gpg_keys`, `signing_failed`). Deviation in the SSH test: without `gpg.ssh.allowedSignersFile` Git cannot verify SSH signatures and `%G?` reads `N` (with an error on stderr), so the test checks the `gpgsig` header instead; M10-05 must treat that case as "Signed (key not available)". The SSH part runs on Unix only (Windows OpenSSH refuses keys whose ACLs it finds too open). The GPG key picker selects by the configured id's suffix (short or long ids). Preferences → Signing sits between Identities and External tools; every change is written right away and the category reloads. A commit (or any write) whose error is a signing error shows "Signing failed" with the hint and Git's output as details. Tests: `signing.rs` unit tests, `crates/git-core/tests/signing.rs`, app `signing_preferences_and_failure_hint`; snapshots `ui_check_signing` checked.

### [x] M10-05 Signature status in the commit details

- Background lookup and cache, badge per the decisions table.

Acceptance: parser tests for every `%G?` value; app test: selecting a commit requests the status once; snapshot with a good and a bad signature.

Note (2026-11-11): done. git-core `signature` module: `parse_signature` and `signature`; Git reports an SSH signature it cannot verify (no allowed signers file) as `N`, so a commit with a `gpgsig` header and `N` becomes "Signed (key not available)" as planned. The status is cached per commit in the tab (`RepoView::signatures`), requested with the changed files; stashes are skipped. Badge: check icon (alert icon for a bad signature), success/danger/muted color, key id as tooltip. Integration coverage in `crates/git-core/tests/signing.rs` (unsigned, SSH without and with allowed signers). Snapshots `ui_check_signature_badge` checked.

### [x] M10-06 Commit template

- Preferences → Repository → Commit with the template section; storage and prefill rules per the decisions table.

Acceptance: integration tests: save without `commit.template` creates the file and the config entry; an existing local template file is edited in place; a global one only after confirmation; comment removal with a custom `core.commentChar`; app tests for prefill (empty message only, not for amend); snapshot.

Note (2026-11-11): done. git-core `commit_template` module (`read_template` with the scope from `git config --show-scope`, `save_template`, `set_template_option`, `split_template`, `strip_comment_lines`); `GitBackend::commit_template_options` (gix) feeds the snapshot. Decision: both options default to on when the keys are unset, which keeps the earlier behavior (the template was always prefilled, comments removed). Change: the template's first line now prefills the summary (file format summary line, blank line, description); before, the whole template went into the description. "Remove comments" also strips comment lines from the description when committing, with `core.commentChar` (`auto` counts as `#`). Prefill only for an empty message, never while amending; operation messages keep their own prefill. A template in a shared (non-repository) config asks "This template is used by all repositories. Change it?". Saving refreshes the tab so the prefill follows. Tests: `crates/git-core/tests/commit_template.rs`, unit tests, app `template_prefills_summary_and_description`, `comments_are_removed_from_the_commit_message`, `commit_template_page_loads_saves_and_confirms_shared_files`; snapshot `ui_check_commit_template` checked.

### [x] M10-07 Copy commit message prompt

Note (planning update 2026-11-11): the "Commit message prompt" section lives on the global Preferences → Templates page (M10-10); build it there directly instead of on Repository → Commit.

- Prompt builder in git-core or a pure app module (no I/O in the view), the message prompt section on the same page, entry points, modal, clipboard.

Acceptance: unit tests for the builder (order of parts, template present/absent, limits, binary and large files, truncation notes); app tests for the entry points and the disabled state; snapshot of the modal.

Note (2026-11-11): done. Builder in git-core `prompt` (`gather` runs `git` in the background, `build` is pure); the diff of a file is taken whole or omitted, so a cut never falls inside a hunk. The template in the prompt follows "Remove comments" of M10-06. Preferences → Templates (new global category after Signing, planning update 2026-11-11) holds the instruction with "Reset to default"; the stash message section follows in M10-10. Entry points: context menu of the Staged Files header (new, with "Unstage all changes"), staged file menu, palette action "Copy commit message prompt…" (disabled with "Stage changes first"). Ctrl+C copies while the dialog is open (only when no widget handled the key). Tests: unit tests in `prompt.rs`, `crates/git-core/tests/prompt.rs`, app `copy_commit_prompt_needs_staged_changes`, `prompt_dialog_copies_and_closes`, `prompt_instruction_is_edited_and_reset`, settings migration test; snapshots `ui_check_commit_prompt` checked.

### [ ] M10-08 Small options

- Auto-prune, `.orig` files, long paths (Windows only), toolbar labels per the decisions table.

Acceptance: unit tests for the fetch and mergetool arguments; settings migration test; toolbar snapshot without labels.

### [ ] M10-09 Documentation

- User guide: `getting-started.md` (start screen, clone, init), `settings.md` (General, Signing, Repository → Commit, Templates, external editor per repository), `daily-workflow.md` (template, prompt, stash template), `coming-from-gitkraken.md`.
- UX-PARITY rows; extensions: commit message prompt, signature status texts, stash message template; deviation "commit message prompt on a global Templates page".
- `docs/user/undo.md`: clone, init and settings under "Cannot be undone".
- `docs/RELEASE-CHECKLIST.md` section "7d. New in M10" (clone HTTPS and SSH with credentials, init, GPG signing with Gpg4win pinentry, SSH signing with the agent, template, prompt, long paths, stash with every preset and custom, external editor per repository).
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed against the implemented behavior.

### [ ] M10-10 Templates page and stash message template

- Preferences → Templates per the decisions table; move the commit message prompt section; stash message template with presets, custom, validation and preview; the stash push uses the rendered template when enabled and no message was entered.

Acceptance: unit tests for rendering (every placeholder, detached HEAD, unknown placeholder, empty custom); integration test: stash with template on → `git stash list` shows the rendered message; off → Git's default; an entered message wins; settings migration test; snapshots of the Templates page (off, preset, custom with an unknown placeholder) in both themes.

### [ ] M10-11 External editor per repository

- Repository page entry and resolution per the decisions table.

Acceptance: unit test for the resolution (repository over global, global only, none); app tests: Alt+E and a file context menu use the repository editor; the toolbar button shows with only a repository editor; snapshot of the repository page.
