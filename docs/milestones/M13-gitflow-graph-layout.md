# M13 – Git flow graph layout

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M12. Last planned milestone before 1.0.0.

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
| Colors | Theme tokens (ADR-048) `flow_colors`: production green, hotfix cyan, release blue, development violet, features cycle through a sub-palette of four colors (pink, red, orange, yellow) per segment, unknown gray. Values per theme (Dark, Light, Classic Dark) chosen by eye after the classic diagram. |
| Labels | In the Branch/Tag column, at the tip row of a segment without a live ref: a faded pill with a dashed outline showing the inferred name (or the type when no name is known). Tooltip on nodes and faded pills: "<type> · <name> (from the merge message / inferred from the history)". |
| Toggle | Per repository, stored as `gitloom.flowLayout` in the repository config (like M10's repository options), default off. Entry points: graph column menu (M11) "Gitflow layout", palette "Toggle Gitflow layout", Preferences → Repository → Gitflow "Show the graph in the Gitflow layout". Disabled with "Initialize Gitflow first" when Git flow is not configured. Everything else (selection, menus, search, hide/solo) works unchanged; hidden branches are left out before classifying. |
| Performance | Classification is part of the snapshot layout step in the background, linear in the number of commits; no frame over 50 ms; measured on the 100k-commit benchmark history with generated Git flow merges (`docs/PERFORMANCE.md`). |
| Undo | View setting only; not an undo entry (ADR-043 amended). |

## Maintainer answers and GitKraken verification items

Answered (maintainer, 2026-11-10/11): order production, hotfix, release, development, features; derive deleted branches from where they fork off and where they are merged; never limit the width calculation to the visible area; unknown segments may be shown in a neutral color.

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

---

## Tasks

### [ ] M13-01 Classifier in git-graph

- Pure module (e.g. `git_graph::flow`) with the input/output of the decisions table and steps 1–4; merge subject parser.
- App side: collect merge subjects and ref tips in the snapshot step (background).

Acceptance: unit tests on hand-built histories for every rule and edge case (compact text snapshots "commit → type/name/source"); integration test in git-core/app building the classic diagram with the `git` CLI (branches deleted after finishing) and checking every commit; parser tests for all three subject forms and non-matching subjects.

### [ ] M13-02 Lane groups and colors

- `LaneHints` from the classification; `flow_colors` tokens for all themes; feature sub-palette per segment.

Acceptance: layout snapshots (text) for the classic history: production lane 0, hotfix, release, development, features right of it; development column stays constant over the whole history; snapshots of the graph in all three themes checked by eye.

### [ ] M13-03 Toggle and persistence

- Setting per repository, three entry points, disabled state, re-layout on toggle without reloading the history.

Acceptance: app tests for all entry points, persistence after restart, disabled without Git flow, default layout unchanged when off (existing layout snapshots unchanged).

### [ ] M13-04 Labels and tooltips

- Faded pills for deleted segments, tooltips on nodes and pills.

Acceptance: app tests for label texts per source; snapshot with deleted feature, release and hotfix labels.

### [ ] M13-05 Performance, documentation, wrap-up

- Benchmark with generated Git flow history; `docs/PERFORMANCE.md`.
- User guide `git-flow.md` (the layout, how types are inferred, limits), `coming-from-gitkraken.md`; UX-PARITY extension row; `docs/RELEASE-CHECKLIST.md` section "7g. New in M13"; CHANGELOG `[Unreleased]`; `docs/ARCHITECTURE.md` "Lane groups" extended by the classifier; ROADMAP backlog entry "cap lanes per group" only if the verification item asks for it.

Acceptance: measurements recorded; docs reviewed.

## After M13

All planned milestones are done. Do not ask for planning. Instead:

1. `docs/ROADMAP.md`: M13 done; "Current milestone" → "Final check and release 1.0.0 (M6-13, M6-15; ADR-042)".
2. `docs/HANDOVER.md`: current state; the maintainer to-do "Final check before 1.0.0" stays open and lists all checklist sections 7 … 7g.
3. `docs/PLANNING-HANDOVER.md`: milestone history lines for M9 … M13 (version "1.0.0 (unreleased)"), section 8 "Next: final check by the maintainer, then 1.0.0".
4. Stop with the milestone report and the line "Bereit für den finalen Test (Checkliste Abschnitte 7 … 7g); 1.0.0 auf Zuruf."
