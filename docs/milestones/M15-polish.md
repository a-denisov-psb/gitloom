# M15 – Polish before 1.0.0

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M14. Requested by the maintainer (2026-11-13, findings #28–#36 of the planning chat, GitKraken 7.5.2 screenshots). Last milestone before the final check.

## Goal

Fix and polish what the maintainer found while using M7–M14: the conflict view handles UTF-16 and 8-bit text, the repository editor picker works and fits, Git flow branches can be finished from the left panel, the Gitflow layout is marked experimental, a narrow Graph column still shows every commit, the commit message is edited through an icon (with the prompt button in the editor), the start screen buttons are calmer, and the diff view gets an icon toolbar and a minimap like GitKraken.

## Scope

In scope:

- Conflict view: UTF-16 (with BOM) and 8-bit text files (#31)
- Preferences → Repository → External editor: picker bug and label (#32)
- Git flow production branch fallback and hints (#33)
- GITFLOW section: finish button on hover (#28); Gitflow layout marked experimental (#29)
- Graph column narrower than its lanes: nodes clamp to the right edge (#30)
- Commit details: edit icon, tooltip only on the icon, prompt button in the editor (#34)
- Start screen buttons (#35)
- Diff header as icons with Blame; diff minimap (#36)

Out of scope:

- Line staging for UTF-16 files (M7-17 limitation stays)
- Whitespace toggle (GitKraken's ¶ button): ideas backlog
- Minimap in the Hunk view, blame and file history

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Conflict encoding (#31) | Cause: a PowerShell-written `readme.md` is UTF-16 LE with BOM; `resolve.rs::is_binary` requires UTF-8 without NUL bytes, so the file is called binary. Each stage is classified as UTF-8 (BOM allowed), UTF-16 LE/BE with BOM (decoded with `diff::decode_utf16`), 8-bit text (no NUL, not UTF-8: mapped byte for byte as Latin-1, so it round-trips exactly) or binary (NUL without a UTF-16 BOM). The stages are merged as UTF-8 text with `git merge-file`; the resolved output is encoded back to the encoding of ours (BOM, byte order and line endings kept). Stages with different encodings get the whole-file choice with "The sides use different encodings (<a>, <b>). Take one side's file:". The view shows the encoding next to the path like the diff header (M7-17). Binary text: "This file is binary, so lines cannot be picked. Take one side's file:". |
| Editor picker (#32) | Root cause: `Message::RepoEditorChosen` with "Use global setting" leaves `repo_editor_custom` set, so the picker stays on Custom until another entry is chosen. Fix: choosing the global entry resets the flag and the input; unsetting a key that is not set is no error. Labels name the editor, not the command: the preset by template, else by the program's file name (first token, quotes removed, stem compared case-insensitively: `code` VS Code, `codium` VSCodium, `subl`/`sublime_text` Sublime Text, `idea`/`idea64` IntelliJ IDEA, `zed` Zed), else the program's file name. This is label only: the global picker keeps "Custom" for such commands. Below the repository picker a muted line "Global: <full command>" (wraps). Picker width stays 320. |
| Production branch (#33) | Not unified: Default branch name (`init.defaultBranch`) applies to new repositories, the Gitflow production branch names an existing branch. `FlowConfig::defaults`: `main` if it exists, else `master` if it exists, else `init.defaultBranch` if set, else `main`. Notes: General → Default branch name: "Used for new repositories (init.defaultBranch in the global Git config). Empty: Gitloom suggests main. Existing repositories keep their branches." Gitflow → Production branch: "Releases and hotfixes are merged and tagged here. Usually main or master of this repository." |
| Finish on hover (#28) | Feature, release and hotfix rows of the GITFLOW section show a flag icon (Lucide `flag`) left of the eye on hover. Tooltip "Finish feature <name>…" / "Finish release <name>…" / "Finish hotfix <name>…"; it opens the existing finish dialog (`Message::FlowFinishRequested`). When finishing is blocked, the icon is muted and its tooltip gives the reason of the context menu entry. Not on the development and production rows; LOCAL rows unchanged. |
| Experimental (#29) | Preferences → Repository → Gitflow: a muted "Experimental" pill (style of the time markers) after "Show the graph in the Gitflow layout"; note: "Experimental: can look messy when the history did not follow Git flow." Column menu "Gitflow layout (experimental)", palette "Toggle Gitflow layout (experimental)". |
| Narrow Graph column (#30) | Replaces the fade of M11. `capacity = max(1, floor((column width − padding) / LANE_WIDTH))`. Overflow is active when the lane count of the loaded history (as used by double click to fit) exceeds `capacity`; the decision is per column, not per row, so lines never break between rows. With overflow, lanes `0 … capacity−2` are drawn as now. The last slot (`capacity−1`) is the overflow slot: the nodes of every lane ≥ `capacity−1` are drawn there in their own lane color and node style (commit, merge, stash, WIP, faded deleted-branch styling of M13), without lines. Edges from visible lanes towards overflowing lanes end at the slot's left border. At the minimum width (one lane) every commit sits on one line without edges, like GitKraken's collapsed graph. Double click to fit is unchanged. |
| Edit icon (#34) | Pencil icon (Lucide `pencil`) at the top right inside the message box, shown while the box is hovered. Enabled: a click opens the editor of M14-05. Blocked: muted, with the M14-05 reason as tooltip on the icon only; the box itself has no tooltip. The message is always the selectable read-only text (also for editable commits); a click on the text no longer opens the editor (deviation from GitKraken, maintainer decision 2026-11-13). Stashes show no icon. In the editor a sparkle button (as in "Committing as …": 13 px, `text_muted`) at the right of the summary row opens "Copy commit message prompt…" built from that commit: diff against the first parent (root commit: against the empty tree), files with +/−, branch, the summaries of the commits before it, commit template and instruction. Same builder and limits as M10, with a source `Staged` or `Commit(<oid>)`; the modal says "for commit <short sha>". |
| Start buttons (#35) | Fill like the initials tile of the commit details (`theme::avatar`: accent at 25 % alpha); title, hint and icon in the normal text color; hover 35 %, pressed 45 %; radius 6. Height about three quarters of today's, title 14, hint 11, icon 18; all three buttons keep one size. WCAG AA in Dark and Light (adapt `start_buttons_reach_wcag_aa`). |
| Diff header (#36) | Icons like GitKraken, order from left after the path: Blame, History, change arrows ↑/↓, Hunk / Inline / Split as an icon segmented toggle, External, Close. Lucide icons, 15 px, hit area 22×22, tooltips with name and shortcut. Suggested: Hunk `list-collapse`, Inline `align-justify`, Split `columns-2`, External `square-arrow-out-up-right`, History `history` (exists); for Blame choose a fitting Lucide icon and justify it in the task note. Blame is new in the header: opens the blame view like "File blame" in the file menu (M13-07), at the commit of a commit diff, otherwise for the working file; disabled with the menu's reason for untracked files, stashes and deleted files. The selection actions ("Stage selected lines" …) stay text buttons. New icons go into the icon set with their license (Lucide ISC, as before). |
| Minimap (#36) | Inline and Split only. A 14 px strip at the right edge of the diff, right of the vertical scrollbar (like GitKraken). Every change line becomes a mark at its row position (rows after word wrap), at least 2 px high: removed red, added green with the theme's diff colors; Split: removed in the left half, added in the right half. The visible range is a translucent rectangle. A click centers the view on that position, dragging moves it, the mouse wheel scrolls like over the text. Marks are computed once per diff layout, not per frame. |

## GitKraken verification items

| Item | Used in |
|---|---|
| What does GitKraken draw when the Graph column is a little narrower than its lanes (not at the minimum)? | M15-05 |
| Does GitKraken's GITFLOW section show hover buttons (e.g. finish)? | M15-04 |
| Does GitKraken show the minimap in the Hunk view too? | M15-09 |
| Which icons does GitKraken use for Blame and History in the diff header (text or icon)? | M15-08 |

## Milestone acceptance

- [ ] A UTF-16 LE file with BOM and an 8-bit text file conflict like UTF-8 files; the resolved file keeps its encoding byte for byte; binary files and mixed encodings say why only whole-file choices exist.
- [ ] "Use global setting" in Preferences → Repository → External editor works from every state; the labels fit the field.
- [ ] Git flow initialization proposes `init.defaultBranch` (else `main`) when neither `main` nor `master` exists; the notes explain both settings.
- [ ] GITFLOW rows finish by the hover flag; the Gitflow layout is marked experimental everywhere it is switched.
- [ ] A narrow Graph column shows every commit; at the minimum all nodes sit on one line.
- [ ] The message is edited through the pencil icon only; the tooltip appears only on the icon; the editor offers the prompt for that commit.
- [ ] Start screen buttons look calmer and keep AA contrast in Dark and Light.
- [ ] Diff header as icons with Blame; minimap in Inline and Split.
- [ ] User guide, UX-PARITY, release checklist 7i and CHANGELOG updated; `./scripts/check.sh` green; CI green.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M15-01 | Conflict view for UTF-16 and 8-bit text | – |
| M15-02 | Repository editor picker | – |
| M15-03 | Production branch fallback and hints | – |
| M15-04 | GITFLOW finish on hover, Gitflow layout experimental | – |
| M15-05 | Narrow Graph column: overflow slot | – |
| M15-06 | Commit details: edit icon and commit prompt | – |
| M15-07 | Start screen buttons | – |
| M15-08 | Diff header icons and Blame | – |
| M15-09 | Diff minimap | M15-08 |
| M15-10 | Documentation | M15-01 … M15-09 |

---

## Tasks

### [x] M15-01 Conflict view for UTF-16 and 8-bit text

Note: ASCII-only versions fit UTF-8 and 8-bit text but not UTF-16 (mixed); UTF-8 with and without BOM count as one encoding (the mark follows ours); an 8-bit output with characters above U+00FF is refused with a message. The finish regression asserts the release branch in both conflict tests.

- Encoding classification, decoding, merge and re-encoding per the decisions table (`git-core::resolve`); `resolve_conflict` writes the output in the encoding of ours.
- Messages for binary files and mixed encodings; encoding label in the conflict view header.
- Check that a Git flow finish that stops in a conflict keeps the topic branch until the merge is committed (M7-09); add a regression test if none exists.

Acceptance: integration tests: UTF-16 LE with BOM and CRLF (the maintainer's bytes), UTF-16 BE, UTF-8 with BOM, 8-bit text with bytes ≥ 0x80 → text conflicts; after picking lines the working tree file and stage 0 are byte-identical to the expected encoding (BOM, byte order, CRLF); a binary file and a UTF-16/UTF-8 mix → whole-file choice with the right message; app snapshot of a UTF-16 conflict with its label.

### [x] M15-02 Repository editor picker

Note: an empty global command reads "Global: none"; unsetting a key that is not set was already no error (covered by `external_tools.rs`).

- Reset of the custom state, labels by program name, "Global: <command>" line.

Acceptance: app tests: Custom → Use global; Custom with an unsaved input → Use global; preset → Use global; repository command set → Use global (key removed); label tests for every preset name, a quoted Windows path (`"C:\…\Zed\Zed.exe" {path}`), an unknown program; snapshot with a long global path in both themes.

### [x] M15-03 Production branch fallback and hints

Note: `init.defaultBranch` is read from the global config when the Preferences open; a form opened before it arrived is updated unless the production field was edited (`FlowConfig::defaults_with`). The General note says "Empty: main is suggested." instead of "Gitloom suggests main" (product name only from `git_core::identity`, identity guard).

- `FlowConfig::defaults` with the `init.defaultBranch` fallback; both notes.

Acceptance: unit tests for all four cases of the order; app test: initialization form in a repository with only `trunk` and `init.defaultBranch=trunk2` proposes `trunk2`.

### [x] M15-04 GITFLOW finish on hover, Gitflow layout experimental

Note: the blocked reason (repository blocked or busy, or "Cannot merge while a merge is in progress") is shared by the flag and the branch menu entry, which now carries it as a hint; the checked-out flow branch has a flag but no eye; the Preferences search entry is "Gitflow layout (experimental)" too.

- Hover flag on feature, release and hotfix rows; blocked state; experimental pill, note and labels.

Acceptance: app tests: flag opens the finish dialog for each type; no flag on development and production; blocked during a merge with the reason; snapshots of a hovered GITFLOW row and of Preferences → Gitflow in both themes.

### [x] M15-05 Narrow Graph column: overflow slot

Note: `MIN_GRAPH_WIDTH` grew by the cell padding (4 px) so the slot's nodes are not cut off at the minimum; the 100k benchmark concerns `git-graph`, which this task does not touch (numbers of this container in `docs/PERFORMANCE.md`).

- Capacity, overflow slot and edge clipping per the decisions table; the fade code is removed.

Acceptance: unit tests for capacity and the slot mapping (fits, one lane short, minimum width); snapshots of the fixture graph at full width, half width and minimum width (Dark and Light, also with the Gitflow layout); 100k-commit benchmark unchanged (`docs/PERFORMANCE.md`).

### [x] M15-06 Commit details: edit icon and commit prompt

Note: `prompt::gather_from` with `PromptSource::{Staged, Commit}`; the diff section reads "Diff of commit <short sha>:" (the instruction stays the user's text); a busy repository mutes the pencil with "Busy: …". Checked in the real app under Xvfb (hover, tooltip on the icon only, editor, prompt dialog). The headless snapshots show the read-only `text_editor` boxes empty (also on `develop` before M15, a simulator artifact; the real app shows the text).

- Pencil icon, tooltip only on the icon, text always selectable; prompt source `Commit(<oid>)` in the builder; sparkle button in the editor.

Acceptance: app tests: a click on the text does not open the editor, the icon does; the blocked icon carries each M14-05 reason; no icon for stashes; the commit prompt contains the commit's diff (root and merge commits included) and not the staged diff; snapshots of the box with the hovered icon (enabled, blocked) and of the editor with the sparkle button in both themes.

### [x] M15-07 Start screen buttons

Note: height 45 (was 60); the contrast test composites the fill over the start screen background (`bg_graph`); `on_fill` and its constant were only used by the old buttons and are removed.

- Style per the decisions table.

Acceptance: contrast test for all states in Dark and Light; start screen snapshots in both themes.

### [ ] M15-08 Diff header icons and Blame

- Icons, order, tooltips; Blame button; icon licenses.

Acceptance: app tests: each button sends its message; Blame disabled with its reason for untracked, stash and deleted files; snapshots of the header in Hunk, Inline and Split, Dark and Light.

### [ ] M15-09 Diff minimap

- Strip, marks, viewport rectangle, click, drag, wheel; Inline and Split only.

Acceptance: unit tests for the mark mapping (with and without word wrap, Split halves); app tests: a click scrolls to the position, dragging follows; snapshots of a long file with changes at the top, middle and end in Inline and Split; no measurable cost while scrolling a 20,000-line diff.

### [ ] M15-10 Documentation

- User guide (`daily-workflow.md`, `coming-from-gitkraken.md`, Git flow page), UX-PARITY: rows for the overflow slot (#30), the pencil-only editing (#34, replaces the M14-05 row "Editable messages are plain text until clicked"), the diff header Blame and the minimap, the GITFLOW hover flag (Gitloom extension until verified); update the UTF-16 row (conflict view now handles UTF-16).
- `docs/RELEASE-CHECKLIST.md` section "7i. New in M15" (UTF-16 conflict from a PowerShell-written file, editor picker, production fallback, hover finish, experimental label, narrow graph, pencil and commit prompt, start buttons, diff icons and Blame, minimap).
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed against the implemented behavior.

## After M15

All planned milestones are done. Do not ask for planning. Instead:

1. `docs/ROADMAP.md`: M15 done; "Current milestone" → "Final check and release 1.0.0 (M6-13, M6-15; ADR-042)" with sections 7 … 7i.
2. `docs/HANDOVER.md`: current state; the maintainer to-do "Final check before 1.0.0" lists all checklist sections 7 … 7i and the open GitKraken verification items of M13, M14 and M15.
3. `docs/PLANNING-HANDOVER.md`: milestone history line for M15 (version "1.0.0 (unreleased)"), section 8 "Next: final check by the maintainer, then 1.0.0".
4. Stop with the milestone report and the line "Bereit für den finalen Test (Checkliste Abschnitte 7 … 7i); 1.0.0 auf Zuruf."
