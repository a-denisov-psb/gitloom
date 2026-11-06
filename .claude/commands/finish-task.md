---
description: Verify, document and hand in the current task for merge
---

1. Run `./scripts/check.sh`. Fix everything until it passes.
2. Check every acceptance criterion of the task in the milestone file. For UI tasks, run the app and check by hand.
3. Tick the task checkbox in the milestone file; add a one-line note for deviations.
4. Update `CHANGELOG.md` → `[Unreleased]` for user-visible changes; update `docs/` if architecture, decisions or UX deviations changed.
5. Commit the docs changes on the feature branch.
6. Merge without asking (ADR-017): `git switch develop && git merge --no-ff <branch> && git push origin develop && git branch -d <branch>` (and delete the remote feature branch if it was pushed).
7. Report exactly one line in German: `<task-id> ✔ – <result>; Abweichung: <deviation>` (omit the deviation part if there is none). Then continue with `/next-task` without pausing.
