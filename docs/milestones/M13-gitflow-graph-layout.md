# M13 – Git flow graph layout

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M12. M14 follows.

## Goal

When Git flow is initialized, the graph can be shown like the classic Git flow diagram: production left, then hotfix, release, development and features to the right, one color per branch type, also for branches that were deleted long ago. A Gitloom extension; GitKraken has no such layout.

## Scope

In scope:

- Pure classifier in `git-graph` that assigns every commit a branch type (and a name when known) from refs, first-parent chains, merge structure and merge messages
- Lane groups through `LaneHints` (M2-15), one color per type, a feature sub-palette
- Toggle per repository (column menu, palette, Preferences → Repository → Gitflow)
- Faded labels for deleted branches, tooltips with the type and its source
- Tests on scripted Git flow histories, performance measurement

Out of scope:

- Changing the default layout
- Git flow layouts without Git flow configuration (e.g. GitHub flow)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Groups | Left to right: 0 production, 1 hotfix, 2 release, 3 development, 4 feature (also other and unknown). Each group reserves as many lanes as it needs at most anywhere in the loaded history (`LaneHints` two-pass layout); never per viewport (the development column must not jump while scrolling). |
| Input | `git-graph` stays free of gix and iced: the classifier takes the commits (parents in Git order), the subject of every merge commit, the ref tips (local and remote-tracking branch names with their commit), and the Git flow names (production, development, feature/release/hotfix prefixes, `FlowConfig` of M7-06). Output per commit: type, optional branch name, source (live ref, merge message, topology, unknown). |
| Step 1: main lines | First-parent chain from the production tip (local branch, else `<remote>/<production>`) = production; first-parent chain from the development tip = development, except commits already production (production wins where both meet, e.g. history before develop existed). |
| Step 2: live branches | For every other branch tip (local first, then remote-tracking without a local twin): walk the first-parent chain until a classified commit; type from the prefix (feature/release/hotfix), other branches = feature group with their name. |
| Step 3: deleted branches | Every remaining commit belongs to a segment: the first-parent chain starting at a non-first parent of a merge commit, ending before the first classified commit (the fork point) or at a root. For each segment collect the groups of all merge commits that merge its tip (targets) and the group of its fork point (origin). Name from the merge subjects: `Merge branch '<name>'[ of <url>][ into <target>]`, `Merge remote-tracking branch '<remote>/<name>'`, `Merge pull request #<n> from <owner>/<name>`. Type: (a) the name's Git flow prefix; a name equal to the development or production branch (e.g. pull merges "Merge branch 'develop' of …") makes the segment development or production; (b) otherwise topology: a target in production and origin production = hotfix; a target in production and origin development = release; target only development and origin development = feature; target only development and origin production = hotfix; targets only in feature segments = feature; origin unknown: target production = release, else feature. Segments are resolved repeatedly until nothing changes (nested features). |
| Step 4: rest | Commits still unclassified (e.g. only reachable from tags) = unknown, feature group, neutral color, tooltip "Branch type unknown". Stash rows take the group of their base commit. |
| Priority | Live ref prefix > merge message > topology > unknown. A hotfix branched from development by mistake keeps its hotfix name. |
| Edge cases | Fast-forward, squash and rebase merges leave commits on the development chain: shown as development (correct by history). Gitloom's own finish uses `--no-ff` (M7). |
| Colors | Theme tokens (ADR-048) `flow_colors`: production green, hotfix cyan, release blue, development violet, features cycle through a sub-palette of four colors (pink, red, orange, yellow) per segment, unknown gray. Values per theme (Dark, Light; ADR-054) chosen by eye after the classic diagram. |
| Labels | In the Branch/Tag column, at the tip row of a segment without a live ref: a faded pill with a dashed outline showing the inferred name (or the type when no name is known). Tooltip on nodes and faded pills: "<type> · <name> (from the merge message / inferred from the history)". |
| Toggle | Per repository, stored as `gitloom.flowLayout` in the repository config (like M10's repository options), default off. Entry points: graph column menu (M11) "Gitflow layout", palette "Toggle Gitflow layout", Preferences → Repository → Gitflow "Show the graph in the Gitflow layout". Disabled with "Initialize Gitflow first" when Git flow is not configured. Everything else (selection, menus, search, hide/solo) works unchanged; hidden branches are left out before classifying. |
| Performance | Classification is part of the snapshot layout step in the background, linear in the number of commits; no frame over 50 ms; measured on the 100k-commit benchmark history with generated Git flow merges (`docs/PERFORMANCE.md`). |
| Undo | View setting only; not an undo entry (ADR-043 amended). |

## Maintainer answers and GitKraken verification items

Answered (maintainer, 2026-11-10/11): order production, hotfix, release, development, features; derive deleted branches from where they fork off and where they are merged; never limit the width calculation to the visible area; unknown segments may be shown in a neutral color.

Context menus (GitKraken 7.5.2 screenshots, 2026-11-12): local branch, remote branch, tag, graph pill, unstaged/staged/commit file, stash and remote menus; commit, stash and remote menus need no change.

Open:

| Item | Used in |
|---|---|
| On real Git flow repositories: is the development column acceptably far left? If not, a follow-up "cap per group" goes to the ideas backlog. | M13-05 |

## Milestone acceptance

- [ ] On the scripted classic Git flow history (v1.0, two features, release 2.0, hotfix for 2.0 → v2.1, all branches deleted) every commit lands in the expected group with the expected name.
- [ ] Live, deleted, pull-merge, nested-feature, fast-forward and wrongly based hotfix cases behave as in the decisions table; unknown commits are neutral.
- [ ] The toggle works from all entry points, persists per repository and is disabled without Git flow; the default layout is unchanged when off.
- [ ] Faded labels and tooltips for deleted branches.
- [ ] No frame over 50 ms; measurements in `docs/PERFORMANCE.md`.
- [ ] User guide, UX-PARITY (extension), release checklist and CHANGELOG updated; `./scripts/check.sh` green; CI green.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M13-01 | Classifier in git-graph | – |
| M13-02 | Lane groups and colors | M13-01 |
| M13-03 | Toggle and persistence | M13-02 |
| M13-04 | Labels and tooltips | M13-02 |
| M13-05 | Performance, documentation, wrap-up | M13-01 … M13-04 |
| M13-06 | Branch, remote branch and tag menus like GitKraken | – |
| M13-07 | File menus like GitKraken | – |

---

## Tasks

### [x] M13-01 Classifier in git-graph

- Pure module (e.g. `git_graph::flow`) with the input/output of the decisions table and steps 1–4; merge subject parser.
- App side: collect merge subjects and ref tips in the snapshot step (background).

Acceptance: unit tests on hand-built histories for every rule and edge case (compact text snapshots "commit → type/name/source"); integration test in git-core/app building the classic diagram with the `git` CLI (branches deleted after finishing) and checking every commit; parser tests for all three subject forms and non-matching subjects.

Note: `git_graph::flow::classify` returns segments (one per first-parent chain: tip, kind, name, source) and the segment of every commit, so labels (M13-04) and the feature sub-palette (M13-02) work per segment. Step 3 builds a segment from every still free commit in topological order (newest first); its targets are all merges that have one of its commits as a non-first parent, its fork point the first parent of its oldest commit; a segment no merge reaches stays unknown (step 4), which also covers tag-only chains. One segment per branch name (a remote twin of a local branch is skipped; `origin/main` serves when `main` is not local). Topology rules as in the table; "origin release/feature with a target in production" counts as release (the table's "origin unknown" rule). App: `RepoSnapshot::flow_classification` collects merge subjects (summaries of merge commits) and branch tips from the loaded snapshot; stash rows take their base commit's segment. Tests: `merge_subjects`, `classic_history_by_merge_messages`, `classic_history_by_topology`, `live_branches_win_and_remote_twins_are_skipped`, `production_wins_and_remote_main_counts`, `pull_merges_nested_features_fast_forward_and_unknown`, `hotfix_into_develop_only_and_missing_parents`; app `classic_git_flow_history_with_deleted_branches` (real repository built with the `git` CLI, topic branches deleted).

### [x] M13-02 Lane groups and colors

- `LaneHints` from the classification; `flow_colors` tokens for all themes; feature sub-palette per segment.

Acceptance: layout snapshots (text) for the classic history: production lane 0, hotfix, release, development, features right of it; development column stays constant over the whole history; snapshots of the graph in Dark and Light checked by eye.

Note: `Classification::lane_hints` gives group and color per commit; flow colors travel as `ColorId(FLOW_COLOR_BASE + n)` (production, hotfix, release, development, four feature colors per segment, unknown) and `theme::lane_color` maps them to the palette token `flow_colors` (Dark and Light, contrast ≥ 3:1 tested), so the graph drawing needed no change. `RepoSnapshot::with_flow_layout(on)` re-lays out the loaded history (no reload); the hints are kept for a WIP row appearing, and a branch filter classifies the filtered history (hidden branches left out first). Tests: git-graph `classic_history_lanes_by_group` (lanes and colors, development column constant), app `flow_layout_groups_lanes_and_survives_status_changes`, `flow_colors_stay_visible`; snapshots `ui_check_flow_layout` (default and flow, Dark and Light) checked by eye.

### [x] M13-03 Toggle and persistence

- Setting per repository, three entry points, disabled state, re-layout on toggle without reloading the history.

Acceptance: app tests for all entry points, persistence after restart, disabled without Git flow, default layout unchanged when off (existing layout snapshots unchanged).

Note: `gitloom.flowLayout` (repository config) is read with the snapshot (`GitBackend::flow_layout`, gix) and written by `git_core::set_flow_layout`; `FlowLayoutSet(on)` writes it and lays the loaded history out again in the background (`RepoSnapshot::with_flow_layout`, no reload), `FlowLayoutApplied` applies the result. Entry points: the graph column menu ("Gitflow layout" check item; disabled with the hint "Initialize Gitflow first"), the palette action "Toggle Gitflow layout" (disabled with the same reason) and a toggle "Show the graph in the Gitflow layout" in Preferences → Repository → Gitflow (shown once Git flow is initialized; also a "Preferences:" deep link). Off, the default layout is computed exactly as before (`layout_with` without hints; unit test compares the layouts). Tests: `flow_layout_entry_points`, `flow_layout_is_stored_in_the_repository` (real repository: off by default, on and off after reload).

### [x] M13-04 Labels and tooltips

- Faded pills for deleted segments, tooltips on nodes and pills.

Acceptance: app tests for label texts per source; snapshot with deleted feature, release and hotfix labels.

Note: the snapshot keeps the classification while the layout is on (`flow_segment`, `flow_label`, `flow_tooltip`). The faded pill sits after the live ref pills of the tip row of a deleted branch (name, or the type when no name is known; none for live branches, unknown commits and stash rows); it has a thin muted outline instead of a dashed one, since iced containers draw no dashed borders (UX detail of a Gitloom extension, no GitKraken counterpart). The tooltip of nodes (the graph cell) and faded pills reads "<type> · <name>" plus "(from the merge message)" / "(inferred from the history)", or "Branch type unknown". Fixed on the way: since M11-04 the canvas starts at the column edge but the Graph column width did not count that padding, so a node in the last lane lost 2 px and its initials (`lanes_width` now includes it; unit test `the_last_lane_fits_its_column`). Tests: `faded_labels_and_tooltips`; snapshots `ui_check_flow_layout` with feature, release and hotfix labels (Dark, Light) checked.

### [x] M13-05 Performance, documentation, wrap-up

- Benchmark with generated Git flow history; `docs/PERFORMANCE.md`.
- User guide `git-flow.md` (the layout, how types are inferred, limits), `coming-from-gitkraken.md`; UX-PARITY extension row; `docs/RELEASE-CHECKLIST.md` section "7g. New in M13"; CHANGELOG `[Unreleased]`; `docs/ARCHITECTURE.md` "Lane groups" extended by the classifier; ROADMAP backlog entry "cap lanes per group" only if the verification item asks for it.

Acceptance: measurements recorded; docs reviewed.

Note: `flow_perf` (git-graph, ignored, release): 100 004 commits classified in 19 ms, Git flow layout 12 ms (default 6 ms); `docs/PERFORMANCE.md` "M13". User guide `git-flow.md` "Gitflow layout", `coming-from-gitkraken.md`, UX-PARITY extension row, `docs/ARCHITECTURE.md` lane groups with the classifier, release checklist "7g. New in M13" (includes the open verification item), CHANGELOG (M13-03, M13-04). The backlog entry "cap lanes per group" waits for the verification item (real repositories); not added.


### [ ] M13-06 Branch, remote branch and tag menus like GitKraken

Found by the maintainer while testing (2026-11-12, GitKraken 7.5.2 screenshots of the local branch, remote branch, tag and graph pill menus). GitKraken's ref menus carry the most used actions of the ref's commit; ours only had ref actions. Not everything is taken over: only what is used often on a ref (see "Not taken over").

Menus (same builder for left panel and graph pills; separators as shown by "---"; `<head>` = checked-out branch or "HEAD"):

Local branch `<b>`:
- Checkout `<b>` (not shown for the checked-out branch, like GitKraken)
- ---
- Pull, Push, Set upstream…, Unset upstream (unchanged)
- ---
- Merge / Rebase / Fast-forward entries (unchanged)
- ---
- Create branch here, Reset `<head>` to this commit…
- ---
- Rename `<b>`…, Delete `<b>` (disabled for the checked-out branch, hint "Checked-out branch")
- ---
- Copy branch name, Copy commit sha
- ---
- Hide, Solo (unchanged)
- ---
- Create tag here

Remote branch `<r>`:
- Merge / Rebase / Fast-forward entries incl. "Fast-forward <local> to <r>" (unchanged, moved to the top like GitKraken)
- ---
- Checkout `<r>`, Create branch here, Reset `<head>` to this commit…, Delete `<r>` (deletes on the remote, confirmation unchanged)
- ---
- Copy branch name, Copy commit sha
- ---
- Hide, Solo (unchanged)
- ---
- Create tag here

Tag `<t>`:
- Push `<t>` to `<remote>` (remote name in the label when the push goes to one known remote; otherwise "Push `<t>`…"; behavior unchanged)
- ---
- Merge / Rebase / Fast-forward entries for the tag's commit (same rules as for branches, using `refs/tags/<t>` peeled)
- ---
- Create branch here, Reset `<head>` to this commit…
- ---
- Delete `<t>` locally, Delete `<t>` from `<remote>` (same label rule as Push)
- ---
- Copy tag name, Copy commit sha

Behavior:
- "Create branch here": start point is the commit the ref points to (tags peeled); no upstream (`git switch --create <name> <sha>`); "Checkout" of a remote branch stays the way to get a tracking branch. Same flow as the commit menu (M3-07): select the commit, scroll it into view, inline input in its branch/tag column, same validation, Enter creates and checks out, Esc cancels. No dead end (ADR-044): if the commit has no visible row (hidden or soloed away, beyond the loaded commits), a small dialog "Create branch at <ref>" with the same name field, validation and "Create branch" / "Cancel".
- "Reset `<head>` to this commit…": the existing reset dialog (Soft/Mixed/Hard) for the ref's commit, same enable rules as the commit menu (`CommitFacts`), undo unchanged.
- "Copy commit sha": full sha of the ref's commit (peeled for tags).
- Long names: labels follow the existing menu width rules (M6-19); names are not shortened.
- Disabled states and hints as for the other write items (read-only, running operation).

Not taken over (documented in UX-PARITY as deliberate deviations): Cherry pick commit, Revert commit and Edit commit message in ref menus (they stay in the commit menu), Create annotated tag here (our tag form covers both), Push and start a pull request (no provider APIs), tag Hide/Solo, Hide/Show all tags, Annotate tag.

Acceptance:
- `menus.rs` tests: order and labels of all three menus; Checkout absent for the checked-out branch; enable rules for Create branch here, Reset and Copy commit sha (incl. read-only); peeled target for annotated tags; merge entries for tags.
- App tests: Create branch here from a local branch, a remote branch (no upstream afterwards) and a tag; fallback dialog for a hidden branch; reset dialog opens for the ref's commit; undo deletes a created branch.
- `docs/UX-PARITY.md`: row "Right click commit/branch" and "Branch button / Ctrl+B" updated (M13-06); deviations listed in the deviations table.
- User guide (branches and tags pages) updated; CHANGELOG `[Unreleased]` → Changed: "Branch, remote branch and tag menus are ordered like GitKraken's and name the branch or tag; they also offer Create branch here, Reset to this commit and Copy commit sha."
- `docs/RELEASE-CHECKLIST.md`: checks in this milestone's section (left panel and graph pill menus; hidden branch → dialog).
- `./scripts/check.sh` green.

### [ ] M13-07 File menus like GitKraken

Found by the maintainer while testing (2026-11-12, GitKraken 7.5.2 screenshots of the unstaged, staged and commit file menus).

Menus ("---" = separator):

Unstaged file:
- Stage, Discard changes, Ignore
- ---
- Open in merge tool (conflicted files only, unchanged), File history, File blame
- ---
- Open in external diff tool, Open in external editor, Open file location
- ---
- Copy file path

Staged file:
- Unstage, Discard changes, Copy commit message prompt…
- ---
- File history, File blame
- ---
- Open in external diff tool, Open in external editor, Open file location
- ---
- Copy file path

File in the commit details (and multi-commit file list):
- File history, File blame
- ---
- Open in external diff tool, Open in external editor, Open file location
- ---
- Copy file path

Behavior:
- File blame: opens the history/blame view in Blame mode (M5-10). Unstaged/staged: the working copy file; commit files: blame at that commit. Disabled for untracked and added-only files with the hint "Untracked files have no history" / "Added in this commit" where blame has nothing to show; deleted files: blame at the parent.
- Open in external diff tool: the configured diff tool (M5/M8 External tools). Unstaged: index vs. working tree; staged: HEAD vs. index; commit files: first parent vs. commit (temporary files from `git show <rev>:<path>`, cleaned up afterwards). Not configured → disabled with hint "Set a diff tool in Preferences → External tools".
- Open file location in commit lists: the file in the working tree; disabled with hint "Not in the working tree" when it does not exist there.
- Discard changes in the staged menu: discards the staged and unstaged changes of that file (back to HEAD) after the existing confirmation; undoable like the unstaged discard (ADR-043).

Not taken over (deliberate deviations in UX-PARITY): Ignore submenu (file / extension / folder), Edit file and Delete file (no in-app editor), Open file in default program (external editor and Open file location cover it).

Acceptance:
- Tests for the menu contents per area (incl. conflicted, untracked, deleted files and read-only).
- App tests: File blame opens Blame mode at the right revision; staged Discard + undo restores index and working tree; external diff of a commit file passes the parent and commit versions (fake tool).
- `docs/UX-PARITY.md` row "Right click file in staging" updated, deviations listed; user guide updated; CHANGELOG `[Unreleased]` → Changed: "File menus are ordered like GitKraken's and add File blame, Open in external diff tool (also for files of a commit), Open file location for commit files and Discard changes for staged files."
- `docs/RELEASE-CHECKLIST.md`: checks in this milestone's section (external diff from a commit file on Windows).
- `./scripts/check.sh` green.

## After M13

No release (ADR-042): when all tasks are done, continue with M14.
