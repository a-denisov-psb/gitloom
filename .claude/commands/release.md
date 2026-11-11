---
description: Release 1.0.0 – only when the maintainer asks (ADR-042)
---

Run only when the maintainer explicitly asks for a release. Follow the release steps in `CONTRIBUTING.md` and the release task (M6-15):

1. Confirm all planned milestones are done, the maintainer's final checklist run is recorded in `docs/HANDOVER.md`, `develop` passes `./scripts/check.sh` and clippy for the Windows target, and the latest CI run on `develop` is green.
2. `git switch -c release/X.Y.Z develop`, bump the workspace version, move `[Unreleased]` in `CHANGELOG.md` to the new version with today's date.
3. Update README status, `docs/ROADMAP.md` and `docs/HANDOVER.md` (current state).
4. Update `docs/PLANNING-HANDOVER.md`: milestone history line, open decisions, anything the next planning chat needs to know.
5. Run `./scripts/check.sh` again and commit on the release branch.
6. Then:
   - `git switch master && git merge --no-ff release/X.Y.Z && git tag -a vX.Y.Z -m "vX.Y.Z"`
   - `git push origin master` and `git push origin vX.Y.Z` (if the tag push is refused, add it to the maintainer to-dos with the exact commands)
   - `git switch develop && git merge --no-ff release/X.Y.Z && git push origin develop`
   - `git branch -d release/X.Y.Z` (and `git push origin --delete release/X.Y.Z` if it was pushed)
7. Watch the CI runs of `master` and the tag with `gh run watch`. A red run takes priority: fix it on `hotfix/X.Y.Z+1` from `master`.
8. Output the release report in German as a table: contents, deviations with reasons, maintainer to-dos, open risks.
