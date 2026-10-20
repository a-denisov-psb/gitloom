---
description: Start the next open task of the current milestone
---

1. Read `CLAUDE.md`, then `docs/ROADMAP.md` → "Current milestone", then that milestone file.
2. Pick the first unchecked task whose dependencies are all checked. If $ARGUMENTS names a task id, take that one instead (check its dependencies).
3. Make sure the working tree is clean and you are on `develop`. If not, stop and report.
4. Report to the maintainer in German, short:
   - task id and goal
   - acceptance criteria
   - implementation plan (steps, files, new dependencies, tests)
   - risks or unclear points
5. Wait for approval. Then create `feature/<task-id>-<slug>` from `develop` and implement in small Conventional Commits.
