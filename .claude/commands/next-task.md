---
description: Start the next open task of the current milestone
---

1. Read `CLAUDE.md`, then `docs/ROADMAP.md` → "Current milestone", then that milestone file.
2. Pick the first unchecked task whose dependencies are all checked. If $ARGUMENTS names a task id, take that one instead (check its dependencies). Skip tasks that need maintainer-only actions (ADR-034) and record them under "Maintainer to-dos" in `docs/HANDOVER.md`.
3. Make sure the working tree is clean and you are on `develop`. If not, stop and report.
4. If all tasks of the milestone are checked or only blocked tasks remain, run `/release` instead.
5. Do not post a plan; start directly. Decide plan gaps yourself and document them (ADR-034). Create `feature/<task-id>-<slug>` from `develop` and implement in small Conventional Commits.
