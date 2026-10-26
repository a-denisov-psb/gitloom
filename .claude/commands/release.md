---
description: Release the current milestone (only when the maintainer asks)
---

Follow the release steps in `CONTRIBUTING.md` and the release task of the current milestone:

1. Confirm all milestone tasks are checked and `develop` passes `./scripts/check.sh`.
2. `git switch -c release/X.Y.Z develop`, bump the workspace version, move `[Unreleased]` in `CHANGELOG.md` to the new version with today's date.
3. Update README status, `docs/ROADMAP.md` (milestone done, next current milestone) and `docs/HANDOVER.md` (current state).
4. Update `docs/PLANNING-HANDOVER.md`: milestone history line, open decisions, anything the next planning chat needs to know.
5. Show the maintainer a summary and ask for approval before merging into `master`.
6. After approval: merge `--no-ff` into `master`, tag `vX.Y.Z`, merge back into `develop`, delete the release branch.
