---
description: Release the current milestone (runs automatically at milestone end)
---

Follow the release steps in `CONTRIBUTING.md` and the release task of the current milestone (ADR-034, no approval needed):

1. Confirm all milestone tasks are checked or blocked by maintainer-only actions (recorded in `docs/HANDOVER.md`). `develop` must pass `./scripts/check.sh` and clippy for the Windows target, and the latest CI run on `develop` must be green.
2. `git switch -c release/X.Y.Z develop`, bump the workspace version, move `[Unreleased]` in `CHANGELOG.md` to the new version with today's date.
3. Update README status, `docs/ROADMAP.md` (milestone done, next current milestone) and `docs/HANDOVER.md` (current state).
4. Update `docs/PLANNING-HANDOVER.md`: milestone history line, open decisions, anything the next planning chat needs to know.
5. Run `./scripts/check.sh` again and commit on the release branch.
6. Without approval:
   - `git switch master && git merge --no-ff release/X.Y.Z && git tag -a vX.Y.Z -m "vX.Y.Z"`
   - `git push origin master` and `git push origin vX.Y.Z`
   - `git switch develop && git merge --no-ff release/X.Y.Z && git push origin develop`
   - `git branch -d release/X.Y.Z` (and `git push origin --delete release/X.Y.Z` if it was pushed)
7. Watch the CI runs of `master` and the tag (Windows/macOS) with `gh run watch`. A red run takes priority: fix it on `hotfix/X.Y.Z+1` from `master` and release a patch version the same way.
8. Output the milestone report in German as a table: tasks, deviations with reasons, maintainer to-dos, open risks.
